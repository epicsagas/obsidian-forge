use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use tracing::{debug, warn};

use crate::config::AiConfig;

/// Summary of AI configuration (for display purposes).
pub struct AiConfigSummary {
    pub provider: String,
    pub model: String,
    pub base_url: String,
    pub api_key: String,
}

/// Unified AI client. Supports:
/// - `ollama`            — local Ollama CLI subprocess
/// - `openai`            — OpenAI API (requires api_key)
/// - `openrouter`        — OpenRouter API (requires api_key)
/// - `lmstudio`          — LM Studio local server (default: http://localhost:1234)
/// - `openai-compatible` — Any OpenAI-compatible endpoint (requires base_url)
#[derive(Clone)]
pub struct AiClient {
    provider: String,
    model: String,
    base_url: String,
    api_key: Option<String>,
    http: reqwest::Client,
    /// Minimum gap between request starts (ms). 0 for local providers.
    min_interval_ms: u64,
}

/// Global request spacing across all clones of [`AiClient`] — one shared
/// clock per process so concurrent batch tasks cannot stomp the interval.
fn request_gate() -> &'static tokio::sync::Mutex<std::time::Instant> {
    static GATE: std::sync::OnceLock<tokio::sync::Mutex<std::time::Instant>> =
        std::sync::OnceLock::new();
    GATE.get_or_init(|| {
        tokio::sync::Mutex::new(std::time::Instant::now() - std::time::Duration::from_secs(3600))
    })
}

/// Wait until at least `min_interval_ms` has passed since the previous
/// remote request started, then claim the next slot.
async fn acquire_request_slot(min_interval_ms: u64) {
    if min_interval_ms == 0 {
        return;
    }
    let mut last = request_gate().lock().await;
    let now = std::time::Instant::now();
    let elapsed = now.duration_since(*last);
    if elapsed < std::time::Duration::from_millis(min_interval_ms) {
        tokio::time::sleep(std::time::Duration::from_millis(
            min_interval_ms - elapsed.as_millis() as u64,
        ))
        .await;
    }
    *last = std::time::Instant::now();
}

impl AiClient {
    pub fn from_config(cfg: &AiConfig) -> Self {
        let base_url = cfg.base_url.clone().unwrap_or_else(|| {
            match cfg.provider.as_str() {
                "openai" => "https://api.openai.com/v1".into(),
                "openrouter" => "https://openrouter.ai/api/v1".into(),
                "lmstudio" => "http://localhost:1234/v1".into(),
                "openai-compatible" => "http://localhost:11434/v1".into(),
                _ => String::new(), // ollama: unused
            }
        });

        // api_key: vault.toml > environment variable
        let api_key = cfg.api_key.clone().or_else(|| match cfg.provider.as_str() {
            "openai" => std::env::var("OPENAI_API_KEY").ok(),
            "openrouter" => std::env::var("OPENROUTER_API_KEY").ok(),
            "openai-compatible" => std::env::var("OPENAI_COMPATIBLE_API_KEY")
                .ok()
                .or_else(|| std::env::var("OPENAI_API_KEY").ok()),
            _ => None,
        });

        // Rate-limit guard: remote providers get a conservative default gap
        // between request starts; local servers and ollama are unthrottled.
        let min_interval_ms = cfg
            .min_request_interval_ms
            .unwrap_or(match cfg.provider.as_str() {
                "ollama" | "lmstudio" => 0,
                _ => 2000,
            });

        Self {
            provider: cfg.provider.clone(),
            model: cfg.model.clone(),
            base_url,
            api_key,
            http: reqwest::Client::new(),
            min_interval_ms,
        }
    }

    /// Send a minimal prompt to verify AI connectivity.
    /// Returns a short confirmation message from the model on success.
    /// 429 (rate-limited) is treated as reachable — the config is correct,
    /// only the request budget is exhausted.
    pub async fn ping(&self) -> Result<String> {
        match self.provider.as_str() {
            "ollama" => {
                // For ollama, just check that the CLI is available
                let output = tokio::process::Command::new("ollama")
                    .arg("list")
                    .output()
                    .await
                    .context("failed to spawn ollama — is it installed?")?;
                if output.status.success() {
                    Ok("ollama running".to_string())
                } else {
                    anyhow::bail!(
                        "ollama not responding: {}",
                        String::from_utf8_lossy(&output.stderr)
                    )
                }
            }
            _ => self.ping_http().await,
        }
    }

    /// HTTP-based ping: send a minimal completion request and interpret the
    /// response status.  429 means "connected but rate-limited" (success).
    async fn ping_http(&self) -> Result<String> {
        #[derive(Serialize)]
        struct Req<'a> {
            model: &'a str,
            messages: Vec<Msg<'a>>,
            max_tokens: u32,
        }
        #[derive(Serialize)]
        struct Msg<'a> {
            role: &'a str,
            content: &'a str,
        }

        if self.base_url.is_empty() {
            anyhow::bail!(
                "provider '{}' requires a base_url — set it in config.toml [ai]",
                self.provider
            );
        }

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));
        let body = Req {
            model: &self.model,
            messages: vec![Msg {
                role: "user",
                content: "Hi",
            }],
            max_tokens: 1,
        };

        let mut req = self
            .http
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body);

        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key);
        }
        if self.provider == "openrouter" {
            req = req
                .header(
                    "HTTP-Referer",
                    "https://github.com/epicsagas/obsidian-forge",
                )
                .header("X-Title", "obsidian-forge");
        }

        let resp = req
            .send()
            .await
            .with_context(|| format!("connection to {} failed", url))?;

        let status = resp.status();
        match status.as_u16() {
            200..=299 => {
                // Try to parse response body for a short confirmation
                let text = resp.text().await.unwrap_or_default();
                // Extract content from JSON response
                if let Some(content) = text
                    .split("\"content\":")
                    .nth(1)
                    .and_then(|s| s.trim().strip_prefix('"'))
                    .and_then(|s| s.split('"').next())
                {
                    Ok(content.to_string())
                } else {
                    Ok("connected".to_string())
                }
            }
            429 => Ok("connected (rate-limited)".to_string()),
            401 | 403 => anyhow::bail!("connected but unauthorized — check API key"),
            404 => anyhow::bail!(
                "connected but model '{}' not found at {} — check model name",
                self.model,
                self.base_url
            ),
            _ => {
                let body = resp.text().await.unwrap_or_default();
                anyhow::bail!("API error {}: {}", status, body)
            }
        }
    }

    /// Return a summary of the current configuration (for status display).
    /// The API key is masked for safety.
    pub fn config_summary(&self) -> AiConfigSummary {
        let key_status = match &self.api_key {
            Some(k) if !k.is_empty() => {
                if k.len() > 8 {
                    format!("{}...{}", &k[..4], &k[k.len() - 4..])
                } else {
                    "****".to_string()
                }
            }
            _ if self.provider == "ollama" || self.provider == "lmstudio" => {
                "not required".to_string()
            }
            _ => "missing".to_string(),
        };

        AiConfigSummary {
            provider: self.provider.clone(),
            model: self.model.clone(),
            base_url: if self.base_url.is_empty() {
                "N/A (ollama)".to_string()
            } else {
                self.base_url.clone()
            },
            api_key: key_status,
        }
    }

    pub async fn summarize(&self, text: &str, max_len: usize) -> Result<String> {
        let prompt = format!(
            "Summarize the following text in {} characters or less. Include only key points:\n\n{}",
            max_len, text
        );
        self.complete(&prompt).await
    }

    /// 노트 내용으로부터 한 줄 요약, 핵심 질문, 연결 제안을 마크다운으로 생성한다.
    /// 대시보드의 ASK AI 버튼용.
    #[cfg(feature = "dashboard-ui")]
    pub async fn insights(&self, title: &str, body: &str) -> Result<String> {
        // 긴 본문은 UTF-8 문자 경계에서 절단 — 토큰/비용 절감
        let body = if body.len() > 6000 {
            let mut end = 6000;
            while !body.is_char_boundary(end) {
                end -= 1;
            }
            &body[..end]
        } else {
            body
        };
        let prompt = format!(
            "다음 Obsidian 노트를 분석하고 마크다운으로 답하라.\n\n\
             제목: {title}\n\n---\n{body}\n---\n\n\
             다음 형식을 그대로 지킬 것:\n\
             **한 줄 요약:** (노트 핵심 한 줄)\n\
             **핵심 질문:**\n- (질문1)\n- (질문2)\n- (질문3)\n\
             **연결 제안:** 이 노트와 엮을 만한 개념이나 다른 노트 2~3개\n"
        );
        self.complete(&prompt).await
    }

    pub async fn generate_json<T: for<'de> Deserialize<'de>>(&self, prompt: &str) -> Result<T> {
        let raw = self.complete(prompt).await?;
        let json_str = extract_json(&raw);
        debug!("Parsing JSON: {}", json_str);
        serde_json::from_str(json_str).with_context(|| format!("JSON parse failed. Raw: {}", raw))
    }

    async fn complete(&self, prompt: &str) -> Result<String> {
        match self.provider.as_str() {
            "ollama" => self.complete_ollama(prompt).await,
            "openai" | "openrouter" | "lmstudio" | "openai-compatible" => {
                if self.base_url.is_empty() {
                    anyhow::bail!(
                        "provider '{}' requires a base_url — set it in vault.toml [ai] or use a known provider",
                        self.provider
                    );
                }
                self.complete_openai_compatible(prompt).await
            }
            other => anyhow::bail!(
                "unknown AI provider '{}'. Valid options: ollama, openai, openrouter, lmstudio, openai-compatible",
                other
            ),
        }
    }

    // -------------------------------------------------------------------------
    // Ollama: CLI subprocess
    // -------------------------------------------------------------------------

    async fn complete_ollama(&self, prompt: &str) -> Result<String> {
        use tokio::io::AsyncWriteExt;

        let mut child = tokio::process::Command::new("ollama")
            .arg("run")
            .arg(&self.model)
            .arg("--")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .context("failed to spawn ollama — is it installed and running?")?;

        if let Some(stdin) = child.stdin.as_mut() {
            stdin.write_all(prompt.as_bytes()).await?;
            stdin.flush().await?;
        }

        let output = child.wait_with_output().await?;
        let response = String::from_utf8_lossy(&output.stdout).trim().to_string();

        if response.is_empty() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            warn!("Empty Ollama response. stderr: {}", stderr);
            anyhow::bail!("Empty response from Ollama");
        }

        Ok(response)
    }

    // -------------------------------------------------------------------------
    // OpenAI-compatible: REST /v1/chat/completions
    // -------------------------------------------------------------------------

    async fn complete_openai_compatible(&self, prompt: &str) -> Result<String> {
        // Retry transient failures (429 / 5xx) with exponential backoff,
        // honouring Retry-After. Keeps free-tier and shared-rate-limit
        // endpoints usable for batch runs without hammering them.
        let max_attempts = 5u32;
        let mut attempt = 0u32;
        loop {
            attempt += 1;
            match self.complete_once(prompt).await {
                Ok(resp) => return Ok(resp),
                Err(e) if attempt < max_attempts && is_transient(&e) => {
                    let backoff = retry_delay(attempt, e.retry_after_ms());
                    warn!(
                        "AI request transient failure (attempt {}/{}), retrying in {:?}: {}",
                        attempt, max_attempts, backoff, e
                    );
                    tokio::time::sleep(backoff).await;
                }
                Err(e) => return Err(e.into()),
            }
        }
    }

    async fn complete_once(&self, prompt: &str) -> Result<String, AiError> {
        #[derive(Serialize)]
        struct Req<'a> {
            model: &'a str,
            messages: Vec<Msg<'a>>,
            temperature: f32,
        }
        #[derive(Serialize)]
        struct Msg<'a> {
            role: &'a str,
            content: &'a str,
        }

        #[derive(Deserialize)]
        struct Resp {
            choices: Vec<Choice>,
        }
        #[derive(Deserialize)]
        struct Choice {
            message: MsgOut,
        }
        #[derive(Deserialize)]
        struct MsgOut {
            content: String,
        }

        let url = format!("{}/chat/completions", self.base_url.trim_end_matches('/'));

        // Space out request starts so batch runs stay under provider rate limits.
        acquire_request_slot(self.min_interval_ms).await;

        let body = Req {
            model: &self.model,
            messages: vec![Msg {
                role: "user",
                content: prompt,
            }],
            temperature: 0.3,
        };

        let mut req = self
            .http
            .post(&url)
            .header("Content-Type", "application/json")
            .json(&body);

        if let Some(key) = &self.api_key {
            req = req.bearer_auth(key);
        }

        if self.provider == "openrouter" {
            req = req
                .header(
                    "HTTP-Referer",
                    "https://github.com/epicsagas/obsidian-forge",
                )
                .header("X-Title", "obsidian-forge");
        }

        let resp = req
            .send()
            .await
            .map_err(|e| AiError::Other(e.to_string()))?;

        let status = resp.status();
        if !status.is_success() {
            let retry_after = resp
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.trim().parse::<u64>().ok())
                .map(|s| s.saturating_mul(1000));
            let body = resp.text().await.unwrap_or_default();
            let code = status.as_u16();
            if code == 429 || code >= 500 {
                return Err(AiError::Transient {
                    status: code,
                    body: truncate_body(&body),
                    retry_after_ms: retry_after,
                });
            }
            return Err(AiError::Permanent {
                status: code,
                body: truncate_body(&body),
            });
        }

        let parsed: Resp = resp
            .json()
            .await
            .map_err(|e| AiError::Other(format!("failed to parse AI API response: {e}")))?;

        parsed
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content.trim().to_string())
            .ok_or(AiError::Other("empty choices in AI API response".into()))
    }
}

/// Error carrier for [`AiClient::complete_once`] that separates transient
/// failures (worth retrying with backoff) from permanent ones.
#[derive(Debug)]
enum AiError {
    Transient {
        status: u16,
        body: String,
        retry_after_ms: Option<u64>,
    },
    Permanent {
        status: u16,
        body: String,
    },
    Other(String),
}

impl std::fmt::Display for AiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AiError::Transient { status, body, .. } => write!(f, "AI API error {status}: {body}"),
            AiError::Permanent { status, body } => write!(f, "AI API error {status}: {body}"),
            AiError::Other(msg) => write!(f, "{msg}"),
        }
    }
}

impl std::error::Error for AiError {}

impl AiError {
    fn retry_after_ms(&self) -> Option<u64> {
        match self {
            AiError::Transient { retry_after_ms, .. } => *retry_after_ms,
            _ => None,
        }
    }
}

fn is_transient(e: &AiError) -> bool {
    matches!(e, AiError::Transient { .. })
}

/// Exponential backoff with jitter: 1s, 2s, 4s, 8s … capped at 30s, or the
/// server-advertised Retry-After if it is longer.
fn retry_delay(attempt: u32, retry_after_ms: Option<u64>) -> std::time::Duration {
    let exp = std::time::Duration::from_millis(1000u64.saturating_mul(1 << (attempt - 1).min(5)));
    let jitter_ms = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_millis())
        .unwrap_or(0)) as u64;
    let with_jitter = exp + std::time::Duration::from_millis(jitter_ms);
    if let Some(ra) = retry_after_ms {
        return std::time::Duration::from_millis(ra).max(with_jitter);
    }
    with_jitter.min(std::time::Duration::from_secs(30))
}

fn truncate_body(body: &str) -> String {
    const MAX: usize = 300;
    if body.chars().count() <= MAX {
        body.to_string()
    } else {
        let cut: String = body.chars().take(MAX).collect();
        format!("{cut}…")
    }
}

// -----------------------------------------------------------------------------
// JSON extraction helpers (unchanged from ollama.rs)
// -----------------------------------------------------------------------------

fn extract_json(raw: &str) -> &str {
    if let Some(start) = raw.find("```json") {
        let inner = &raw[start + 7..];
        if let Some(end) = inner.find("```") {
            return inner[..end].trim();
        }
    }
    if let Some(start) = raw.find("```") {
        let inner = &raw[start + 3..];
        if let Some(end) = inner.find("```") {
            let candidate = inner[..end].trim();
            if candidate.starts_with('{') || candidate.starts_with('[') {
                return candidate;
            }
        }
    }
    let obj = raw.find('{');
    let arr = raw.find('[');
    match (obj, arr) {
        (Some(o), Some(a)) if a < o => extract_balanced(raw, a, '[', ']'),
        (Some(o), _) => extract_balanced(raw, o, '{', '}'),
        (None, Some(a)) => extract_balanced(raw, a, '[', ']'),
        _ => raw.trim(),
    }
}

fn extract_balanced(s: &str, start: usize, open: char, close: char) -> &str {
    let mut depth = 0usize;
    for (i, &b) in s.as_bytes()[start..].iter().enumerate() {
        if b == open as u8 {
            depth += 1;
        }
        if b == close as u8 {
            depth = depth.saturating_sub(1);
            if depth == 0 {
                return s[start..=start + i].trim();
            }
        }
    }
    s[start..].trim()
}
