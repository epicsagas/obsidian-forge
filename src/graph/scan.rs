use anyhow::Result;
use rayon::prelude::*;
use std::{
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
};
use tracing::debug;
use walkdir::WalkDir;

use crate::config::ForgeConfig;

#[derive(Debug)]
pub struct ProjectProfile {
    pub name: String,
    pub concepts: HashMap<String, Vec<PathBuf>>,
    pub docs: Vec<PathBuf>,
}

pub fn scan_all_projects(vault_root: &Path, config: &ForgeConfig) -> Result<Vec<ProjectProfile>> {
    let system_dirs = config.all_system_dirs();
    let exclude = config.projects.exclude.clone();

    let project_dirs: Vec<(PathBuf, String)> = fs::read_dir(vault_root)?
        .flat_map(|e| e.ok())
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

    let profiles: Vec<ProjectProfile> = project_dirs
        .par_iter()
        .filter_map(|(path, name)| match scan_project(path, name, config) {
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
