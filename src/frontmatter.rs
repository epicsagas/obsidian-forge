use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fs, path::Path, sync::OnceLock};
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
    dry_run: bool,
) -> Result<FrontmatterResult> {
    let mut result = FrontmatterResult::default();

    for entry in WalkDir::new(vault_root)
        .into_iter()
        .filter_entry(|e| !is_vault_excluded(e.path()))
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
                fixed: !dry_run,
            });
            if !dry_run {
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
                fixed: !dry_run,
            });
            if !dry_run {
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
                    fixed: !dry_run,
                });

                if !dry_run {
                    let fm = format!(
                        "---\nproject: {}\ntags: [{}, layer/raw, {}]\n---\n",
                        project_name, project_name, doc_type
                    );
                    modified = format!("{}{}", fm, content);
                    file_fixed = true;
                }
            }
        }

        if file_fixed && !dry_run {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn make_config() -> ForgeConfig {
        ForgeConfig::default_for("test-vault")
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

        let result = normalize_frontmatter(vault, &make_config(), true).unwrap();
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

        let result = normalize_frontmatter(vault, &make_config(), true).unwrap();
        assert_eq!(result.scanned, 1);
        assert!(result.issues.iter().any(|i| i.issue == "broken_yaml_list"));
        // dry_run: should NOT modify the file
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

        let result = normalize_frontmatter(vault, &make_config(), false).unwrap();
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

        let result = normalize_frontmatter(vault, &make_config(), true).unwrap();
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

        let result = normalize_frontmatter(vault, &make_config(), true).unwrap();
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
        assert!(!issue.fixed); // dry run
    }

    #[test]
    fn test_fix_closing_brace_malform() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path();
        let file = vault.join("test.md");
        let original = "---project: my-project\ntags: [my-project]\n---\nSome content\n";
        fs::write(&file, original).unwrap();

        let result = normalize_frontmatter(vault, &make_config(), false).unwrap();
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

        let result = normalize_frontmatter(vault, &make_config(), false).unwrap();
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

        let result = normalize_frontmatter(vault, &make_config(), true).unwrap();
        assert_eq!(result.scanned, 1); // only good.md
        assert!(result.issues.is_empty());
    }

    #[test]
    fn test_dry_run_does_not_modify() {
        let dir = tempfile::tempdir().unwrap();
        let vault = dir.path();
        let file = vault.join("test.md");
        let original = "---project: my-project\ntags: [my-project]\n---\nContent\n";
        fs::write(&file, original).unwrap();

        let result = normalize_frontmatter(vault, &make_config(), true).unwrap();
        assert_eq!(result.fixed, 0);

        let content = fs::read_to_string(&file).unwrap();
        assert_eq!(content, original);
    }
}
