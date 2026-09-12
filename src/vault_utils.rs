use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

/// Directory names excluded from every vault walk, matched per path
/// component at ANY depth (kg-overhaul-blueprint-2026-09 Phase 0: nested
/// junk like `01-Projects/foo/node_modules/` was previously indexed whole).
/// Any dot-prefixed component (`.venv`, `.windsurf`, …) is excluded by rule.
pub const VAULT_EXCLUDED_DIRS: &[&str] = &[
    "_template",
    "seeded",
    "harness-engineering",
    "node_modules",
    "01-Projects",
    // Public release bundles — standalone git repos nested in the vault
    // (e.g. 04-Writing/paper/*/release/). These ship to readers and must
    // never receive vault frontmatter, tags, or [[wikilinks]].
    "release",
];

pub fn is_vault_excluded(path: &Path, vault_root: &Path) -> bool {
    // Match components BELOW the vault root only, so a vault that happens to
    // live under a dot-dir or excluded name (e.g. ~/work/.env-mirror/vault)
    // is not excluded wholesale.
    let rel = path.strip_prefix(vault_root).unwrap_or(path);
    for component in rel.components() {
        if let std::path::Component::Normal(os_str) = component
            && let Some(name) = os_str.to_str()
            && (name.starts_with('.') || VAULT_EXCLUDED_DIRS.contains(&name))
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

/// Returns `true` if `path` **is** a directory (strictly below `vault_root`)
/// that contains its own `.git` — i.e. the top-level boundary of a nested,
/// independently-versioned repository (e.g. a public `release/` bundle, the #38
/// scenario) that the vault fixers must skip. The `.git` may be either a
/// directory (standalone repo) or a *file* (a `gitdir` pointer used by git
/// worktrees and submodules); `exists()` catches both, whereas an `is_dir()`
/// check would miss the file case.
///
/// This is intentionally an **O(1)** check on `path` itself, not an ancestor
/// walk. Every vault walker routes exclusion through `filter_entry`, which
/// prunes the *entire subtree* of any rejected directory. So when walkdir
/// reaches a nested repo's top directory we reject it here once, and its files
/// are never visited — no per-file ancestor probing needed. (`check_links`'s
/// `collect_md_files`/`collect_txt_stems` likewise use `filter_entry` for this
/// pruning to hold.)
///
/// The `path != vault_root` guard keeps the vault's *own* root `.git` from being
/// treated as nested (production vaults are themselves git repos). walkdir
/// builds every entry path from the exact `vault_root` passed to
/// `WalkDir::new`, so the root entry compares byte-equal and is never pruned.
fn is_inside_nested_repo(path: &Path, vault_root: &Path) -> bool {
    path.is_dir() && path != vault_root && path.join(".git").exists()
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nested_junk_excluded_at_any_depth() {
        let root = Path::new("/vault");

        // Nested junk dirs (blueprint Phase 0): excluded at any depth.
        for junk in [
            "01-Projects/app/node_modules/pkg/readme.md",
            "99-Archives/projects/proj/research/.venv/lib.py.md",
            "02-Areas/x/seeded/note.md",
            "03-Resources/y/release/bundle.md",
            "04-Writing/.windsurf/cache.md",
        ] {
            assert!(
                is_vault_excluded(&root.join(junk), root),
                "{} should be excluded",
                junk
            );
        }

        // Regular content is not.
        assert!(!is_vault_excluded(
            &root.join("99-Archives/projects/proj/PRD.md"),
            root
        ));
    }

    #[test]
    fn test_exclusion_scopes_to_vault_root() {
        // A vault that lives under a dot-dir or an excluded name must not be
        // excluded wholesale — only components below the vault root count.
        let root = Path::new("/work/.env-mirror/release/vault");
        assert!(!is_vault_excluded(&root.join("note.md"), root));
        // …but junk below the root still is.
        assert!(is_vault_excluded(&root.join(".trash/note.md"), root));
    }
}
