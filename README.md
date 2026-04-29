<div align="center">

# ⚒️ obsidian-forge

**Obsidian vault generator, automation daemon, and graph strengthener**

[![License](https://img.shields.io/badge/License-Apache%202.0-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](https://www.rust-lang.org)
[![Crates.io](https://img.shields.io/crates/v/obsidian-forge.svg)](https://crates.io/crates/obsidian-forge)
[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-FFDD00?style=flat&logo=buy-me-a-coffee&logoColor=black)](https://buymeacoffee.com/epicsaga)

**Single binary. Multi-vault. Zero config to get started.**

[English](#) · [中文](docs/README_zh-CN.md) · [日本語](docs/README_ja.md) · [한국어](docs/README_ko.md) · [Español](docs/README_es.md) · [Português](docs/README_pt-BR.md) · [Français](docs/README_fr.md) · [Deutsch](docs/README_de.md) · [Русский](docs/README_ru.md) · [Türkçe](docs/README_tr.md)

</div>

---

## What is obsidian-forge?

`obsidian-forge` is a Rust CLI that scaffolds, automates, and maintains [Obsidian](https://obsidian.md) vaults. It runs as a background daemon watching your inbox, strengthening your knowledge graph, and syncing to git — so you can focus on writing.

```
of init my-brain          # scaffold a new vault in seconds
of daemon install         # register as a macOS login item
# → your vault now auto-processes, auto-links, and auto-commits
# "of" is a built-in short alias for "obsidian-forge"
```

---

## Features

| | Feature | Description |
|---|---|---|
| 🏗️ | **Vault scaffolding** | PARA layout, bundled templates, `.obsidian` config, git init |
| 🔗 | **Graph strengthening** | Backlinks, bridge notes, related-project links, auto-tags |
| 📥 | **Inbox processing** | Frontmatter injection, AI classification, PARA routing |
| 🔄 | **Sync cycle** | MOC rebuild → graph → auto git commit/push on a timer |
| 🗂️ | **Multi-vault** | One daemon manages all vaults; enable, pause, or disable per vault |
| ⚙️ | **Settings store** | Import plugins/themes from one vault and push to all others |
| 🤖 | **AI metadata** | Ollama, OpenAI, OpenRouter, LM Studio, or any OpenAI-compatible endpoint |
| 📄 | **PDF → Markdown** | Converts via `marker_single` with `pdftotext` fallback |
| 🍎 | **Login item** | Installs as a macOS LaunchAgent — auto-starts, auto-restarts |
| ♻️ | **Idempotent** | Safe to run any operation multiple times; no duplicate output |

---

## Installation

### via cargo-binstall (fastest - pre-built binaries)

```bash
cargo install obsidian-forge
# installs both `obsidian-forge` and `of` (short alias)
```

> Requires [`cargo-binstall`](https://github.com/cargo-bins/cargo-binstall) installed first:
> `cargo install cargo-binstall`

### via crates.io

```bash
cargo install obsidian-forge
# installs both `obsidian-forge` and `of` (short alias)
```

### From source

```bash
git clone https://github.com/epicsagas/obsidian-forge.git
cd obsidian-forge
cargo install --path .
# installs both `obsidian-forge` and `of` (short alias)
```

### Platform Support

| Platform | Status |
|---|---|
| macOS | ✅ Fully supported (including LaunchAgent daemon) |
| Linux | ✅ Fully supported |
| Windows | ⚠️ Partially supported (no LaunchAgent equivalent; foreground watch works) |

### Prerequisites

| Tool | Required | Purpose |
|---|---|---|
| Rust 1.75+ | ✅ | Build |
| git | ✅ | Vault versioning |
| Ollama / OpenAI / OpenRouter / LM Studio | ⬜ optional | AI tagging (`process-all`) |
| marker_single | ⬜ optional | High-quality PDF conversion |

---

## Quick Start

```bash
# 1. Create a new vault
of init my-brain

# 2. Open in Obsidian → File → Open Vault → my-brain

# 3. Register it with the global config
of vault add ~/my-brain

# 4. Install the background daemon
of daemon install

# Done — drop notes into 00-Inbox/ and obsidian-forge handles the rest
```

---

## Commands

### Vault Initialization

```bash
obsidian-forge init <name>
obsidian-forge init <name> --path ~/vaults
obsidian-forge init <name> --clone-settings-from ~/other-vault

# Re-run on an existing vault to repair/upgrade (idempotent — never overwrites)
obsidian-forge init my-brain --path ~/
```

### Multi-Vault Management

```bash
obsidian-forge vault add <path> [--name <alias>]
obsidian-forge vault remove <name>          # unregister (files kept)
obsidian-forge vault list                   # NAME / ENABLED / WATCH / PATH
obsidian-forge vault enable  <name>
obsidian-forge vault disable <name>         # exclude from sync and watch
obsidian-forge vault pause   <name>         # skip daemon; manual sync ok
obsidian-forge vault resume  <name>
```

### Settings Store

Sync `.obsidian/` plugins, themes, and snippets across all vaults.

```bash
obsidian-forge settings import <vault>      # pull settings into global store
obsidian-forge settings push   <vault>      # push global settings to one vault
obsidian-forge settings push-all            # push to ALL registered vaults
obsidian-forge settings status
```

### One-off Operations

```bash
obsidian-forge sync               [--vault <name>]   # MOC → graph → git
obsidian-forge update-mocs        [--vault <name>]
obsidian-forge strengthen-graph   [--vault <name>]
obsidian-forge process-all        [--vault <name>]   # AI inbox processing
```

### Background Daemon (macOS LaunchAgent)

```bash
obsidian-forge daemon install     # write plist + bootstrap (login item)
obsidian-forge daemon uninstall   # bootout + remove plist
obsidian-forge daemon start
obsidian-forge daemon stop
obsidian-forge daemon status      # shows PID and last exit code
```

> Logs → `~/.obsidian-forge/logs/obsidian-forge/forge.log`

### Foreground Watch

```bash
obsidian-forge watch              # all watchable vaults
obsidian-forge watch --vault <name>
```

---

## Configuration

`vault.toml` is created automatically by `init`. Every value has a sensible default.

```toml
[vault]
name            = "my-brain"
layout          = "para"           # only layout currently supported
inbox_dir       = "00-Inbox"
zettelkasten_dir= "10-Zettelkasten"
archive_dir     = "99-Archives"
attachments_dir = "Attachments"
templates_dir   = "obsidian-templates"

[graph]
backlinks        = true
bridge_notes     = true
auto_tags        = true
related_projects = true
# [[graph.concepts]]
# name     = "AI"
# keywords = ["machine learning", "LLM", "neural"]
# tags     = ["ai", "ml"]

[sync]
git_auto_commit  = true
git_auto_push    = true
interval_minutes = 60

[ai]
# provider: ollama | openai | openrouter | lmstudio | openai-compatible
provider = "ollama"
model    = "gemma3"
base_url = "http://192.168.0.28:1234/v1"  # required for openai-compatible; others have defaults
# api_key  = ""                          # optional — env var is preferred (see below)

[daemon]
label   = "com.obsidian-forge.watch"
log_dir = "~/.obsidian-forge/logs"
```

**API keys** are resolved in this order:

1. `api_key` in `[ai]` section (config.toml or vault.toml) — *avoid committing secrets*
2. Environment variable (see table below)
3. `~/.config/obsidian-forge/.env` file — **recommended** (auto-loaded, never committed)

| Provider | Environment variable | Notes |
|---|---|---|
| `openai` | `OPENAI_API_KEY` | [Get key →](https://platform.openai.com/api-keys) |
| `openrouter` | `OPENROUTER_API_KEY` | [Get key →](https://openrouter.ai/keys) |
| `openai-compatible` | `OPENAI_COMPATIBLE_API_KEY` | falls back to `OPENAI_API_KEY` |
| `ollama` / `lmstudio` | — | no key needed |

**Setting up API keys with `.env` (recommended):**

```bash
# Create the .env file (never committed to git)
cat > ~/.config/obsidian-forge/.env << 'EOF'
# Uncomment the line(s) for your provider(s):
# OPENAI_API_KEY=sk-...
# OPENROUTER_API_KEY=sk-or-...
# OPENAI_COMPATIBLE_API_KEY=...
EOF
```

> If both `OPENAI_COMPATIBLE_API_KEY` and `OPENAI_API_KEY` are set, the
> provider-specific one takes precedence. This lets you use `openai` and
> `openai-compatible` with different keys simultaneously.

**Config resolution:**

```
$VAULT_PATH                              # env override
│
├── auto-detection (walks up from CWD)  # looks for vault.toml or 00-Inbox/
│
~/.config/obsidian-forge/config.toml    # global: registered vaults
<vault>/vault.toml                      # per-vault settings
```

---

## Architecture

```
obsidian-forge/
├── src/
│   ├── main.rs        CLI (clap), multi-vault dispatch, sync loop
│   ├── config.rs      vault.toml + global config structs
│   ├── init.rs        vault scaffolding, settings import/push
│   ├── moc.rs         MOC hub file generation
│   ├── graph.rs       backlinks, bridge notes, auto-tags
│   ├── git.rs         auto commit + push (conventional commits)
│   ├── notes.rs       inbox processing + PARA routing
│   ├── converter.rs   PDF → Markdown
│   ├── ai.rs          AI client (Ollama, OpenAI-compatible providers)
│   ├── prompts.rs     LLM prompt templates
│   └── watcher.rs     filesystem watcher (notify crate)
└── vault.toml         per-vault config (created by init)
```

---

## Contributing

Contributions are welcome! Please read [CONTRIBUTING.md](CONTRIBUTING.md) before submitting a pull request.

```bash
git clone https://github.com/epicsagas/obsidian-forge.git
cd obsidian-forge
cargo build
cargo test
```

---

## Links

- 📚 **Documentation**: This README + inline code documentation
- 🐛 **Issues**: [GitHub Issues](https://github.com/epicsagas/obsidian-forge/issues)
- 💬 **Discussions**: [GitHub Discussions](https://github.com/epicsagas/obsidian-forge/discussions)
- 📦 **Crates.io**: [obsidian-forge](https://crates.io/crates/obsidian-forge)

---

## License

Apache 2.0 © 2026 [epicsagas](https://github.com/epicsagas)
