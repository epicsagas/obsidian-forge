use anyhow::{Result, bail};
use futures::stream::{self, StreamExt};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fs, path::{Path, PathBuf}, sync::OnceLock};
use tracing::info;
use walkdir::WalkDir;

use crate::config::ForgeConfig;
use crate::vault_utils::{doc_type_tag, frontmatter_re, is_vault_excluded};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FrontmatterResult {
    pub scanned: usize,
    pub issues: Vec<FrontmatterIssue>,
    pub fixed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrontmatterIssue {
    pub file: String,
    pub issue: String,
    pub detail: String,
    pub fixed: bool,
}

impl std::fmt::Display for FrontmatterResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "=== Frontmatter Normalization ===")?;
        writeln!(f, "Scanned: {} files", self.scanned)?;
        if self.issues.is_empty() {
            writeln!(f, "No issues found.")?;
        } else {
            writeln!(f, "Issues ({}):", self.issues.len())?;
            for issue in &self.issues {
                let status = if issue.fixed { "FIXED" } else { "TODO" };
                writeln!(
                    f,
                    "  [{}] {} — {} ({})",
                    status, issue.file, issue.detail, issue.issue
                )?;
            }
        }
        if self.fixed > 0 {
            writeln!(f, "Fixed: {} files", self.fixed)?;
        }
        Ok(())
    }
}

fn closing_brace_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"^---\s*project:(.*)$").expect("valid closing brace malform regex")
    })
}

fn empty_tags_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?m)^tags:\s*\[\]\s*$").expect("valid empty tags regex"))
}

/// Detect closing brace malform: first line is `---project:...` instead of `---\nproject:...`.
fn detect_closing_brace_malform(content: &str) -> bool {
    let first_line = content.lines().next().unwrap_or("");
    closing_brace_re().is_match(first_line)
}

/// Detect broken YAML list: `tags:` on its own line followed by a non-list value.
fn detect_broken_yaml_list(content: &str) -> bool {
    let lines: Vec<&str> = content.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if line.trim() == "tags:" && i + 1 < lines.len() {
            let next = lines[i + 1].trim();
            if !next.starts_with("- ") && !next.starts_with('[') && !next.is_empty() {
                return true;
            }
        }
    }
    false
}

/// Fix closing brace malform: replace `---project:X` on first line with `---\nproject:X`.
fn fix_closing_brace_malform(content: &str) -> String {
    let first_line = content.lines().next().unwrap_or("");
    if let Some(caps) = closing_brace_re().captures(first_line) {
        let rest = caps.get(1).unwrap().as_str().trim();
        let remainder = if let Some(nl) = content.find('\n') {
            &content[nl + 1..]
        } else {
            ""
        };
        format!("---\nproject: {}\n{}", rest, remainder)
    } else {
        content.to_string()
    }
}

/// Fix broken YAML list: insert `tags: []` after a bare `tags:` line when the next
/// line is not a list item.
fn fix_broken_yaml_list(content: &str) -> String {
    let mut new_lines = Vec::new();
    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        if line.trim() == "tags:" && i + 1 < lines.len() {
            let next = lines[i + 1].trim();
            if !next.starts_with("- ") && !next.starts_with('[') && !next.is_empty() {
                // Replace bare `tags:` with `tags: []`
                new_lines.push("tags: []".to_string());
                i += 1;
                continue;
            }
        }
        new_lines.push(line.to_string());
        i += 1;
    }
    let mut result = new_lines.join("\n");
    if content.ends_with('\n') {
        result.push('\n');
    }
    result
}

pub fn normalize_frontmatter(
    vault_root: &Path,
    _config: &ForgeConfig,
    fix: bool,
) -> Result<FrontmatterResult> {
    let mut result = FrontmatterResult::default();

    for entry in WalkDir::new(vault_root)
        .into_iter()
        .filter_entry(|e| !is_vault_excluded(e.path(), vault_root))
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && e.path().extension().is_some_and(|ext| ext == "md"))
    {
        result.scanned += 1;
        let path = entry.path();
        let rel = path
            .strip_prefix(vault_root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let mut modified = content.clone();
        let mut file_fixed = false;

        // 1. Closing brace malform: ---project: on the first line
        if detect_closing_brace_malform(&modified) {
            result.issues.push(FrontmatterIssue {
                file: rel.clone(),
                issue: "closing_brace_malform".into(),
                detail: "Opening --- and project: on same line".into(),
                fixed: fix,
            });
            if fix {
                modified = fix_closing_brace_malform(&modified);
                file_fixed = true;
            }
        }

        // 2. Broken YAML list: tags: on its own line followed by non-list
        if detect_broken_yaml_list(&modified) {
            result.issues.push(FrontmatterIssue {
                file: rel.clone(),
                issue: "broken_yaml_list".into(),
                detail: "tags: followed by non-list value".into(),
                fixed: fix,
            });
            if fix {
                modified = fix_broken_yaml_list(&modified);
                file_fixed = true;
            }
        }

        // 3. Empty tags: report only
        if empty_tags_re().is_match(&modified) {
            result.issues.push(FrontmatterIssue {
                file: rel.clone(),
                issue: "empty_tags".into(),
                detail: "tags: [] — user decision required".into(),
                fixed: false,
            });
        }

        // 4. Missing frontmatter in PRIMARY docs
        if !frontmatter_re().is_match(&modified)
            && let Some(filename) = path.file_name().and_then(|n| n.to_str())
            && let Some(doc_type) = doc_type_tag(filename)
        {
            let parts: Vec<&str> = rel.split('/').collect();
            let is_project_doc = parts
                .windows(2)
                .any(|w| matches!(w, ["99-Archives", "projects"]));
            if is_project_doc {
                let project_name = rel
                    .split('/')
                    .skip_while(|s| *s != "projects")
                    .nth(1)
                    .unwrap_or("unknown");

                result.issues.push(FrontmatterIssue {
                    file: rel.clone(),
                    issue: "missing_frontmatter".into(),
                    detail: format!("PRIMARY doc {} has no frontmatter", filename),
                    fixed: fix,
                });

                if fix {
                    let fm = format!(
                        "---\nproject: {}\ntags: [{}, layer/raw, {}]\n---\n",
                        project_name, project_name, doc_type
                    );
                    modified = format!("{}{}", fm, content);
                    file_fixed = true;
                }
            }
        }

        if file_fixed {
            fs::write(path, &modified)?;
            result.fixed += 1;
            info!("Fixed frontmatter issues in {}", rel);
        }
    }

    if result.issues.is_empty() {
        info!(
            "No frontmatter issues found ({} files scanned)",
            result.scanned
        );
    } else {
        info!(
            "Found {} frontmatter issue(s) in {} files, fixed {}",
            result.issues.len(),
            result
                .issues
                .iter()
                .map(|i| &i.file)
                .collect::<HashSet<_>>()
                .len(),
            result.fixed,
        );
    }

    Ok(result)
}

// ---------------------------------------------------------------------------
// AI fill: generate frontmatter for docs that have none
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct AiFrontmatter {
    project: String,
    title: String,
    summary: String,
    tags: Vec<String>,
}

/// Minimum body length (chars) for a doc to be worth an AI fill call.
const FILL_MIN_BODY_CHARS: usize = 200;

/// Generate frontmatter via AI for markdown files that have none.
///
/// Batch is paced by [`crate::ai`]'s global request throttle and honors
/// `max_concurrent`, but the effective concurrency is capped at 3 so
/// rate-limited providers are not hammered. The inbox is skipped — those
/// files belong to the `process-all` flow.
pub async fn fill_missing_frontmatter(
    vault_root: &Path,
    config: &ForgeConfig,
) -> Result<FrontmatterResult> {
    let mut result = FrontmatterResult::default();
    let inbox = vault_root.join(&config.vault.inbox_dir);

    let mut candidates: Vec<(PathBuf, String)> = Vec::new(); // (path, rel)
    for entry in WalkDir::new(vault_root)
        .into_iter()
        .filter_entry(|e| !is_vault_excluded(e.path(), vault_root))
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && e.path().extension().is_some_and(|ext| ext == "md"))
    {
        result.scanned += 1;
        let path = entry.path();
        if path.starts_with(&inbox) {
            continue;
        }
        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        if frontmatter_re().is_match(&content) {
            continue;
        }
        if content.chars().count() < FILL_MIN_BODY_CHARS {
            continue;
        }
        let rel = path
            .strip_prefix(vault_root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();
        candidates.push((path.to_path_buf(), rel));
    }

    if candidates.is_empty() {
        info!("No frontmatter-less docs worth filling");
        return Ok(result);
    }

    let client = crate::ai::AiClient::from_config(&config.ai);
    let concurrency = config.ai.max_concurrent.unwrap_or(5).clamp(1, 3);

    let outcomes = stream::iter(candidates)
        .map(|(path, rel)| {
            let client = client.clone();
            async move {
                match fill_one(&path, &rel, &client).await {
                    Ok(()) => {
                        info!("AI-filled frontmatter: {}", rel);
                        Some(rel)
                    }
                    Err(e) => {
                        tracing::warn!("AI fill failed for {}: {}", rel, e);
                        None
                    }
                }
            }
        })
        .buffer_unordered(concurrency)
        .collect::<Vec<_>>()
        .await;

    for filled in outcomes.into_iter().flatten() {
        result.issues.push(FrontmatterIssue {
            file: filled,
            issue: "ai_filled".into(),
            detail: "frontmatter generated from content".into(),
            fixed: true,
        });
        result.fixed += 1;
    }

    Ok(result)
}

/// Excerpt `content` to at most `max` chars, cutting on a char boundary.
pub fn excerpt(content: &str, max: usize) -> String {
    if content.chars().count() <= max {
        content.to_string()
    } else {
        content.chars().take(max).collect()
    }
}

async fn fill_one(path: &Path, rel: &str, client: &crate::ai::AiClient) -> Result<()> {
    let content = fs::read_to_string(path)?;

    let prompt = format!(
        "다음 마크다운 문서의 메타데이터를 추출해 JSON으로만 답하라. \
         다른 설명이나 코드펜스 없이 순수 JSON 한 개만 출력할 것.\n\
         형식: {{\"project\": \"kebab-case 프로젝트명\", \"title\": \"문서 제목\", \
         \"summary\": \"100자 이내 요약\", \
         \"tags\": [\"layer/raw\", \"type/...\", \"topics/...\" 3~5개 계층형 태그 — type은 prd|architecture|convention|decision|progress|debt|reference|report|spec|plan|research|strategy|note 중 하나]}}\n\n\
         경로 힌트: {rel}\n\n---\n{}\n---",
        excerpt(&content, 3000)
    );

    let fm: AiFrontmatter = client.generate_json(&prompt).await?;
    if fm.project.trim().is_empty() || fm.tags.is_empty() {
        bail!("AI returned unusable frontmatter for {}", rel);
    }

    let today = chrono_like_today();
    let tags = fm.tags.join(", ");
    let frontmatter = format!(
        "---\nproject: {}\ntitle: {}\nsummary: \"{}\"\ntags: [{}]\ncreated: {}\n---\n",
        fm.project.trim(),
        fm.title.trim().replace('"', "'"),
        fm.summary.trim().replace('"', "'"),
        tags,
        today,
    );

    fs::write(path, format!("{frontmatter}{content}"))?;
    Ok(())
}

/// Local-date ISO string (YYYY-MM-DD) without pulling a date crate.
fn chrono_like_today() -> String {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = now / 86_400;
    // Civil-from-days algorithm (Howard Hinnant) — UTC date, good enough for a stamp.
    let z = days as i64 + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> ForgeConfig {
        ForgeConfig::default_for("test-vault")
    }

    #[test]
    fn test_excerpt_respects_char_boundary() {
        let s = "가".repeat(500);
        let cut = excerpt(&s, 300);
        assert_eq!(cut.chars().count(), 300);
        assert_eq!(excerpt(&s, 10_000), s);
    }

    #[test]
    fn test_chrono_like_today_format() {
        let today = chrono_like_today();
        assert_eq!(today.len(), 10);
        assert_eq!(today.as_bytes()[4], b'-');
        assert_eq!(today.as_bytes()[7], b'-');
    }

    #[test]
    fn test_detect_closing_brace_malform() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path();
        let file = vault.join("test.md");
        fs::write(
            &file,
            "---project: my-project\ntags: [my-project]\n---\nSome content\n",
        )
        .unwrap();

        let result = normalize_frontmatter(vault, &make_config(), false).unwrap();
        assert_eq!(result.scanned, 1);
        assert!(
            result
                .issues
                .iter()
                .any(|i| i.issue == "closing_brace_malform")
        );
    }

    #[test]
    fn test_detect_broken_yaml_list() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path();
        let file = vault.join("test.md");
        fs::write(&file, "---\ntags:\ncreated: 2024-01-01\n---\nContent\n").unwrap();

        let result = normalize_frontmatter(vault, &make_config(), false).unwrap();
        assert_eq!(result.scanned, 1);
        assert!(result.issues.iter().any(|i| i.issue == "broken_yaml_list"));
        // No --fix: should NOT modify the file
        let content = fs::read_to_string(&file).unwrap();
        assert!(content.contains("tags:\n"));
        assert!(!content.contains("tags: []"));
    }

    #[test]
    fn test_fix_broken_yaml_list() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path();
        let file = vault.join("test.md");
        fs::write(&file, "---\ntags:\ncreated: 2024-01-01\n---\nContent\n").unwrap();

        let result = normalize_frontmatter(vault, &make_config(), true).unwrap();
        assert_eq!(result.fixed, 1);

        let fixed = fs::read_to_string(&file).unwrap();
        assert!(
            fixed.contains("tags: []"),
            "expected 'tags: []' in output, got:\n{}",
            fixed
        );
        assert!(
            fixed.contains("created: 2024-01-01"),
            "next line should be preserved, got:\n{}",
            fixed
        );
        assert!(
            fixed.contains("Content"),
            "body should be preserved, got:\n{}",
            fixed
        );
        // Bare `tags:` without [] should no longer appear
        assert!(
            !fixed.contains("tags:\n"),
            "bare 'tags:' should be replaced with 'tags: []', got:\n{}",
            fixed
        );
    }

    #[test]
    fn test_detect_empty_tags() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path();
        let file = vault.join("test.md");
        fs::write(&file, "---\nproject: my-project\ntags: []\n---\nContent\n").unwrap();

        let result = normalize_frontmatter(vault, &make_config(), false).unwrap();
        assert_eq!(result.scanned, 1);
        assert!(result.issues.iter().any(|i| i.issue == "empty_tags"));
        // empty_tags should never be auto-fixed
        assert!(
            result
                .issues
                .iter()
                .all(|i| i.issue != "empty_tags" || !i.fixed)
        );
    }

    #[test]
    fn test_detect_missing_frontmatter() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path();
        let project_dir = vault
            .join("99-Archives")
            .join("projects")
            .join("my-project");
        fs::create_dir_all(&project_dir).unwrap();
        let file = project_dir.join("PRD.md");
        fs::write(
            &file,
            "# Product Requirements\n\nSome content without frontmatter.\n",
        )
        .unwrap();

        let result = normalize_frontmatter(vault, &make_config(), false).unwrap();
        let missing = result
            .issues
            .iter()
            .find(|i| i.issue == "missing_frontmatter");
        assert!(
            missing.is_some(),
            "expected missing_frontmatter issue, got: {:?}",
            result.issues
        );
        let issue = missing.unwrap();
        assert!(issue.detail.contains("PRD.md"));
        assert!(!issue.fixed); // no --fix
    }

    #[test]
    fn test_fix_closing_brace_malform() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path();
        let file = vault.join("test.md");
        let original = "---project: my-project\ntags: [my-project]\n---\nSome content\n";
        fs::write(&file, original).unwrap();

        let result = normalize_frontmatter(vault, &make_config(), true).unwrap();
        assert_eq!(result.fixed, 1);

        let fixed = fs::read_to_string(&file).unwrap();
        assert!(fixed.starts_with("---\n"));
        assert!(fixed.contains("project: my-project"));
        assert!(!fixed.contains("---project:"));
        assert!(fixed.contains("tags: [my-project]"));
        assert!(fixed.contains("Some content"));
    }

    #[test]
    fn test_fix_missing_frontmatter() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path();
        let project_dir = vault.join("99-Archives").join("projects").join("alcove");
        fs::create_dir_all(&project_dir).unwrap();
        let file = project_dir.join("ARCHITECTURE.md");
        fs::write(&file, "# Architecture\n\nSystem design.\n").unwrap();

        let result = normalize_frontmatter(vault, &make_config(), true).unwrap();
        assert_eq!(result.fixed, 1);

        let fixed = fs::read_to_string(&file).unwrap();
        assert!(fixed.starts_with("---\n"));
        assert!(fixed.contains("project: alcove"));
        assert!(fixed.contains("type/architecture"));
        assert!(fixed.contains("layer/raw"));
        assert!(fixed.contains("# Architecture"));
    }

    #[test]
    fn test_skip_system_dirs() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path();

        // Create a file in .obsidian/ that should be skipped
        let obsidian_dir = vault.join(".obsidian");
        fs::create_dir_all(&obsidian_dir).unwrap();
        fs::write(obsidian_dir.join("malformed.md"), "---project: bad\n---\n").unwrap();

        // Create a valid file at root
        fs::write(vault.join("good.md"), "---\nproject: ok\n---\nFine\n").unwrap();

        let result = normalize_frontmatter(vault, &make_config(), false).unwrap();
        assert_eq!(result.scanned, 1); // only good.md
        assert!(result.issues.is_empty());
    }

    #[test]
    fn test_no_fix_does_not_modify() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path();
        let file = vault.join("test.md");
        let original = "---project: my-project\ntags: [my-project]\n---\nContent\n";
        fs::write(&file, original).unwrap();

        let result = normalize_frontmatter(vault, &make_config(), false).unwrap();
        assert_eq!(result.fixed, 0);

        let content = fs::read_to_string(&file).unwrap();
        assert_eq!(content, original);
    }
}
