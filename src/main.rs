mod ai;
mod book;
mod check_links;
mod check_tags;
mod config;
mod converter;
mod frontmatter;
mod git;
mod graph;
mod index;
mod init;
mod moc;
mod notes;
mod prompts;
mod vault_utils;
mod watcher;

use anyhow::Result;
use clap::{Parser, Subcommand};
use std::{
    fs,
    path::{Path, PathBuf},
};
use tracing_subscriber::{EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

use config::{ForgeConfig, GlobalConfig, default_vault_toml_template};

#[derive(Parser)]
#[command(name = "obsidian-forge")]
#[command(about = "Obsidian vault generator, automation daemon, and graph strengthener")]
#[command(version)]
struct Cli {
    /// Vault root path (defaults to auto-detection)
    #[arg(long, env = "VAULT_PATH")]
    vault_path: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize an Obsidian vault (new or existing directory).
    /// Safe to re-run on existing vaults — only adds missing structure, never overwrites.
    /// Use this to repair or upgrade a vault after updating obsidian-forge.
    Init {
        /// Vault name (directory name to create or adopt)
        name: String,
        /// Parent directory (defaults to current directory)
        #[arg(long, default_value = ".")]
        path: String,
        /// Clone .obsidian/ settings (plugins, snippets, themes) from another vault
        #[arg(long)]
        clone_settings_from: Option<String>,
    },

    /// Clone .obsidian/ settings from one vault to another
    CloneSettings {
        /// Source vault name or path
        source: String,
        /// Target vault name or path
        target: String,
    },

    /// Manage global .obsidian/ settings store (~/.config/obsidian-forge/)
    Settings {
        #[command(subcommand)]
        action: SettingsAction,
    },

    /// Manage registered vaults
    Vault {
        #[command(subcommand)]
        action: VaultAction,
    },

    /// Process all existing notes in Inbox once
    ProcessAll {
        /// Specific vault name (from global config)
        #[arg(long)]
        vault: Option<String>,
    },

    /// Watch all registered vaults (daemon mode)
    Watch {
        /// Watch only this vault
        #[arg(long)]
        vault: Option<String>,
        /// Sync interval in seconds (overrides daemon.interval_seconds in config)
        #[arg(long)]
        interval: Option<u64>,
    },

    /// Rebuild all project hub files (MOCs)
    UpdateMocs {
        #[arg(long)]
        vault: Option<String>,
    },

    /// Strengthen Obsidian graph
    StrengthenGraph {
        #[arg(long)]
        vault: Option<String>,
    },

    /// Graph operations: health, orphans, extract, tags, strengthen
    Graph {
        #[command(subcommand)]
        action: GraphAction,
        /// Specific vault name (from global config)
        #[arg(long)]
        vault: Option<String>,
    },

    /// Check tag health for PRIMARY docs (missing layer, type, project tags)
    CheckTags {
        /// Auto-fix missing tags
        #[arg(long)]
        fix: bool,
        /// Scope: "project" (project docs only) or "vault" (includes Resources)
        #[arg(long, default_value = "vault")]
        scope: check_tags::TagScope,
        /// Specific vault name (from global config)
        #[arg(long)]
        vault: Option<String>,
    },

    /// Check for broken wikilinks and fix filename/extension mismatches
    CheckLinks {
        /// Auto-fix filename and extension mismatches
        #[arg(long)]
        fix: bool,
        /// Specific vault name (from global config)
        #[arg(long)]
        vault: Option<String>,
    },

    /// Normalize YAML frontmatter malformations
    NormalizeFrontmatter {
        /// Auto-fix detected malformations
        #[arg(long)]
        fix: bool,
        /// Specific vault name (from global config)
        #[arg(long)]
        vault: Option<String>,
    },

    /// Run full sync cycle: MOC → Graph → Git
    Sync {
        /// Sync only this vault (omit for all enabled vaults)
        #[arg(long)]
        vault: Option<String>,
    },

    /// Diagnose vault health (alias for status)
    Doctor {
        /// Specific vault name (from global config)
        #[arg(long)]
        vault: Option<String>,
        /// Skip AI connectivity test
        #[arg(long)]
        no_ping: bool,
    },

    /// Show vault and AI configuration status
    Status {
        /// Specific vault name (from global config)
        #[arg(long)]
        vault: Option<String>,
        /// Skip AI connectivity test
        #[arg(long)]
        no_ping: bool,
    },

    /// Manage the background daemon (macOS LaunchAgent)
    Daemon {
        #[command(subcommand)]
        action: DaemonAction,
    },

    /// Manage book writing projects within the vault
    Book {
        #[command(subcommand)]
        action: BookAction,
        #[arg(long)]
        vault: Option<String>,
    },
}

#[derive(Subcommand)]
enum VaultAction {
    /// Register an existing vault
    Add {
        /// Path to the vault directory
        path: String,
        /// Custom name (defaults to directory name)
        #[arg(long)]
        name: Option<String>,
    },
    /// Unregister a vault (files are kept)
    Remove { name: String },
    /// List all registered vaults
    List,
    /// Disable a vault (excluded from sync and watch)
    Disable { name: String },
    /// Re-enable a vault
    Enable { name: String },
    /// Pause daemon watching for a vault (sync still works manually)
    Pause { name: String },
    /// Resume daemon watching for a vault
    Resume { name: String },
}

#[derive(Subcommand)]
enum DaemonAction {
    /// Enable the LaunchAgent daemon (register plist and start)
    #[command(name = "enable", alias = "install")]
    Enable {
        /// Start immediately (default behavior, accepted for consistency)
        #[arg(long)]
        now: bool,
    },
    /// Disable the LaunchAgent daemon (stop and remove plist)
    #[command(name = "disable", alias = "uninstall")]
    Disable {
        /// Stop immediately (default behavior, accepted for consistency)
        #[arg(long)]
        now: bool,
    },
    /// Start the installed LaunchAgent
    Start,
    /// Stop the running LaunchAgent
    Stop,
    /// Show daemon status
    Status,
    /// Restart the daemon (stop then start)
    Restart,
}

#[derive(Subcommand)]
enum GraphAction {
    /// Show graph statistics and health metrics
    Health,
    /// List orphan notes (no incoming or outgoing links)
    Orphans {
        /// Attempt to auto-link orphans to relevant MOCs using AI
        #[arg(long)]
        auto_link: bool,
        /// Exclude files under */seeded/ directories
        #[arg(long)]
        exclude_seeded: bool,
        /// Minimum body length in characters (excluding frontmatter) to be included
        #[arg(long, default_value = "0")]
        min_chars: usize,
    },
    /// Extract wikilinks and (optionally) AI relationships
    Extract {
        /// Skip AI relationship extraction, only parse wikilinks
        #[arg(long)]
        no_ai: bool,
    },
    /// Normalize and cluster tags into hierarchical structure
    Tags {
        /// Show suggestions without applying changes
        #[arg(long)]
        dry_run: bool,
    },
    /// Run the full graph strengthening pipeline
    Strengthen,
    /// Generate the agent entry point index.md at the vault root
    Index,
}

#[derive(Subcommand)]
enum SettingsAction {
    /// Import .obsidian/ settings from a vault into the global store
    Import {
        /// Vault name (from global config) or path
        source: String,
    },
    /// Push global settings to a vault's .obsidian/
    Push {
        /// Vault name (from global config) or path
        target: String,
    },
    /// Push global settings to ALL registered vaults
    PushAll,
    /// Show global settings store status
    Status,
}

#[derive(Subcommand)]
enum BookAction {
    /// Initialize a new book project in 01-Projects/
    Init {
        name: String,
        #[arg(long, default_value = "non-fiction")]
        genre: String,
        #[arg(long, default_value = "ko")]
        lang: String,
    },
    /// Show all book projects status
    Status { name: Option<String> },
    /// Export book project to standalone directory (Velith compatible)
    Export {
        name: String,
        #[arg(long, default_value = ".")]
        output: String,
    },
    /// Sync vault notes tagged for this book into sources/
    Sync { name: String },
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env from home dir or CWD (whichever exists).
    let home_env = dirs_home().join(config::GLOBAL_DIR).join(".env");
    if home_env.exists() {
        dotenv::from_path(&home_env).ok();
    } else {
        dotenv::from_path(".env").ok();
    }
    setup_logging();

    let cli = Cli::parse();

    match &cli.command {
        Commands::Init {
            name,
            path,
            clone_settings_from,
        } => {
            let target = fs::canonicalize(path).unwrap_or_else(|_| PathBuf::from(path));
            init::init_vault(name, &target)?;

            let target_vault = target.join(name);
            if let Some(source) = clone_settings_from {
                // Explicit flag: clone from a specific vault
                let source_path = resolve_vault_path(source)?;
                init::clone_obsidian_settings(&source_path, &target_vault)?;
            } else if GlobalConfig::has_settings() {
                // Auto-apply from global settings store
                init::apply_global_settings(&target_vault)?;
            }
            return Ok(());
        }
        Commands::CloneSettings { source, target } => {
            let source_path = resolve_vault_path(source)?;
            let target_path = resolve_vault_path(target)?;
            return init::clone_obsidian_settings(&source_path, &target_path);
        }
        Commands::Settings { action } => {
            return handle_settings_action(action);
        }
        Commands::Vault { action } => {
            return handle_vault_action(action);
        }
        Commands::Daemon { action } => {
            check_daemon_deprecated_args();
            return handle_daemon_action(action);
        }
        Commands::Book { action, vault } => {
            let vault_path = if let Some(name) = vault {
                let global = GlobalConfig::load()?;
                global
                    .find_vault(name)
                    .map(|e| PathBuf::from(&e.path))
                    .ok_or_else(|| anyhow::anyhow!("Vault '{}' not found in global config", name))?
            } else {
                config::resolve_vault(cli.vault_path.clone()).unwrap_or_else(|_| {
                    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
                })
            };
            return handle_book_action(action, &vault_path);
        }
        _ => {}
    }

    // Commands that target vault(s)
    match cli.command {
        Commands::Watch {
            vault: filter,
            interval,
        } => {
            run_watch(filter, interval).await?;
        }
        Commands::Sync { vault: filter } => {
            run_sync_all(filter)?;
        }
        Commands::ProcessAll { vault: filter } => {
            let (vault, config) = resolve_single_vault(cli.vault_path, filter)?;
            notes::process_all(&vault, &config).await?;
        }
        Commands::UpdateMocs { vault: filter } => {
            let (vault, config) = resolve_single_vault(cli.vault_path, filter)?;
            moc::update_all_mocs(&vault, &config)?;
        }
        Commands::StrengthenGraph { vault: filter } => {
            let (vault, config) = resolve_single_vault(cli.vault_path, filter)?;
            graph::strengthen_graph(&vault, &config)?;
        }
        Commands::Graph {
            ref action,
            vault: filter,
        } => {
            let (vault, config) = resolve_single_vault(cli.vault_path, filter)?;
            handle_graph_action(action, &vault, &config).await?;
        }
        Commands::CheckTags {
            fix,
            scope,
            vault: filter,
        } => {
            let (vault, config) = resolve_single_vault(cli.vault_path, filter)?;
            let result = check_tags::check_tags(&vault, &config, fix, scope)?;
            println!("{}", result);
        }
        Commands::CheckLinks { fix, vault: filter } => {
            let (vault, config) = resolve_single_vault(cli.vault_path, filter)?;
            let result = check_links::check_links(&vault, &config, fix)?;
            println!("{}", result);
        }
        Commands::NormalizeFrontmatter { fix, vault: filter } => {
            let (vault, config) = resolve_single_vault(cli.vault_path, filter)?;
            let result = frontmatter::normalize_frontmatter(&vault, &config, fix)?;
            println!("{}", result);
        }
        Commands::Doctor {
            vault: filter,
            no_ping,
        } => {
            run_status_command(cli.vault_path, filter, no_ping).await?;
        }
        Commands::Status {
            vault: filter,
            no_ping,
        } => {
            run_status_command(cli.vault_path, filter, no_ping).await?;
        }
        _ => unreachable!(),
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Graph subcommands
// ---------------------------------------------------------------------------

async fn handle_graph_action(
    action: &GraphAction,
    vault: &Path,
    config: &ForgeConfig,
) -> Result<()> {
    match action {
        GraphAction::Health => {
            let health = graph::graph_health(vault, config)?;
            println!("{}", health);
        }
        GraphAction::Orphans {
            auto_link,
            exclude_seeded,
            min_chars,
        } => {
            if *auto_link {
                let g = graph::build_vault_graph(vault, config)?;
                let linked = graph::auto_link_orphans(vault, config, &g).await?;
                if linked.is_empty() {
                    println!("No orphans auto-linked.");
                } else {
                    println!("Auto-linked {} orphans:", linked.len());
                    for f in &linked {
                        println!("  - {}", f);
                    }
                }
            } else {
                let orphans = graph::detect_orphans(vault, config, *exclude_seeded, *min_chars)?;
                if orphans.is_empty() {
                    println!("No orphan notes found.");
                } else {
                    println!("Found {} orphan notes:", orphans.len());
                    for f in &orphans {
                        println!("  - {}", f);
                    }
                }
            }
        }
        GraphAction::Extract { no_ai } => {
            let g = graph::build_vault_graph(vault, config)?;
            println!(
                "Extracted graph: {} files, {} links, {} orphans",
                g.all_files.len(),
                g.total_links(),
                g.orphan_count()
            );

            if !no_ai {
                let relationships = graph::extract_relationships(vault, config, &g).await?;
                if relationships.is_empty() {
                    println!("No relationships extracted.");
                } else {
                    println!("Extracted {} relationships:", relationships.len());
                    for r in &relationships {
                        println!(
                            "  {} --{}-> {} ({:.0}%)",
                            r.source,
                            r.relation,
                            r.target,
                            r.confidence * 100.0
                        );
                    }
                    graph::save_relationships_manifest(vault, &relationships)?;
                }
            }
        }
        GraphAction::Tags { dry_run } => {
            let result = graph::normalize_tags(vault, config, *dry_run).await?;
            if *dry_run {
                println!("{}", result);
                println!("\n(dry run — no changes applied)");
            } else {
                println!("{}", result);
            }
        }
        GraphAction::Strengthen => {
            graph::strengthen_graph(vault, config)?;
            println!("Graph strengthening complete.");
        }
        GraphAction::Index => {
            index::generate_index(vault, config)?;
            println!("index.md generated.");
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Daemon management (macOS LaunchAgent)
// ---------------------------------------------------------------------------

fn warn_deprecated(old: &str, new: &str) {
    eprintln!(
        "[deprecated] '{}' is deprecated, use '{}' instead.",
        old, new
    );
}

/// Check raw CLI args for deprecated alias usage after the `daemon` subcommand.
fn check_daemon_deprecated_args() {
    let args: Vec<String> = std::env::args().collect();
    // Find the position of "daemon" and check the next token
    if let Some(idx) = args.iter().position(|a| a == "daemon")
        && let Some(sub) = args.get(idx + 1)
    {
        match sub.as_str() {
            "install" => warn_deprecated("daemon install", "daemon enable"),
            "uninstall" => warn_deprecated("daemon uninstall", "daemon disable"),
            _ => {}
        }
    }
}

fn handle_daemon_action(action: &DaemonAction) -> Result<()> {
    let label = daemon_label();
    let plist_path = launch_agents_dir().join(format!("{}.plist", label));

    match action {
        DaemonAction::Enable { .. } => {
            let exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("obsidian-forge"));
            let log_dir = dirs_home().join(".obsidian-forge/logs");
            fs::create_dir_all(&log_dir)?;

            let plist = build_plist(&label, &exe, &log_dir);
            let plist_parent = plist_path
                .parent()
                .ok_or_else(|| anyhow::anyhow!("invalid plist path: {}", plist_path.display()))?;
            fs::create_dir_all(plist_parent)?;
            fs::write(&plist_path, plist)?;
            println!("✅ Plist written: {}", plist_path.display());

            if is_agent_loaded(&label) {
                println!("ℹ️  Already loaded — restarting to apply changes");
                let _ = launchctl(&["bootout", &format!("gui/{}/{}", uid(), label)]);
            }

            launchctl(&[
                "bootstrap",
                &format!("gui/{}", uid()),
                &plist_path.to_string_lossy(),
            ])?;
            println!("✅ Daemon enabled and started (label: {})", label);
            println!("   Logs: {}/forge.log", log_dir.display());
        }
        DaemonAction::Disable { .. } => {
            let _ = launchctl(&["bootout", &format!("gui/{}/{}", uid(), label)]);
            if plist_path.exists() {
                fs::remove_file(&plist_path)?;
                println!("✅ Plist removed: {}", plist_path.display());
            }
            println!("✅ Daemon disabled");
        }
        DaemonAction::Start => {
            if !plist_path.exists() {
                anyhow::bail!("Daemon not installed. Run `obsidian-forge daemon enable` first.");
            }
            if is_agent_loaded(&label) {
                println!("ℹ️  Daemon already running ({})", label);
                return Ok(());
            }
            launchctl(&[
                "bootstrap",
                &format!("gui/{}", uid()),
                &plist_path.to_string_lossy(),
            ])?;
            println!("▶️  Daemon started ({})", label);
        }
        DaemonAction::Stop => {
            launchctl(&["bootout", &format!("gui/{}/{}", uid(), label)])?;
            println!("⏹️  Daemon stopped ({})", label);
        }
        DaemonAction::Restart => {
            if !plist_path.exists() {
                anyhow::bail!("Daemon not installed. Run `obsidian-forge daemon enable` first.");
            }
            let _ = launchctl(&["bootout", &format!("gui/{}/{}", uid(), label)]);
            launchctl(&[
                "bootstrap",
                &format!("gui/{}", uid()),
                &plist_path.to_string_lossy(),
            ])?;
            println!("🔄 Daemon restarted ({})", label);
        }
        DaemonAction::Status => {
            // ── Plist installation ──
            let installed = plist_path.exists();
            println!("Daemon");
            println!("  Label:       {}", label);
            println!(
                "  Plist:       {} ({})",
                plist_path.display(),
                if installed {
                    "installed"
                } else {
                    "not installed"
                }
            );

            // ── Running state from launchctl ──
            let mut pid: Option<u32> = None;
            let mut last_exit: Option<u32> = None;

            match std::process::Command::new("launchctl")
                .args(["list", &label])
                .output()
            {
                Ok(out) if out.status.success() => {
                    let stdout = String::from_utf8_lossy(&out.stdout);
                    // Parse structured output: "PID" = <number>; , "LastExitStatus" = <number>;
                    for line in stdout.lines() {
                        let trimmed = line.trim();
                        if let Some(rest) = trimmed.strip_prefix("\"PID\"")
                            && let Some(n) = extract_plist_int(rest)
                        {
                            pid = Some(n);
                        } else if let Some(rest) = trimmed.strip_prefix("\"LastExitStatus\"")
                            && let Some(n) = extract_plist_int(rest)
                        {
                            last_exit = Some(n);
                        }
                    }

                    if let Some(p) = pid {
                        println!("  Status:      🟢 running (PID {})", p);
                    } else if installed {
                        println!("  Status:      🔴 stopped");
                    }
                    if let Some(code) = last_exit
                        && pid.is_none()
                    {
                        println!("  Last Exit:   {}", code);
                    }
                }
                _ => {
                    println!("  Status:      ⚫ not loaded");
                }
            }

            // ── Scheduling interval ──
            let interval_secs = GlobalConfig::load()
                .ok()
                .and_then(|g| g.daemon)
                .and_then(|d| d.interval_seconds)
                .unwrap_or_else(|| {
                    // Fallback: legacy sync.interval_minutes → seconds
                    GlobalConfig::load()
                        .ok()
                        .and_then(|g| g.sync)
                        .and_then(|s| s.interval_minutes)
                        .map(|m| m * 60)
                        .unwrap_or(300)
                });
            println!(
                "  Interval:    {}s ({})",
                interval_secs,
                format_duration(interval_secs)
            );

            // ── Log files ──
            let log_dir = dirs_home().join(".obsidian-forge/logs");
            let stdout_log = log_dir.join("forge.log");
            let stderr_log = log_dir.join("forge.err");
            print!("  Stdout log:  {} ", stdout_log.display());
            if stdout_log.exists() {
                match fs::metadata(&stdout_log) {
                    Ok(meta) => println!("({})", format_bytes(meta.len())),
                    Err(_) => println!(),
                }
            } else {
                println!("(not created)");
            }
            print!("  Stderr log:  {} ", stderr_log.display());
            if stderr_log.exists() {
                match fs::metadata(&stderr_log) {
                    Ok(meta) => println!("({})", format_bytes(meta.len())),
                    Err(_) => println!(),
                }
            } else {
                println!("(not created)");
            }

            // ── Registered vaults ──
            if let Ok(global) = GlobalConfig::load() {
                let watchable = global.watchable_vaults();
                let total = global.vaults.len();
                let enabled_count = global.enabled_vaults().len();
                println!(
                    "  Vaults:      {} registered, {} enabled, {} watchable",
                    total,
                    enabled_count,
                    watchable.len()
                );
                if !watchable.is_empty() {
                    for v in &watchable {
                        let path = PathBuf::from(&v.path);
                        let exists = if path.exists() { "✓" } else { "✗ missing" };
                        println!("               · {} ({}) {}", v.name, v.path, exists);
                    }
                }
            }
        }
    }
    Ok(())
}

fn build_plist(label: &str, exe: &Path, log_dir: &Path) -> String {
    let home = dirs_home();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{label}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
        <string>watch</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <dict>
        <key>SuccessfulExit</key>
        <false/>
    </dict>
    <key>ThrottleInterval</key>
    <integer>30</integer>
    <key>WorkingDirectory</key>
    <string>{home}</string>
    <key>StandardOutPath</key>
    <string>{log}/forge.log</string>
    <key>StandardErrorPath</key>
    <string>{log}/forge.err</string>
    <key>EnvironmentVariables</key>
    <dict>
        <key>LOG_TO_FILE</key>
        <string>1</string>
        <key>HOME</key>
        <string>{home}</string>
        <key>PATH</key>
        <string>/usr/local/bin:/opt/homebrew/bin:/usr/bin:/bin</string>
    </dict>
</dict>
</plist>
"#,
        label = label,
        exe = exe.display(),
        log = log_dir.display(),
        home = home.display(),
    )
}

fn launch_agents_dir() -> PathBuf {
    dirs_home().join("Library").join("LaunchAgents")
}

fn dirs_home() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("~"))
}

fn daemon_label() -> String {
    "com.obsidian-forge.watch".to_string()
}

fn uid() -> String {
    std::process::Command::new("id")
        .arg("-u")
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .unwrap_or_else(|| "501".to_string())
}

fn launchctl(args: &[&str]) -> Result<()> {
    let status = std::process::Command::new("launchctl")
        .args(args)
        .status()?;
    if !status.success() {
        anyhow::bail!("launchctl {} failed (exit: {})", args.join(" "), status);
    }
    Ok(())
}

fn is_agent_loaded(label: &str) -> bool {
    std::process::Command::new("launchctl")
        .args(["list", label])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Book subcommand
// ---------------------------------------------------------------------------

fn handle_book_action(action: &BookAction, vault_path: &Path) -> Result<()> {
    match action {
        BookAction::Init { name, genre, lang } => {
            book::init_book_project(name, vault_path, genre, lang)?;
        }
        BookAction::Status { name } => {
            book::show_book_status(name.as_deref(), vault_path)?;
        }
        BookAction::Export { name, output } => {
            let output_path = book::output_path_from(output, vault_path);
            book::export_book(name, vault_path, &output_path)?;
        }
        BookAction::Sync { name } => {
            book::sync_sources(name, vault_path)?;
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Settings management
// ---------------------------------------------------------------------------

fn handle_settings_action(action: &SettingsAction) -> Result<()> {
    match action {
        SettingsAction::Import { source } => {
            let source_path = resolve_vault_path(source)?;
            init::import_settings(&source_path)?;
        }
        SettingsAction::Push { target } => {
            let target_path = resolve_vault_path(target)?;
            init::push_settings(&target_path)?;
        }
        SettingsAction::PushAll => {
            let global = GlobalConfig::load()?;
            if global.vaults.is_empty() {
                no_vaults_hint();
                return Ok(());
            }
            for entry in &global.vaults {
                let vault_path = PathBuf::from(&entry.path);
                if vault_path.exists() {
                    println!("\n📦 {}", entry.name);
                    init::push_settings(&vault_path)?;
                } else {
                    println!("\n⚠️  Skipping {} (path not found)", entry.name);
                }
            }
        }
        SettingsAction::Status => {
            let store = GlobalConfig::settings_dir();
            println!("Global settings store: {}", store.display());
            if GlobalConfig::has_settings() {
                for dir in config::SETTINGS_DIRS {
                    let p = store.join(dir);
                    if p.is_dir() {
                        let count = fs::read_dir(&p).map(|r| r.count()).unwrap_or(0);
                        println!("  ✓ {}/  ({} items)", dir, count);
                    } else {
                        println!("  ✗ {}/  (not present)", dir);
                    }
                }
                for file in config::SETTINGS_FILES {
                    let p = store.join(file);
                    if p.is_file() {
                        println!("  ✓ {}", file);
                    } else {
                        println!("  ✗ {}  (not present)", file);
                    }
                }
            } else {
                println!("  (empty — run `settings import <vault>` to populate)");
            }
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Vault management
// ---------------------------------------------------------------------------

fn handle_vault_action(action: &VaultAction) -> Result<()> {
    let mut global = GlobalConfig::load().unwrap_or_default();

    match action {
        VaultAction::Add { path, name } => {
            let abs = fs::canonicalize(path).unwrap_or_else(|_| PathBuf::from(path));
            let vault_name = name.clone().unwrap_or_else(|| {
                abs.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unnamed")
                    .to_string()
            });

            // Create vault.toml if missing (commented examples; defaults from global + serde)
            let vault_toml = abs.join(config::CONFIG_FILE);
            if !vault_toml.exists() {
                fs::write(&vault_toml, default_vault_toml_template(&vault_name))?;
                println!("  Created vault.toml in {}", abs.display());
            }

            global.add_vault(&vault_name, &abs.to_string_lossy());
            if global.seed_missing_tooling_sections() {
                println!(
                    "  Seeded default [projects], [graph], [sync], [ai], [daemon] in {}",
                    GlobalConfig::path().display()
                );
            }
            global.save()?;
            println!("✅ Registered: {} → {}", vault_name, abs.display());
        }
        VaultAction::Remove { name } => {
            if global.remove_vault(name) {
                global.save()?;
                println!("✅ Removed: {} (files kept)", name);
            } else {
                println!("⚠️  Vault not found: {}", name);
            }
        }
        VaultAction::List => {
            if global.vaults.is_empty() {
                println!("No vaults registered. Use `obsidian-forge init` or `vault add`.");
            } else {
                println!("{:<20} {:<8} {:<8} PATH", "NAME", "ENABLED", "WATCH");
                println!("{}", "-".repeat(72));
                for v in &global.vaults {
                    let enabled = if v.enabled { "✓" } else { "✗" };
                    let watch = if v.watch { "✓" } else { "✗" };
                    println!("{:<20} {:<8} {:<8} {}", v.name, enabled, watch, v.path);
                }
            }
        }
        VaultAction::Disable { name } => {
            if let Some(v) = global.find_vault_mut(name) {
                v.enabled = false;
                v.watch = false;
                global.save()?;
                println!("✅ Disabled: {} (excluded from sync and watch)", name);
            } else {
                println!("⚠️  Vault not found: {}", name);
            }
        }
        VaultAction::Enable { name } => {
            if let Some(v) = global.find_vault_mut(name) {
                v.enabled = true;
                v.watch = true;
                global.save()?;
                println!("✅ Enabled: {} (sync + watch)", name);
            } else {
                println!("⚠️  Vault not found: {}", name);
            }
        }
        VaultAction::Pause { name } => {
            if let Some(v) = global.find_vault_mut(name) {
                v.watch = false;
                global.save()?;
                println!("⏸️  Paused: {} (daemon skip, manual sync OK)", name);
            } else {
                println!("⚠️  Vault not found: {}", name);
            }
        }
        VaultAction::Resume { name } => {
            if let Some(v) = global.find_vault_mut(name) {
                v.watch = true;
                global.save()?;
                println!("▶️  Resumed: {} (daemon active)", name);
            } else {
                println!("⚠️  Vault not found: {}", name);
            }
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Multi-vault watch
// ---------------------------------------------------------------------------

async fn run_watch(filter: Option<String>, interval_override: Option<u64>) -> Result<()> {
    let global = GlobalConfig::load()?;
    let vaults: Vec<_> = match &filter {
        Some(name) => global
            .vaults
            .iter()
            .filter(|v| v.name == *name && v.enabled && v.watch)
            .collect(),
        None => global.watchable_vaults(),
    };

    if vaults.is_empty() {
        if global.vaults.is_empty() {
            no_vaults_hint();
            return Ok(());
        }
        // Fallback: try single-vault mode from CWD / VAULT_PATH
        let vault = config::resolve_vault(None)?;
        let config = ForgeConfig::load(&vault)?;
        let vault_for_sync = vault.clone();
        let config_for_sync = config.clone();
        let interval_secs = resolve_interval(&config, interval_override);
        tokio::spawn(async move {
            use tokio::time::{Duration, sleep};
            loop {
                run_sync_cycle(&vault_for_sync, &config_for_sync);
                sleep(Duration::from_secs(interval_secs)).await;
            }
        });
        watcher::watch_inbox(&vault, &config).await?;
        return Ok(());
    }

    tracing::info!("Watching {} vault(s)", vaults.len());

    // Spawn sync + watcher tasks for each vault
    let mut handles = Vec::new();

    for entry in &vaults {
        let vault_path = PathBuf::from(&entry.path);
        let vault_name = entry.name.clone();

        if !vault_path.exists() {
            tracing::warn!("Vault path does not exist, skipping: {}", entry.path);
            continue;
        }

        let config = match ForgeConfig::load(&vault_path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!("Failed to load config for {}: {:?}", vault_name, e);
                continue;
            }
        };

        // Resolve interval: CLI override > daemon.interval_seconds > sync.interval_minutes (legacy)
        let interval_secs = resolve_interval(&config, interval_override);

        // Sync loop for this vault
        let vp = vault_path.clone();
        let cfg = config.clone();
        tokio::spawn(async move {
            use tokio::time::{Duration, sleep};
            loop {
                tracing::debug!("Sync cycle: {}", vp.display());
                run_sync_cycle(&vp, &cfg);
                sleep(Duration::from_secs(interval_secs)).await;
            }
        });

        // Inbox watcher for this vault
        let vp = vault_path.clone();
        let cfg = config.clone();
        let handle = tokio::spawn(async move {
            if let Err(e) = watcher::watch_inbox(&vp, &cfg).await {
                tracing::error!("Watcher failed for {}: {:?}", vp.display(), e);
            }
        });
        handles.push(handle);

        tracing::info!(
            "Started watch for: {} ({}) - interval: {}s",
            vault_name,
            vault_path.display(),
            interval_secs
        );
    }

    // Wait for all watchers (they run forever)
    for h in handles {
        let _ = h.await;
    }

    Ok(())
}

/// Resolve sync interval with priority: CLI override > daemon.interval_seconds > sync.interval_minutes
fn resolve_interval(config: &ForgeConfig, cli_override: Option<u64>) -> u64 {
    if let Some(secs) = cli_override {
        return secs.max(1);
    }
    // Use daemon.interval_seconds if set (Some)
    if let Some(secs) = config.daemon.interval_seconds {
        return secs.max(1);
    }
    // Fallback to legacy sync.interval_minutes (convert to seconds)
    if let Some(mins) = config.sync.interval_minutes {
        return mins.max(1) * 60;
    }
    // Default 5 minutes
    300
}

// ---------------------------------------------------------------------------
// Multi-vault sync
// ---------------------------------------------------------------------------

fn run_sync_all(filter: Option<String>) -> Result<()> {
    let global = GlobalConfig::load()?;
    let vaults: Vec<_> = match &filter {
        Some(name) => global
            .vaults
            .iter()
            .filter(|v| v.name == *name && v.enabled)
            .collect(),
        None => global.enabled_vaults(),
    };

    if vaults.is_empty() {
        if global.vaults.is_empty() {
            no_vaults_hint();
            return Ok(());
        }
        // Fallback: single vault from CWD
        let vault = config::resolve_vault(None)?;
        let config = ForgeConfig::load(&vault)?;
        run_sync_cycle(&vault, &config);
        return Ok(());
    }

    for entry in &vaults {
        let vault_path = PathBuf::from(&entry.path);
        if !vault_path.exists() {
            tracing::warn!("Skipping {}: path does not exist", entry.name);
            continue;
        }
        match ForgeConfig::load(&vault_path) {
            Ok(config) => {
                println!("Syncing: {} ...", entry.name);
                run_sync_cycle(&vault_path, &config);
                println!("  ✓ {}", entry.name);
            }
            Err(e) => tracing::warn!("Skipping {}: {:?}", entry.name, e),
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn run_sync_cycle(vault: &Path, config: &ForgeConfig) {
    if let Err(e) = moc::update_all_mocs(vault, config) {
        tracing::warn!("[{}] MOC update error: {:?}", config.vault.name, e);
    }
    if let Err(e) = index::generate_index(vault, config) {
        tracing::warn!("[{}] Index generation error: {:?}", config.vault.name, e);
    }
    if let Err(e) = graph::strengthen_graph(vault, config) {
        tracing::warn!("[{}] Graph error: {:?}", config.vault.name, e);
    }
    if config.sync.git_auto_commit
        && let Err(e) = git::auto_commit_and_push(vault, config.sync.git_auto_push)
    {
        tracing::warn!("[{}] Git error: {:?}", config.vault.name, e);
    }
}

/// Resolve a single vault from --vault flag, --vault-path, or CWD.
///
/// Priority:
/// 1. `--vault <name>` — look up by name in global config
/// 2. `--vault-path` / `VAULT_PATH` env — direct path
/// 3. CWD walk-up — look for `vault.toml` or `00-Inbox`
/// 4. Global config fallback — single registered vault → auto-select; multiple → list
fn resolve_single_vault(
    vault_path: Option<String>,
    filter: Option<String>,
) -> Result<(PathBuf, ForgeConfig)> {
    // 1. Explicit --vault <name>
    if let Some(name) = filter {
        let global = GlobalConfig::load()?;
        if let Some(entry) = global.find_vault(&name) {
            let p = PathBuf::from(&entry.path);
            let c = ForgeConfig::load(&p)?;
            return Ok((p, c));
        }
        anyhow::bail!("Vault '{}' not found in global config", name);
    }

    // 2 & 3. --vault-path / VAULT_PATH / CWD walk-up
    if let Ok(vault) = config::resolve_vault(vault_path.clone()) {
        let config = ForgeConfig::load(&vault)?;
        return Ok((vault, config));
    }

    // 4. Global config fallback
    let global = GlobalConfig::load()?;
    let enabled: Vec<_> = global.enabled_vaults();
    match enabled.len() {
        0 => {
            anyhow::bail!(
                "No vaults registered. Run `of init <name>` to create one, \
                 or `of vault add <path>` to register an existing vault."
            );
        }
        1 => {
            let entry = enabled[0];
            let p = PathBuf::from(&entry.path);
            let c = ForgeConfig::load(&p)?;
            Ok((p, c))
        }
        _ => {
            let names: Vec<String> = enabled.iter().map(|v| v.name.clone()).collect();
            anyhow::bail!(
                "Multiple vaults registered. Specify one with --vault <name>.\n\
                 Available: {}",
                names.join(", ")
            );
        }
    }
}

// ---------------------------------------------------------------------------
// Status
// ---------------------------------------------------------------------------

async fn run_status_command(
    vault_path: Option<String>,
    filter: Option<String>,
    no_ping: bool,
) -> Result<()> {
    // --vault <name> 또는 --vault-path 지정 → 단일 vault
    if filter.is_some() || vault_path.is_some() {
        let (vault, config) = resolve_single_vault(vault_path, filter)?;
        return run_status(&vault, &config, no_ping).await;
    }

    // CWD walk-up으로 단일 vault 감지 시 그것만 사용
    if let Ok(vault) = config::resolve_vault(None) {
        let config = ForgeConfig::load(&vault)?;
        return run_status(&vault, &config, no_ping).await;
    }

    // 글로벌 설정 fallback: 등록된 모든 vault 출력
    let global = GlobalConfig::load()?;
    let enabled: Vec<_> = global.enabled_vaults();
    match enabled.len() {
        0 => anyhow::bail!(
            "No vaults registered. Run `of init <name>` to create one, \
             or `of vault add <path>` to register an existing vault."
        ),
        1 => {
            let p = PathBuf::from(&enabled[0].path);
            let c = ForgeConfig::load(&p)?;
            run_status(&p, &c, no_ping).await
        }
        _ => {
            let mut first = true;
            for entry in &enabled {
                let p = PathBuf::from(&entry.path);
                match ForgeConfig::load(&p) {
                    Ok(c) => {
                        if !first {
                            println!("{}", "─".repeat(60));
                        }
                        first = false;
                        if let Err(e) = run_status(&p, &c, no_ping).await {
                            println!("  ⚠️  Error reading vault '{}': {}", entry.name, e);
                        }
                    }
                    Err(e) => {
                        eprintln!("warning: skipping '{}' — {}", entry.name, e);
                    }
                }
            }
            Ok(())
        }
    }
}

async fn run_status(vault: &Path, config: &ForgeConfig, no_ping: bool) -> Result<()> {
    // ── Vault ──
    println!("Vault");
    println!("  Name:       {}", config.vault.name);
    println!("  Path:       {}", vault.display());

    // Total note count (all .md files excluding .obsidian/ and system dirs)
    let system_dirs = config.all_system_dirs();
    let total_notes = count_markdown_notes(vault, &system_dirs);
    println!("  Notes:      {}", total_notes);

    // Last sync time (derived from most recent git commit timestamp)
    let last_sync = std::process::Command::new("git")
        .args(["log", "-1", "--format=%cs"])
        .current_dir(vault)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .filter(|s| !s.is_empty());
    match last_sync {
        Some(ref ts) => println!("  Last sync:  {}", ts),
        None => println!("  Last sync:  (no commits yet)"),
    }

    // LaunchAgent status (macOS only)
    let label = daemon_label();
    let plist_path = launch_agents_dir().join(format!("{}.plist", label));
    if cfg!(target_os = "macos") {
        if plist_path.exists() {
            let running = std::process::Command::new("launchctl")
                .args(["list", &label])
                .output()
                .ok()
                .filter(|o| o.status.success())
                .and_then(|o| {
                    let s = String::from_utf8_lossy(&o.stdout);
                    s.lines()
                        .find(|l| l.trim().starts_with("\"PID\""))
                        .and_then(|l| {
                            extract_plist_int(l.trim().strip_prefix("\"PID\"").unwrap_or(""))
                        })
                });
            if let Some(pid) = running {
                println!("  LaunchAgent: running (PID {})", pid);
            } else {
                println!("  LaunchAgent: installed (stopped)");
            }
        } else {
            println!("  LaunchAgent: not installed");
        }
    }

    // Git working tree status
    let git_branch = std::process::Command::new("git")
        .args(["rev-parse", "--abbrev-ref", "HEAD"])
        .current_dir(vault)
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
    match git_branch {
        Some(ref branch) => {
            let dirty = std::process::Command::new("git")
                .args(["status", "--porcelain"])
                .current_dir(vault)
                .output()
                .ok()
                .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
                .filter(|s| !s.is_empty());
            match dirty {
                Some(ref changes) => {
                    let count = changes.lines().count();
                    println!(
                        "  Git:        {} ({} uncommitted change{})",
                        branch,
                        count,
                        if count == 1 { "" } else { "s" }
                    );
                }
                None => println!("  Git:        {} (clean)", branch),
            }
        }
        None => println!("  Git:        (not a git repo)"),
    }

    // Configuration file path
    let config_path = vault.join(config::CONFIG_FILE);
    println!("  Config:     {}", config_path.display());

    let inbox = vault.join(&config.vault.inbox_dir);
    let inbox_count = if inbox.exists() {
        fs::read_dir(&inbox).map(|r| r.count()).unwrap_or(0)
    } else {
        0
    };
    println!(
        "  Inbox:      {} ({}, {} items)",
        config.vault.inbox_dir,
        if inbox.exists() { "exists" } else { "missing" },
        inbox_count
    );

    // ── AI ──
    let client = ai::AiClient::from_config(&config.ai);
    let summary = client.config_summary();

    println!();
    println!("AI");
    println!("  Provider:   {}", summary.provider);
    println!("  Model:      {}", summary.model);
    println!("  Base URL:   {}", summary.base_url);
    println!(
        "  API Key:    {}",
        if summary.api_key == "missing" {
            "⚠️  missing".to_string()
        } else if summary.api_key == "not required" {
            "not required".to_string()
        } else {
            summary.api_key.clone()
        }
    );

    if no_ping {
        println!("  Ping:       skipped (--no-ping)");
    } else {
        print!("  Ping:       ");
        match client.ping().await {
            Ok(resp) => {
                let display = if resp.len() > 80 {
                    format!("{}…", &resp[..80])
                } else {
                    resp
                };
                println!("✅ connected ({})", display);
            }
            Err(e) => println!("❌ {}", e),
        }
    }

    // ── Graph ──
    println!();
    println!("Graph");
    println!(
        "  Backlinks:      {}",
        if config.graph.backlinks {
            "✓ on"
        } else {
            "✗ off"
        }
    );
    println!(
        "  Bridge notes:   {}",
        if config.graph.bridge_notes {
            "✓ on"
        } else {
            "✗ off"
        }
    );
    println!(
        "  Auto tags:      {}",
        if config.graph.auto_tags {
            "✓ on"
        } else {
            "✗ off"
        }
    );
    println!(
        "  Related:        {}",
        if config.graph.related_projects {
            "✓ on"
        } else {
            "✗ off"
        }
    );
    if !config.graph.concepts.is_empty() {
        println!("  Concepts:       {} defined", config.graph.concepts.len());
    }

    // ── Sync ──
    println!();
    println!("Sync");
    println!(
        "  Git commit: {}",
        if config.sync.git_auto_commit {
            "enabled"
        } else {
            "disabled"
        }
    );
    println!(
        "  Git push:   {}",
        if config.sync.git_auto_push {
            "enabled"
        } else {
            "disabled"
        }
    );

    // ── Scheduling ──
    println!();
    println!("Scheduling");
    let interval_secs = resolve_interval(config, None);
    println!(
        "  Sync interval:  {} ({})",
        format_duration(interval_secs),
        interval_secs
    );

    Ok(())
}

/// Resolve a vault name (from global config) or path to an absolute path.
fn resolve_vault_path(name_or_path: &str) -> Result<PathBuf> {
    // Try global config first
    if let Ok(global) = GlobalConfig::load()
        && let Some(entry) = global.find_vault(name_or_path)
    {
        return Ok(PathBuf::from(&entry.path));
    }
    // Treat as path
    let p = PathBuf::from(name_or_path);
    if p.exists() {
        Ok(fs::canonicalize(&p).unwrap_or(p))
    } else {
        anyhow::bail!(
            "'{}' is not a registered vault name or valid path",
            name_or_path
        )
    }
}

fn no_vaults_hint() {
    eprintln!("No vaults registered. Run `of init <name>` to create your first vault.");
}

/// Count all `.md` files in the vault root, excluding system directories.
fn count_markdown_notes(vault: &Path, system_dirs: &[String]) -> usize {
    use walkdir::WalkDir;
    let system_set: std::collections::HashSet<&str> =
        system_dirs.iter().map(|s| s.as_str()).collect();
    WalkDir::new(vault)
        .into_iter()
        .filter_entry(|entry| {
            // Skip system directories at the top level
            if entry.depth() == 1
                && let Some(name) = entry.file_name().to_str()
                && system_set.contains(name)
            {
                return false;
            }
            true
        })
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file() && e.path().extension().is_some_and(|ext| ext == "md"))
        .count()
}

// ---------------------------------------------------------------------------
// Display helpers
// ---------------------------------------------------------------------------

/// Extract an integer value from a launchctl plist line fragment like `= 12345;`.
fn extract_plist_int(s: &str) -> Option<u32> {
    s.split('=')
        .nth(1)?
        .trim()
        .trim_end_matches(';')
        .trim()
        .parse()
        .ok()
}

/// Format seconds into a human-readable duration string (e.g., "5m", "2h 30m", "1h").
fn format_duration(secs: u64) -> String {
    if secs < 60 {
        format!("{}s", secs)
    } else if secs < 3600 {
        let m = secs / 60;
        let s = secs % 60;
        if s == 0 {
            format!("{}m", m)
        } else {
            format!("{}m {}s", m, s)
        }
    } else {
        let h = secs / 3600;
        let rem = secs % 3600;
        let m = rem / 60;
        if m == 0 {
            format!("{}h", h)
        } else {
            format!("{}h {}m", h, m)
        }
    }
}

/// Format byte count into human-readable size (e.g., "1.2 KB", "3.4 MB").
fn format_bytes(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    const GB: u64 = 1024 * 1024 * 1024;
    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{} B", bytes)
    }
}

fn setup_logging() {
    let log_dir = std::env::var("HOME")
        .map(|h| PathBuf::from(h).join(config::GLOBAL_DIR).join("logs"))
        .unwrap_or_else(|_| PathBuf::from("logs"));
    fs::create_dir_all(&log_dir).ok();

    let log_file_path = log_dir.join("forge.log");
    let use_file_log = !std::io::IsTerminal::is_terminal(&std::io::stdout())
        || std::env::var("LOG_TO_FILE").is_ok();

    if use_file_log {
        let file = match fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_file_path)
        {
            Ok(f) => f,
            Err(_) => {
                // Can't create log file — fall back to stderr logging.
                eprintln!(
                    "Warning: cannot create log file at {}, falling back to stderr",
                    log_file_path.display()
                );
                fmt()
                    .with_env_filter(EnvFilter::from_default_env())
                    .with_target(false)
                    .init();
                return;
            }
        };
        let layer = fmt::layer()
            .with_writer(std::sync::Mutex::new(file))
            .with_target(false)
            .with_file(true)
            .with_line_number(true)
            .with_ansi(false);
        tracing_subscriber::registry()
            .with(EnvFilter::from_default_env())
            .with(layer)
            .init();
    } else {
        fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .with_target(false)
            .with_file(true)
            .with_line_number(true)
            .init();
    }
}
