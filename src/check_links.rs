use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path, sync::OnceLock};
use tracing::{info, warn};
use walkdir::WalkDir;

use crate::config::ForgeConfig;
use crate::graph::wikilinks::build_vault_graph;
use crate::vault_utils::is_vault_excluded;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LinkCheckResult {
    pub total_links: usize,
    pub broken: Vec<BrokenLink>,
    pub fixed: Vec<BrokenLink>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum LinkIssue {
    Unresolved,
    FilenameMismatch,
    ExtensionMismatch,
}

impl std::fmt::Display for LinkIssue {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LinkIssue::Unresolved => write!(f, "unresolved"),
            LinkIssue::FilenameMismatch => write!(f, "filename_mismatch"),
            LinkIssue::ExtensionMismatch => write!(f, "extension_mismatch"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokenLink {
    pub source: String,
    pub target: String,
    pub issue: LinkIssue,
    pub fix_applied: String,
}

impl std::fmt::Display for LinkCheckResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        writeln!(f, "=== Link Check ===")?;
        writeln!(f, "Total links: {}", self.total_links)?;
        if self.broken.is_empty() {
            writeln!(f, "No broken links found.")?;
        } else {
            writeln!(f, "Broken links ({}):", self.broken.len())?;
            for link in &self.broken {
                writeln!(f, "  {} -> {} [{}]", link.source, link.target, link.issue)?;
            }
        }
        if !self.fixed.is_empty() {
            writeln!(f, "Fixed ({}):", self.fixed.len())?;
            for link in &self.fixed {
                writeln!(
                    f,
                    "  {} -> {} — {}",
                    link.source, link.target, link.fix_applied
                )?;
            }
        }
        Ok(())
    }
}

fn wikilink_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\[\[([^\]|]+?)(?:\|([^\]]+?))?\]\]").expect("valid wikilink regex")
    })
}

/// Collect all `.md` file stems (relative path without extension) mapped to their full relative paths.
fn collect_md_files(vault_root: &Path) -> BTreeMap<String, String> {
    WalkDir::new(vault_root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let p = e.path();
            p.is_file()
                && p.extension()
                    .and_then(|s| s.to_str())
                    .is_some_and(|ext| ext == "md")
        })
        .filter(|e| !is_vault_excluded(e.path()))
        .filter_map(|e| {
            let rel = e.path().strip_prefix(vault_root).ok()?;
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            let stem = rel.with_extension("").to_string_lossy().replace('\\', "/");
            Some((stem, rel_str))
        })
        .collect()
}

/// Collect all `.txt` file stems (relative path without extension) as a lowercase-keyed map
/// (lowercase stem → original-case stem).
fn collect_txt_stems(vault_root: &Path) -> BTreeMap<String, String> {
    WalkDir::new(vault_root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let p = e.path();
            p.is_file()
                && p.extension()
                    .and_then(|s| s.to_str())
                    .is_some_and(|ext| ext == "txt")
        })
        .filter(|e| !is_vault_excluded(e.path()))
        .filter_map(|e| {
            let rel = e.path().strip_prefix(vault_root).ok()?;
            let stem = rel.with_extension("").to_string_lossy().replace('\\', "/");
            Some((stem.to_lowercase(), stem))
        })
        .collect()
}

/// Build a reverse index: lowercase(short_stem) → full relative path, for O(1) stem-only lookups.
fn build_stem_index(md_files: &BTreeMap<String, String>) -> BTreeMap<String, String> {
    let mut index = BTreeMap::new();
    for (stem, full) in md_files {
        let short = stem.split('/').next_back().unwrap_or(stem).to_lowercase();
        match index.entry(short) {
            std::collections::btree_map::Entry::Vacant(e) => {
                e.insert(full.clone());
            }
            std::collections::btree_map::Entry::Occupied(e) => {
                warn!(
                    "Stem collision: '{}' and '{}' both resolve to '{}'; keeping first",
                    full,
                    e.get(),
                    e.key()
                );
            }
        }
    }
    index
}

/// Check if a raw wikilink target resolves to an existing .md file.
fn resolve_raw_target(
    target: &str,
    md_files: &BTreeMap<String, String>,
    stem_index: &BTreeMap<String, String>,
) -> Option<String> {
    let key = target.to_lowercase();
    if let Some(full) = md_files.get(&key) {
        return Some(full.clone());
    }
    let short_key = key.split('/').next_back()?.to_string();
    stem_index.get(&short_key).cloned()
}

/// Try to find an existing .md file that matches `target` with hyphens swapped to spaces or vice versa.
fn find_normalized_match<'a>(
    target: &str,
    md_files: &'a BTreeMap<String, String>,
    stem_index: &'a BTreeMap<String, String>,
) -> Option<&'a String> {
    let target_lower = target.to_lowercase();

    // Try hyphens → spaces
    let with_spaces = target_lower.replace('-', " ");
    if let Some(full) = md_files.get(&with_spaces) {
        return Some(full);
    }
    let short_spaces = with_spaces.split('/').next_back()?.to_string();
    if let Some(full) = stem_index.get(&short_spaces) {
        return Some(full);
    }

    // Try spaces → hyphens
    let with_hyphens = target_lower.replace(' ', "-");
    if let Some(full) = md_files.get(&with_hyphens) {
        return Some(full);
    }
    let short_hyphens = with_hyphens.split('/').next_back()?.to_string();
    stem_index.get(&short_hyphens)
}

/// Check if a file has incoming links using its current name.
fn has_incoming_links(graph: &crate::graph::wikilinks::VaultGraph, file_path: &str) -> bool {
    graph
        .incoming
        .get(file_path)
        .is_some_and(|sources| !sources.is_empty())
}

/// Replace all occurrences of `[[old_target]]` and `[[old_target|...]]` with `[[new_target]]`
/// (preserving aliases) in the source file content.
/// Note: compiles a regex per call — acceptable since --fix is not a hot path for a CLI tool.
fn replace_wikilink_in_file(
    vault_root: &Path,
    source_rel: &str,
    old_target: &str,
    new_target: &str,
) -> Result<()> {
    let source_path = vault_root.join(source_rel);
    let content = fs::read_to_string(&source_path)?;

    let pattern = regex::escape(old_target);
    let re = Regex::new(&format!(r"\[\[{pattern}(\|[^\]]+?)?\]\]"))?;

    let new_content = re.replace_all(&content, |caps: &regex::Captures| {
        if caps.get(1).is_some() {
            let alias = caps.get(1).unwrap().as_str();
            format!("[[{new_target}|{}]]", &alias[1..])
        } else {
            format!("[[{new_target}]]")
        }
    });

    fs::write(&source_path, new_content.as_ref())?;
    Ok(())
}

/// Parse wikilinks from content, returning raw target strings.
fn parse_raw_targets(content: &str) -> Vec<String> {
    let re = wikilink_re();
    re.captures_iter(content)
        .filter_map(|cap| {
            let raw = cap.get(1)?.as_str().trim().to_string();
            if raw.is_empty() {
                return None;
            }
            Some(raw)
        })
        .collect()
}

fn handle_extension_mismatch(
    vault_root: &Path,
    rel_path: &str,
    raw_target: &str,
    txt_original: &str,
    fix: bool,
    broken: &mut Vec<BrokenLink>,
    fixed: &mut Vec<BrokenLink>,
) {
    if !fix {
        broken.push(BrokenLink {
            source: rel_path.to_string(),
            target: raw_target.to_string(),
            issue: LinkIssue::ExtensionMismatch,
            fix_applied: String::new(),
        });
        return;
    }

    let old_path = vault_root.join(format!("{}.txt", txt_original));
    let new_path = vault_root.join(format!("{}.md", txt_original));

    if new_path.exists() {
        broken.push(BrokenLink {
            source: rel_path.to_string(),
            target: raw_target.to_string(),
            issue: LinkIssue::ExtensionMismatch,
            fix_applied: format!("Skipped: {}.md already exists", txt_original),
        });
        return;
    }

    match fs::rename(&old_path, &new_path) {
        Ok(()) => {
            info!(
                "Renamed {}.txt → {}.md (extension mismatch fix)",
                txt_original, txt_original
            );
            fixed.push(BrokenLink {
                source: rel_path.to_string(),
                target: raw_target.to_string(),
                issue: LinkIssue::ExtensionMismatch,
                fix_applied: format!("Renamed {}.txt → {}.md", txt_original, txt_original),
            });
        }
        Err(e) => {
            info!("Failed to rename {}.txt: {}", txt_original, e);
            broken.push(BrokenLink {
                source: rel_path.to_string(),
                target: raw_target.to_string(),
                issue: LinkIssue::ExtensionMismatch,
                fix_applied: String::new(),
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn handle_filename_mismatch(
    vault_root: &Path,
    graph: &crate::graph::wikilinks::VaultGraph,
    rel_path: &str,
    raw_target: &str,
    target_clean: &str,
    matching_file: &str,
    fix: bool,
    broken: &mut Vec<BrokenLink>,
    fixed: &mut Vec<BrokenLink>,
) {
    let matching_stem = matching_file
        .strip_suffix(".md")
        .unwrap_or(matching_file)
        .to_string();

    if !fix {
        broken.push(BrokenLink {
            source: rel_path.to_string(),
            target: raw_target.to_string(),
            issue: LinkIssue::FilenameMismatch,
            fix_applied: String::new(),
        });
        return;
    }

    // Only rename if no incoming links use the current name; otherwise fix the link in source
    if !has_incoming_links(graph, matching_file) {
        let old_path = vault_root.join(matching_file);
        let new_file = format!("{}.md", target_clean);
        let new_path = vault_root.join(&new_file);

        match fs::rename(&old_path, &new_path) {
            Ok(()) => {
                info!(
                    "Renamed {} → {} (filename mismatch fix)",
                    matching_file, new_file
                );
                fixed.push(BrokenLink {
                    source: rel_path.to_string(),
                    target: raw_target.to_string(),
                    issue: LinkIssue::FilenameMismatch,
                    fix_applied: format!("Renamed {} → {}", matching_file, new_file),
                });
            }
            Err(e) => {
                info!("Failed to rename {}: {}", matching_file, e);
                broken.push(BrokenLink {
                    source: rel_path.to_string(),
                    target: raw_target.to_string(),
                    issue: LinkIssue::FilenameMismatch,
                    fix_applied: String::new(),
                });
            }
        }
    } else {
        match replace_wikilink_in_file(vault_root, rel_path, target_clean, &matching_stem) {
            Ok(()) => {
                info!(
                    "Fixed wikilink in {} : [[{}]] → [[{}]]",
                    rel_path, target_clean, matching_stem
                );
                fixed.push(BrokenLink {
                    source: rel_path.to_string(),
                    target: raw_target.to_string(),
                    issue: LinkIssue::FilenameMismatch,
                    fix_applied: format!(
                        "Updated [[{}]] → [[{}]] in {}",
                        target_clean, matching_stem, rel_path
                    ),
                });
            }
            Err(e) => {
                info!("Failed to update wikilink in {}: {}", rel_path, e);
                broken.push(BrokenLink {
                    source: rel_path.to_string(),
                    target: raw_target.to_string(),
                    issue: LinkIssue::FilenameMismatch,
                    fix_applied: String::new(),
                });
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn check_file_links(
    vault_root: &Path,
    graph: &crate::graph::wikilinks::VaultGraph,
    rel_path: &str,
    content: &str,
    md_files: &BTreeMap<String, String>,
    txt_stems: &BTreeMap<String, String>,
    stem_index: &BTreeMap<String, String>,
    fix: bool,
    broken: &mut Vec<BrokenLink>,
    fixed: &mut Vec<BrokenLink>,
) -> usize {
    let raw_targets = parse_raw_targets(content);
    let link_count = raw_targets.len();

    for raw_target in raw_targets {
        let target_clean = raw_target
            .strip_suffix(".md")
            .unwrap_or(&raw_target)
            .to_string();

        if resolve_raw_target(&target_clean, md_files, stem_index).is_some() {
            continue;
        }

        let target_lower = target_clean.to_lowercase();
        if let Some(txt_original) = txt_stems.get(&target_lower) {
            handle_extension_mismatch(
                vault_root,
                rel_path,
                &raw_target,
                txt_original,
                fix,
                broken,
                fixed,
            );
            continue;
        }

        if let Some(matching_file) = find_normalized_match(&target_clean, md_files, stem_index) {
            handle_filename_mismatch(
                vault_root,
                graph,
                rel_path,
                &raw_target,
                &target_clean,
                matching_file,
                fix,
                broken,
                fixed,
            );
            continue;
        }

        broken.push(BrokenLink {
            source: rel_path.to_string(),
            target: raw_target.clone(),
            issue: LinkIssue::Unresolved,
            fix_applied: String::new(),
        });
    }

    link_count
}

pub fn check_links(vault_root: &Path, config: &ForgeConfig, fix: bool) -> Result<LinkCheckResult> {
    let graph = build_vault_graph(vault_root, config)?;
    let md_files = collect_md_files(vault_root);
    let txt_stems = collect_txt_stems(vault_root);
    let stem_index = build_stem_index(&md_files);

    let mut total_links = 0usize;
    let mut broken: Vec<BrokenLink> = Vec::new();
    let mut fixed: Vec<BrokenLink> = Vec::new();

    for rel_path in md_files.values() {
        let full_path = vault_root.join(rel_path);
        let content = match fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        total_links += check_file_links(
            vault_root,
            &graph,
            rel_path,
            &content,
            &md_files,
            &txt_stems,
            &stem_index,
            fix,
            &mut broken,
            &mut fixed,
        );
    }

    info!(
        "Link check complete: {} total, {} broken, {} fixed",
        total_links,
        broken.len(),
        fixed.len()
    );

    Ok(LinkCheckResult {
        total_links,
        broken,
        fixed,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_config() -> ForgeConfig {
        ForgeConfig::default_for("test-vault")
    }

    fn write_md(dir: &Path, name: &str, content: &str) {
        fs::write(dir.join(name), content).unwrap()
    }

    #[test]
    fn test_detect_broken_link() {
        let tmp = TempDir::new().unwrap();
        let config = make_config();

        write_md(tmp.path(), "source.md", "Link to [[Nonexistent]]");
        write_md(tmp.path(), "other.md", "Hello world");

        let result = check_links(tmp.path(), &config, false).unwrap();

        assert_eq!(result.broken.len(), 1);
        assert_eq!(result.broken[0].target, "Nonexistent");
        assert_eq!(result.broken[0].issue, LinkIssue::Unresolved);
        assert!(result.broken[0].fix_applied.is_empty());
        assert!(result.fixed.is_empty());
    }

    #[test]
    fn test_fix_extension_mismatch() {
        let tmp = TempDir::new().unwrap();
        let config = make_config();

        write_md(tmp.path(), "source.md", "Link to [[Some Note]]");
        fs::write(tmp.path().join("Some Note.txt"), "content").unwrap();

        let result = check_links(tmp.path(), &config, true).unwrap();

        assert_eq!(result.fixed.len(), 1);
        assert_eq!(result.fixed[0].issue, LinkIssue::ExtensionMismatch);
        assert!(result.fixed[0].fix_applied.contains("Some Note.txt"));
        assert!(tmp.path().join("Some Note.md").exists());
        assert!(!tmp.path().join("Some Note.txt").exists());
    }

    #[test]
    fn test_rename_collision_guard() {
        let tmp = TempDir::new().unwrap();
        let config = make_config();

        write_md(tmp.path(), "source.md", "Link to [[Note]]");
        // Both .txt and .md exist — link resolves to .md, no fix needed
        fs::write(tmp.path().join("Note.txt"), "txt content").unwrap();
        write_md(tmp.path(), "Note.md", "md content");

        let result = check_links(tmp.path(), &config, true).unwrap();

        // Link resolves to existing Note.md — no broken, no fixed
        assert!(result.broken.is_empty());
        assert!(result.fixed.is_empty());
        // Original .md should be preserved
        assert_eq!(
            fs::read_to_string(tmp.path().join("Note.md")).unwrap(),
            "md content"
        );
    }

    #[test]
    fn test_fix_filename_mismatch() {
        let tmp = TempDir::new().unwrap();
        let config = make_config();

        write_md(tmp.path(), "source.md", "Link to [[My-Note]]");
        write_md(tmp.path(), "My Note.md", "content here");

        let result = check_links(tmp.path(), &config, true).unwrap();

        assert_eq!(result.fixed.len(), 1);
        assert_eq!(result.fixed[0].issue, LinkIssue::FilenameMismatch);
        assert!(
            result.fixed[0].fix_applied.contains("My Note")
                || result.fixed[0].fix_applied.contains("My-Note")
        );
    }

    #[test]
    fn test_no_fix_without_flag() {
        let tmp = TempDir::new().unwrap();
        let config = make_config();

        write_md(tmp.path(), "source.md", "Link to [[Nonexistent]]");
        write_md(tmp.path(), "other.md", "Link to [[Some Note]]");
        fs::write(tmp.path().join("Some Note.txt"), "content").unwrap();

        let result = check_links(tmp.path(), &config, false).unwrap();

        assert!(result.fixed.is_empty());
        assert_eq!(result.broken.len(), 2);
        assert!(tmp.path().join("Some Note.txt").exists());
        assert!(!tmp.path().join("Some Note.md").exists());
    }

    #[test]
    fn test_strip_suffix_not_trim_end_matches() {
        // Verify that "README.md" is handled correctly with strip_suffix
        let raw = "README.md".to_string();
        let target_clean = raw.strip_suffix(".md").unwrap_or(&raw).to_string();
        assert_eq!(target_clean, "README");

        // And a case where .md is not at the end
        let raw2 = "my.md.note".to_string();
        let target_clean2 = raw2.strip_suffix(".md").unwrap_or(&raw2).to_string();
        assert_eq!(target_clean2, "my.md.note");
    }
}
