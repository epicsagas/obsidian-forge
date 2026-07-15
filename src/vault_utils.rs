use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

pub const VAULT_EXCLUDED_DIRS: &[&str] = &[
    ".obsidian",
    ".git",
    ".claude",
    ".alcove",
    ".obsidian-forge",
    "_template",
    "seeded",
    "harness-engineering",
    "01-Projects",
    // Public release bundles — standalone git repos nested in the vault
    // (e.g. 04-Writing/paper/*/release/). These ship to readers and must
    // never receive vault frontmatter, tags, or [[wikilinks]]. Matched per
    // path-component so any nested depth is excluded.
    "release",
];

pub fn is_vault_excluded(path: &Path, vault_root: &Path) -> bool {
    for component in path.components() {
        if let std::path::Component::Normal(os_str) = component
            && let Some(name) = os_str.to_str()
            && VAULT_EXCLUDED_DIRS.contains(&name)
        {
            return true;
        }
    }
    // A directory that embeds its own `.git` (a nested standalone repo,
    // e.g. a public `release/` bundle) is an exclusion boundary: files inside
    // it must never receive vault metadata. The vault's own root `.git` is
    // explicitly NOT treated as nested, so the surrounding vault stays scanned.
    is_inside_nested_repo(path, vault_root)
}

/// Returns `true` if `path` lies inside a directory (strictly below
/// `vault_root`) that contains its own `.git` — i.e. a nested,
/// independently-versioned repository that the vault fixers must skip.
fn is_inside_nested_repo(path: &Path, vault_root: &Path) -> bool {
    let mut current = if path.is_dir() {
        Some(path)
    } else {
        path.parent()
    };
    while let Some(dir) = current {
        // Stop at the vault root (its own `.git` is not a nested repo) or the
        // filesystem root (avoid probing above the vault).
        if dir == vault_root || dir.parent().is_none() {
            break;
        }
        if dir.join(".git").is_dir() {
            return true;
        }
        current = dir.parent();
    }
    false
}

pub fn frontmatter_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // `(?s)` makes `.` match newlines. `.*?` is lazy so it stops at the FIRST `---\n`,
    // meaning body content containing `---` is safe. `(.*)` captures the rest.
    RE.get_or_init(|| Regex::new(r"(?s)^---\n(.*?)\n---\n(.*)$").expect("valid frontmatter regex"))
}

/// Reassemble a markdown file from its YAML body and the document body.
///
/// Centralizes the `---\n...\n---\n` framing so the closing delimiter always
/// sits on its own line. The YAML captured by [`frontmatter_re`] excludes the
/// newline preceding the closing `---`, so `trim_end()` guards every writer
/// (regex-built, serde-built, or otherwise) against gluing `---` to the last key.
/// See issue #25 for the bug this consolidates.
pub fn reassemble_frontmatter(yaml: &str, body: &str) -> String {
    format!("---\n{}\n---\n{}", yaml.trim_end(), body)
}

pub fn doc_type_tag(filename: &str) -> Option<&'static str> {
    match filename {
        "PRD.md" => Some("type/prd"),
        "ARCHITECTURE.md" => Some("type/architecture"),
        "CONVENTIONS.md" => Some("type/convention"),
        "DECISIONS.md" => Some("type/decision"),
        "PROGRESS.md" => Some("type/progress"),
        "DEBT.md" => Some("type/debt"),
        "SECRETS_MAP.md" => Some("type/reference"),
        "CODE_INDEX.md" => Some("type/reference"),
        _ => None,
    }
}

pub fn supplementary_doc_type_tag(dir_name: &str) -> Option<&'static str> {
    match dir_name {
        "reports" => Some("type/report"),
        "specs" => Some("type/spec"),
        "plans" => Some("type/plan"),
        "research" => Some("type/research"),
        "strategy" => Some("type/strategy"),
        _ => None,
    }
}
