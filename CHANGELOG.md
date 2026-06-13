# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.6] - 2026-06-13

### Fixed

- `doctor`: inbox item count now ignores dotfiles (e.g. `.DS_Store`) and system dirs, consistent with the MOC/index scanners

## [0.2.5] - 2026-06-12

### Added

- Claude Code and Codex plugin manifests with skills directory (`.claude-plugin/`, `.codex-plugin/`)
- Agent KB: `index.md` generation and Obsidian Linter config for inbox classification

### Fixed

- Inbox processor: handle CRLF line endings and improve subcategory classification
- All `book-forge` references renamed to `Velith`
- README: missing links in static badges

### Changed

- README: replaced skill install with plugin install, added Antigravity section, synced all translations
- Bump `reqwest` from 0.13.3 to 0.13.4
- Bump `chrono` from 0.4.44 to 0.4.45

## [0.2.4] - 2026-05-25

### Added

- **Vault maintenance automation** (`of vault doctor`, `of vault maintenance`): new subcommands for vault health diagnostics and automated cleanup/repair.

## [0.2.3] - 2026-05-24

### Changed

- Bump `serde_json` from 1.0.149 to 1.0.150.

## [0.2.2] - 2026-05-24

### Added

- Shell and PowerShell one-line installers with SHA-256 verification.
- `cargo-binstall` metadata for pre-built binary support.
- Homebrew formula auto-publish via `publish-homebrew-formula` CI job.

### Fixed

- Removed `aarch64-linux-musl` target due to ports.ubuntu.com outage.
- Homebrew publish job no longer blocks releases.

## [0.2.1] - 2026-05-23

### Changed

- Collapsed nested conditionals and enhanced `status` output readability.

### Fixed

- CI: added `publish-homebrew-formula` job to release workflow.
- Bumped `actions/checkout` from 4 to 6, `actions/upload-artifact` from 4 to 7, `actions/download-artifact` from 7 to 8.

## [0.2.0] - 2026-05-16

### Added

- **Book project management** (`of book`): new subcommand group for vault-integrated writing projects.
  - `of book init <name> [--genre <genre>] [--lang <lang>]` — scaffold a book project under `01-Projects/<name>/` with `drafts/`, `edits/`, `publish/cover/`, `PRD.md`, `STYLE.md`, and a `sources/` symlink to `03-Resources/`.
  - `of book status [<name>]` — tabular view of draft / edit / publish phase completion across all book projects (or one specific project).
  - `of book export <name> [--output <dir>]` — copy a book project to a standalone directory compatible with `Velith`.
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

## [0.1.10] - 2026-05-12

### Added

- One-line installer scripts (shell + PowerShell) with SHA-256 verification.

### Fixed

- Correct archive filename and extraction path in installer.
- Remove unsupported `extra-artifacts` from dist-workspace.toml.

## [0.1.9] - 2026-05-12

### Fixed

- Make daemon `enable` and `start` idempotent for already-loaded LaunchAgents.
- Remove invalid crate `publish-job` value from dist config.

### Changed

- Sync all README translations with current English source.
- Add crate publish job for crates.io.

## [0.1.8] - 2026-05-11

### Added

- `cargo-binstall` metadata for pre-built binary support.

### Changed

- Restructure and standardize README installation section per OSS standards.

## [0.1.7] - 2026-05-11

### Added

- Migrate to cargo-dist v0.31.0 for cross-platform distribution.
- macOS code signing and notarization setup.

### Fixed

- Unify CI workflow: check/test/audit/SBOM jobs, fix cyclonedx flags.
- Change rust-toolchain channel from 1.88 to stable.
- Homebrew publish made non-blocking (`HOMEBREW_TAP_TOKEN` optional).

## [0.1.6] - 2026-05-10

### Added

- **AI suggest-then-move intake pipeline** with policy layer and type mapping validation (#9).
- Ontology seeding and template distribution automation during vault init.
- `daemon restart` and `doctor` commands.
- Enhanced README with settings management and graph operations sections.

### Changed

- CLI: `daemon install/uninstall` renamed to `enable/disable` (old names work as aliases).

### Fixed

- Use `GLOBAL_DIR` constant for `.env` and log paths.

## [0.1.5] - 2026-04-30

### Changed

- Bump `softprops/action-gh-release` from 2 to 3 (#6).

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

[Unreleased]: https://github.com/epicsagas/obsidian-forge/compare/v0.2.5...HEAD
[0.2.5]: https://github.com/epicsagas/obsidian-forge/compare/v0.2.4...v0.2.5
[0.2.4]: https://github.com/epicsagas/obsidian-forge/compare/v0.2.3...v0.2.4
[0.2.3]: https://github.com/epicsagas/obsidian-forge/compare/v0.2.2...v0.2.3
[0.2.2]: https://github.com/epicsagas/obsidian-forge/compare/v0.2.1...v0.2.2
[0.2.1]: https://github.com/epicsagas/obsidian-forge/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/epicsagas/obsidian-forge/compare/v0.1.10...v0.2.0
[0.1.10]: https://github.com/epicsagas/obsidian-forge/compare/v0.1.9...v0.1.10
[0.1.9]: https://github.com/epicsagas/obsidian-forge/compare/v0.1.8...v0.1.9
[0.1.8]: https://github.com/epicsagas/obsidian-forge/compare/v0.1.7...v0.1.8
[0.1.7]: https://github.com/epicsagas/obsidian-forge/compare/v0.1.6...v0.1.7
[0.1.6]: https://github.com/epicsagas/obsidian-forge/compare/v0.1.5...v0.1.6
[0.1.5]: https://github.com/epicsagas/obsidian-forge/compare/v0.1.4...v0.1.5
[0.1.0]: https://github.com/epicsagas/obsidian-forge/releases/tag/v0.1.0
