use anyhow::Result;
use rayon::prelude::*;
use regex::Regex;
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::Path,
    sync::OnceLock,
};
use tracing::info;
use walkdir::WalkDir;

use crate::config::ForgeConfig;

#[derive(Debug, Clone)]
pub struct Wikilink {
    pub raw_target: String,
    #[allow(dead_code)]
    pub alias: Option<String>,
    pub resolved_path: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct VaultGraph {
    pub outgoing: BTreeMap<String, BTreeSet<String>>,
    pub incoming: BTreeMap<String, BTreeSet<String>>,
    pub links: BTreeMap<String, Vec<Wikilink>>,
    pub all_files: BTreeSet<String>,
}

impl VaultGraph {
    pub fn orphan_count(&self) -> usize {
        self.all_files
            .iter()
            .filter(|f| {
                self.outgoing.get(*f).is_none_or(|s| s.is_empty())
                    && self.incoming.get(*f).is_none_or(|s| s.is_empty())
            })
            .count()
    }

    pub fn orphans(&self) -> Vec<&str> {
        self.all_files
            .iter()
            .filter(|f| {
                self.outgoing.get(*f).is_none_or(|s| s.is_empty())
                    && self.incoming.get(*f).is_none_or(|s| s.is_empty())
            })
            .map(|s| s.as_str())
            .collect()
    }

    pub fn total_links(&self) -> usize {
        self.outgoing.values().map(|s| s.len()).sum()
    }

    pub fn broken_links(&self) -> Vec<(String, String)> {
        let mut broken = Vec::new();
        for (source, targets) in &self.outgoing {
            for target in targets {
                if !self.all_files.contains(target) {
                    broken.push((source.clone(), target.clone()));
                }
            }
        }
        broken
    }

    pub fn hub_notes(&self, top_n: usize) -> Vec<(String, usize)> {
        let mut hubs: Vec<(String, usize)> = self
            .incoming
            .iter()
            .map(|(path, links)| (path.clone(), links.len()))
            .filter(|(_, count)| *count > 0)
            .collect();
        hubs.sort_by_key(|b| std::cmp::Reverse(b.1));
        hubs.truncate(top_n);
        hubs
    }

    pub fn connected_components(&self) -> usize {
        let mut visited = BTreeSet::new();
        let mut components = 0;

        for node in &self.all_files {
            if visited.contains(node) {
                continue;
            }
            components += 1;
            let mut stack = vec![node.clone()];
            while let Some(current) = stack.pop() {
                if visited.contains(&current) {
                    continue;
                }
                visited.insert(current.clone());
                if let Some(neighbors) = self.outgoing.get(&current) {
                    for n in neighbors {
                        if !visited.contains(n) {
                            stack.push(n.clone());
                        }
                    }
                }
                if let Some(neighbors) = self.incoming.get(&current) {
                    for n in neighbors {
                        if !visited.contains(n) {
                            stack.push(n.clone());
                        }
                    }
                }
            }
        }

        components
    }
}

fn wikilink_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"\[\[([^\]|]+?)(?:\|([^\]]+?))?\]\]").expect("valid wikilink regex")
    })
}

/// Replace fenced code-block regions with blank lines so `[[ ]]` syntax inside
/// them is not mistaken for wikilinks. Handles ``` ``` ``` and `~~~` fences; a
/// fence opened with one marker only closes on the same marker. Indented code
/// blocks and inline code spans are intentionally left untouched.
///
/// Shared by both `graph health` and `check-links` so the two commands agree.
pub(crate) fn strip_fenced_code_blocks(content: &str) -> String {
    let mut out = String::with_capacity(content.len());
    let mut fence: Option<char> = None;

    for line in content.split_inclusive('\n') {
        let trimmed = line.trim_start();
        let opens_backtick = trimmed.starts_with("```");
        let opens_tilde = trimmed.starts_with("~~~");

        match fence {
            Some(open) => {
                // Inside a fence — drop the line content but keep the newline so
                // line numbering is preserved. Watch for a matching close fence.
                out.push('\n');
                if match open {
                    '`' => opens_backtick,
                    '~' => opens_tilde,
                    _ => false,
                } {
                    fence = None;
                }
            }
            None => {
                if opens_backtick {
                    fence = Some('`');
                    out.push('\n');
                } else if opens_tilde {
                    fence = Some('~');
                    out.push('\n');
                } else {
                    out.push_str(line);
                }
            }
        }
    }

    out
}

pub fn build_vault_graph(vault_root: &Path, _config: &ForgeConfig) -> Result<VaultGraph> {
    let md_files: Vec<(String, String)> = WalkDir::new(vault_root)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let p = e.path();
            p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("md")
        })
        .filter(|e| {
            let p = e.path();
            !p.components().any(|c| {
                let os = c.as_os_str();
                os == ".git"
                    || os == ".obsidian"
                    || os == ".obsidian-forge"
                    || os == ".alcove"
                    || os == ".claude"
            })
        })
        .filter_map(|e| {
            let p = e.path();
            let rel = p.strip_prefix(vault_root).ok()?;
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            let stem = rel.with_extension("").to_string_lossy().replace('\\', "/");
            Some((rel_str, stem))
        })
        .collect();

    let file_index: BTreeMap<String, String> = md_files
        .iter()
        .map(|(full, stem)| {
            let key = stem.to_lowercase();
            (key, full.clone())
        })
        .collect();

    let stem_index: BTreeMap<String, String> = md_files
        .iter()
        .filter_map(|(full, stem)| {
            let short = stem.split('/').next_back()?.to_lowercase();
            Some((short, full.clone()))
        })
        .collect();

    let parsed: Vec<(String, Vec<Wikilink>)> = md_files
        .par_iter()
        .filter_map(|(rel_str, _)| {
            let full_path = vault_root.join(rel_str);
            let content = fs::read_to_string(&full_path).ok()?;

            let links = parse_wikilinks(&content);
            Some((rel_str.clone(), links))
        })
        .collect();

    let mut graph = VaultGraph {
        all_files: md_files.into_iter().map(|(f, _)| f).collect(),
        ..Default::default()
    };

    for (source, links) in parsed {
        let mut resolved_targets = BTreeSet::new();

        for link in &links {
            let target = resolve_link(&link.raw_target, &file_index, &stem_index);
            if let Some(ref resolved) = target {
                resolved_targets.insert(resolved.clone());
            }
        }

        if !resolved_targets.is_empty() {
            graph
                .outgoing
                .insert(source.clone(), resolved_targets.clone());
            for target in &resolved_targets {
                graph
                    .incoming
                    .entry(target.clone())
                    .or_default()
                    .insert(source.clone());
            }
        }

        let resolved_links: Vec<Wikilink> = links
            .into_iter()
            .map(|mut l| {
                l.resolved_path = resolve_link(&l.raw_target, &file_index, &stem_index);
                l
            })
            .collect();

        if !resolved_links.is_empty() {
            graph.links.insert(source, resolved_links);
        }
    }

    info!(
        "Vault graph built: {} files, {} links, {} orphans",
        graph.all_files.len(),
        graph.total_links(),
        graph.orphan_count()
    );

    Ok(graph)
}

fn parse_wikilinks(content: &str) -> Vec<Wikilink> {
    let content = strip_fenced_code_blocks(content);
    let re = wikilink_re();
    re.captures_iter(&content)
        .filter_map(|cap| {
            let raw = cap.get(1)?.as_str().trim().to_string();
            if raw.is_empty() {
                return None;
            }
            let alias = cap.get(2).map(|m| m.as_str().trim().to_string());
            Some(Wikilink {
                raw_target: raw,
                alias,
                resolved_path: None,
            })
        })
        .collect()
}

fn resolve_link(
    target: &str,
    file_index: &BTreeMap<String, String>,
    stem_index: &BTreeMap<String, String>,
) -> Option<String> {
    let target_clean = target.replace('\\', "/");
    let key = target_clean.to_lowercase();

    if let Some(full) = file_index.get(&key) {
        return Some(full.clone());
    }

    let key_no_ext = key.trim_end_matches(".md").to_string();
    if let Some(full) = file_index.get(&key_no_ext) {
        return Some(full.clone());
    }

    let short_key = key_no_ext.split('/').next_back()?.to_string();
    if let Some(full) = stem_index.get(&short_key) {
        return Some(full.clone());
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_wikilink() {
        let links = parse_wikilinks("See [[My Note]] for details.");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].raw_target, "My Note");
        assert!(links[0].alias.is_none());
    }

    #[test]
    fn test_parse_aliased_wikilink() {
        let links = parse_wikilinks("See [[path/to/note|Display Text]] for details.");
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].raw_target, "path/to/note");
        assert_eq!(links[0].alias.as_deref(), Some("Display Text"));
    }

    #[test]
    fn test_parse_multiple_wikilinks() {
        let links = parse_wikilinks("[[A]] and [[B|b]] then [[C]]");
        assert_eq!(links.len(), 3);
        assert_eq!(links[0].raw_target, "A");
        assert_eq!(links[1].raw_target, "B");
        assert_eq!(links[2].raw_target, "C");
    }

    #[test]
    fn test_parse_empty_wikilink_ignored() {
        let links = parse_wikilinks("[[]] and [[  ]]");
        assert!(links.is_empty());
    }

    #[test]
    fn test_parse_skips_backtick_fenced_code_blocks() {
        // Regression for #27: `[[ -f file ]]` (bash test) and `[[providers]]`
        // (YAML-ish identifier) inside a fenced block must not be treated as links.
        let content = "See [[Real Link]]\n\n\
```bash\n\
if [[ -f file ]]; then echo ok; fi\n\
```\n\n\
```yaml\n\
[[providers]]\n\
config = true\n\
```\n";
        let links = parse_wikilinks(content);
        assert_eq!(links.len(), 1);
        assert_eq!(links[0].raw_target, "Real Link");
    }

    #[test]
    fn test_parse_skips_tilde_fenced_code_blocks() {
        // `~~~` fences and mixed markers: a ``` fence must not be closed by `~~~`.
        let content = "~~~\n[[inside-tilde]]\n~~~\n[[outside]]\n\
```\n[[should-not-leak]]\n```\n[[after]]\n";
        let links: Vec<String> = parse_wikilinks(content)
            .into_iter()
            .map(|l| l.raw_target)
            .collect();
        assert_eq!(links, vec!["outside".to_string(), "after".to_string()]);
    }

    #[test]
    fn test_resolve_link_exact() {
        let mut idx = BTreeMap::new();
        idx.insert("notes/my note".into(), "notes/my note.md".into());
        let stem_idx = BTreeMap::new();

        let result = resolve_link("notes/my note", &idx, &stem_idx);
        assert_eq!(result, Some("notes/my note.md".to_string()));
    }

    #[test]
    fn test_resolve_link_case_insensitive() {
        let mut idx = BTreeMap::new();
        idx.insert("notes/my note".into(), "notes/My Note.md".into());
        let stem_idx = BTreeMap::new();

        let result = resolve_link("Notes/My Note", &idx, &stem_idx);
        assert_eq!(result, Some("notes/My Note.md".to_string()));
    }

    #[test]
    fn test_resolve_link_stem_fallback() {
        let idx = BTreeMap::new();
        let mut stem_idx = BTreeMap::new();
        stem_idx.insert("my note".into(), "deep/nested/My Note.md".into());

        let result = resolve_link("My Note", &idx, &stem_idx);
        assert_eq!(result, Some("deep/nested/My Note.md".to_string()));
    }

    #[test]
    fn test_resolve_link_not_found() {
        let idx = BTreeMap::new();
        let stem_idx = BTreeMap::new();

        let result = resolve_link("Nonexistent", &idx, &stem_idx);
        assert!(result.is_none());
    }

    #[test]
    fn test_vault_graph_orphans() {
        let mut graph = VaultGraph::default();
        graph.all_files.insert("a.md".into());
        graph.all_files.insert("b.md".into());
        graph.all_files.insert("c.md".into());

        graph
            .outgoing
            .insert("a.md".into(), BTreeSet::from(["b.md".into()]));
        graph
            .incoming
            .insert("b.md".into(), BTreeSet::from(["a.md".into()]));

        assert_eq!(graph.orphan_count(), 1);
        assert_eq!(graph.orphans(), vec!["c.md"]);
    }

    #[test]
    fn test_vault_graph_broken_links() {
        let mut graph = VaultGraph::default();
        graph.all_files.insert("a.md".into());
        graph
            .outgoing
            .insert("a.md".into(), BTreeSet::from(["missing.md".into()]));

        let broken = graph.broken_links();
        assert_eq!(broken.len(), 1);
        assert_eq!(broken[0], ("a.md".into(), "missing.md".into()));
    }

    #[test]
    fn test_vault_graph_connected_components() {
        let mut graph = VaultGraph::default();
        graph.all_files.insert("a.md".into());
        graph.all_files.insert("b.md".into());
        graph.all_files.insert("c.md".into());

        graph
            .outgoing
            .insert("a.md".into(), BTreeSet::from(["b.md".into()]));
        graph
            .incoming
            .insert("b.md".into(), BTreeSet::from(["a.md".into()]));

        assert_eq!(graph.connected_components(), 2);
    }

    #[test]
    fn test_vault_graph_hub_notes() {
        let mut graph = VaultGraph::default();
        graph.all_files.insert("hub.md".into());
        graph.all_files.insert("a.md".into());
        graph.all_files.insert("b.md".into());

        graph.incoming.insert(
            "hub.md".into(),
            BTreeSet::from(["a.md".into(), "b.md".into()]),
        );

        let hubs = graph.hub_notes(5);
        assert_eq!(hubs.len(), 1);
        assert_eq!(hubs[0], ("hub.md".into(), 2));
    }
}
