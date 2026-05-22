use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{collections::HashSet, fs, path::Path, sync::OnceLock};
use tracing::info;
use walkdir::WalkDir;

use crate::config::ForgeConfig;
use crate::vault_utils::{
    doc_type_tag, frontmatter_re, is_vault_excluded, supplementary_doc_type_tag,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum TagScope {
    Project,
    Vault,
}

// ---------------------------------------------------------------------------
// Result types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TagCheckResult {
    pub scanned: usize,
    pub issues: Vec<TagIssue>,
    pub fixed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TagIssue {
    pub file: String,
    pub issue: String,
    pub detail: String,
    pub fixed: bool,
}

impl std::fmt::Display for TagCheckResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "=== Tag Check ===")?;
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

// ---------------------------------------------------------------------------
// Doc type mapping
// ---------------------------------------------------------------------------

#[allow(dead_code)]
const PRIMARY_FILES: &[&str] = &[
    "PRD.md",
    "ARCHITECTURE.md",
    "CONVENTIONS.md",
    "DECISIONS.md",
    "PROGRESS.md",
    "DEBT.md",
    "SECRETS_MAP.md",
    "CODE_INDEX.md",
];

#[allow(dead_code)]
const SUPPLEMENTARY_DIRS: &[&str] = &["reports", "specs", "plans", "research", "strategy"];

// ---------------------------------------------------------------------------
// Frontmatter parsing
// ---------------------------------------------------------------------------

fn tags_array_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?m)^tags:\s*\[(.*?)\]").expect("valid tags array regex"))
}

fn tags_list_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?m)^tags:\s*$").expect("valid tags list header regex"))
}

fn tags_list_item_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?m)^\s+-\s+(.+)$").expect("valid tags list item regex"))
}

fn parse_tags_from_yaml(yaml: &str) -> Vec<String> {
    // Try YAML array format: tags: [tag1, tag2, ...]
    if let Some(caps) = tags_array_re().captures(yaml) {
        let tags_str = caps.get(1).unwrap().as_str();
        return tags_str
            .split(',')
            .map(|t| t.trim().trim_matches('"').trim_matches('\'').to_string())
            .filter(|t| !t.is_empty())
            .collect();
    }

    // Try YAML list format:
    // tags:
    //   - tag1
    //   - tag2
    if tags_list_re().is_match(yaml) {
        return tags_list_item_re()
            .captures_iter(yaml)
            .map(|c| {
                c.get(1)
                    .unwrap()
                    .as_str()
                    .trim()
                    .trim_matches('"')
                    .trim_matches('\'')
                    .to_string()
            })
            .filter(|t| !t.is_empty())
            .collect();
    }

    Vec::new()
}

// ---------------------------------------------------------------------------
// YAML malform detection
// ---------------------------------------------------------------------------

fn detect_yaml_malform(yaml: &str) -> Option<String> {
    // Detect `]project:` on the same line (missing newline before `project:` field)
    for line in yaml.lines() {
        if line.contains("]project:") {
            return Some(line.to_string());
        }
    }
    None
}

fn fix_yaml_malform(yaml: &str) -> String {
    // Fix `]project:` → `]\nproject:`
    yaml.replace("]project:", "]\nproject:")
}

// ---------------------------------------------------------------------------
// Tag injection
// ---------------------------------------------------------------------------

fn inject_tags_into_yaml(yaml: &str, tags_to_add: &[String]) -> String {
    let existing = parse_tags_from_yaml(yaml);
    let mut all_tags: Vec<String> = existing;

    for tag in tags_to_add {
        if !all_tags.iter().any(|t| t == tag) {
            all_tags.push(tag.clone());
        }
    }

    let new_tags_str = all_tags.join(", ");

    // If tags line exists as array format, replace it
    if tags_array_re().is_match(yaml) {
        return tags_array_re()
            .replace(yaml, format!("tags: [{}]", new_tags_str).as_str())
            .to_string();
    }

    // If tags line exists as list format, replace with array format
    if tags_list_re().is_match(yaml) {
        // Remove all list items under tags:
        let without_items = tags_list_item_re().replace_all(yaml, "").to_string();
        // Replace the tags: line itself
        return tags_list_re()
            .replace(&without_items, format!("tags: [{}]", new_tags_str).as_str())
            .to_string();
    }

    // No tags line exists — add after the --- opener
    format!("tags: [{}]\n{}", new_tags_str, yaml)
}

// ---------------------------------------------------------------------------
// Main check function
// ---------------------------------------------------------------------------

pub fn check_tags(
    vault_root: &Path,
    _config: &ForgeConfig,
    fix: bool,
    scope: TagScope,
) -> Result<TagCheckResult> {
    let mut result = TagCheckResult::default();
    let fm = frontmatter_re();

    // --- Scan project docs in 99-Archives/projects/{project}/ ---
    let projects_dir = vault_root.join("99-Archives").join("projects");
    if projects_dir.exists() {
        scan_project_docs(&projects_dir, vault_root, fm, fix, &mut result)?;
    }

    // --- Scan 03-Resources/Laws-Of-Software-Engineering/ (vault scope only) ---
    if scope == TagScope::Vault {
        let laws_dir = vault_root
            .join("03-Resources")
            .join("Laws-Of-Software-Engineering");
        if laws_dir.exists() {
            scan_resource_docs(&laws_dir, vault_root, fm, fix, &mut result)?;
        }
    }

    result.fixed = result.issues.iter().filter(|i| i.fixed).count();
    info!(
        "Tag check complete: scanned {}, issues {}, fixed {}",
        result.scanned,
        result.issues.len(),
        result.fixed,
    );

    Ok(result)
}

fn scan_project_docs(
    projects_dir: &Path,
    vault_root: &Path,
    fm: &Regex,
    fix: bool,
    result: &mut TagCheckResult,
) -> Result<()> {
    for entry in WalkDir::new(projects_dir)
        .into_iter()
        .filter_entry(|e| !is_vault_excluded(e.path()))
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
    {
        let path = entry.path();
        let relative = path
            .strip_prefix(vault_root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Determine project folder name (direct child of projects/)
        let project_name = extract_project_name(path, projects_dir);

        // Determine expected type tag
        let type_tag = if let Some(tt) = doc_type_tag(file_name) {
            Some(tt.to_string())
        } else {
            // Check supplementary dirs
            let parent_dir_name = path
                .parent()
                .and_then(|p| p.file_name())
                .and_then(|n| n.to_str())
                .unwrap_or("");
            supplementary_doc_type_tag(parent_dir_name).map(|t| t.to_string())
        };

        // Only check PRIMARY and known supplementary docs
        if type_tag.is_none() {
            continue;
        }

        result.scanned += 1;

        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let (yaml, body) = if let Some(caps) = fm.captures(&content) {
            (
                caps.get(1).unwrap().as_str().to_string(),
                caps.get(2).unwrap().as_str().to_string(),
            )
        } else {
            // No frontmatter — report missing tags
            if let Some(ref tt) = type_tag {
                for missing in [
                    "layer/raw",
                    tt.as_str(),
                    project_name.as_deref().unwrap_or(""),
                ]
                .iter()
                .filter(|m| !m.is_empty())
                {
                    let issue_type = if *missing == "layer/raw" {
                        "missing_layer"
                    } else if missing.starts_with("type/") {
                        "missing_type"
                    } else {
                        "missing_project"
                    };
                    result.issues.push(TagIssue {
                        file: relative.clone(),
                        issue: issue_type.to_string(),
                        detail: format!("Expected tag: {}", missing),
                        fixed: false,
                    });
                }
            }
            continue;
        };

        let tags = parse_tags_from_yaml(&yaml);
        let tags_set: HashSet<&str> = tags.iter().map(|s| s.as_str()).collect();

        let mut missing_tags: Vec<String> = Vec::new();

        // Check layer/raw
        if !tags_set.contains("layer/raw") {
            result.issues.push(TagIssue {
                file: relative.clone(),
                issue: "missing_layer".to_string(),
                detail: "Expected tag: layer/raw".to_string(),
                fixed: false,
            });
            missing_tags.push("layer/raw".to_string());
        }

        // Check type tag
        if let Some(ref tt) = type_tag
            && !tags_set.contains(tt.as_str())
        {
            result.issues.push(TagIssue {
                file: relative.clone(),
                issue: "missing_type".to_string(),
                detail: format!("Expected tag: {}", tt),
                fixed: false,
            });
            missing_tags.push(tt.clone());
        }

        // Check project tag
        if let Some(ref pn) = project_name
            && !tags_set.contains(pn.as_str())
        {
            result.issues.push(TagIssue {
                file: relative.clone(),
                issue: "missing_project".to_string(),
                detail: format!("Expected tag: {}", pn),
                fixed: false,
            });
            missing_tags.push(pn.clone());
        }

        // Check YAML malform
        if let Some(malformed_line) = detect_yaml_malform(&yaml) {
            result.issues.push(TagIssue {
                file: relative.clone(),
                issue: "yaml_malform".to_string(),
                detail: format!("Malformed YAML: {}", malformed_line),
                fixed: false,
            });
        }

        // Apply fixes
        if fix && (!missing_tags.is_empty() || detect_yaml_malform(&yaml).is_some()) {
            let mut fixed_yaml = yaml.clone();

            // Fix malform first
            if detect_yaml_malform(&fixed_yaml).is_some() {
                fixed_yaml = fix_yaml_malform(&fixed_yaml);
            }

            // Inject missing tags
            if !missing_tags.is_empty() {
                fixed_yaml = inject_tags_into_yaml(&fixed_yaml, &missing_tags);
            }

            let new_content = format!("---\n{}---\n{}", fixed_yaml, body);
            if new_content != content {
                fs::write(path, &new_content)?;

                // Mark matching issues as fixed
                for issue in result.issues.iter_mut().rev() {
                    if issue.file == relative && !issue.fixed {
                        issue.fixed = true;
                    }
                }
            }
        }
    }

    Ok(())
}

fn scan_resource_docs(
    laws_dir: &Path,
    vault_root: &Path,
    fm: &Regex,
    fix: bool,
    result: &mut TagCheckResult,
) -> Result<()> {
    for entry in WalkDir::new(laws_dir)
        .into_iter()
        .filter_entry(|e| !is_vault_excluded(e.path()))
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| e.path().extension().is_some_and(|ext| ext == "md"))
    {
        let path = entry.path();
        let relative = path
            .strip_prefix(vault_root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();

        result.scanned += 1;

        let content = match fs::read_to_string(path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let Some(caps) = fm.captures(&content) else {
            // No frontmatter
            let mut missing_tags: Vec<String> = Vec::new();
            for tag in ["layer/raw", "type/reference"] {
                result.issues.push(TagIssue {
                    file: relative.clone(),
                    issue: if tag == "layer/raw" {
                        "missing_layer"
                    } else {
                        "missing_type"
                    }
                    .to_string(),
                    detail: format!("Expected tag: {}", tag),
                    fixed: false,
                });
                missing_tags.push(tag.to_string());
            }

            if fix {
                let yaml = format!("tags: [{}]\n", missing_tags.join(", "));
                let new_content = format!("---\n{}---\n{}", yaml, content);
                fs::write(path, &new_content)?;
                for issue in result.issues.iter_mut().rev() {
                    if issue.file == relative && !issue.fixed {
                        issue.fixed = true;
                    }
                }
            }
            continue;
        };

        let yaml = caps.get(1).unwrap().as_str().to_string();
        let body = caps.get(2).unwrap().as_str().to_string();
        let tags = parse_tags_from_yaml(&yaml);
        let tags_set: HashSet<&str> = tags.iter().map(|s| s.as_str()).collect();

        let mut missing_tags: Vec<String> = Vec::new();

        if !tags_set.contains("layer/raw") {
            result.issues.push(TagIssue {
                file: relative.clone(),
                issue: "missing_layer".to_string(),
                detail: "Expected tag: layer/raw".to_string(),
                fixed: false,
            });
            missing_tags.push("layer/raw".to_string());
        }

        if !tags_set.contains("type/reference") {
            result.issues.push(TagIssue {
                file: relative.clone(),
                issue: "missing_type".to_string(),
                detail: "Expected tag: type/reference".to_string(),
                fixed: false,
            });
            missing_tags.push("type/reference".to_string());
        }

        if fix && !missing_tags.is_empty() {
            let fixed_yaml = inject_tags_into_yaml(&yaml, &missing_tags);
            let new_content = format!("---\n{}---\n{}", fixed_yaml, body);
            if new_content != content {
                fs::write(path, &new_content)?;
                for issue in result.issues.iter_mut().rev() {
                    if issue.file == relative && !issue.fixed {
                        issue.fixed = true;
                    }
                }
            }
        }
    }

    Ok(())
}

/// Extract the project folder name from a file path under `projects/`.
/// E.g. `99-Archives/projects/alcove/PRD.md` → `alcove`
fn extract_project_name(file_path: &Path, projects_dir: &Path) -> Option<String> {
    let relative = file_path.strip_prefix(projects_dir).ok()?;
    let first = relative.components().next()?;
    first.as_os_str().to_str().map(|s| s.to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_vault() -> TempDir {
        let dir = TempDir::new().expect("temp dir");
        let projects = dir.path().join("99-Archives").join("projects");
        fs::create_dir_all(projects.join("test-project")).expect("dirs");
        let resources = dir
            .path()
            .join("03-Resources")
            .join("Laws-Of-Software-Engineering");
        fs::create_dir_all(&resources).expect("dirs");
        dir
    }

    fn make_config() -> ForgeConfig {
        ForgeConfig::default_for("test-vault")
    }

    fn write_file(dir: &Path, rel_path: &str, content: &str) {
        let full = dir.join(rel_path);
        if let Some(parent) = full.parent() {
            fs::create_dir_all(parent).ok();
        }
        fs::write(full, content).expect("write");
    }

    fn read_file(dir: &Path, rel_path: &str) -> String {
        fs::read_to_string(dir.join(rel_path)).expect("read")
    }

    #[test]
    fn test_doc_type_mapping() {
        assert_eq!(doc_type_tag("PRD.md"), Some("type/prd"));
        assert_eq!(doc_type_tag("ARCHITECTURE.md"), Some("type/architecture"));
        assert_eq!(doc_type_tag("CONVENTIONS.md"), Some("type/convention"));
        assert_eq!(doc_type_tag("DECISIONS.md"), Some("type/decision"));
        assert_eq!(doc_type_tag("PROGRESS.md"), Some("type/progress"));
        assert_eq!(doc_type_tag("DEBT.md"), Some("type/debt"));
        assert_eq!(doc_type_tag("SECRETS_MAP.md"), Some("type/reference"));
        assert_eq!(doc_type_tag("CODE_INDEX.md"), Some("type/reference"));
        assert_eq!(doc_type_tag("random.md"), None);
    }

    #[test]
    fn test_supplementary_doc_type_mapping() {
        assert_eq!(supplementary_doc_type_tag("reports"), Some("type/report"));
        assert_eq!(supplementary_doc_type_tag("specs"), Some("type/spec"));
        assert_eq!(supplementary_doc_type_tag("plans"), Some("type/plan"));
        assert_eq!(
            supplementary_doc_type_tag("research"),
            Some("type/research")
        );
        assert_eq!(
            supplementary_doc_type_tag("strategy"),
            Some("type/strategy")
        );
        assert_eq!(supplementary_doc_type_tag("other"), None);
    }

    #[test]
    fn test_check_primary_doc_missing_tags() {
        let vault = create_test_vault();
        let config = make_config();

        // File with no tags at all
        write_file(
            vault.path(),
            "99-Archives/projects/test-project/PRD.md",
            "---\ntitle: Test\n---\n# PRD\n",
        );

        let result = check_tags(vault.path(), &config, false, TagScope::Project).expect("check");

        assert_eq!(result.scanned, 1);
        assert!(
            result.issues.len() >= 3,
            "Expected at least 3 issues (layer, type, project), got {}",
            result.issues.len()
        );

        let issue_types: Vec<&str> = result.issues.iter().map(|i| i.issue.as_str()).collect();
        assert!(issue_types.contains(&"missing_layer"));
        assert!(issue_types.contains(&"missing_type"));
        assert!(issue_types.contains(&"missing_project"));
        assert_eq!(result.fixed, 0);
    }

    #[test]
    fn test_check_yaml_malform() {
        let vault = create_test_vault();
        let config = make_config();

        // File with ]project: on same line
        write_file(
            vault.path(),
            "99-Archives/projects/test-project/PRD.md",
            "---\ntags: [layer/raw, type/prd]project: foo\ntitle: Test\n---\n# PRD\n",
        );

        let result = check_tags(vault.path(), &config, false, TagScope::Project).expect("check");

        let malform_issues: Vec<&TagIssue> = result
            .issues
            .iter()
            .filter(|i| i.issue == "yaml_malform")
            .collect();
        assert_eq!(malform_issues.len(), 1, "Expected 1 yaml_malform issue");
        assert!(malform_issues[0].detail.contains("]project:"));
    }

    #[test]
    fn test_fix_adds_missing_tags() {
        let vault = create_test_vault();
        let config = make_config();

        write_file(
            vault.path(),
            "99-Archives/projects/test-project/PRD.md",
            "---\ntitle: Test\n---\n# PRD\n",
        );

        let result = check_tags(vault.path(), &config, true, TagScope::Project).expect("check");

        assert!(result.fixed > 0, "Expected at least one fix");

        let fixed_content = read_file(vault.path(), "99-Archives/projects/test-project/PRD.md");

        assert!(
            fixed_content.contains("layer/raw"),
            "Should contain layer/raw"
        );
        assert!(
            fixed_content.contains("type/prd"),
            "Should contain type/prd"
        );
        assert!(
            fixed_content.contains("test-project"),
            "Should contain project tag"
        );
        assert!(fixed_content.contains("# PRD"), "Body should be preserved");
    }

    #[test]
    fn test_fix_yaml_malform() {
        let vault = create_test_vault();
        let config = make_config();

        write_file(
            vault.path(),
            "99-Archives/projects/test-project/ARCHITECTURE.md",
            "---\ntags: [layer/raw, type/architecture]project: test-project\ntitle: Test\n---\n# Arch\n",
        );

        let result = check_tags(vault.path(), &config, true, TagScope::Project).expect("check");

        let malform_fixed = result
            .issues
            .iter()
            .any(|i| i.issue == "yaml_malform" && i.fixed);
        assert!(malform_fixed, "YAML malform should be fixed");

        let fixed = read_file(
            vault.path(),
            "99-Archives/projects/test-project/ARCHITECTURE.md",
        );
        assert!(
            fixed.contains("]\nproject:"),
            "Should have newline after ] before project:"
        );
    }

    #[test]
    fn test_scope_project_skips_resources() {
        let vault = create_test_vault();
        let config = make_config();

        // Resource file missing tags
        write_file(
            vault.path(),
            "03-Resources/Laws-Of-Software-Engineering/law1.md",
            "---\ntitle: Some Law\n---\nContent\n",
        );
        // Project file missing tags
        write_file(
            vault.path(),
            "99-Archives/projects/test-project/DEBT.md",
            "---\ntitle: Debt\n---\n# Debt\n",
        );

        let result = check_tags(vault.path(), &config, false, TagScope::Project).expect("check");

        // Only project doc should be scanned
        assert_eq!(result.scanned, 1);
        // Resource issues should not appear
        let resource_issues: Vec<&TagIssue> = result
            .issues
            .iter()
            .filter(|i| i.file.contains("03-Resources"))
            .collect();
        assert!(resource_issues.is_empty());
    }

    #[test]
    fn test_scope_vault_includes_resources() {
        let vault = create_test_vault();
        let config = make_config();

        write_file(
            vault.path(),
            "03-Resources/Laws-Of-Software-Engineering/law1.md",
            "---\ntitle: Some Law\n---\nContent\n",
        );
        write_file(
            vault.path(),
            "99-Archives/projects/test-project/DEBT.md",
            "---\ntitle: Debt\n---\n# Debt\n",
        );

        let result = check_tags(vault.path(), &config, false, TagScope::Vault).expect("check");

        assert!(result.scanned >= 2, "Should scan at least 2 files");
        let resource_issues: Vec<&TagIssue> = result
            .issues
            .iter()
            .filter(|i| i.file.contains("03-Resources"))
            .collect();
        assert!(
            !resource_issues.is_empty(),
            "Resource issues should be present"
        );
    }

    #[test]
    fn test_existing_tags_not_flagged() {
        let vault = create_test_vault();
        let config = make_config();

        write_file(
            vault.path(),
            "99-Archives/projects/test-project/PROGRESS.md",
            "---\ntags: [layer/raw, type/progress, test-project]\ntitle: Progress\n---\n# Progress\n",
        );

        let result = check_tags(vault.path(), &config, false, TagScope::Project).expect("check");

        assert_eq!(result.scanned, 1);
        assert!(
            result.issues.is_empty(),
            "No issues expected for fully tagged file"
        );
    }

    #[test]
    fn test_supplementary_dir_tags() {
        let vault = create_test_vault();
        let config = make_config();

        fs::create_dir_all(
            vault
                .path()
                .join("99-Archives/projects/test-project/reports"),
        )
        .ok();
        write_file(
            vault.path(),
            "99-Archives/projects/test-project/reports/weekly.md",
            "---\ntitle: Weekly\n---\n# Weekly Report\n",
        );

        let result = check_tags(vault.path(), &config, false, TagScope::Project).expect("check");

        assert_eq!(result.scanned, 1);
        let type_issues: Vec<&TagIssue> = result
            .issues
            .iter()
            .filter(|i| i.issue == "missing_type")
            .collect();
        assert!(!type_issues.is_empty());
        assert!(type_issues[0].detail.contains("type/report"));
    }

    #[test]
    fn test_excluded_dirs_skipped() {
        let vault = create_test_vault();
        let config = make_config();

        // _template dir should be excluded
        fs::create_dir_all(vault.path().join("99-Archives/projects/_template")).ok();
        write_file(
            vault.path(),
            "99-Archives/projects/_template/PRD.md",
            "---\ntitle: Template\n---\n# Template\n",
        );

        // seeded dir should be excluded
        fs::create_dir_all(
            vault
                .path()
                .join("99-Archives/projects/test-project/seeded"),
        )
        .ok();
        write_file(
            vault.path(),
            "99-Archives/projects/test-project/seeded/PRD.md",
            "---\ntitle: Seeded\n---\n# Seeded\n",
        );

        let result = check_tags(vault.path(), &config, false, TagScope::Project).expect("check");

        assert_eq!(result.scanned, 0, "Excluded dirs should not be scanned");
    }

    #[test]
    fn test_parse_tags_array_format() {
        let yaml = "tags: [layer/raw, type/prd, my-project]\ntitle: Test\n";
        let tags = parse_tags_from_yaml(yaml);
        assert_eq!(tags, vec!["layer/raw", "type/prd", "my-project"]);
    }

    #[test]
    fn test_parse_tags_list_format() {
        let yaml = "tags:\n  - layer/raw\n  - type/prd\n  - my-project\ntitle: Test\n";
        let tags = parse_tags_from_yaml(yaml);
        assert_eq!(tags, vec!["layer/raw", "type/prd", "my-project"]);
    }

    #[test]
    fn test_parse_tags_empty() {
        let yaml = "title: Test\n";
        let tags = parse_tags_from_yaml(yaml);
        assert!(tags.is_empty());
    }
}
