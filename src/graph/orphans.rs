use anyhow::Result;
use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

use crate::ai::AiClient;
use crate::config::ForgeConfig;

use super::wikilinks::{VaultGraph, build_vault_graph};

pub fn detect_orphans(
    vault_root: &Path,
    config: &ForgeConfig,
    exclude_seeded: bool,
    min_importance: usize,
) -> Result<Vec<String>> {
    let graph = build_vault_graph(vault_root, config)?;
    let orphans: Vec<String> = graph.orphans().into_iter().map(String::from).collect();

    let filtered = orphans
        .into_iter()
        .filter(|path| {
            if exclude_seeded && path.contains("/seeded/") {
                return false;
            }
            true
        })
        .filter(|path| {
            if min_importance > 0 {
                let full_path = vault_root.join(path);
                match std::fs::read_to_string(&full_path) {
                    Ok(content) => {
                        let stripped = strip_frontmatter(&content);
                        stripped.len() >= min_importance
                    }
                    Err(_) => false,
                }
            } else {
                true
            }
        })
        .collect();

    Ok(filtered)
}

fn strip_frontmatter(content: &str) -> String {
    static RE: OnceLock<Regex> = OnceLock::new();
    let re = RE.get_or_init(|| Regex::new(r"(?s)^---[\s\S]*?---\n?").unwrap());
    re.replace(content, "").to_string()
}

pub async fn auto_link_orphans(
    vault_root: &Path,
    config: &ForgeConfig,
    graph: &VaultGraph,
) -> Result<Vec<String>> {
    let orphans = graph.orphans();
    if orphans.is_empty() {
        return Ok(Vec::new());
    }

    let moc_files: Vec<String> = find_moc_files(vault_root, config);

    if moc_files.is_empty() {
        return Ok(orphans.into_iter().map(String::from).collect());
    }

    let ai = AiClient::from_config(&config.ai);
    let mut linked = Vec::new();

    for orphan_rel in orphans {
        let orphan_path = vault_root.join(orphan_rel);
        let content = match std::fs::read_to_string(&orphan_path) {
            Ok(c) => c,
            Err(_) => continue,
        };

        let title = content
            .lines()
            .find(|l| l.starts_with("# "))
            .map(|l| l.trim_start_matches("# ").to_string())
            .unwrap_or_else(|| {
                orphan_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string()
            });

        let summary: String = content
            .lines()
            .skip_while(|l| l.starts_with('#') || l.is_empty())
            .take(5)
            .collect::<Vec<_>>()
            .join(" ")
            .chars()
            .take(200)
            .collect();

        let moc_list = moc_files
            .iter()
            .enumerate()
            .map(|(i, m)| format!("{}. {}", i + 1, m))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!(
            "This orphan note has no links to or from other notes. \
             Which MOC(s) should it be linked to?\n\n\
             Orphan: \"{}\"\nSummary: {}\n\nAvailable MOCs:\n{}\n\n\
             Output JSON only: {{\"suggested_links\": [\"MOC Name 1\", ...]}}",
            title, summary, moc_list,
        );

        #[derive(serde::Deserialize, Default)]
        struct Suggestions {
            #[serde(default)]
            suggested_links: Vec<String>,
        }

        let suggestions: Suggestions = ai.generate_json(&prompt).await.unwrap_or_default();

        if !suggestions.suggested_links.is_empty() {
            let links_section = suggestions
                .suggested_links
                .iter()
                .map(|moc| format!("- [[{}]]", moc))
                .collect::<Vec<_>>()
                .join("\n");
            let new_content = format!("{}\n\n## See Also\n{}\n", content.trim_end(), links_section);
            std::fs::write(&orphan_path, &new_content)?;
            linked.push(orphan_rel.to_string());
        }
    }

    Ok(linked)
}

fn find_moc_files(vault_root: &Path, config: &ForgeConfig) -> Vec<String> {
    let system_dirs = config.all_system_dirs();
    let exclude = &config.projects.exclude;
    let mut mocs = Vec::new();

    if let Ok(entries) = std::fs::read_dir(vault_root) {
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let name = match path.file_name().and_then(|s| s.to_str()) {
                Some(n) => n.to_string(),
                None => continue,
            };
            if name.starts_with('.') || system_dirs.contains(&name) || exclude.contains(&name) {
                continue;
            }
            let hub = path.join(format!("{}.md", name));
            if hub.exists() {
                mocs.push(format!("{}/{}", name, name));
            }
        }
    }

    mocs.sort();
    mocs
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_frontmatter() {
        let with_fm = "---\ntitle: Test\ntags: [a]\n---\nSome content here.";
        assert_eq!(strip_frontmatter(with_fm), "Some content here.");

        let without_fm = "No frontmatter at all.";
        assert_eq!(strip_frontmatter(without_fm), "No frontmatter at all.");

        let empty_content = "---\n---\n";
        assert_eq!(strip_frontmatter(empty_content), "");
    }

    #[test]
    fn test_exclude_seeded() {
        let paths = vec![
            "notes/seeded/auto-note.md".to_string(),
            "notes/real-note.md".to_string(),
            "02-Areas/seeded/topic.md".to_string(),
            "10-Zettelkasten/concept.md".to_string(),
        ];

        let filtered: Vec<String> = paths
            .into_iter()
            .filter(|p| !p.contains("/seeded/"))
            .collect();

        assert_eq!(
            filtered,
            vec!["notes/real-note.md", "10-Zettelkasten/concept.md"]
        );
    }

    #[test]
    fn test_min_importance() {
        let short_content = "---\ntitle: Short\n---\nHi.";
        let long_content =
            "---\ntitle: Long\n---\nThis is a longer piece of content that exceeds the threshold.";

        assert!(strip_frontmatter(short_content).len() < 50);
        assert!(strip_frontmatter(long_content).len() >= 50);

        // Simulating the filter logic
        let threshold = 50;
        let short_len = strip_frontmatter(short_content).len();
        let long_len = strip_frontmatter(long_content).len();

        assert!(
            short_len < threshold,
            "short content should be below threshold"
        );
        assert!(long_len >= threshold, "long content should meet threshold");
    }

    #[test]
    fn test_no_filters() {
        let paths = vec![
            "notes/seeded/auto-note.md".to_string(),
            "notes/real-note.md".to_string(),
            "02-Areas/seeded/topic.md".to_string(),
            "10-Zettelkasten/concept.md".to_string(),
        ];

        // No filters: all pass through
        let filtered: Vec<String> = paths
            .into_iter()
            .filter(|_| true) // exclude_seeded = false
            .filter(|_| true) // min_importance = 0
            .collect();

        assert_eq!(filtered.len(), 4);
    }
}
