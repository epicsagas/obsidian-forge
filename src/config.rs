use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tracing::{debug, info};

/// Per-vault config file.
pub const CONFIG_FILE: &str = "vault.toml";
/// Global config directory.
pub const GLOBAL_DIR: &str = ".config/obsidian-forge";
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

// ---------------------------------------------------------------------------
// Global config (multi-vault)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct GlobalConfig {
    #[serde(default)]
    pub vaults: Vec<VaultEntry>,
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

    /// Path to the global templates store: `~/.config/obsidian-forge/templates/`
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
            return Ok(Self::default());
        }
        let text = fs::read_to_string(&p)?;
        Ok(toml::from_str(&text)?)
    }

    pub fn save(&self) -> Result<()> {
        let p = Self::path();
        fs::create_dir_all(p.parent().unwrap())?;
        fs::write(&p, toml::to_string_pretty(self)?)?;
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

    pub fn remove_vault(&mut self, name: &str) -> bool {
        let before = self.vaults.len();
        self.vaults.retain(|v| v.name != name);
        self.vaults.len() < before
    }

    pub fn find_vault(&self, name: &str) -> Option<&VaultEntry> {
        self.vaults.iter().find(|v| v.name == name)
    }

    pub fn find_vault_mut(&mut self, name: &str) -> Option<&mut VaultEntry> {
        self.vaults.iter_mut().find(|v| v.name == name)
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
    pub graph: GraphConfig,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectsConfig {
    #[serde(default = "default_detect")]
    pub detect: String,
    #[serde(default)]
    pub exclude: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphConfig {
    #[serde(default = "yes")]
    pub backlinks: bool,
    #[serde(default = "yes")]
    pub bridge_notes: bool,
    #[serde(default = "yes")]
    pub auto_tags: bool,
    #[serde(default = "yes")]
    pub related_projects: bool,
    #[serde(default)]
    pub concepts: Vec<ConceptDef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConceptDef {
    pub name: String,
    pub keywords: Vec<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SyncConfig {
    #[serde(default)]
    pub git_auto_commit: bool,
    #[serde(default)]
    pub git_auto_push: bool,
    #[serde(default = "default_interval")]
    pub interval_minutes: u64,
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
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonConfig {
    #[serde(default = "default_label")]
    pub label: String,
    #[serde(default = "default_log_dir")]
    pub log_dir: String,
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
fn default_detect() -> String {
    "top-level-dirs".into()
}
fn default_interval() -> u64 {
    5
}
fn default_provider() -> String {
    "ollama".into()
}
fn default_model() -> String {
    "gemma3".into()
}
fn default_label() -> String {
    "com.obsidian-forge.watch".into()
}
fn default_log_dir() -> String {
    "~/.config/obsidian-forge/logs".into()
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

impl Default for GraphConfig {
    fn default() -> Self {
        Self {
            backlinks: true,
            bridge_notes: true,
            auto_tags: true,
            related_projects: true,
            concepts: Vec::new(),
        }
    }
}

impl Default for SyncConfig {
    fn default() -> Self {
        Self {
            git_auto_commit: false,
            git_auto_push: false,
            interval_minutes: default_interval(),
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
        }
    }
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            label: default_label(),
            log_dir: default_log_dir(),
        }
    }
}

// ---------------------------------------------------------------------------
// Loading
// ---------------------------------------------------------------------------

impl ForgeConfig {
    /// Load config from `vault.toml` in the given vault root.
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
        let config: ForgeConfig = toml::from_str(&text)?;
        info!("Loaded config from {}", path.display());
        Ok(config)
    }

    /// Save config to `vault.toml`.
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

    /// Build the default config for `init`.
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
            },
            projects: ProjectsConfig::default(),
            graph: GraphConfig::default(),
            sync: SyncConfig {
                git_auto_commit: true,
                git_auto_push: true,
                interval_minutes: 5,
            },
            ai: AiConfig::default(),
            daemon: DaemonConfig::default(),
        }
    }
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
