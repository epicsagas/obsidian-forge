use anyhow::Result;
use std::{
    fs,
    path::{Path, PathBuf},
};
use tracing::warn;

/// Initialize a new book project under `01-Projects/{name}/` within the vault.
pub fn init_book_project(name: &str, vault_path: &Path, genre: &str, lang: &str) -> Result<()> {
    let book_root = vault_path.join("01-Projects").join(name);
    fs::create_dir_all(&book_root)?;

    let subdirs = ["drafts", "edits", "publish/cover", "sources"];
    for d in &subdirs {
        fs::create_dir_all(book_root.join(d))?;
    }

    // PRD.md template
    let prd_path = book_root.join("PRD.md");
    if !prd_path.exists() {
        let prd = format!(
            "---\ntitle: {name}\ngenre: {genre}\nlanguage: {lang}\ntarget_audience: \nword_count_goal: 0\n---\n\n# {name}\n\n## Overview\n\n## Target Audience\n\n## Goals\n\n## Word Count Target\n"
        );
        fs::write(&prd_path, prd)?;
    }

    // STYLE.md template
    let style_path = book_root.join("STYLE.md");
    if !style_path.exists() {
        let style = "---\ntype: style-guide\n---\n\n# Style Guide\n\n## Voice\n\n## Tone\n\n## Forbidden Patterns\n";
        fs::write(&style_path, style)?;
    }

    // sources/ -> ../../03-Resources (relative symlink)
    let sources_dir = book_root.join("sources");
    // Remove and recreate as symlink if it's currently an empty directory
    if sources_dir.is_dir() && !sources_dir.is_symlink() {
        // Only convert if empty
        if fs::read_dir(&sources_dir)?.next().is_none() {
            fs::remove_dir(&sources_dir)?;
            create_symlink(Path::new("../../03-Resources"), &sources_dir)?;
        }
    } else if !sources_dir.exists() {
        create_symlink(Path::new("../../03-Resources"), &sources_dir)?;
    }

    println!("Book project initialized: 01-Projects/{name}/");
    Ok(())
}

/// Show status of all book projects (or a specific one) in the vault.
pub fn show_book_status(name: Option<&str>, vault_path: &Path) -> Result<()> {
    let projects_dir = vault_path.join("01-Projects");
    if !projects_dir.exists() {
        println!("No 01-Projects directory found in vault.");
        return Ok(());
    }

    let mut found = false;

    // Header
    println!(
        "{:<24} {:<10} {:<8} {:<8} {:<8} {:<8}",
        "PROJECT", "GENRE", "DRAFT", "EDIT", "PUBLISH", "SOURCES"
    );
    println!("{}", "-".repeat(72));

    for entry in fs::read_dir(&projects_dir)? {
        let entry = entry?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let project_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        // Filter by name if specified
        if name.is_some_and(|n| project_name != n) {
            continue;
        }

        // Must have both PRD.md and STYLE.md to be a book project
        if !path.join("PRD.md").exists() || !path.join("STYLE.md").exists() {
            continue;
        }

        found = true;

        // Read genre from PRD.md frontmatter (simple scan)
        let genre = read_frontmatter_field(&path.join("PRD.md"), "genre").unwrap_or_default();

        let draft_ok = marker_exists(&path, "drafts");
        let edit_ok = marker_exists(&path, "edits");
        let publish_ok = marker_exists(&path, "publish");
        let sources_ok = path.join("sources").exists();

        println!(
            "{:<24} {:<10} {:<8} {:<8} {:<8} {:<8}",
            project_name,
            genre,
            if draft_ok { "done" } else { "-" },
            if edit_ok { "done" } else { "-" },
            if publish_ok { "done" } else { "-" },
            if sources_ok { "linked" } else { "-" },
        );
    }

    if !found {
        if let Some(n) = name {
            println!("No book project found: {n}");
        } else {
            println!("No book projects found in 01-Projects/");
        }
    }

    Ok(())
}

/// Export a book project to a standalone directory compatible with book-forge.
pub fn export_book(name: &str, vault_path: &Path, output_path: &Path) -> Result<()> {
    let book_root = vault_path.join("01-Projects").join(name);
    if !book_root.exists() {
        anyhow::bail!("Book project not found: 01-Projects/{name}");
    }

    let dest = output_path.join(name);
    fs::create_dir_all(&dest)?;

    copy_dir_filtered(&book_root, &dest)?;

    println!("Exported: 01-Projects/{name} → {}", dest.display());
    Ok(())
}

/// Sync vault notes tagged `book/{name}` into `sources/` as symlinks.
pub fn sync_sources(name: &str, vault_path: &Path) -> Result<()> {
    let book_root = vault_path.join("01-Projects").join(name);
    let sources_dir = book_root.join("sources");

    // If sources/ is a symlink, resolve it to get the real target directory
    let sources_real = if sources_dir.is_symlink() {
        match fs::canonicalize(&sources_dir) {
            Ok(p) => p,
            Err(_) => {
                warn!("sources/ symlink is dangling, skipping sync");
                return Ok(());
            }
        }
    } else if sources_dir.is_dir() {
        sources_dir.clone()
    } else {
        fs::create_dir_all(&sources_dir)?;
        sources_dir.clone()
    };

    let tag_pattern = format!("book/{name}");
    let mut linked = 0usize;

    // Walk vault looking for notes with matching tag
    for entry in walkdir::WalkDir::new(vault_path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("md") {
            continue;
        }
        // Skip the book project itself
        if path.starts_with(&book_root) {
            continue;
        }

        if note_has_tag(path, &tag_pattern) {
            let file_name = path.file_name().unwrap_or_default();
            let link_path = sources_real.join(file_name);
            if link_path.exists() || link_path.is_symlink() {
                continue;
            }
            if let Err(e) = create_symlink(path, &link_path) {
                warn!("Failed to link {:?}: {e}", path);
            } else {
                linked += 1;
            }
        }
    }

    println!("Synced {linked} note(s) tagged '{tag_pattern}' into sources/");
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Check whether a phase directory has any content (non-empty = done marker).
fn marker_exists(book_root: &Path, dir: &str) -> bool {
    let p = book_root.join(dir);
    if !p.exists() {
        return false;
    }
    fs::read_dir(&p)
        .map(|mut r| r.next().is_some())
        .unwrap_or(false)
}

/// Read a YAML frontmatter field from a markdown file.
fn read_frontmatter_field(path: &Path, field: &str) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let inner = text.strip_prefix("---\n")?.split("\n---").next()?;
    for line in inner.lines() {
        if let Some(rest) = line.strip_prefix(&format!("{field}:")) {
            return Some(rest.trim().to_string());
        }
    }
    None
}

/// Check whether a markdown note's frontmatter tags contain the given tag.
fn note_has_tag(path: &Path, tag: &str) -> bool {
    let text = match fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return false,
    };
    let Some(inner) = text
        .strip_prefix("---\n")
        .and_then(|s| s.split("\n---").next())
    else {
        return false;
    };
    inner.contains(tag)
}

/// Recursively copy a directory, skipping .obsidian/, sources/ symlinks, and hidden dirs.
fn copy_dir_filtered(src: &Path, dst: &Path) -> Result<()> {
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let name = entry.file_name();
        let name_str = name.to_string_lossy();

        // Skip hidden directories and sources symlink
        if name_str.starts_with('.') || name_str == "sources" {
            continue;
        }

        let src_path = entry.path();
        let dst_path = dst.join(&name);

        if src_path.is_symlink() {
            // Skip symlinks entirely in export
            continue;
        }

        if src_path.is_dir() {
            fs::create_dir_all(&dst_path)?;
            copy_dir_filtered(&src_path, &dst_path)?;
        } else {
            if let Err(e) = fs::copy(&src_path, &dst_path) {
                warn!("Failed to copy {:?}: {e}", src_path);
            }
        }
    }
    Ok(())
}

#[cfg(unix)]
fn create_symlink(target: &Path, link: &Path) -> Result<()> {
    std::os::unix::fs::symlink(target, link)?;
    Ok(())
}

#[cfg(not(unix))]
fn create_symlink(_target: &Path, _link: &Path) -> Result<()> {
    anyhow::bail!("Symlinks are not supported on this platform");
}

// ---------------------------------------------------------------------------
// Helper type for export path
// ---------------------------------------------------------------------------

pub fn output_path_from(output: &str, vault_path: &Path) -> PathBuf {
    let p = PathBuf::from(output);
    if p.is_absolute() {
        p
    } else {
        vault_path.join(p)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_vault() -> TempDir {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir_all(dir.path().join("01-Projects")).unwrap();
        fs::create_dir_all(dir.path().join("03-Resources")).unwrap();
        dir
    }

    #[test]
    fn test_init_creates_directory_structure() {
        let vault = make_vault();
        init_book_project("my-book", vault.path(), "non-fiction", "ko").unwrap();

        let root = vault.path().join("01-Projects/my-book");
        assert!(root.exists(), "book root should exist");
        assert!(root.join("PRD.md").exists(), "PRD.md should exist");
        assert!(root.join("STYLE.md").exists(), "STYLE.md should exist");
        assert!(root.join("drafts").exists(), "drafts/ should exist");
        assert!(root.join("edits").exists(), "edits/ should exist");
        assert!(
            root.join("publish/cover").exists(),
            "publish/cover/ should exist"
        );
    }

    #[test]
    fn test_prd_contains_frontmatter_fields() {
        let vault = make_vault();
        init_book_project("test-book", vault.path(), "fiction", "en").unwrap();

        let prd = fs::read_to_string(vault.path().join("01-Projects/test-book/PRD.md")).unwrap();
        assert!(prd.contains("genre: fiction"));
        assert!(prd.contains("language: en"));
        assert!(prd.contains("title: test-book"));
    }

    #[test]
    fn test_style_md_created() {
        let vault = make_vault();
        init_book_project("style-test", vault.path(), "non-fiction", "ko").unwrap();

        let style_path = vault.path().join("01-Projects/style-test/STYLE.md");
        assert!(style_path.exists());
        let content = fs::read_to_string(&style_path).unwrap();
        assert!(content.contains("Voice"));
        assert!(content.contains("Tone"));
        assert!(content.contains("Forbidden Patterns"));
    }

    #[test]
    fn test_init_idempotent() {
        let vault = make_vault();
        // Write custom content to PRD.md
        init_book_project("idem-book", vault.path(), "non-fiction", "ko").unwrap();
        let prd_path = vault.path().join("01-Projects/idem-book/PRD.md");
        fs::write(&prd_path, "custom content").unwrap();

        // Second init should not overwrite
        init_book_project("idem-book", vault.path(), "fiction", "en").unwrap();
        let content = fs::read_to_string(&prd_path).unwrap();
        assert_eq!(content, "custom content");
    }

    #[test]
    fn test_show_status_finds_book_projects() {
        let vault = make_vault();
        init_book_project("book-a", vault.path(), "fiction", "ko").unwrap();
        // Non-book project (no STYLE.md)
        let non_book = vault.path().join("01-Projects/not-a-book");
        fs::create_dir_all(&non_book).unwrap();
        fs::write(non_book.join("PRD.md"), "# not a book").unwrap();

        // Should not panic
        show_book_status(None, vault.path()).unwrap();
    }

    #[test]
    fn test_show_status_specific_name() {
        let vault = make_vault();
        init_book_project("book-b", vault.path(), "non-fiction", "en").unwrap();
        show_book_status(Some("book-b"), vault.path()).unwrap();
    }

    #[test]
    fn test_export_creates_output_directory() {
        let vault = make_vault();
        init_book_project("export-book", vault.path(), "non-fiction", "ko").unwrap();

        let output_dir = tempfile::tempdir().unwrap();
        export_book("export-book", vault.path(), output_dir.path()).unwrap();

        let exported = output_dir.path().join("export-book");
        assert!(exported.exists(), "exported directory should exist");
        assert!(exported.join("PRD.md").exists(), "PRD.md should be copied");
        assert!(
            exported.join("STYLE.md").exists(),
            "STYLE.md should be copied"
        );
    }

    #[test]
    fn test_export_skips_symlinks() {
        let vault = make_vault();
        init_book_project("sym-book", vault.path(), "non-fiction", "ko").unwrap();

        let output_dir = tempfile::tempdir().unwrap();
        export_book("sym-book", vault.path(), output_dir.path()).unwrap();

        // sources/ (symlink) should not appear in export
        let exported_sources = output_dir.path().join("sym-book/sources");
        assert!(
            !exported_sources.exists(),
            "sources symlink should not be exported"
        );
    }

    #[test]
    fn test_sync_sources_with_tagged_notes() {
        let vault = make_vault();
        init_book_project("sync-book", vault.path(), "non-fiction", "ko").unwrap();

        // Remove sources symlink to use a regular dir for testing
        let sources = vault.path().join("01-Projects/sync-book/sources");
        if sources.is_symlink() {
            fs::remove_file(&sources).unwrap();
        }
        fs::create_dir_all(&sources).unwrap();

        // Create a note tagged for this book
        let note_dir = vault.path().join("10-Zettelkasten");
        fs::create_dir_all(&note_dir).unwrap();
        let note = note_dir.join("tagged-note.md");
        fs::write(&note, "---\ntags: [book/sync-book]\n---\n\n# Tagged Note\n").unwrap();

        sync_sources("sync-book", vault.path()).unwrap();

        let link = sources.join("tagged-note.md");
        assert!(
            link.exists() || link.is_symlink(),
            "tagged note should be linked in sources/"
        );
    }

    #[test]
    fn test_read_frontmatter_field() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.md");
        fs::write(
            &path,
            "---\ngenre: fiction\nlanguage: ko\n---\n\n# Content\n",
        )
        .unwrap();

        assert_eq!(
            read_frontmatter_field(&path, "genre"),
            Some("fiction".into())
        );
        assert_eq!(read_frontmatter_field(&path, "language"), Some("ko".into()));
        assert_eq!(read_frontmatter_field(&path, "missing"), None);
    }

    #[test]
    fn test_note_has_tag() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("note.md");
        fs::write(&path, "---\ntags: [book/my-book, status/draft]\n---\n").unwrap();

        assert!(note_has_tag(&path, "book/my-book"));
        assert!(!note_has_tag(&path, "book/other-book"));
    }

    #[test]
    fn test_marker_exists_empty_dir() {
        let vault = make_vault();
        init_book_project("marker-test", vault.path(), "non-fiction", "ko").unwrap();
        let root = vault.path().join("01-Projects/marker-test");

        // Empty drafts/ should return false
        assert!(!marker_exists(&root, "drafts"));

        // Write a file → should return true
        fs::write(root.join("drafts/ch01.md"), "# Chapter 1").unwrap();
        assert!(marker_exists(&root, "drafts"));
    }
}
