use anyhow::Result;
use rayon::prelude::*;
use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};
use tracing::{debug, info};
use walkdir::WalkDir;

use crate::config::ForgeConfig;

/// Regenerate hub files for every project folder in the vault.
pub fn update_all_mocs(vault_root: &Path, config: &ForgeConfig) -> Result<()> {
    let system_dirs = config.all_system_dirs();
    let exclude = config.projects.exclude.clone();

    // Collect project directories first
    let project_dirs: Vec<(PathBuf, String)> = fs::read_dir(vault_root)?
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_dir() { return None; }

            let name = path.file_name().and_then(|s| s.to_str())?.to_string();

            if name.starts_with('.')
                || system_dirs.contains(&name)
                || exclude.contains(&name)
            {
                return None;
            }

            Some((path, name))
        })
        .collect();

    // Process MOCs in parallel
    project_dirs.par_iter().for_each(|(path, name)| {
        if let Err(e) = update_moc_for_project(path, name, vault_root) {
            debug!("Failed to update MOC for {}: {:?}", name, e);
        }
    });

    update_home_moc(vault_root, config)?;
    Ok(())
}

fn update_moc_for_project(project_dir: &Path, project_name: &str, vault_root: &Path) -> Result<()> {
    let mut groups: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for entry in WalkDir::new(project_dir)
        .min_depth(1)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let p = entry.path();
        if !p.is_file() { continue; }
        if p.extension().and_then(|s| s.to_str()) != Some("md") { continue; }

        let filename = p.file_stem().and_then(|s| s.to_str()).unwrap_or("").to_string();
        if filename == project_name { continue; }

        // Skip source code
        if p.components().any(|c| c.as_os_str() == "src" || c.as_os_str() == "target") {
            continue;
        }

        let rel = p.strip_prefix(vault_root).unwrap_or(p);
        let link = rel.with_extension("").to_string_lossy().replace('\\', "/");

        let group_key = if entry.depth() == 1 {
            "Core Docs".to_string()
        } else {
            p.parent()
                .and_then(|par| par.strip_prefix(project_dir).ok())
                .map(|rel_par| rel_par.to_string_lossy().replace('\\', "/"))
                .unwrap_or_else(|| "Core Docs".to_string())
        };

        groups.entry(group_key).or_default().push(format!("- [[{}]]", link));
    }

    if groups.is_empty() {
        debug!("No markdown files in {}, skipping MOC", project_name);
        return Ok(());
    }

    let mut content = format!(
        "---\nproject: {}\ntype: moc\ntags: [{}]\n---\n\n# {} MOC\n\n",
        project_name, project_name, capitalize(project_name),
    );

    if let Some(links) = groups.get("Core Docs") {
        content.push_str("## Core Docs\n");
        for link in links { content.push_str(link); content.push('\n'); }
        content.push('\n');
    }

    for (group, links) in &groups {
        if group == "Core Docs" { continue; }
        content.push_str(&format!("## {}\n", capitalize_path(group)));
        for link in links { content.push_str(link); content.push('\n'); }
        content.push('\n');
    }

    let moc_path = project_dir.join(format!("{}.md", project_name));
    let existing = fs::read_to_string(&moc_path).unwrap_or_default();

    let preserved = extract_preserved_sections(&existing);
    if !preserved.is_empty() {
        content.push_str(&preserved);
    }

    if existing != content {
        fs::write(&moc_path, &content)?;
        info!("MOC updated: {}", moc_path.display());
    }

    Ok(())
}

fn update_home_moc(vault_root: &Path, config: &ForgeConfig) -> Result<()> {
    let home_path = vault_root.join("Home.md");
    let existing = fs::read_to_string(&home_path).unwrap_or_default();

    let system_dirs = config.all_system_dirs();
    let exclude = &config.projects.exclude;

    let mut project_links = Vec::new();
    for entry in fs::read_dir(vault_root)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() { continue; }
        let name = match path.file_name().and_then(|s| s.to_str()) {
            Some(n) => n.to_string(),
            None => continue,
        };
        if name.starts_with('.')
            || system_dirs.contains(&name)
            || exclude.contains(&name)
        {
            continue;
        }
        let hub = path.join(format!("{}.md", name));
        if hub.exists() {
            project_links.push(format!("- [[{}/{}]]", name, name));
        }
    }
    project_links.sort();

    if project_links.is_empty() { return Ok(()); }

    let projects_block = format!("## Projects\n{}\n", project_links.join("\n"));
    let new_content = if existing.contains("## Projects") {
        replace_section(&existing, "## Projects", &projects_block)
    } else {
        format!("{}\n\n{}", existing.trim_end(), projects_block)
    };

    if existing != new_content {
        fs::write(&home_path, &new_content)?;
        info!("Home.md updated with {} project links", project_links.len());
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers (public for graph.rs)
// ---------------------------------------------------------------------------

pub fn replace_section(content: &str, header: &str, replacement: &str) -> String {
    let mut result = String::new();
    let mut in_section = false;
    let mut replaced = false;

    for line in content.lines() {
        if line.starts_with(header) {
            in_section = true;
            replaced = true;
            result.push_str(replacement);
            if !replacement.ends_with('\n') { result.push('\n'); }
            continue;
        }
        if in_section && (line.starts_with("## ") || line.starts_with("# ")) {
            in_section = false;
            result.push_str(line);
            result.push('\n');
            continue;
        }
        if in_section { continue; }
        result.push_str(line);
        result.push('\n');
    }

    if !replaced { result.push_str(replacement); }
    result
}

fn extract_preserved_sections(content: &str) -> String {
    let preserved_headers = ["## Related Projects", "## Key Concepts"];
    let mut result = String::new();
    let mut in_section = false;

    for line in content.lines() {
        if preserved_headers.iter().any(|h| line.starts_with(h)) {
            in_section = true;
        } else if in_section && line.starts_with("## ") {
            in_section = false;
        }
        if in_section {
            result.push_str(line);
            result.push('\n');
        }
    }

    if !result.is_empty() { format!("\n{}", result) } else { String::new() }
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        None => String::new(),
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
    }
}

fn capitalize_path(s: &str) -> String {
    s.split('/').map(capitalize).collect::<Vec<_>>().join(" / ")
}
