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
// Shared helpers
// ---------------------------------------------------------------------------

/// Check if `tag` is present in `tags_set`. If not, push a TagIssue and return
/// the tag as `Some(String)` (for adding to a missing_tags vec).
fn check_missing_tag(
    tags_set: &HashSet<&str>,
    tag: &str,
    issue_type: &str,
    file: &str,
    issues: &mut Vec<TagIssue>,
) -> Option<String> {
    if tags_set.contains(tag) {
        return None;
    }
    issues.push(TagIssue {
        file: file.to_string(),
        issue: issue_type.to_string(),
        detail: format!("Expected tag: {}", tag),
        fixed: false,
    });
    Some(tag.to_string())
}

/// Apply tag injection and optional YAML malform fix, then write the file and
/// mark the relevant issues as fixed.
#[allow(clippy::too_many_arguments)]
fn apply_tag_fixes(
    path: &Path,
    original_content: &str,
    yaml: &str,
    body: &str,
    fix: bool,
    missing_tags: &[String],
    malform_line: Option<&str>,
    result: &mut TagCheckResult,
    first_issue_idx: usize,
) -> Result<bool> {
    if !fix || (missing_tags.is_empty() && malform_line.is_none()) {
        return Ok(false);
    }

    let mut fixed_yaml = yaml.to_string();

    // Fix malform first
    if malform_line.is_some() {
        fixed_yaml = fix_yaml_malform(&fixed_yaml);
    }

    // Inject missing tags
    if !missing_tags.is_empty() {
        fixed_yaml = inject_tags_into_yaml(&fixed_yaml, missing_tags);
    }

    let new_content = crate::vault_utils::reassemble_frontmatter(&fixed_yaml, body);
    if new_content != original_content {
        fs::write(path, &new_content)?;
        for issue in result.issues.iter_mut().skip(first_issue_idx) {
            issue.fixed = true;
        }
        return Ok(true);
    }
    Ok(false)
}

// ---------------------------------------------------------------------------
// Shared tag-check logic for files WITH frontmatter
// ---------------------------------------------------------------------------

/// A required tag to check: `(tag_value, issue_type)`.
type RequiredTag<'a> = (&'a str, &'a str);

/// Check a list of required tags against parsed frontmatter and apply fixes.
///
/// This encapsulates the common pattern shared by both `scan_project_docs` and
/// `scan_resource_docs` for files that already have YAML frontmatter:
/// 1. Parse tags from YAML, build HashSet
/// 2. Check each required tag via `check_missing_tag`
/// 3. Apply fixes via `apply_tag_fixes`
///
/// `first_issue_idx` should be the index of the first issue recorded for this
/// file (before calling this function), so that `apply_tag_fixes` can mark all
/// issues — including any pre-existing ones like yaml_malform — as fixed.
#[allow(clippy::too_many_arguments)]
fn check_and_fix_tags(
    yaml: &str,
    body: &str,
    path: &Path,
    relative: &str,
    content: &str,
    required_tags: &[RequiredTag<'_>],
    fix: bool,
    malform_line: Option<&str>,
    first_issue_idx: usize,
    result: &mut TagCheckResult,
) -> Result<()> {
    let tags = parse_tags_from_yaml(yaml);
    let tags_set: HashSet<&str> = tags.iter().map(|s| s.as_str()).collect();

    let mut missing_tags: Vec<String> = Vec::new();

    for (tag, issue_type) in required_tags {
        if let Some(t) = check_missing_tag(&tags_set, tag, issue_type, relative, &mut result.issues)
        {
            missing_tags.push(t);
        }
    }

    apply_tag_fixes(
        path,
        content,
        yaml,
        body,
        fix,
        &missing_tags,
        malform_line,
        result,
        first_issue_idx,
    )?;

    Ok(())
}

/// Fix a file that has NO frontmatter by injecting tags into a new frontmatter block.
fn apply_no_frontmatter_fixes(
    path: &Path,
    original_content: &str,
    fix: bool,
    missing_tags: &[String],
    result: &mut TagCheckResult,
    first_issue_idx: usize,
) -> Result<bool> {
    if !fix || missing_tags.is_empty() {
        return Ok(false);
    }

    let yaml = format!("tags: [{}]\n", missing_tags.join(", "));
    let new_content = crate::vault_utils::reassemble_frontmatter(&yaml, original_content);
    if new_content != *original_content {
        fs::write(path, &new_content)?;
        for issue in result.issues.iter_mut().skip(first_issue_idx) {
            issue.fixed = true;
        }
        return Ok(true);
    }
    Ok(false)
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
        .filter_entry(|e| !is_vault_excluded(e.path(), vault_root))
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
            // No frontmatter — report and fix missing tags (shared pattern)
            let mut missing_tags: Vec<String> = Vec::new();
            let first_issue_idx = result.issues.len();

            let mut required: Vec<(&str, &str)> = vec![("layer/raw", "missing_layer")];
            if let Some(ref tt) = type_tag {
                required.push((tt.as_str(), "missing_type"));
            }
            if let Some(ref pn) = project_name {
                required.push((pn.as_str(), "missing_project"));
            }

            for (tag, issue_type) in &required {
                if let Some(t) = check_missing_tag(
                    &HashSet::new(),
                    tag,
                    issue_type,
                    &relative,
                    &mut result.issues,
                ) {
                    missing_tags.push(t);
                }
            }

            apply_no_frontmatter_fixes(
                path,
                &content,
                fix,
                &missing_tags,
                result,
                first_issue_idx,
            )?;
            continue;
        };

        // Build required tag list for the common checker
        let mut required_tags: Vec<RequiredTag<'_>> = vec![("layer/raw", "missing_layer")];
        if let Some(ref tt) = type_tag {
            required_tags.push((tt.as_str(), "missing_type"));
        }
        if let Some(ref pn) = project_name {
            required_tags.push((pn.as_str(), "missing_project"));
        }

        // Record first_issue_idx BEFORE any issues for this file, so that
        // apply_tag_fixes can mark all of them (malform + missing tags) as fixed.
        let first_issue_idx = result.issues.len();

        // Check YAML malform (project-docs specific)
        let has_malform = detect_yaml_malform(&yaml);
        if let Some(malformed_line) = &has_malform {
            result.issues.push(TagIssue {
                file: relative.clone(),
                issue: "yaml_malform".to_string(),
                detail: format!("Malformed YAML: {}", malformed_line),
                fixed: false,
            });
        }

        // Shared tag-check + fix
        check_and_fix_tags(
            &yaml,
            &body,
            path,
            &relative,
            &content,
            &required_tags,
            fix,
            has_malform.as_deref(),
            first_issue_idx,
            result,
        )?;
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
        .filter_entry(|e| !is_vault_excluded(e.path(), vault_root))
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
            // No frontmatter — report and fix missing tags
            let mut missing_tags: Vec<String> = Vec::new();
            let first_issue_idx = result.issues.len();
            for tag in [
                ("layer/raw", "missing_layer"),
                ("type/reference", "missing_type"),
            ] {
                if let Some(t) =
                    check_missing_tag(&HashSet::new(), tag.0, tag.1, &relative, &mut result.issues)
                {
                    missing_tags.push(t);
                }
            }

            apply_no_frontmatter_fixes(
                path,
                &content,
                fix,
                &missing_tags,
                result,
                first_issue_idx,
            )?;
            continue;
        };

        let yaml = caps.get(1).unwrap().as_str().to_string();
        let body = caps.get(2).unwrap().as_str().to_string();

        let required_tags: &[RequiredTag<'_>] = &[
            ("layer/raw", "missing_layer"),
            ("type/reference", "missing_type"),
        ];

        let first_issue_idx = result.issues.len();

        check_and_fix_tags(
            &yaml,
            &body,
            path,
            &relative,
            &content,
            required_tags,
            fix,
            None,
            first_issue_idx,
            result,
        )?;
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
    use crate::frontmatter::normalize_frontmatter;
    use crate::graph::strengthen_graph;
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
    fn test_fix_keeps_closing_delimiter_on_own_line() {
        // Regression for #25: `check-tags --fix` glued the closing `---` to the last
        // frontmatter key when injecting a missing project tag (e.g. `created: 2026-05-14---`).
        let vault = create_test_vault();
        let config = make_config();

        write_file(
            vault.path(),
            "99-Archives/projects/test-project/PRD.md",
            "---\ntags: [layer/raw, type/prd]\ncreated: 2026-05-14\n---\n# PRD\n",
        );

        let result = check_tags(vault.path(), &config, true, TagScope::Project).expect("check");
        assert!(result.fixed > 0, "Expected the project tag to be injected");

        let fixed_content = read_file(vault.path(), "99-Archives/projects/test-project/PRD.md");

        // The closing delimiter must sit on its own line, preceded by a newline.
        assert!(
            !fixed_content.contains("2026-05-14---"),
            "closing --- must not be glued to the last key; got:\n{fixed_content}"
        );
        assert!(
            fixed_content.contains("2026-05-14\n---\n"),
            "closing --- should be on its own line after the last key; got:\n{fixed_content}"
        );
        // The whole frontmatter must still match the standard delimiter pattern.
        assert!(
            crate::vault_utils::frontmatter_re().is_match(&fixed_content),
            "frontmatter should match the standard delimiter pattern; got:\n{fixed_content}"
        );
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

    #[test]
    fn test_fix_project_doc_no_frontmatter() {
        let vault = create_test_vault();
        let config = make_config();

        // Project doc with NO frontmatter at all
        write_file(
            vault.path(),
            "99-Archives/projects/test-project/PRD.md",
            "# Product Requirements\n\nSome content.\n",
        );

        // --fix should inject frontmatter with tags
        let result = check_tags(vault.path(), &config, true, TagScope::Project).expect("check");

        assert!(result.fixed > 0, "Should fix at least one file");

        let fixed_content = read_file(vault.path(), "99-Archives/projects/test-project/PRD.md");
        assert!(
            fixed_content.contains("layer/raw"),
            "Should contain layer/raw tag"
        );
        assert!(
            fixed_content.contains("type/prd"),
            "Should contain type/prd tag"
        );
        assert!(
            fixed_content.contains("test-project"),
            "Should contain project tag"
        );
        assert!(
            fixed_content.contains("# Product Requirements"),
            "Body should be preserved"
        );
    }

    #[test]
    fn test_nested_repo_excluded_from_fixers() {
        // Scope: the frontmatter + tag fixers (99-Archives/projects). A nested
        // standalone git repo must be left byte-identical while a real project
        // doc is fixed. The graph/inject_backlinks path is covered separately by
        // `test_nested_repo_excluded_from_graph`.
        let vault = create_test_vault();
        let config = make_config();

        // A normal project doc — should be fixed by both fixers.
        write_file(
            vault.path(),
            "99-Archives/projects/proj/PRD.md",
            "# PRD\n\nNo frontmatter yet.\n",
        );

        // A nested standalone git repo (e.g. a public `release/` bundle).
        // It must never be touched by vault fixers, independent of the dir name.
        let repo = vault
            .path()
            .join("99-Archives")
            .join("projects")
            .join("proj")
            .join("release");
        fs::create_dir_all(repo.join(".git")).expect("mkdir .git");
        fs::write(repo.join(".git").join("HEAD"), "").expect("write HEAD");
        let nested = repo.join("paper.md");
        fs::write(&nested, "Original body, no frontmatter.\n").expect("write nested");

        // Both fixers run with --fix.
        let fm = normalize_frontmatter(vault.path(), &config, true).expect("frontmatter");
        assert!(fm.scanned >= 1, "walker should still scan the vault");
        let _ = check_tags(vault.path(), &config, true, TagScope::Project).expect("tags");

        // The nested repo file must be byte-identical (untouched) by the fixers.
        let after = fs::read_to_string(&nested).expect("read nested");
        assert_eq!(
            after, "Original body, no frontmatter.\n",
            "nested repo file must be left untouched by vault fixers"
        );

        // Sanity: the real doc WAS modified by the frontmatter fixer.
        let proj = fs::read_to_string(vault.path().join("99-Archives/projects/proj/PRD.md"))
            .expect("read proj");
        assert!(
            proj.starts_with("---\n"),
            "project doc should be fixed by frontmatter fixer"
        );
    }

    #[test]
    fn test_nested_repo_excluded_from_graph() {
        // Scope: strengthen-graph / inject_backlinks, the second #38 contaminator
        // that writes `## See Also` footers. A nested repo must be left
        // byte-identical while a real top-level project doc receives a backlink.
        let vault = create_test_vault();
        let mut config = make_config();
        // Focus the graph pipeline on the backlinks contaminator only, so any
        // unrelated graph step can't mask a regression in inject_backlinks.
        config.graph.bridge_notes = false;
        config.graph.related_projects = false;
        config.graph.auto_tags = false;

        // The graph scanner only reaches top-level project dirs (99-Archives is a
        // system dir it skips), so the backlinks scenario lives under 04-Writing.
        write_file(
            vault.path(),
            "04-Writing/paper/proj/PRD.md",
            "# PRD\n\nGraph body.\n",
        );

        // A nested standalone git repo at the canonical #38 depth.
        let repo = vault
            .path()
            .join("04-Writing")
            .join("paper")
            .join("proj")
            .join("release");
        fs::create_dir_all(repo.join(".git")).expect("mkdir .git");
        fs::write(repo.join(".git").join("HEAD"), "").expect("write HEAD");
        let nested = repo.join("paper.md");
        fs::write(&nested, "Original body, no frontmatter.\n").expect("write nested");

        strengthen_graph(vault.path(), &config).expect("strengthen graph");

        let after = fs::read_to_string(&nested).expect("read nested");
        assert_eq!(
            after, "Original body, no frontmatter.\n",
            "nested release bundle must be left untouched by inject_backlinks"
        );

        // Sanity: the real top-level doc DID receive a backlink footer.
        let graph_doc = fs::read_to_string(vault.path().join("04-Writing/paper/proj/PRD.md"))
            .expect("read graph doc");
        assert!(
            graph_doc.contains("## See Also"),
            "top-level project doc should receive a backlink footer from the graph pipeline"
        );
    }

    #[test]
    fn test_nested_repo_gitfile_excluded() {
        let vault = create_test_vault();
        let config = make_config();

        // A nested git *worktree/submodule*: its `.git` is a FILE (a gitdir
        // pointer), not a directory. The old `is_dir()`-only check missed this;
        // `exists()` must exclude it so the file stays untouched.
        let repo = vault
            .path()
            .join("99-Archives")
            .join("projects")
            .join("proj")
            .join("release");
        fs::create_dir_all(&repo).expect("mkdir release");
        fs::write(repo.join(".git"), "gitdir: ../.git/worktrees/example\n")
            .expect("write .git file");
        let nested = repo.join("paper.md");
        fs::write(&nested, "Original body, no frontmatter.\n").expect("write nested");

        let _ = normalize_frontmatter(vault.path(), &config, true).expect("frontmatter");
        let _ = check_tags(vault.path(), &config, true, TagScope::Project).expect("tags");

        let after = fs::read_to_string(&nested).expect("read nested");
        assert_eq!(
            after, "Original body, no frontmatter.\n",
            "nested repo with a `.git` FILE must be left untouched by vault fixers"
        );
    }
}
