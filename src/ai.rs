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

        Self {
            provider: cfg.provider.clone(),
            model: cfg.model.clone(),
            base_url,
            api_key,
            http: reqwest::Client::new(),
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
                .header("HTTP-Referer", "https://github.com/epicsagas/obsidian-forge")
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
            .with_context(|| format!("request to {} failed", url))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            anyhow::bail!("AI API error {}: {}", status, body);
        }

        let parsed: Resp = resp
            .json()
            .await
            .context("failed to parse AI API response")?;

        parsed
            .choices
            .into_iter()
            .next()
            .map(|c| c.message.content.trim().to_string())
            .ok_or_else(|| anyhow::anyhow!("empty choices in AI API response"))
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
