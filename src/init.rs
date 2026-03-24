use anyhow::Result;
use std::{fs, path::Path, process::Command};
use tracing::{info, warn};

use crate::config::{ForgeConfig, GlobalConfig, CONFIG_FILE, SETTINGS_DIRS, SETTINGS_FILES};
use std::os::unix::fs::symlink;

/// Initialize a vault. Works for both new and existing directories.
/// - New directory: creates it, scaffolds everything, git init + first commit
/// - Existing directory: only adds missing structure, preserves everything that's already there
pub fn init_vault(name: &str, target: &Path) -> Result<()> {
    let vault_root = target.join(name);
    let is_existing = vault_root.exists();

    if is_existing {
        info!(
            "Adopting existing directory as vault: {}",
            vault_root.display()
        );
    } else {
        info!("Creating new vault: {}", vault_root.display());
        fs::create_dir_all(&vault_root)?;
    }

    let mut created = Vec::new();
    let mut skipped = Vec::new();

    adopt_directory_verbose(&vault_root, name, &mut created, &mut skipped)?;

    // git init only if not already a repo
    if !vault_root.join(".git").exists() {
        let _ = Command::new("git")
            .args(["init"])
            .current_dir(&vault_root)
            .output();
        created.push(".git (initialized)".into());
    } else {
        skipped.push(".git (already a repo)".into());
    }

    // First commit only for brand-new vaults
    if !is_existing {
        let _ = Command::new("git")
            .args(["add", "-A"])
            .current_dir(&vault_root)
            .output();
        let _ = Command::new("git")
            .args(["commit", "-m", "feat: initialize vault with obsidian-forge"])
            .current_dir(&vault_root)
            .output();
    }

    register_and_print(&vault_root, name);

    // Report what happened
    if !created.is_empty() {
        println!("\n  Created:");
        for c in &created {
            println!("    + {}", c);
        }
    }
    if !skipped.is_empty() {
        println!("\n  Preserved (already existed):");
        for s in &skipped {
            println!("    = {}", s);
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Core adoption logic — adds only what's missing
// ---------------------------------------------------------------------------

fn adopt_directory_verbose(
    vault_root: &Path,
    name: &str,
    created: &mut Vec<String>,
    skipped: &mut Vec<String>,
) -> Result<()> {
    // ── PARA folders (create only if missing) ────────────────────────────
    let dirs = [
        "00-Inbox",
        "01-Projects",
        "02-Areas",
        "03-Resources/Reference/Articles-Papers",
        "03-Resources/Reference/Books-Notes",
        "03-Resources/Reference/Tutorials-Guides",
        "03-Resources/Reference/Cheat-Sheets",
        "03-Resources/Technical",
        "10-Zettelkasten",
        "99-Archives/PDF-Archive",
        "Attachments",
        "Chats",
        "Clippings",
    ];
    for d in &dirs {
        let p = vault_root.join(d);
        if !p.exists() {
            fs::create_dir_all(&p)?;
            created.push(format!("{}/", d));
        }
    }

    // ── Templates: symlink vault/obsidian-templates → global templates dir ──
    link_global_templates(vault_root, created, skipped)?;

    // ── Home.md ──────────────────────────────────────────────────────────
    write_if_missing(
        vault_root,
        "Home.md",
        &format!(
            "---\ntype: home\ntags: [home]\n---\n\n# {}\n\n## Projects\n\n\
             ## Key Concepts\n\n\
             ## Quick Links\n- [[Inbox-Dashboard]]\n\n\
             ```dataview\nLIST\nFROM \"\"\nWHERE type = \"moc\"\nSORT file.name ASC\n```\n",
            name,
        ),
        created,
        skipped,
    )?;

    // ── Inbox Dashboard ──────────────────────────────────────────────────
    write_if_missing(
        vault_root,
        "Inbox-Dashboard.md",
        "---\ntype: dashboard\ntags: [dashboard]\n---\n\n\
         # Inbox Dashboard\n\n## Processing Statistics\n- Processed notes: 0\n",
        created,
        skipped,
    )?;

    // ── vault.toml ───────────────────────────────────────────────────────
    if !vault_root.join(CONFIG_FILE).exists() {
        let config = ForgeConfig::default_for(name);
        config.save(vault_root)?;
        created.push(CONFIG_FILE.into());
    } else {
        skipped.push(format!("{} (keeping existing config)", CONFIG_FILE));
    }

    // ── .obsidian (preserve everything if exists) ────────────────────────
    let obsidian = vault_root.join(".obsidian");
    if !obsidian.exists() {
        // Fresh .obsidian with minimal config
        fs::create_dir_all(&obsidian)?;
        fs::write(
            obsidian.join("app.json"),
            r#"{"showLineNumber":true,"spellcheck":true}"#,
        )?;
        fs::write(
            obsidian.join("appearance.json"),
            r#"{"baseFontSize":16,"theme":"obsidian"}"#,
        )?;
        fs::write(
            obsidian.join("templates.json"),
            r#"{"folder":"obsidian-templates"}"#,
        )?;
        created.push(".obsidian/ (fresh config)".into());
    } else {
        // Existing .obsidian: inject templates.json only if missing.
        let tpl_json = obsidian.join("templates.json");
        if !tpl_json.exists() {
            fs::write(&tpl_json, r#"{"folder":"obsidian-templates"}"#)?;
            created.push(".obsidian/templates.json (template folder configured)".into());
        }
        skipped.push(".obsidian/ (plugins, themes, settings preserved)".into());
    }

    // ── .gitignore (append missing entries, don't overwrite) ─────────────
    let gitignore_path = vault_root.join(".gitignore");
    let needed_entries = [
        ".obsidian/workspace.json",
        ".obsidian/graph.json",
        ".trash/",
        ".DS_Store",
        ".env",
        "logs/",
        "temp_conversions/",
        "target/",
    ];

    if gitignore_path.exists() {
        let existing = fs::read_to_string(&gitignore_path).unwrap_or_default();
        let mut appended = Vec::new();
        for entry in &needed_entries {
            if !existing.contains(entry) {
                appended.push(*entry);
            }
        }
        if !appended.is_empty() {
            let mut content = existing;
            if !content.ends_with('\n') {
                content.push('\n');
            }
            content.push_str("# Added by obsidian-forge\n");
            for entry in &appended {
                content.push_str(entry);
                content.push('\n');
            }
            fs::write(&gitignore_path, content)?;
            created.push(format!(".gitignore (appended {} entries)", appended.len()));
        } else {
            skipped.push(".gitignore (all entries present)".into());
        }
    } else {
        let content = needed_entries.join("\n") + "\n";
        fs::write(&gitignore_path, content)?;
        created.push(".gitignore".into());
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn write_if_missing(
    vault_root: &Path,
    filename: &str,
    content: &str,
    created: &mut Vec<String>,
    skipped: &mut Vec<String>,
) -> Result<()> {
    let p = vault_root.join(filename);
    if p.exists() {
        skipped.push(filename.into());
    } else {
        fs::write(&p, content)?;
        created.push(filename.into());
    }
    Ok(())
}

/// Ensure `~/.config/obsidian-forge/templates/` exists and contains the bundled defaults.
/// Then create `vault/obsidian-templates` as a symlink pointing there.
/// If `vault/obsidian-templates` already exists as a real directory, leave it untouched.
fn link_global_templates(
    vault_root: &Path,
    created: &mut Vec<String>,
    skipped: &mut Vec<String>,
) -> Result<()> {
    let global_tpl = GlobalConfig::templates_dir();
    fs::create_dir_all(&global_tpl)?;

    // Write any missing bundled templates into the global store.
    let templates: &[(&str, &str)] = &[
        ("Daily-Note.md", include_str!("templates/Daily-Note.md")),
        ("ZK-Note.md", include_str!("templates/ZK-Note.md")),
        ("Project-Note.md", include_str!("templates/Project-Note.md")),
        (
            "Quick-Capture.md",
            include_str!("templates/Quick-Capture.md"),
        ),
        ("MOC.md", include_str!("templates/MOC.md")),
        ("Meeting-Note.md", include_str!("templates/Meeting-Note.md")),
        ("Book-Note.md", include_str!("templates/Book-Note.md")),
        ("Code-Snippet.md", include_str!("templates/Code-Snippet.md")),
        (
            "Weekly-Review.md",
            include_str!("templates/Weekly-Review.md"),
        ),
        ("Goal.md", include_str!("templates/Goal.md")),
        (
            "Habit-Tracker.md",
            include_str!("templates/Habit-Tracker.md"),
        ),
        (
            "Monthly-Retrospective.md",
            include_str!("templates/Monthly-Retrospective.md"),
        ),
    ];
    for (name, content) in templates {
        let p = global_tpl.join(name);
        if !p.exists() {
            fs::write(&p, content)?;
        }
    }

    let link_path = vault_root.join("obsidian-templates");

    // Already a symlink — check if it's dangling and recreate if needed.
    if link_path.is_symlink() {
        if link_path.exists() {
            skipped.push(format!(
                "obsidian-templates → {} (symlink already set)",
                global_tpl.display()
            ));
            return Ok(());
        }
        // Dangling symlink — remove and recreate.
        fs::remove_file(&link_path)?;
        symlink(&global_tpl, &link_path)?;
        created.push(format!(
            "obsidian-templates → {} (dangling symlink replaced)",
            global_tpl.display()
        ));
        return Ok(());
    }

    // Real directory exists (e.g. vault created before this feature) — leave it alone.
    if link_path.is_dir() {
        skipped.push("obsidian-templates/ (real directory kept; not converted to symlink)".into());
        return Ok(());
    }

    symlink(&global_tpl, &link_path)?;
    created.push(format!(
        "obsidian-templates → {} (symlink)",
        global_tpl.display()
    ));

    Ok(())
}

/// Apply global settings from `~/.config/obsidian-forge/` to a vault's `.obsidian/`.
pub fn apply_global_settings(target: &Path) -> Result<()> {
    let store = GlobalConfig::settings_dir();
    if !GlobalConfig::has_settings() {
        println!("  No global settings found. Use `obsidian-forge settings import <vault>` first.");
        return Ok(());
    }

    let tgt_obs = target.join(".obsidian");
    fs::create_dir_all(&tgt_obs)?;

    copy_settings(&store, &tgt_obs, "global store")?;
    println!("✅ Global settings applied → {}", target.display());
    Ok(())
}

/// Import .obsidian/ settings from a vault into the global settings store.
pub fn import_settings(source: &Path) -> Result<()> {
    let src_obs = source.join(".obsidian");
    if !src_obs.exists() {
        anyhow::bail!("Source has no .obsidian/ directory: {}", source.display());
    }

    let store = GlobalConfig::settings_dir();
    fs::create_dir_all(&store)?;

    copy_settings(&src_obs, &store, &source.display().to_string())?;
    println!(
        "✅ Settings imported from {} → global store",
        source.display()
    );
    Ok(())
}

/// Push global settings to a vault's .obsidian/.
pub fn push_settings(target: &Path) -> Result<()> {
    let store = GlobalConfig::settings_dir();
    if !GlobalConfig::has_settings() {
        anyhow::bail!("Global settings store is empty. Run `settings import` first.");
    }

    let tgt_obs = target.join(".obsidian");
    fs::create_dir_all(&tgt_obs)?;

    copy_settings(&store, &tgt_obs, "global store")?;
    println!("✅ Global settings pushed → {}", target.display());
    Ok(())
}

/// Clone .obsidian/ settings from one vault to another (vault-to-vault).
pub fn clone_obsidian_settings(source: &Path, target: &Path) -> Result<()> {
    let src_obs = source.join(".obsidian");
    let tgt_obs = target.join(".obsidian");

    if !src_obs.exists() {
        anyhow::bail!(
            "Source vault has no .obsidian/ directory: {}",
            source.display()
        );
    }
    fs::create_dir_all(&tgt_obs)?;

    copy_settings(&src_obs, &tgt_obs, &source.display().to_string())?;
    println!(
        "✅ Settings cloned from {} → {}",
        source.display(),
        target.display()
    );
    Ok(())
}

/// Copy settings directories and files from src to dst.
fn copy_settings(src: &Path, dst: &Path, _label: &str) -> Result<()> {
    for dir in SETTINGS_DIRS {
        let src_dir = src.join(dir);
        if src_dir.is_dir() {
            copy_dir_recursive(&src_dir, &dst.join(dir))?;
            println!("  Copied: {}/", dir);
        }
    }
    for file in SETTINGS_FILES {
        let src_file = src.join(file);
        if src_file.is_file() {
            fs::copy(&src_file, dst.join(file))?;
            println!("  Copied: {}", file);
        }
    }
    Ok(())
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        if src_path.is_dir() {
            copy_dir_recursive(&src_path, &dst_path)?;
        } else {
            fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

fn register_and_print(vault_root: &Path, name: &str) {
    let mut global = GlobalConfig::load().unwrap_or_default();
    global.add_vault(name, &vault_root.to_string_lossy());
    if let Err(e) = global.save() {
        warn!("Failed to update global config: {:?}", e);
    }

    println!("✅ Vault ready: {}", vault_root.display());
    println!("✅ Registered in global config as '{}'", name);
    println!(
        "   Open in Obsidian: File → Open Vault → {}",
        vault_root.display()
    );
    println!("   Start daemon:     obsidian-forge watch");
}
