## What This Is

`obsidian-forge` (`of`) is a Rust CLI daemon for Obsidian vault lifecycle management. It is the **companion project to alcove** in the AI agent documentation ecosystem — alcove provides read-access (MCP server), obsidian-forge provides write-automation (daemon).

## Build & Run

```bash
cargo build --release
./target/release/obsidian-forge --help
```

No external services required for core features. Ollama needed only for AI metadata (`process-all`).

## Architecture

Single binary, 11 top-level modules + graph/ subdirectory:

| Module | Purpose |
|--------|---------|
| `main.rs` | CLI (clap), multi-vault dispatch, sync loop |
| `config.rs` | `vault.toml` (per-vault) + `~/.obsidian-forge/config.toml` (global) |
| `init.rs` | Vault scaffolding (PARA dirs, templates, .obsidian, git) |
| `moc.rs` | Auto-generate `{project}/{project}.md` hub files |
| `graph/` | Graph strengthening pipeline (9 submodules: tags, wikilinks, relationships, bridges, orphans, autotag, scan, health, backlinks) |
| `git.rs` | Auto commit + push with conventional commit messages |
| `notes.rs` | Inbox processing: frontmatter, classification, PARA move |
| `converter.rs` | PDF → Markdown (marker_single / pdftotext fallback) |
| `ai.rs` | Unified AI client (Ollama, OpenAI, OpenRouter, LM Studio, OpenAI-compatible) |
| `prompts.rs` | LLM prompt templates (bundled defaults + YAML override) |
| `watcher.rs` | Filesystem watcher for Inbox (notify crate) |

## Config Hierarchy

```
~/.config/obsidian-forge/config.toml   # Global: which vaults to manage
~/my-vault/vault.toml                  # Per-vault: layout, graph, sync, AI settings
```

## Key Design Decisions

- **Config over code**: All vault-specific values come from `vault.toml`, never hardcoded
- **Idempotent operations**: Running MOC/graph/tag operations multiple times produces no extra changes
- **Single daemon**: `watch` command manages all registered vaults in one process via tokio tasks
- **Graceful degradation**: If Ollama unavailable, AI features skip silently; if git push fails, retry next cycle

## Conventions

- Commit messages: Conventional Commits (`feat:`, `fix:`, `refactor:`, `docs:`)
- All modules accept `&ForgeConfig` — never read config internally
- Templates bundled via `include_str!` — no runtime file dependencies
- Graph strengthening checks `config.graph.*` bools before each operation
