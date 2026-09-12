use anyhow::Result;
use futures::stream::{self, StreamExt};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, HashSet},
    fs,
    path::Path,
};
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

/// Collect all `.md` file stems (relative path without extension) mapped to their full relative paths.
fn collect_md_files(vault_root: &Path) -> BTreeMap<String, String> {
    WalkDir::new(vault_root)
        .into_iter()
        // `filter_entry` prunes the whole subtree of an excluded directory
        // (nested repos, system dirs) so those files are never visited. This
        // must be a `filter_entry`, not a post-hoc `.filter()`, for the O(1)
        // `is_inside_nested_repo` boundary check to correctly exclude a nested
        // repo's contents. See `vault_utils::is_inside_nested_repo`.
        .filter_entry(|e| !is_vault_excluded(e.path(), vault_root))
        .filter_map(|e| e.ok())
        .filter(|e| {
            let p = e.path();
            p.is_file()
                && p.extension()
                    .and_then(|s| s.to_str())
                    .is_some_and(|ext| ext == "md")
        })
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
        // See `collect_md_files`: `filter_entry` prunes excluded subtrees so a
        // nested repo's files are never visited (required for the O(1)
        // `is_inside_nested_repo` boundary check to hold).
        .filter_entry(|e| !is_vault_excluded(e.path(), vault_root))
        .filter_map(|e| e.ok())
        .filter(|e| {
            let p = e.path();
            p.is_file()
                && p.extension()
                    .and_then(|s| s.to_str())
                    .is_some_and(|ext| ext == "txt")
        })
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

/// Resolve a raw wikilink target by delegating to the graph resolver, so
/// `check-links` and `graph health` share one resolution path (issue #27).
/// `check-links` layers its own hyphen/space normalization
/// ([`find_normalized_match`]) on top as a fallback.
fn resolve_raw_target(
    target: &str,
    md_files: &BTreeMap<String, String>,
    stem_index: &BTreeMap<String, String>,
) -> Option<String> {
    crate::graph::wikilinks::resolve_link(target, md_files, stem_index)
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

/// Parse wikilink targets from content, reusing the graph extractor so the two
/// commands share one fence-skipping implementation (issue #27).
fn parse_raw_targets(content: &str) -> Vec<String> {
    crate::graph::wikilinks::parse_wikilinks(content)
        .into_iter()
        .map(|w| w.raw_target)
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
            warn!("Failed to rename {}.txt: {}", txt_original, e);
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

        if new_path.exists() {
            warn!(
                "Skip rename {} → {}: target already exists",
                matching_file, new_file
            );
            broken.push(BrokenLink {
                source: rel_path.to_string(),
                target: raw_target.to_string(),
                issue: LinkIssue::FilenameMismatch,
                fix_applied: format!("Skipped: {} already exists", new_file),
            });
        } else {
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
                    warn!("Failed to rename {}: {}", matching_file, e);
                    broken.push(BrokenLink {
                        source: rel_path.to_string(),
                        target: raw_target.to_string(),
                        issue: LinkIssue::FilenameMismatch,
                        fix_applied: String::new(),
                    });
                }
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
                warn!("Failed to update wikilink in {}: {}", rel_path, e);
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

// ---------------------------------------------------------------------------
// AI suggestions
// ---------------------------------------------------------------------------

/// Max source files to run AI link suggestions for in one invocation.
const SUGGEST_FILE_CAP: usize = 50;
/// Candidate filenames shown to the model per broken link (local prefilter).
const CANDIDATES_PER_LINK: usize = 8;

/// Rank vault stems by naive word overlap with the link target — cheap local
/// prefilter so the AI only adjudicates plausible matches.
fn rank_candidates<'a>(
    target: &str,
    stems: impl Iterator<Item = &'a String>,
) -> Vec<String> {
    let target_lower = target.to_lowercase();
    let words: Vec<&str> = target_lower
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.chars().count() >= 3)
        .collect();
    let mut scored: Vec<(usize, &String)> = stems
        .map(|stem| {
            let lower = stem.to_lowercase();
            let score = words
                .iter()
                .filter(|w| lower.contains(*w))
                .count();
            (score, stem)
        })
        .filter(|(score, _)| *score > 0)
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0));
    scored
        .into_iter()
        .take(CANDIDATES_PER_LINK)
        .map(|(_, s)| s.clone())
        .collect()
}

/// AI-suggest target files for unresolved wikilinks.
///
/// Read-only: prints `[SUGGEST]` lines per broken link. Nothing is renamed or
/// rewritten — apply good suggestions by editing the link (or re-run
/// `check-links --fix` for mechanical mismatches).
pub async fn suggest_link_targets(
    vault_root: &Path,
    config: &ForgeConfig,
) -> Result<String> {
    let result = check_links(vault_root, config, false)?;

    // Unresolved links grouped per source file.
    let mut per_file: Vec<(String, Vec<String>)> = Vec::new();
    for link in &result.broken {
        if link.issue != LinkIssue::Unresolved {
            continue;
        }
        match per_file.iter_mut().find(|(f, _)| *f == link.source) {
            Some((_, targets)) => targets.push(link.target.clone()),
            None => per_file.push((link.source.clone(), vec![link.target.clone()])),
        }
    }
    per_file.truncate(SUGGEST_FILE_CAP);

    if per_file.is_empty() {
        return Ok("=== Link Suggestions ===\nNo unresolved links — nothing to suggest.".into());
    }

    let all_stems: Vec<String> = collect_md_files(vault_root).keys().cloned().collect();
    let client = crate::ai::AiClient::from_config(&config.ai);
    let concurrency = config.ai.max_concurrent.unwrap_or(5).clamp(1, 3);

    let mut out = String::from("=== Link Suggestions ===\n");
    let files_with_unresolved = result
        .broken
        .iter()
        .filter(|l| l.issue == LinkIssue::Unresolved)
        .map(|l| l.source.clone())
        .collect::<HashSet<_>>()
        .len();
    out.push_str(&format!(
        "Files with unresolved links: {} (suggesting for up to {})\n",
        files_with_unresolved,
        per_file.len()
    ));

    let suggestions = stream::iter(per_file)
        .map(|(source, targets)| {
            let client = client.clone();
            let all_stems = all_stems.clone();
            async move {
                let mut pairs: Vec<(String, Vec<String>)> = targets
                    .into_iter()
                    .map(|t| {
                        let cands = rank_candidates(&t, all_stems.iter());
                        (t, cands)
                    })
                    .collect();
                pairs.retain(|(_, cands)| !cands.is_empty());
                if pairs.is_empty() {
                    return None;
                }
                match suggest_one(&client, &pairs).await {
                    Ok(resolved) => Some((source, resolved)),
                    Err(e) => {
                        tracing::warn!("Link suggestion failed for {}: {}", source, e);
                        None
                    }
                }
            }
        })
        .buffer_unordered(concurrency)
        .filter_map(|x| async move { x })
        .collect::<Vec<_>>()
        .await;

    for (source, resolved) in suggestions {
        for (target, candidate) in resolved {
            match candidate {
                Some(c) => out.push_str(&format!(
                    "  [SUGGEST] {source} -> [[{target}]] ⇒ {c}\n"
                )),
                None => out.push_str(&format!(
                    "  [SUGGEST] {source} -> [[{target}]] ⇒ (no match)\n"
                )),
            }
        }
    }
    out.push_str("\nNothing applied — review, then fix the links manually.\n");
    Ok(out)
}

/// One AI call per source file: model picks the best candidate per target or
/// returns null.
async fn suggest_one(
    client: &crate::ai::AiClient,
    pairs: &[(String, Vec<String>)],
) -> Result<Vec<(String, Option<String>)>> {
    #[derive(serde::Deserialize)]
    struct Resolve {
        target: String,
        best_match: Option<String>,
    }

    let listing: String = pairs
        .iter()
        .map(|(target, cands)| {
            format!(
                "- [[{target}]] 후보: [{}]",
                cands.join(", ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n");

    let prompt = format!(
        "다음은 Obsidian 볼트의 깨진 위키링크 목록이다. 각 링크가 가리키려던 실제 노트를 후보 중에서 고르라. \
         파일명: 확장자 제외 상대 경로. 맥락상 어느 후보도 맞지 않으면 best_match는 null.\n\n{listing}\n\n\
         JSON 배열로만 답하라. 형식: [{{\"target\": \"링크명\", \"best_match\": \"후보 경로 또는 null\"}}]"
    );

    let resolved: Vec<Resolve> = client.generate_json(&prompt).await?;
    let valid: Vec<String> = pairs
        .iter()
        .flat_map(|(_, cands)| cands.iter().cloned())
        .collect();
    Ok(resolved
        .into_iter()
        .map(|r| {
            let best = r
                .best_match
                .filter(|b| !b.trim().is_empty() && valid.iter().any(|v| v == b));
            (r.target, best)
        })
        .collect())
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

    #[test]
    fn test_rename_preserves_existing_target() {
        let tmp = TempDir::new().unwrap();
        let config = make_config();

        // [[My-Note]] → normalized match is "My Note.md", rename target is "My-Note.md"
        // Since "My-Note.md" doesn't exist, rename proceeds and must preserve body
        write_md(tmp.path(), "source.md", "Link to [[My-Note]]");
        write_md(tmp.path(), "My Note.md", "original body");

        let result = check_links(tmp.path(), &config, true).unwrap();

        assert_eq!(result.fixed.len(), 1);
        // Verify the renamed file preserved its content
        assert!(
            tmp.path().join("My-Note.md").exists(),
            "Renamed file should exist"
        );
        assert!(!tmp.path().join("My Note.md").exists(), "Old name removed");
        assert_eq!(
            fs::read_to_string(tmp.path().join("My-Note.md")).unwrap(),
            "original body"
        );
    }

    #[test]
    fn test_rank_candidates_prefilter() {
        let stems = vec![
            "99-Archives/projects/alcove/ARCHITECTURE".to_string(),
            "10-Zettelkasten/MCP Protocol".to_string(),
            "02-Areas/rust-notes".to_string(),
        ];
        let ranked = rank_candidates("architecture", stems.iter());
        assert_eq!(ranked[0], "99-Archives/projects/alcove/ARCHITECTURE");
        // No word overlap at all → no candidates.
        assert!(rank_candidates("zzzqqq", stems.iter()).is_empty());
    }

    #[test]
    fn test_check_links_ignores_fenced_code_blocks() {
        // Regression for #27: wikilinks inside fenced code blocks must not be
        // flagged as broken, while a real (unresolvable) link outside a fence still is.
        let tmp = TempDir::new().unwrap();
        let config = make_config();

        write_md(
            tmp.path(),
            "source.md",
            "```bash\nif [[ -f file ]]; then echo ok; fi\n```\n\n\
```yaml\n[[providers]]\nconfig = true\n```\n\n\
Real link: [[Nonexistent]]\n",
        );

        let result = check_links(tmp.path(), &config, false).unwrap();

        assert_eq!(
            result.broken.len(),
            1,
            "only the real link should be flagged; got: {:?}",
            result.broken
        );
        assert_eq!(result.broken[0].target, "Nonexistent");
    }
}
