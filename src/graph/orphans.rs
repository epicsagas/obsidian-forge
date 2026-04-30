use anyhow::Result;
use std::path::Path;

use crate::ai::AiClient;
use crate::config::ForgeConfig;

use super::wikilinks::{build_vault_graph, VaultGraph};

pub fn detect_orphans(vault_root: &Path, config: &ForgeConfig) -> Result<Vec<String>> {
    let graph = build_vault_graph(vault_root, config)?;
    Ok(graph.orphans().into_iter().map(String::from).collect())
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
