use anyhow::Result;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, fs, path::Path, sync::OnceLock};
use tracing::info;
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokenLink {
    pub source: String,
    pub target: String,
    pub issue: String, // "unresolved", "filename_mismatch", "extension_mismatch"
    pub fix_applied: String, // description of fix, empty if not fixed
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

/// Collect all `.txt` file stems (relative path without extension) as a lowercase-keyed set.
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
        index.entry(short).or_insert_with(|| full.clone());
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

pub fn check_links(vault_root: &Path, config: &ForgeConfig, fix: bool) -> Result<LinkCheckResult> {
    let graph = build_vault_graph(vault_root, config)?;
    let md_files = collect_md_files(vault_root);
    let txt_stems = collect_txt_stems(vault_root);
    let stem_index = build_stem_index(&md_files);

    let mut total_links = 0usize;
    let mut broken: Vec<BrokenLink> = Vec::new();
    let mut fixed: Vec<BrokenLink> = Vec::new();

    // Scan all .md files for wikilinks and check each raw target
    for rel_path in md_files.values() {
        let full_path = vault_root.join(rel_path);
        let content = match fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let raw_targets = parse_raw_targets(&content);
        total_links += raw_targets.len();

        for raw_target in raw_targets {
            // Strip .md suffix if user included it in the wikilink
            let target_clean = raw_target.trim_end_matches(".md").to_string();

            // Already resolves? Skip.
            if resolve_raw_target(&target_clean, &md_files, &stem_index).is_some() {
                continue;
            }

            // Check extension mismatch: target exists as .txt
            let target_lower = target_clean.to_lowercase();
            if let Some(txt_original) = txt_stems.get(&target_lower) {
                if fix {
                    let old_path = vault_root.join(format!("{}.txt", txt_original));
                    let new_path = vault_root.join(format!("{}.md", txt_original));

                    match fs::rename(&old_path, &new_path) {
                        Ok(()) => {
                            info!(
                                "Renamed {}.txt → {}.md (extension mismatch fix)",
                                txt_original, txt_original
                            );
                            fixed.push(BrokenLink {
                                source: rel_path.clone(),
                                target: raw_target.clone(),
                                issue: "extension_mismatch".to_string(),
                                fix_applied: format!(
                                    "Renamed {}.txt → {}.md",
                                    txt_original, txt_original
                                ),
                            });
                        }
                        Err(e) => {
                            info!("Failed to rename {}.txt: {}", txt_original, e);
                            broken.push(BrokenLink {
                                source: rel_path.clone(),
                                target: raw_target.clone(),
                                issue: "extension_mismatch".to_string(),
                                fix_applied: String::new(),
                            });
                        }
                    }
                } else {
                    broken.push(BrokenLink {
                        source: rel_path.clone(),
                        target: raw_target.clone(),
                        issue: "extension_mismatch".to_string(),
                        fix_applied: String::new(),
                    });
                }
                continue;
            }

            // Check filename mismatch: hyphens ↔ spaces
            if let Some(matching_file) =
                find_normalized_match(&target_clean, &md_files, &stem_index)
            {
                // Extract the actual stem from the matching file path
                let matching_stem = matching_file.trim_end_matches(".md").to_string();

                if fix {
                    // Only rename the file if it has NO incoming links using its current name.
                    // Otherwise, fix the broken link in the source file.
                    if !has_incoming_links(&graph, matching_file) {
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
                                    source: rel_path.clone(),
                                    target: raw_target.clone(),
                                    issue: "filename_mismatch".to_string(),
                                    fix_applied: format!(
                                        "Renamed {} → {}",
                                        matching_file, new_file
                                    ),
                                });
                            }
                            Err(e) => {
                                info!("Failed to rename {}: {}", matching_file, e);
                                broken.push(BrokenLink {
                                    source: rel_path.clone(),
                                    target: raw_target.clone(),
                                    issue: "filename_mismatch".to_string(),
                                    fix_applied: String::new(),
                                });
                            }
                        }
                    } else {
                        // File has incoming links — fix the broken link in the source instead
                        match replace_wikilink_in_file(
                            vault_root,
                            rel_path,
                            &target_clean,
                            &matching_stem,
                        ) {
                            Ok(()) => {
                                info!(
                                    "Fixed wikilink in {} : [[{}]] → [[{}]]",
                                    rel_path, target_clean, matching_stem
                                );
                                fixed.push(BrokenLink {
                                    source: rel_path.clone(),
                                    target: raw_target.clone(),
                                    issue: "filename_mismatch".to_string(),
                                    fix_applied: format!(
                                        "Updated [[{}]] → [[{}]] in {}",
                                        target_clean, matching_stem, rel_path
                                    ),
                                });
                            }
                            Err(e) => {
                                info!("Failed to update wikilink in {}: {}", rel_path, e);
                                broken.push(BrokenLink {
                                    source: rel_path.clone(),
                                    target: raw_target.clone(),
                                    issue: "filename_mismatch".to_string(),
                                    fix_applied: String::new(),
                                });
                            }
                        }
                    }
                } else {
                    broken.push(BrokenLink {
                        source: rel_path.clone(),
                        target: raw_target.clone(),
                        issue: "filename_mismatch".to_string(),
                        fix_applied: String::new(),
                    });
                }
                continue;
            }

            // Truly unresolved link — no .md match, no .txt match, no normalization match
            broken.push(BrokenLink {
                source: rel_path.clone(),
                target: raw_target.clone(),
                issue: "unresolved".to_string(),
                fix_applied: String::new(),
            });
        }
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

    fn create_test_vault(dir: &Path) -> ForgeConfig {
        let _ = fs::remove_dir_all(dir);
        fs::create_dir_all(dir).unwrap();

        let vault_toml = r#"
[vault]
name = "test"
layout = "para"
inbox_dir = "00-Inbox"
zettelkasten_dir = "10-Zettelkasten"
archive_dir = "99-Archives"
attachments_dir = "Attachments"
templates_dir = "obsidian-templates"
system_dirs = []
"#;
        fs::write(dir.join("vault.toml"), vault_toml).unwrap();

        toml::from_str(vault_toml).unwrap()
    }

    fn write_md(dir: &Path, name: &str, content: &str) {
        fs::write(dir.join(name), content).unwrap()
    }

    #[test]
    fn test_detect_broken_link() {
        let tmp = std::env::temp_dir().join("of_check_links_broken");
        let config = create_test_vault(&tmp);

        write_md(&tmp, "source.md", "Link to [[Nonexistent]]");
        write_md(&tmp, "other.md", "Hello world");

        let result = check_links(&tmp, &config, false).unwrap();

        assert_eq!(result.broken.len(), 1);
        assert_eq!(result.broken[0].target, "Nonexistent");
        assert_eq!(result.broken[0].issue, "unresolved");
        assert!(result.broken[0].fix_applied.is_empty());
        assert!(result.fixed.is_empty());

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_fix_extension_mismatch() {
        let tmp = std::env::temp_dir().join("of_check_links_ext");
        let config = create_test_vault(&tmp);

        write_md(&tmp, "source.md", "Link to [[Some Note]]");
        // Create a .txt file that should match the wikilink target
        fs::write(tmp.join("Some Note.txt"), "content").unwrap();

        let result = check_links(&tmp, &config, true).unwrap();

        assert_eq!(result.fixed.len(), 1);
        assert_eq!(result.fixed[0].issue, "extension_mismatch");
        assert!(result.fixed[0].fix_applied.contains("Some Note.txt"));
        assert!(tmp.join("Some Note.md").exists());
        assert!(!tmp.join("Some Note.txt").exists());

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_fix_filename_mismatch() {
        let tmp = std::env::temp_dir().join("of_check_links_filename");
        let config = create_test_vault(&tmp);

        // Source references [[My-Note]] but file is "My Note.md"
        write_md(&tmp, "source.md", "Link to [[My-Note]]");
        write_md(&tmp, "My Note.md", "content here");

        let result = check_links(&tmp, &config, true).unwrap();

        assert_eq!(result.fixed.len(), 1);
        assert_eq!(result.fixed[0].issue, "filename_mismatch");
        // The file "My Note.md" has no incoming links (source links to "My-Note", not "My Note"),
        // so it should be renamed to match the link target
        assert!(
            result.fixed[0].fix_applied.contains("My Note")
                || result.fixed[0].fix_applied.contains("My-Note")
        );

        let _ = fs::remove_dir_all(&tmp);
    }

    #[test]
    fn test_no_fix_without_flag() {
        let tmp = std::env::temp_dir().join("of_check_links_nofix");
        let config = create_test_vault(&tmp);

        write_md(&tmp, "source.md", "Link to [[Nonexistent]]");
        write_md(&tmp, "other.md", "Link to [[Some Note]]");
        // Extension mismatch: .txt instead of .md
        fs::write(tmp.join("Some Note.txt"), "content").unwrap();

        let result = check_links(&tmp, &config, false).unwrap();

        assert!(result.fixed.is_empty());
        assert_eq!(result.broken.len(), 2);
        // No files should have been renamed
        assert!(tmp.join("Some Note.txt").exists());
        assert!(!tmp.join("Some Note.md").exists());

        let _ = fs::remove_dir_all(&tmp);
    }
}
