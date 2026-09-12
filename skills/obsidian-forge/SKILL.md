---
name: obsidian-forge
description: obsidian-forge (alias `of`) CLI skill for creating and managing Obsidian vaults, processing inbox notes with AI classification, running vault integrity checks, and managing the background daemon. Use when initializing vaults, automating PARA routing, or debugging vault sync issues.
---

# obsidian-forge CLI (`of`)

## Quick Start

```bash
# New vault (also registers it with the global config)
of init my-vault --path ~/Documents
of daemon enable    # macOS: auto-start background watcher

# Check state
of doctor --no-ping
of daemon status
```

## Core Patterns

### Diagnose before acting
Always run these first when user reports a problem:
```bash
of doctor --no-ping    # vault config, inbox, git, AI connectivity
of daemon status       # daemon running?
RUST_LOG=debug of sync # verbose output for errors
```

### Note processing (AI required)
```bash
of process-all [--vault <name>]
```
Reads `00-Inbox/`, calls AI provider, injects frontmatter, moves file to PARA folder.
AI provider must be configured in `vault.toml [ai]`. If it fails, read the error — unknown provider or missing `base_url` will have a clear message.

### Vault integrity (mechanical, no AI needed)
```bash
of check-tags [--fix]            [--vault <name>]  # missing layer/type/project tags
of check-links [--fix]           [--vault <name>]  # broken wikilinks (code-fence aware)
of normalize-frontmatter [--fix] [--vault <name>]  # YAML malformations
of graph health                  [--vault <name>]  # note/link/orphan/broken metrics
of sync [--vault <name>]                           # graph health check → git in one shot
```
All idempotent — safe to re-run.

## Key Facts

- Config hierarchy: `vault.toml` (per-vault) overrides nothing — it IS the source of truth
- PARA folders: `00-Inbox/` → `01-Projects/` `02-Areas/` `03-Resources/` `99-Archives/`
- Daemon logs: `~/.obsidian-forge/logs/forge.log`
- Vault registration: `of init` (safe to re-run on an existing vault); per-vault `enabled`/`watch` flags live in `~/.config/obsidian-forge/config.toml`
- Graph semantics (bridge notes, auto-tags, auto-MOCs) were retired in 0.4.0 — `of` is mechanical maintenance only; knowledge-graph extraction belongs to the LLM pipeline (knowledge-os)
