---
name: obsidian-forge
description: obsidian-forge (alias `of`) CLI skill for creating and managing Obsidian vaults, processing inbox notes with AI classification, strengthening knowledge graphs, syncing plugins across vaults, and running the background daemon. Use when initializing vaults, automating PARA routing, managing MOCs, or debugging vault sync and graph operations.
---

# obsidian-forge CLI (`of`)

## Quick Start

```bash
# New vault
of init my-vault --path ~/Documents
of vault add ~/Documents/my-vault
of daemon install   # macOS: auto-start background watcher

# Check state
of vault list
of daemon status
```

## Core Patterns

### Diagnose before acting
Always run these first when user reports a problem:
```bash
of vault list           # registered? enabled? watched?
of settings status      # .obsidian store populated?
of daemon status        # daemon running?
RUST_LOG=debug of sync  # verbose output for errors
```

### Note processing (AI required)
```bash
of process-all [--vault <name>]
```
Reads `Inbox/`, calls AI provider, injects frontmatter, moves file to PARA folder.
AI provider must be configured in `vault.toml [ai]`. If it fails, read the error — unknown provider or missing `base_url` will have a clear message.

### Graph + MOC (no AI needed)
```bash
of update-mocs [--vault <name>]      # rebuild hub files
of strengthen-graph [--vault <name>] # backlinks, bridge notes, auto-tags
of sync [--vault <name>]             # MOC + graph + git in one shot
```
All idempotent — safe to re-run.

### Multi-vault plugin sync
```bash
of settings import <vault>   # vault → global store
of settings push-all         # global store → all vaults
```

### Vault lifecycle
```bash
of vault disable/enable <name>  # exclude/include from sync + watch
of vault pause/resume <name>    # daemon toggle only; manual sync still works
of vault remove <name>          # unregister (files kept)
```

## Key Facts

- Config hierarchy: `vault.toml` (per-vault) overrides nothing — it IS the source of truth
- PARA folders: `Inbox/` → `01-Projects/` `02-Areas/` `03-Resources/` `99-Archives/`
- Daemon logs: `~/Library/Logs/of/forge.log`
- `graph.*` bools in `vault.toml` gate each graph operation individually
