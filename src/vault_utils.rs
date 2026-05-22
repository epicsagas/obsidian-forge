use regex::Regex;
use std::path::Path;
use std::sync::OnceLock;

pub const VAULT_EXCLUDED_DIRS: &[&str] = &[
    ".obsidian",
    ".git",
    ".claude",
    ".alcove",
    "_template",
    "seeded",
    "harness-engineering",
    "01-Projects",
];

pub fn is_vault_excluded(path: &Path) -> bool {
    for component in path.components() {
        if let std::path::Component::Normal(os_str) = component
            && let Some(name) = os_str.to_str()
            && VAULT_EXCLUDED_DIRS.contains(&name)
        {
            return true;
        }
    }
    false
}

pub fn frontmatter_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    // `(?s)` makes `.` match newlines. `.*?` is lazy so it stops at the FIRST `---\n`,
    // meaning body content containing `---` is safe. `(.*)` captures the rest.
    RE.get_or_init(|| Regex::new(r"(?s)^---\n(.*?)\n---\n(.*)$").expect("valid frontmatter regex"))
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
