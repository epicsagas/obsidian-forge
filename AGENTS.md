# CLAUDE.md

Guidance for Claude Code when working in this repository.

## What This Is

`obsidian-forge` is a standalone Rust CLI tool that creates, automates, and manages Obsidian vaults. It supports multiple vaults from a single binary/daemon.

## Build & Run

```bash
cargo build --release
./target/release/obsidian-forge --help
```

No external services required for core features. Ollama needed only for AI metadata (`process-all`).

## Architecture

Single binary, 11 modules:

| Module | Purpose |
|--------|---------|
| `main.rs` | CLI (clap), multi-vault dispatch, sync loop |
| `config.rs` | `vault.toml` (per-vault) + `~/.config/obsidian-forge/config.toml` (global) |
| `init.rs` | Vault scaffolding (PARA dirs, templates, .obsidian, git) |
| `moc.rs` | Auto-generate `{project}/{project}.md` hub files |
| `graph.rs` | Graph strengthening: backlinks, bridge notes, related projects, auto-tags |
| `git.rs` | Auto commit + push with conventional commit messages |
| `notes.rs` | Inbox processing: frontmatter, classification, PARA move |
| `converter.rs` | PDF → Markdown (marker_single / pdftotext fallback) |
| `ollama.rs` | Ollama client with robust JSON extraction |
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
