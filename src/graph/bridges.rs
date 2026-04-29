use anyhow::Result;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    path::Path,
};
use tracing::info;

use crate::config::ForgeConfig;
use crate::moc::replace_section;

use super::scan::ProjectProfile;

#[derive(Debug)]
pub struct ConceptBridge {
    pub name: String,
    pub tags: Vec<String>,
    pub projects: BTreeMap<String, Vec<std::path::PathBuf>>,
}

pub fn detect_bridges(profiles: &[ProjectProfile], config: &ForgeConfig) -> Vec<ConceptBridge> {
    let mut concept_map: BTreeMap<String, BTreeMap<String, Vec<std::path::PathBuf>>> =
        BTreeMap::new();

    for profile in profiles {
        for (concept, doc_paths) in &profile.concepts {
            concept_map
                .entry(concept.clone())
                .or_default()
                .insert(profile.name.clone(), doc_paths.clone());
        }
    }

    concept_map
        .into_iter()
        .filter(|(_, projects)| projects.len() >= 2)
        .map(|(name, projects)| {
            let tags = config
                .graph
                .concepts
                .iter()
                .find(|c| c.name == name)
                .map(|c| c.tags.clone())
                .unwrap_or_default();
            ConceptBridge {
                name,
                tags,
                projects,
            }
        })
        .collect()
}

pub fn generate_bridge_notes(
    vault_root: &Path,
    bridges: &[ConceptBridge],
    zk_dir: &str,
) -> Result<()> {
    let zk_path = vault_root.join(zk_dir);
    fs::create_dir_all(&zk_path)?;

    for bridge in bridges {
        let note_path = zk_path.join(format!("{}.md", bridge.name));
        let existing = fs::read_to_string(&note_path).unwrap_or_default();

        let mut project_links: Vec<String> = bridge
            .projects
            .keys()
            .map(|p| format!("- [[{}/{}]]", p, p))
            .collect();
        project_links.sort();
        let projects_section = format!("## Related Projects\n{}", project_links.join("\n"));

        if existing.is_empty() {
            let tags_str = bridge.tags.join(", ");
            let title = bridge.name.replace('-', " ");
            let content = format!(
                "---\ntags: [{}]\n---\n\n# {}\n\n> Auto-generated bridge note.\n\n{}\n",
                tags_str, title, projects_section,
            );
            fs::write(&note_path, &content)?;
            info!("Bridge note created: {}", bridge.name);
        } else {
            let new_content = if existing.contains("## Related Projects") {
                replace_section(
                    &existing,
                    "## Related Projects",
                    &format!("{}\n", projects_section),
                )
            } else {
                format!("{}\n{}\n", existing.trim_end(), projects_section)
            };

            if existing != new_content {
                fs::write(&note_path, &new_content)?;
                info!("Bridge note updated: {}", bridge.name);
            }
        }
    }

    Ok(())
}

pub fn update_related_projects(
    vault_root: &Path,
    profiles: &[ProjectProfile],
    bridges: &[ConceptBridge],
    zk_dir: &str,
) -> Result<()> {
    let mut adjacency: HashMap<String, BTreeSet<String>> = HashMap::new();
    let mut project_concepts: HashMap<String, BTreeSet<String>> = HashMap::new();

    for bridge in bridges {
        let project_names: Vec<&String> = bridge.projects.keys().collect();
        for i in 0..project_names.len() {
            for j in (i + 1)..project_names.len() {
                adjacency
                    .entry(project_names[i].clone())
                    .or_default()
                    .insert(project_names[j].clone());
                adjacency
                    .entry(project_names[j].clone())
                    .or_default()
                    .insert(project_names[i].clone());
            }
        }
        for pn in bridge.projects.keys() {
            project_concepts
                .entry(pn.clone())
                .or_default()
                .insert(bridge.name.clone());
        }
    }

    for profile in profiles {
        let hub_path = vault_root
            .join(&profile.name)
            .join(format!("{}.md", profile.name));
        if !hub_path.exists() {
            continue;
        }

        let existing = fs::read_to_string(&hub_path)?;
        let related = adjacency.get(&profile.name);
        let concepts = project_concepts.get(&profile.name);

        if related.is_none() && concepts.is_none() {
            continue;
        }

        let mut section = String::from("## Related Projects\n");
        if let Some(rel) = related {
            for r in rel {
                let shared: Vec<&str> = bridges
                    .iter()
                    .filter(|b| {
                        b.projects.contains_key(&profile.name) && b.projects.contains_key(r)
                    })
                    .map(|b| b.name.as_str())
                    .collect();
                let desc = if shared.is_empty() {
                    String::new()
                } else {
                    format!(" — shared: {}", shared.join(", "))
                };
                section.push_str(&format!("- [[{}/{}]]{}\n", r, r, desc));
            }
        }

        if let Some(concepts) = concepts {
            section.push_str("\n## Key Concepts\n");
            for c in concepts {
                section.push_str(&format!("- [[{}/{}]]\n", zk_dir, c));
            }
        }

        let new_content = if existing.contains("## Related Projects") {
            let cleaned = remove_sections(&existing, &["## Related Projects", "## Key Concepts"]);
            format!("{}\n\n{}\n", cleaned.trim_end(), section)
        } else {
            format!("{}\n\n{}\n", existing.trim_end(), section)
        };

        if existing != new_content {
            fs::write(&hub_path, &new_content)?;
            info!("Related projects updated: {}", profile.name);
        }
    }

    Ok(())
}

fn remove_sections(content: &str, headers: &[&str]) -> String {
    let mut result = String::new();
    let mut in_removed = false;

    for line in content.lines() {
        if headers.iter().any(|h| line.starts_with(h)) {
            in_removed = true;
            continue;
        }
        if in_removed && (line.starts_with("## ") || line.starts_with("# ")) {
            if headers.iter().any(|h| line.starts_with(h)) {
                continue;
            }
            in_removed = false;
        }
        if in_removed {
            continue;
        }
        result.push_str(line);
        result.push('\n');
    }

    result
}
