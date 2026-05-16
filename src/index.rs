use anyhow::Result;
use std::{fs, path::Path};
use tracing::info;
use walkdir::WalkDir;

use crate::config::ForgeConfig;

/// Generate a static `index.md` file at the vault root that serves as the AI agent entry point.
///
/// The file contains links to all areas, active projects, zettelkasten notes, key hub concepts,
/// and governance documents. It is idempotent — safe to re-run. The file is only written when
/// the generated content differs from the existing file.
pub fn generate_index(vault_root: &Path, config: &ForgeConfig) -> Result<()> {
    let vault_name = &config.vault.name;
    let system_dirs = config.all_system_dirs();
    let exclude = &config.projects.exclude;

    let mut content = String::new();

    // Frontmatter
    content.push_str("---\n");
    content.push_str("project: root\n");
    content.push_str("type: agent-index\n");
    content.push_str("tags: [agent-entry, layer/raw]\n");
    content.push_str("---\n\n");

    // Header
    content.push_str(&format!("# {} -- Agent Index\n\n", vault_name));

    // Section: Areas
    let areas = collect_md_files(&vault_root.join("02-Areas"), vault_root);
    content.push_str("## Areas\n");
    if areas.is_empty() {
        content.push_str("<!-- No area notes found -->\n");
    } else {
        for link in &areas {
            content.push_str(&format!("- [[{}]]\n", link));
        }
    }
    content.push('\n');

    // Section: Active Projects
    let projects = collect_project_links(vault_root, &system_dirs, exclude);
    content.push_str("## Active Projects\n");
    if projects.is_empty() {
        content.push_str("<!-- No active projects found -->\n");
    } else {
        for link in &projects {
            content.push_str(&format!("- [[{}]]\n", link));
        }
    }
    content.push('\n');

    // Section: Zettelkasten (Wiki Layer)
    let zk_dir = vault_root.join(&config.vault.zettelkasten_dir);
    let zk_files = collect_md_files(&zk_dir, vault_root);
    content.push_str("## Zettelkasten (Wiki Layer)\n");
    if zk_files.is_empty() {
        content.push_str("<!-- No zettelkasten notes found -->\n");
    } else {
        for link in &zk_files {
            content.push_str(&format!("- [[{}]]\n", link));
        }
    }
    content.push('\n');

    // Section: Key Concepts (hub notes by incoming link count)
    let hubs = compute_hub_notes(vault_root, config, 10);
    content.push_str("## Key Concepts\n");
    if hubs.is_empty() {
        content.push_str("<!-- Build vault graph first to see key concepts -->\n");
    } else {
        for (path, count) in &hubs {
            let stem = Path::new(path)
                .with_extension("")
                .to_string_lossy()
                .replace('\\', "/");
            content.push_str(&format!("- [[{}]] ({} incoming)\n", stem, count));
        }
    }
    content.push('\n');

    // Section: Governance
    content.push_str("## Governance\n");
    content.push_str("- [[TAGGING]]\n");
    content.push_str("- [[Home]]\n");
    if vault_root.join("README.md").exists() {
        content.push_str("- [[README]]\n");
    }
    content.push('\n');

    // Only write if content changed
    let index_path = vault_root.join("index.md");
    let existing = fs::read_to_string(&index_path).unwrap_or_default();

    if existing != content {
        fs::write(&index_path, &content)?;
        info!("index.md generated at {}", index_path.display());
    } else {
        info!("index.md unchanged, skipping write");
    }

    Ok(())
}

/// Collect all *.md files under a directory, returning relative paths without extension.
fn collect_md_files(dir: &Path, vault_root: &Path) -> Vec<String> {
    if !dir.exists() {
        return Vec::new();
    }

    let mut files: Vec<String> = WalkDir::new(dir)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| {
            let p = e.path();
            p.is_file() && p.extension().and_then(|s| s.to_str()) == Some("md")
        })
        .filter(|e| {
            !e.path().components().any(|c| {
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
            let stem = rel.with_extension("").to_string_lossy().replace('\\', "/");
            Some(stem)
        })
        .collect();

    files.sort();
    files
}

/// Collect links to project directories: link to the project hub MOC if it exists,
/// otherwise link to the project folder.
fn collect_project_links(
    vault_root: &Path,
    system_dirs: &[String],
    exclude: &[String],
) -> Vec<String> {
    let archive_projects = vault_root.join("99-Archives").join("projects");
    if !archive_projects.is_dir() {
        return Vec::new();
    }

    let mut links = Vec::new();

    if let Ok(entries) = fs::read_dir(&archive_projects) {
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

            // Link to project hub MOC if it exists
            let hub_moc = path.join(format!("{}.md", name));
            if hub_moc.exists() {
                let rel = hub_moc
                    .strip_prefix(vault_root)
                    .unwrap_or(&hub_moc)
                    .with_extension("")
                    .to_string_lossy()
                    .replace('\\', "/");
                links.push(rel);
            } else {
                // Link to the project directory itself
                let rel = path
                    .strip_prefix(vault_root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .replace('\\', "/");
                links.push(format!("{}/", rel));
            }
        }
    }

    links.sort();
    links
}

/// Compute hub notes (top N by incoming link count) from the vault graph.
/// Returns an empty vec if the graph cannot be built.
fn compute_hub_notes(
    vault_root: &Path,
    config: &ForgeConfig,
    top_n: usize,
) -> Vec<(String, usize)> {
    match crate::graph::build_vault_graph(vault_root, config) {
        Ok(graph) => graph.hub_notes(top_n),
        Err(_) => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn setup_test_vault(dir: &Path) -> ForgeConfig {
        // Create PARA structure
        fs::create_dir_all(dir.join("02-Areas")).unwrap();
        fs::create_dir_all(dir.join("99-Archives/projects")).unwrap();
        fs::create_dir_all(dir.join("10-Zettelkasten")).unwrap();

        ForgeConfig::default_for("TestVault")
    }

    #[test]
    fn test_generate_index_creates_file_with_expected_sections() {
        let tmp = tempfile::tempdir().unwrap();
        let vault_root = tmp.path();
        let config = setup_test_vault(vault_root);

        // Create sample area note
        fs::write(vault_root.join("02-Areas/Rust.md"), "# Rust\n").unwrap();

        // Create sample project with hub MOC
        let proj_dir = vault_root.join("99-Archives/projects/my-project");
        fs::create_dir_all(&proj_dir).unwrap();
        fs::write(proj_dir.join("my-project.md"), "# My Project MOC\n").unwrap();

        // Create sample zettelkasten note
        fs::write(
            vault_root.join("10-Zettelkasten/atomic-concept.md"),
            "# Atomic Concept\n",
        )
        .unwrap();

        generate_index(vault_root, &config).unwrap();

        let index_path = vault_root.join("index.md");
        assert!(index_path.exists(), "index.md should be created");

        let content = fs::read_to_string(&index_path).unwrap();

        // Check frontmatter
        assert!(content.contains("project: root"));
        assert!(content.contains("type: agent-index"));
        assert!(content.contains("tags: [agent-entry, layer/raw]"));

        // Check header
        assert!(content.contains("# TestVault -- Agent Index"));

        // Check sections exist
        assert!(content.contains("## Areas"));
        assert!(content.contains("## Active Projects"));
        assert!(content.contains("## Zettelkasten (Wiki Layer)"));
        assert!(content.contains("## Key Concepts"));
        assert!(content.contains("## Governance"));

        // Check area link
        assert!(content.contains("[[02-Areas/Rust]]"));

        // Check project link (hub MOC)
        assert!(content.contains("[[99-Archives/projects/my-project/my-project]]"));

        // Check zettelkasten link
        assert!(content.contains("[[10-Zettelkasten/atomic-concept]]"));

        // Check governance links
        assert!(content.contains("[[TAGGING]]"));
        assert!(content.contains("[[Home]]"));
    }

    #[test]
    fn test_generate_index_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let vault_root = tmp.path();
        let config = setup_test_vault(vault_root);

        // First run
        generate_index(vault_root, &config).unwrap();
        let first = fs::read_to_string(vault_root.join("index.md")).unwrap();

        // Second run — same content, no change
        generate_index(vault_root, &config).unwrap();
        let second = fs::read_to_string(vault_root.join("index.md")).unwrap();

        assert_eq!(
            first, second,
            "index.md should be identical on second run (idempotent)"
        );
    }

    #[test]
    fn test_generate_index_empty_vault() {
        let tmp = tempfile::tempdir().unwrap();
        let vault_root = tmp.path();
        let config = setup_test_vault(vault_root);

        generate_index(vault_root, &config).unwrap();

        let content = fs::read_to_string(vault_root.join("index.md")).unwrap();

        // Should have placeholder comments for empty sections
        assert!(content.contains("<!-- No area notes found -->"));
        assert!(content.contains("<!-- No active projects found -->"));
        assert!(content.contains("<!-- No zettelkasten notes found -->"));
        assert!(content.contains("<!-- Build vault graph first to see key concepts -->"));
    }

    #[test]
    fn test_generate_index_project_without_hub_moc() {
        let tmp = tempfile::tempdir().unwrap();
        let vault_root = tmp.path();
        let config = setup_test_vault(vault_root);

        // Create project directory without hub MOC
        let proj_dir = vault_root.join("99-Archives/projects/bare-project");
        fs::create_dir_all(&proj_dir).unwrap();
        fs::write(proj_dir.join("PRD.md"), "# PRD\n").unwrap();

        generate_index(vault_root, &config).unwrap();

        let content = fs::read_to_string(vault_root.join("index.md")).unwrap();

        // Should link to the directory when no hub MOC exists
        assert!(content.contains("99-Archives/projects/bare-project/"));
    }

    #[test]
    fn test_generate_index_with_readme() {
        let tmp = tempfile::tempdir().unwrap();
        let vault_root = tmp.path();
        let config = setup_test_vault(vault_root);

        fs::write(vault_root.join("README.md"), "# README\n").unwrap();

        generate_index(vault_root, &config).unwrap();

        let content = fs::read_to_string(vault_root.join("index.md")).unwrap();
        assert!(content.contains("[[README]]"));
    }

    #[test]
    fn test_generate_index_without_readme() {
        let tmp = tempfile::tempdir().unwrap();
        let vault_root = tmp.path();
        let config = setup_test_vault(vault_root);

        generate_index(vault_root, &config).unwrap();

        let content = fs::read_to_string(vault_root.join("index.md")).unwrap();
        assert!(!content.contains("[[README]]"));
    }

    #[test]
    fn test_collect_md_files_skips_system_dirs() {
        let tmp = tempfile::tempdir().unwrap();
        let vault_root = tmp.path();
        let areas_dir = vault_root.join("02-Areas");

        fs::create_dir_all(areas_dir.join(".git")).unwrap();
        fs::create_dir_all(areas_dir.join(".obsidian")).unwrap();
        fs::write(areas_dir.join("Rust.md"), "# Rust\n").unwrap();
        fs::write(areas_dir.join(".git/config"), "stuff\n").unwrap();
        fs::write(areas_dir.join(".obsidian/config"), "stuff\n").unwrap();

        let files = collect_md_files(&areas_dir, vault_root);
        assert_eq!(files, vec!["02-Areas/Rust"]);
    }
}
