use anyhow::Result;
use rayon::prelude::*;
use regex::Regex;
use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    fs,
    path::{Path, PathBuf},
};
use tracing::{debug, info};
use walkdir::WalkDir;

use crate::config::ForgeConfig;
use crate::moc::replace_section;

// ---------------------------------------------------------------------------
// Public entry point
// ---------------------------------------------------------------------------

pub fn strengthen_graph(vault_root: &Path, config: &ForgeConfig) -> Result<()> {
    info!("Graph strengthening started");

    let profiles = scan_all_projects(vault_root, config)?;
    info!("Scanned {} projects", profiles.len());

    let bridges = detect_bridges(&profiles, config);
    info!("Detected {} cross-project concepts", bridges.len());

    if config.graph.bridge_notes {
        generate_bridge_notes(vault_root, &bridges, &config.vault.zettelkasten_dir)?;
    }

    if config.graph.backlinks {
        inject_backlinks(vault_root, &profiles, &config.vault.zettelkasten_dir)?;
    }

    if config.graph.related_projects {
        update_related_projects(
            vault_root,
            &profiles,
            &bridges,
            &config.vault.zettelkasten_dir,
        )?;
    }

    if config.graph.auto_tags {
        auto_tag_documents(&profiles, config)?;
    }

    info!("Graph strengthening complete");
    Ok(())
}

// ---------------------------------------------------------------------------
// Data structures
// ---------------------------------------------------------------------------

#[derive(Debug)]
struct ProjectProfile {
    name: String,
    concepts: HashMap<String, Vec<PathBuf>>,
    docs: Vec<PathBuf>,
}

#[derive(Debug)]
struct ConceptBridge {
    name: String,
    tags: Vec<String>,
    projects: BTreeMap<String, Vec<PathBuf>>,
}

// ---------------------------------------------------------------------------
// Scan projects
// ---------------------------------------------------------------------------

fn scan_all_projects(vault_root: &Path, config: &ForgeConfig) -> Result<Vec<ProjectProfile>> {
    let system_dirs = config.all_system_dirs();
    let exclude = config.projects.exclude.clone();

    // Collect project directories first (sequential I/O is cheap for directory listing)
    let project_dirs: Vec<(PathBuf, String)> = fs::read_dir(vault_root)?
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_dir() {
                return None;
            }

            let name = path.file_name().and_then(|s| s.to_str())?.to_string();
            if name.starts_with('.') || system_dirs.contains(&name) || exclude.contains(&name) {
                return None;
            }

            Some((path, name))
        })
        .collect();

    // Scan projects in parallel using rayon
    let profiles: Vec<ProjectProfile> = project_dirs
        .par_iter()
        .filter_map(|(path, name)| {
            match scan_project(path, name, config) {
                Ok(profile) => {
                    if profile.docs.is_empty() {
                        None
                    } else {
                        Some(profile)
                    }
                }
                Err(e) => {
                    debug!("Failed to scan project {}: {:?}", name, e);
                    None
                }
            }
        })
        .collect();

    Ok(profiles)
}

fn scan_project(
    project_dir: &Path,
    project_name: &str,
    config: &ForgeConfig,
) -> Result<ProjectProfile> {
    let mut docs = Vec::new();
    let mut concepts: HashMap<String, Vec<PathBuf>> = HashMap::new();

    for entry in WalkDir::new(project_dir)
        .min_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let p = entry.path();
        if !p.is_file() {
            continue;
        }
        if p.extension().and_then(|s| s.to_str()) != Some("md") {
            continue;
        }

        let stem = p.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        if stem == project_name {
            continue;
        }

        if p.components()
            .any(|c| c.as_os_str() == "src" || c.as_os_str() == "target")
        {
            continue;
        }

        let content = match fs::read_to_string(p) {
            Ok(c) => c.to_lowercase(),
            Err(_) => continue,
        };

        docs.push(p.to_path_buf());

        for concept in &config.graph.concepts {
            if concept.keywords.iter().any(|kw| content.contains(kw)) {
                concepts
                    .entry(concept.name.clone())
                    .or_default()
                    .push(p.to_path_buf());
            }
        }
    }

    Ok(ProjectProfile {
        name: project_name.to_string(),
        concepts,
        docs,
    })
}

// ---------------------------------------------------------------------------
// Detect bridges
// ---------------------------------------------------------------------------

fn detect_bridges(profiles: &[ProjectProfile], config: &ForgeConfig) -> Vec<ConceptBridge> {
    let mut concept_map: BTreeMap<String, BTreeMap<String, Vec<PathBuf>>> = BTreeMap::new();

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

// ---------------------------------------------------------------------------
// Generate bridge notes
// ---------------------------------------------------------------------------

fn generate_bridge_notes(vault_root: &Path, bridges: &[ConceptBridge], zk_dir: &str) -> Result<()> {
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

// ---------------------------------------------------------------------------
// Inject backlinks
// ---------------------------------------------------------------------------

fn inject_backlinks(vault_root: &Path, profiles: &[ProjectProfile], zk_dir: &str) -> Result<()> {
    let zk_path = vault_root.join(zk_dir);

    // Cache all zettelkasten files once to avoid repeated directory reads
    let zk_files: Vec<(String, String)> = match fs::read_dir(&zk_path) {
        Ok(entries) => entries
            .flatten()
            .filter_map(|entry| {
                let p = entry.path();
                if p.extension().and_then(|s| s.to_str()) != Some("md") {
                    return None;
                }
                let stem = p.file_stem().and_then(|s| s.to_str())?.to_string();
                let name_lower = stem.replace('-', " ").to_lowercase();
                Some((stem, name_lower))
            })
            .collect(),
        Err(_) => Vec::new(),
    };

    // Process each profile's documents in parallel
    profiles.par_iter().for_each(|profile| {
        let hub_link = format!("[[{}/{}]]", profile.name, profile.name);

        profile.docs.iter().for_each(|doc_path| {
            let content = match fs::read_to_string(doc_path) {
                Ok(c) => c,
                Err(_) => return,
            };

            if content.lines().count() < 3 {
                return;
            }

            let content_lower = content.to_lowercase();
            let mut related: Vec<String> = Vec::new();

            // Use cached zk_files instead of reading directory each time
            for (stem, name_lower) in &zk_files {
                let words: Vec<&str> = name_lower.split_whitespace().collect();
                if words.len() >= 2 && words.iter().all(|w| content_lower.contains(w)) {
                    related.push(format!("- [[{}/{}]]", zk_dir, stem));
                }
            }

            let mut see_also = format!("## See Also\n- {}", hub_link);
            for link in &related {
                see_also.push('\n');
                see_also.push_str(link);
            }
            see_also.push('\n');

            let new_content = if content.contains("## See Also") {
                replace_section(&content, "## See Also", &see_also)
            } else {
                format!("{}\n\n{}", content.trim_end(), see_also)
            };

            if content != new_content {
                let _ = fs::write(doc_path, &new_content);
                debug!("Backlink injected: {}", doc_path.display());
            }
        });
    });

    info!("Backlink injection complete");
    Ok(())
}

// ---------------------------------------------------------------------------
// Related projects in hub files
// ---------------------------------------------------------------------------

fn update_related_projects(
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
            section.push_str(&format!("\n## Key Concepts\n"));
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

// ---------------------------------------------------------------------------
// Auto-tag
// ---------------------------------------------------------------------------

fn auto_tag_documents(profiles: &[ProjectProfile], config: &ForgeConfig) -> Result<()> {
    let fm_re = Regex::new(r"(?s)^---\n(.*?)\n---\n(.*)$").unwrap();
    let tags_re = Regex::new(r"(?m)^tags:\s*\[").unwrap();

    // Use atomic counter for thread-safe counting
    use std::sync::atomic::{AtomicUsize, Ordering};
    let tagged_count = AtomicUsize::new(0);

    // Process all documents in parallel
    profiles.par_iter().for_each(|profile| {
        let project_name = profile.name.clone();
        let project_tags = config.graph.concepts.clone();

        profile.docs.iter().for_each(|doc_path| {
            let content = match fs::read_to_string(doc_path) {
                Ok(c) => c,
                Err(_) => return,
            };

            if tags_re.is_match(&content) {
                return;
            }

            let content_lower = content.to_lowercase();
            let mut tags: BTreeSet<String> = BTreeSet::new();
            tags.insert(project_name.clone());

            for concept in &project_tags {
                if concept.keywords.iter().any(|kw| content_lower.contains(kw)) {
                    for tag in &concept.tags {
                        if tag != "evergreen" {
                            tags.insert(tag.clone());
                        }
                    }
                }
            }

            let stem = doc_path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
            match stem {
                "PRD" => {
                    tags.insert("prd".into());
                }
                "ARCHITECTURE" => {
                    tags.insert("architecture".into());
                }
                "CONVENTIONS" => {
                    tags.insert("conventions".into());
                }
                "DECISIONS" => {
                    tags.insert("decisions".into());
                    tags.insert("adr".into());
                }
                "PROGRESS" => {
                    tags.insert("progress".into());
                }
                "DEBT" => {
                    tags.insert("tech-debt".into());
                }
                "SECRETS_MAP" => {
                    tags.insert("secrets".into());
                }
                _ => {}
            }

            if tags.is_empty() {
                return;
            }

            let tags_str = tags.into_iter().collect::<Vec<_>>().join(", ");

            let new_content = if let Some(caps) = fm_re.captures(&content) {
                let yaml = caps.get(1).unwrap().as_str();
                let body = caps.get(2).unwrap().as_str();
                if yaml.contains("tags:") {
                    return;
                }
                format!("---\n{}\ntags: [{}]\n---\n{}", yaml, tags_str, body)
            } else {
                format!("---\ntags: [{}]\n---\n\n{}", tags_str, content)
            };

            let _ = fs::write(doc_path, &new_content);
            tagged_count.fetch_add(1, Ordering::Relaxed);
            debug!("Auto-tagged: {}", doc_path.display());
        });
    });

    let count = tagged_count.load(Ordering::Relaxed);
    if count > 0 {
        info!("Auto-tagged {} documents", count);
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
