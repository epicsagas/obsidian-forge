use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tracing::{debug, info};

/// Per-vault config file.
pub const CONFIG_FILE: &str = "vault.toml";
/// Global config directory.
pub const GLOBAL_DIR: &str = ".obsidian-forge";
/// Global config file name.
pub const GLOBAL_FILE: &str = "config.toml";
/// Subdirectories and files managed in the global settings store.
pub const SETTINGS_DIRS: &[&str] = &["plugins", "snippets", "themes"];
pub const SETTINGS_FILES: &[&str] = &[
    "appearance.json",
    "community-plugins.json",
    "hotkeys.json",
    "app.json",
];

/// Prepended to `config.toml` on every save so users can read purpose of each table without docs.
const GLOBAL_CONFIG_PREAMBLE: &str = r#"# obsidian-forge — global configuration
# Path: ~/.config/obsidian-forge/config.toml
#
# ── Global-only settings (do NOT override in vault.toml) ─────────────────
#   [ai]        Shared AI backend: provider, model, base_url, api_key, max_concurrent.
#               Individual vaults may override `model` and `max_concurrent` only.
#   [daemon]    Single watch/sync process for all vaults: label, log_dir, interval_seconds.
#               Not meaningful per-vault — always managed here.
#
# ── Shared defaults (vault.toml can override per-vault) ───────────────────
#   [projects]  Default exclude list for graph/link scanning.
#   [sync]      Default git auto commit/push and sync interval.
#
# ── Vault registry ────────────────────────────────────────────────────────
#   [[vaults]]  Registered vaults with name, path, enabled, watch flags.
#               Managed automatically by `obsidian-forge init`.
#
# ── [ai] optional keys (omitted from generated TOML when unset) ───────────
# base_url = "https://api.openai.com/v1"       # openai
# base_url = "https://openrouter.ai/api/v1"    # openrouter
# base_url = "http://localhost:1234/v1"        # lmstudio
# base_url = "http://localhost:11434/v1"       # openai-compatible
# (ollama: base_url unused, uses local CLI)
#
# ── API key resolution (priority: config < env var < .env file) ───────────
#   Prefer ~/.config/obsidian-forge/.env over setting api_key here:
#
#     provider="openai"            → OPENAI_API_KEY
#     provider="openrouter"        → OPENROUTER_API_KEY
#     provider="openai-compatible" → OPENAI_COMPATIBLE_API_KEY  (fallback: OPENAI_API_KEY)
#     provider="ollama"/"lmstudio" → (no key needed)
#
"#;

// ---------------------------------------------------------------------------
// Global config (multi-vault)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalConfig {
    #[serde(default)]
    pub vaults: Vec<VaultEntry>,
    /// Shared project detection defaults (vault.toml `[projects]` overrides when present).
    #[serde(default)]
    pub projects: Option<ProjectsConfig>,
    #[serde(default)]
    pub sync: Option<SyncConfig>,
    #[serde(default)]
    pub ai: Option<AiConfig>,
    #[serde(default)]
    pub daemon: Option<DaemonConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultEntry {
    pub name: String,
    pub path: String,
    #[serde(default = "yes")]
    pub enabled: bool,
    #[serde(default = "yes")]
    pub watch: bool,
}

impl GlobalConfig {
    /// Path to `~/.config/obsidian-forge/config.toml`.
    pub fn path() -> PathBuf {
        dirs_home().join(GLOBAL_DIR).join(GLOBAL_FILE)
    }

    /// Path to the global settings store: `~/.config/obsidian-forge/`
    pub fn settings_dir() -> PathBuf {
        dirs_home().join(GLOBAL_DIR)
    }

    /// Path to the global templates store: `~/.obsidian-forge/templates/`
    pub fn templates_dir() -> PathBuf {
        dirs_home().join(GLOBAL_DIR).join("templates")
    }

    /// Returns true if global settings store has any content (plugins/, snippets/, etc.)
    pub fn has_settings() -> bool {
        let base = Self::settings_dir();
        SETTINGS_DIRS.iter().any(|d| base.join(d).is_dir())
            || SETTINGS_FILES.iter().any(|f| base.join(f).is_file())
    }

    pub fn load() -> Result<Self> {
        let p = Self::path();
        if !p.exists() {
            return Ok(Self {
                vaults: Vec::new(),
                projects: None,
                sync: None,
                ai: None,
                daemon: None,
            });
        }
        let text = fs::read_to_string(&p)?;
        Ok(toml::from_str(&text)?)
    }

    pub fn save(&self) -> Result<()> {
        let p = Self::path();
        let parent = p
            .parent()
            .ok_or_else(|| anyhow::anyhow!("invalid config path: {}", p.display()))?;
        fs::create_dir_all(parent)?;
        let body = toml::to_string_pretty(self)?;
        let mut out = String::with_capacity(GLOBAL_CONFIG_PREAMBLE.len() + body.len() + 4);
        out.push_str(GLOBAL_CONFIG_PREAMBLE);
        out.push('\n');
        out.push_str(&body);
        fs::write(&p, out)?;
        debug!("Global config saved to {}", p.display());
        Ok(())
    }

    pub fn add_vault(&mut self, name: &str, path: &str) {
        // Remove existing entry with same name or path
        // Remove existing entry with same name OR same path
        self.vaults.retain(|v| v.name != name && v.path != path);
        self.vaults.push(VaultEntry {
            name: name.to_string(),
            path: path.to_string(),
            enabled: true,
            watch: true,
        });
    }

    pub fn find_vault(&self, name: &str) -> Option<&VaultEntry> {
        self.vaults.iter().find(|v| v.name == name)
    }

    /// Return all vaults that should be watched by the daemon.
    pub fn watchable_vaults(&self) -> Vec<&VaultEntry> {
        self.vaults
            .iter()
            .filter(|v| v.enabled && v.watch)
            .collect()
    }

    /// Return all enabled vaults (for sync).
    pub fn enabled_vaults(&self) -> Vec<&VaultEntry> {
        self.vaults.iter().filter(|v| v.enabled).collect()
    }

    /// Fills shared sections (`projects`, `sync`, `ai`, `daemon`) when absent (`None`).
    /// Called after `init` so the global file lists every knob with defaults.
    ///
    /// Returns `true` if any section was added (global file will gain new keys on save).
    pub fn seed_missing_tooling_sections(&mut self) -> bool {
        let mut added = false;
        if self.projects.is_none() {
            self.projects = Some(ProjectsConfig::default());
            added = true;
        }
        if self.sync.is_none() {
            self.sync = Some(SyncConfig::default());
            added = true;
        }
        if self.ai.is_none() {
            self.ai = Some(AiConfig::default());
            added = true;
        }
        if self.daemon.is_none() {
            self.daemon = Some(DaemonConfig::default());
            added = true;
        }
        added
    }
}

fn dirs_home() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("~"))
}

// ---------------------------------------------------------------------------
// Per-vault config
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ForgeConfig {
    pub vault: VaultConfig,
    #[serde(default)]
    pub projects: ProjectsConfig,
    #[serde(default)]
    pub sync: SyncConfig,
    #[serde(default)]
    pub ai: AiConfig,
    #[serde(default)]
    pub daemon: DaemonConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VaultConfig {
    pub name: String,
    #[serde(default = "default_layout")]
    pub layout: String,
    #[serde(default = "default_inbox")]
    pub inbox_dir: String,
    #[serde(default = "default_zettelkasten")]
    pub zettelkasten_dir: String,
    #[serde(default = "default_archive")]
    pub archive_dir: String,
    #[serde(default = "default_attachments")]
    pub attachments_dir: String,
    #[serde(default = "default_templates")]
    pub templates_dir: String,
    #[serde(default)]
    pub system_dirs: Vec<String>,
    /// Static, hand-maintained files that sync/regeneration must never overwrite
    /// once they exist. `index.md` is the agent entry point and is protected by
    /// default (see AGENTS.md). Generation on first run (file absent) is allowed.
    #[serde(default = "default_protected_files")]
    pub protected_files: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ProjectsConfig {
    /// Reserved for future detection modes; **currently ignored**. MOC/graph scan only uses
    /// top-level directories under the vault root (same as `top-level-dirs`).
    #[serde(default = "default_detect")]
    pub detect: String,
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SyncConfig {
    #[serde(default)]
    pub git_auto_commit: bool,
    #[serde(default)]
    pub git_auto_push: bool,
    #[serde(default = "default_interval_option")]
    pub interval_minutes: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AiConfig {
    /// Provider: "ollama" | "openai" | "openrouter" | "lmstudio" | "openai-compatible"
    #[serde(default = "default_provider")]
    pub provider: String,
    #[serde(default = "default_model")]
    pub model: String,
    /// Base URL override (required for lmstudio/openai-compatible, optional for openai/openrouter)
    #[serde(default)]
    pub base_url: Option<String>,
    /// API key (required for openai/openrouter, ignored for ollama/lmstudio)
    #[serde(default)]
    pub api_key: Option<String>,
    /// Maximum concurrent AI requests (for parallel processing)
    #[serde(default = "default_max_concurrent")]
    pub max_concurrent: Option<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    #[serde(default = "default_label")]
    pub label: String,
    #[serde(default = "default_log_dir")]
    pub log_dir: String,
    /// Watch/sync interval in seconds (default: 300 = 5 minutes)
    #[serde(default = "default_interval_seconds_option")]
    pub interval_seconds: Option<u64>,
}

// ---------------------------------------------------------------------------
// Defaults
// ---------------------------------------------------------------------------

fn default_layout() -> String {
    "para".into()
}
fn default_inbox() -> String {
    "00-Inbox".into()
}
fn default_zettelkasten() -> String {
    "10-Zettelkasten".into()
}
fn default_archive() -> String {
    "99-Archives".into()
}
fn default_attachments() -> String {
    "Attachments".into()
}
fn default_templates() -> String {
    "obsidian-templates".into()
}
fn default_protected_files() -> Vec<String> {
    vec!["index.md".to_string()]
}
fn default_detect() -> String {
    "top-level-dirs".into()
}
fn default_interval_option() -> Option<u64> {
    Some(60) // 60 minutes
}
fn default_interval_seconds_option() -> Option<u64> {
    Some(3600) // 60 minutes in seconds
}
fn default_provider() -> String {
    "ollama".into()
}
fn default_model() -> String {
    "gemma3".into()
}
fn default_max_concurrent() -> Option<usize> {
    Some(5)
}
fn default_label() -> String {
    "com.obsidian-forge.watch".into()
}
fn default_log_dir() -> String {
    "~/.obsidian-forge/logs".into()
}
fn yes() -> bool {
    true
}

impl Default for ProjectsConfig {
    fn default() -> Self {
        Self {
            detect: default_detect(),
            exclude: vec!["_template".into()],
        }
    }
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            provider: default_provider(),
            model: default_model(),
            base_url: None,
            api_key: None,
            max_concurrent: default_max_concurrent(),
        }
    }
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            label: default_label(),
            log_dir: default_log_dir(),
            interval_seconds: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

/// Apply `~/.config/obsidian-forge/config.toml` onto a vault-loaded config.
fn merge_global_into_forge(config: &mut ForgeConfig, global: &GlobalConfig) {
    if let Some(ref gp) = global.projects
        && config.projects == ProjectsConfig::default()
    {
        config.projects = gp.clone();
    }
    if let Some(ref global_sync) = global.sync {
        config.sync = SyncConfig {
            git_auto_commit: config.sync.git_auto_commit,
            git_auto_push: config.sync.git_auto_push,
            interval_minutes: config
                .sync
                .interval_minutes
                .or(global_sync.interval_minutes),
        };
    }
    if let Some(ref global_ai) = global.ai {
        config.ai = AiConfig {
            provider: if config.ai.provider == default_provider()
                && global_ai.provider != default_provider()
            {
                global_ai.provider.clone()
            } else {
                config.ai.provider.clone()
            },
            model: if config.ai.model == default_model() && global_ai.model != default_model() {
                global_ai.model.clone()
            } else {
                config.ai.model.clone()
            },
            base_url: config.ai.base_url.clone().or(global_ai.base_url.clone()),
            api_key: config.ai.api_key.clone().or(global_ai.api_key.clone()),
            max_concurrent: config.ai.max_concurrent.or(global_ai.max_concurrent),
        };
    }
    if let Some(ref global_daemon) = global.daemon {
        config.daemon = DaemonConfig {
            label: if config.daemon.label == default_label()
                && global_daemon.label != default_label()
            {
                global_daemon.label.clone()
            } else {
                config.daemon.label.clone()
            },
            log_dir: if config.daemon.log_dir == default_log_dir()
                && global_daemon.log_dir != default_log_dir()
            {
                global_daemon.log_dir.clone()
            } else {
                config.daemon.log_dir.clone()
            },
            interval_seconds: config
                .daemon
                .interval_seconds
                .or(global_daemon.interval_seconds),
        };
    }
}

impl ForgeConfig {
    /// Load config from `vault.toml` in the given vault root.
    /// Merges with global config: vault.toml values override global defaults.
    pub fn load(vault_root: &Path) -> Result<Self> {
        let path = vault_root.join(CONFIG_FILE);
        if !path.exists() {
            bail!(
                "No {} found in {}. Run `obsidian-forge init` first.",
                CONFIG_FILE,
                vault_root.display()
            );
        }
        let text = fs::read_to_string(&path)?;
        let mut config: ForgeConfig = toml::from_str(&text)?;

        if let Ok(global) = GlobalConfig::load() {
            merge_global_into_forge(&mut config, &global);
        }

        info!("Loaded config from {}", path.display());
        Ok(config)
    }

    /// Save config to `vault.toml`.
    #[allow(dead_code)] // Public API; CLI uses `default_vault_toml_template` for new files
    pub fn save(&self, vault_root: &Path) -> Result<()> {
        let path = vault_root.join(CONFIG_FILE);
        let text = toml::to_string_pretty(self)?;
        fs::write(&path, text)?;
        debug!("Config saved to {}", path.display());
        Ok(())
    }

    /// Collect all directory names that should be skipped when scanning projects.
    pub fn all_system_dirs(&self) -> Vec<String> {
        let mut dirs = vec![
            self.vault.inbox_dir.clone(),
            self.vault.zettelkasten_dir.clone(),
            self.vault.archive_dir.clone(),
            self.vault.attachments_dir.clone(),
            self.vault.templates_dir.clone(),
            "01-Projects".into(),
            "02-Areas".into(),
            "03-Resources".into(),
            "Chats".into(),
            "Clippings".into(),
            "temp_conversions".into(),
            "logs".into(),
            "target".into(),
            ".obsidian".into(),
            ".git".into(),
            ".alcove".into(),
            ".claude".into(),
        ];
        dirs.extend(self.vault.system_dirs.clone());
        dirs
    }

    /// In-memory defaults matching an active-only `vault.toml` (see `default_vault_toml_template`).
    #[allow(dead_code)] // Exercised by unit tests
    pub fn default_for(name: &str) -> Self {
        Self {
            vault: VaultConfig {
                name: name.to_string(),
                layout: default_layout(),
                inbox_dir: default_inbox(),
                zettelkasten_dir: default_zettelkasten(),
                archive_dir: default_archive(),
                attachments_dir: default_attachments(),
                templates_dir: default_templates(),
                system_dirs: Vec::new(),
                protected_files: default_protected_files(),
            },
            projects: ProjectsConfig::default(),
            // sync, ai, daemon omitted - use global config defaults
            sync: SyncConfig::default(),
            ai: AiConfig::default(),
            daemon: DaemonConfig::default(),
        }
    }
}

/// Initial `vault.toml` for `init`. Only `[vault]` is active.
///
/// Shared defaults (`projects`, `sync`) and global-only settings (`ai`, `daemon`) live in
/// `~/.config/obsidian-forge/config.toml`. Uncomment a section below only to override for this vault.
pub fn default_vault_toml_template(vault_name: &str) -> String {
    format!(
        r#"# vault.toml — settings for THIS vault only
#
# ── Always vault-specific (edit freely) ──────────────────────────────────
#   [vault]     Display name and folder layout (inbox, PARA paths, templates, …).
#
# ── Per-vault overrides (uncomment to differ from global config.toml) ─────
#   [projects]  Exclude specific top-level dirs from graph/link scanning.
#               (node_modules and dot-dirs are always excluded at any depth.)
#   [sync]      Git auto commit/push and sync interval for this vault.
#   [ai]        Override model or concurrency for AI operations in this vault.
#               (provider / base_url / api_key are global infrastructure — set in config.toml)
#
# ── Global-only (manage in ~/.config/obsidian-forge/config.toml) ──────────
#   [daemon]    Single watch/sync process for all vaults; not meaningful per-vault.
#   [ai]        provider, base_url, api_key — shared AI backend.
#

[vault]
name = "{vault_name}"
layout = "para"
inbox_dir = "00-Inbox"
zettelkasten_dir = "10-Zettelkasten"
archive_dir = "99-Archives"
attachments_dir = "Attachments"
templates_dir = "obsidian-templates"
system_dirs = []
protected_files = ["index.md"]  # sync must not overwrite static files; set to [] to auto-regenerate index.md

# ── Per-vault overrides ───────────────────────────────────────────────────

# [projects]
# detect = "top-level-dirs"   # reserved for future use
# exclude = ["_template"]

# [sync]
# git_auto_commit = false
# git_auto_push = false
# interval_minutes = 60

# [ai]
# model = "gemma3"        # override AI model for this vault only
# max_concurrent = 5      # override concurrency limit for this vault only
"#,
        vault_name = vault_name,
    )
}

/// Resolve the vault root from an optional path argument.
/// Walks up from CWD looking for `vault.toml` or `00-Inbox`.
pub fn resolve_vault(input: Option<String>) -> Result<PathBuf> {
    if let Some(p) = input {
        return canonicalize(PathBuf::from(p));
    }
    if let Ok(env_p) = std::env::var("VAULT_PATH") {
        return canonicalize(PathBuf::from(env_p));
    }
    let mut cur = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    for _ in 0..6 {
        if cur.join(CONFIG_FILE).is_file() || cur.join("00-Inbox").is_dir() {
            return canonicalize(cur);
        }
        if !cur.pop() {
            break;
        }
    }
    bail!(
        "No vault root detected. Set VAULT_PATH, run from within a vault, \
         or use `obsidian-forge init <name>` to create one."
    );
}

fn canonicalize(p: PathBuf) -> Result<PathBuf> {
    Ok(fs::canonicalize(&p).unwrap_or(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_vault_toml_template_parses() {
        let s = default_vault_toml_template("my-vault");
        let cfg: ForgeConfig = toml::from_str(&s).expect("template TOML");
        assert_eq!(cfg.vault.name, "my-vault");
        assert_eq!(cfg.projects.detect, ProjectsConfig::default().detect);
    }

    #[test]
    fn test_forge_config_default_for() {
        let cfg = ForgeConfig::default_for("my-vault");
        assert_eq!(cfg.vault.name, "my-vault");
        assert_eq!(cfg.vault.inbox_dir, "00-Inbox");
        assert_eq!(cfg.vault.zettelkasten_dir, "10-Zettelkasten");
        assert_eq!(cfg.vault.archive_dir, "99-Archives");
        assert_eq!(cfg.vault.attachments_dir, "Attachments");
        assert_eq!(cfg.vault.templates_dir, "obsidian-templates");
        assert_eq!(cfg.vault.layout, "para");
    }

    #[test]
    fn test_ai_config_defaults() {
        let cfg = AiConfig::default();
        assert_eq!(cfg.provider, "ollama");
        assert_eq!(cfg.model, "gemma3");
        assert!(cfg.api_key.is_none());
        assert!(cfg.base_url.is_none());
        assert_eq!(cfg.max_concurrent, Some(5));
    }

    #[test]
    fn test_sync_config_default_interval() {
        // Rust's Default derive gives None for Option fields;
        // the Some(60) default only applies during serde deserialization.
        let cfg = SyncConfig::default();
        assert!(!cfg.git_auto_commit);
        assert!(!cfg.git_auto_push);
        assert_eq!(cfg.interval_minutes, None);
    }

    #[test]
    fn test_global_config_add_vault_replaces_duplicate() {
        let mut global = GlobalConfig::default();
        global.add_vault("test", "/tmp/test");
        assert_eq!(global.vaults.len(), 1);
        assert_eq!(global.vaults[0].name, "test");

        // Add duplicate name should replace
        global.add_vault("test", "/tmp/test2");
        assert_eq!(global.vaults.len(), 1);
        assert_eq!(global.vaults[0].path, "/tmp/test2");
    }

    #[test]
    fn test_global_config_watchable_vaults() {
        let mut global = GlobalConfig::default();
        global.add_vault("active", "/tmp/active");
        global.add_vault("paused", "/tmp/paused");
        global.vaults[1].watch = false;
        let watchable = global.watchable_vaults();
        assert_eq!(watchable.len(), 1);
        assert_eq!(watchable[0].name, "active");
    }

    #[test]
    fn test_all_system_dirs_includes_defaults() {
        let cfg = ForgeConfig::default_for("vault");
        let dirs = cfg.all_system_dirs();
        assert!(dirs.contains(&"00-Inbox".to_string()));
        assert!(dirs.contains(&"10-Zettelkasten".to_string()));
        assert!(dirs.contains(&".git".to_string()));
        assert!(dirs.contains(&".obsidian".to_string()));
    }

    #[test]
    fn test_vault_protected_files_default_applied_on_parse() {
        // Existing vault.toml files have no `protected_files` key; serde must apply the
        // default so index.md is protected without users having to opt in.
        let toml_v = r#"
[vault]
name = "v"
layout = "para"
inbox_dir = "00-Inbox"
zettelkasten_dir = "10-Zettelkasten"
archive_dir = "99-Archives"
attachments_dir = "Attachments"
templates_dir = "obsidian-templates"
system_dirs = []
"#;
        let config: ForgeConfig = toml::from_str(toml_v).expect("parse");
        assert_eq!(
            config.vault.protected_files,
            vec!["index.md".to_string()],
            "missing protected_files key should default to [\"index.md\"]"
        );
    }

    #[test]
    fn test_global_toml_parses_with_preamble() {
        let body = r#"[[vaults]]
name = "v"
path = "/tmp/v"

[sync]
git_auto_commit = false
git_auto_push = false
"#;
        let full = format!("{GLOBAL_CONFIG_PREAMBLE}\n{body}");
        let g: GlobalConfig = toml::from_str(&full).expect("parse with preamble");
        assert_eq!(g.vaults.len(), 1);
        assert_eq!(g.vaults[0].name, "v");
    }

    #[test]
    fn test_merge_global_replaces_default_projects() {
        let mut config: ForgeConfig =
            toml::from_str(&default_vault_toml_template("v")).expect("vault template");
        let global = GlobalConfig {
            projects: Some(ProjectsConfig {
                detect: "from-global".into(),
                exclude: vec!["only-global".into()],
            }),
            ..GlobalConfig::default()
        };
        merge_global_into_forge(&mut config, &global);
        assert_eq!(config.projects.detect, "from-global");
        assert_eq!(config.projects.exclude, vec!["only-global".to_string()]);
    }

    #[test]
    fn test_vault_toml_with_legacy_graph_block_still_parses() {
        // Existing vault.toml files may still carry the deprecated [graph] block;
        // unknown keys must be ignored, not rejected.
        let toml_v = r#"
[vault]
name = "v"
layout = "para"
inbox_dir = "00-Inbox"
zettelkasten_dir = "10-Zettelkasten"
archive_dir = "99-Archives"
attachments_dir = "Attachments"
templates_dir = "obsidian-templates"
system_dirs = []

[graph]
backlinks = false
concepts = []
"#;
        let config: ForgeConfig = toml::from_str(toml_v).expect("parse with legacy [graph]");
        assert_eq!(config.vault.name, "v");
    }
}
