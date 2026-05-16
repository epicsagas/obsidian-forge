# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] - 2026-05-16

### Added

- **Book project management** (`of book`): new subcommand group for vault-integrated writing projects.
  - `of book init <name> [--genre <genre>] [--lang <lang>]` — scaffold a book project under `01-Projects/<name>/` with `drafts/`, `edits/`, `publish/cover/`, `PRD.md`, `STYLE.md`, and a `sources/` symlink to `03-Resources/`.
  - `of book status [<name>]` — tabular view of draft / edit / publish phase completion across all book projects (or one specific project).
  - `of book export <name> [--output <dir>]` — copy a book project to a standalone directory compatible with `book-forge`.
  - `of book sync <name>` — walk the vault and link notes tagged `book/<name>` into `sources/` as symlinks.
- `BookConfig` struct with `book_dir` option (default: `01-Projects`).
- Two new bundled templates: `Book-Project-MOC.md` and `Book-Chapter-Draft.md`.
- Automated seeding of `AGENTS.md` and `TAGGING.md` during vault initialization.
- New standard project templates: `PRD.md`, `ARCHITECTURE.md`, `DECISIONS.md`, `CONVENTIONS.md`, `PROGRESS.md`, `DEBT.md`, `SECRETS_MAP.md`.
- Comprehensive ontology concepts in default `vault.toml` template (Ontology, Search Quality, AI Agent, etc.).

### Changed

- CLI: `daemon install` renamed to `daemon enable` (old name works as deprecated alias).
- CLI: `daemon uninstall` renamed to `daemon disable` (old name works as deprecated alias).
- Updated base templates (`MOC.md`, `ZK-Note.md`, `Project-Note.md`) with mandatory Karpathy 3-Layer metadata and hierarchical tags.
- Enabled `ai_relationships`, `tag_hierarchy`, and `orphan_detection` by default in new vaults.

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

[Unreleased]: https://github.com/epicsagas/obsidian-forge/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/epicsagas/obsidian-forge/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/epicsagas/obsidian-forge/releases/tag/v0.1.0
