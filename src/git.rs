use anyhow::Result;
use std::path::Path;
use std::process::Command;
use tracing::{debug, info, warn};

/// Check for uncommitted changes, commit with a conventional message, and push.
pub fn auto_commit_and_push(vault_root: &Path, do_push: bool) -> Result<bool> {
    let status = Command::new("git")
        .args(["status", "--porcelain"])
        .current_dir(vault_root)
        .output()?;
    let status_str = String::from_utf8_lossy(&status.stdout);

    if status_str.trim().is_empty() {
        debug!("No changes to commit");
        return Ok(false);
    }

    let lines: Vec<&str> = status_str.lines().collect();
    let message = build_commit_message(&lines);

    info!("Auto-committing {} changed files: {}", lines.len(), message);

    let add = Command::new("git")
        .args(["add", "-A"])
        .current_dir(vault_root)
        .output()?;
    if !add.status.success() {
        warn!("git add failed: {}", String::from_utf8_lossy(&add.stderr));
        return Ok(false);
    }

    let commit = Command::new("git")
        .args(["commit", "-m", &message])
        .current_dir(vault_root)
        .output()?;
    if !commit.status.success() {
        let stderr = String::from_utf8_lossy(&commit.stderr);
        if stderr.contains("nothing to commit") {
            return Ok(false);
        }
        warn!("git commit failed: {}", stderr);
        return Ok(false);
    }

    info!("Committed: {}", message);

    if do_push {
        let push = Command::new("git")
            .args(["push"])
            .current_dir(vault_root)
            .output()?;
        if !push.status.success() {
            warn!("git push failed: {}", String::from_utf8_lossy(&push.stderr));
        } else {
            info!("Pushed to remote");
        }
    }

    Ok(true)
}

pub(crate) fn build_commit_message(changes: &[&str]) -> String {
    let mut has_moc = false;
    let mut has_zettel = false;
    let mut has_src = false;

    for line in changes {
        let file = line.get(3..).unwrap_or("").trim();
        if file.contains("/src/") || file.ends_with(".rs") {
            has_src = true;
        }
        if file.contains('/') {
            let parts: Vec<&str> = file.split('/').collect();
            if parts.len() >= 2 && file.ends_with(&format!("{}.md", parts[0])) {
                has_moc = true;
            }
        }
        if file.contains("Zettelkasten") || file.contains("zettelkasten") {
            has_zettel = true;
        }
    }

    let mut parts = Vec::new();
    if has_moc {
        parts.push("MOC");
    }
    if has_zettel {
        parts.push("bridge notes");
    }
    if has_src {
        parts.push("source");
    }
    if parts.is_empty() {
        parts.push("vault content");
    }

    format!(
        "chore(vault): auto-update {} ({} files)",
        parts.join(", "),
        changes.len()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_commit_message_vault_content() {
        let changes = &["M  some-note.md", "A  another-note.md"];
        let msg = build_commit_message(changes);
        assert!(msg.contains("vault content"));
        assert!(msg.contains("2 files"));
        assert!(msg.starts_with("chore(vault):"));
    }

    #[test]
    fn test_commit_message_moc() {
        let changes = &["M  MyProject/MyProject.md"];
        let msg = build_commit_message(changes);
        assert!(msg.contains("MOC"));
    }

    #[test]
    fn test_commit_message_zettelkasten() {
        let changes = &["M  10-Zettelkasten/rust.md"];
        let msg = build_commit_message(changes);
        assert!(msg.contains("bridge notes"));
    }

    #[test]
    fn test_commit_message_combined() {
        let changes = &[
            "M  MyProject/MyProject.md",
            "M  10-Zettelkasten/rust.md",
            "A  some-note.md",
        ];
        let msg = build_commit_message(changes);
        assert!(msg.contains("3 files"));
    }

    #[test]
    fn test_commit_message_empty() {
        let changes: &[&str] = &[];
        let msg = build_commit_message(changes);
        assert!(msg.starts_with("chore(vault):"));
        assert!(msg.contains("0 files"));
    }
}
