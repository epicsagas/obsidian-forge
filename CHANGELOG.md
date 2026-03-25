# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-03-25

### Added

**Core vault management**
- `init` — scaffold a new or adopt an existing directory as a PARA-layout Obsidian vault
- `vault add / remove / list / enable / disable / pause / resume` — manage multiple vaults from one binary
- `clone-settings` / `settings import|push|push-all|status` — share `.obsidian/` plugins, themes, and snippets across vaults via a global settings store

**Automation**
- `process-all` — batch-process Inbox notes: AI summarisation, tagging, classification, and PARA filing
- `watch` — filesystem daemon that processes new notes and PDFs in real time
- `update-mocs` — regenerate project hub files (Maps of Content) from folder structure
- `strengthen-graph` — inject backlinks, create bridge notes, update related-project sections, and auto-tag documents
- `sync` — run a full MOC → graph → git cycle across all enabled vaults

**AI providers**
- Ollama (local subprocess)
- OpenAI (`/v1/chat/completions`)
- OpenRouter
- LM Studio
- Any OpenAI-compatible endpoint via `base_url`

**PDF support**
- Automatic PDF → Markdown conversion via `marker_single` (primary) or `pdftotext` (fallback)
- Converted PDFs are archived to `99-Archives/PDF-Archive/`

**Configuration**
- Two-level config hierarchy: `~/.obsidian-forge/config.toml` (global defaults) + `vault.toml` (per-vault overrides)
- Sane defaults for all fields; minimal required configuration
- Environment variables `OPENAI_API_KEY` / `OPENROUTER_API_KEY` for secure API key handling

**Daemon (macOS)**
- `daemon install|uninstall|start|stop|status` — manage a LaunchAgent that runs the watch loop automatically at login

**Templates**
- 12 bundled Obsidian templates (Daily Note, ZK Note, MOC, Meeting Note, etc.) installed to a shared global store and symlinked into each vault

### Technical

- Single binary (`obsidian-forge` / `of` alias), no external runtime dependencies
- Async runtime: Tokio with selective feature flags
- Parallel graph and MOC operations via Rayon
- Concurrent AI processing with configurable `max_concurrent` limit
- Conventional Commits for all auto-generated vault commits
- Apache-2.0 license

[Unreleased]: https://github.com/epicsagas/obsidian-forge/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/epicsagas/obsidian-forge/releases/tag/v0.1.0
