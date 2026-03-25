# Contributing to obsidian-forge

Thank you for your interest in contributing! This document covers how to get started, the project conventions, and the PR process.

## Getting Started

```bash
git clone https://github.com/epicsagas/obsidian-forge.git
cd obsidian-forge
cargo build
cargo test
```

### Recommended tools

- [rust-analyzer](https://rust-analyzer.github.io/) — IDE support
- [cargo-watch](https://github.com/watchexec/cargo-watch) — auto-rebuild on save: `cargo watch -x check`

## Project Structure

| Module | Responsibility |
|---|---|
| `main.rs` | CLI parsing (clap), command dispatch |
| `config.rs` | Config structs, load/save, vault resolution |
| `init.rs` | Vault scaffolding, settings import/push |
| `moc.rs` | MOC hub file generation |
| `graph.rs` | Backlinks, bridge notes, auto-tags |
| `git.rs` | Auto commit/push logic |
| `notes.rs` | Inbox processing and PARA routing |
| `converter.rs` | PDF → Markdown conversion |
| `ollama.rs` | Ollama HTTP client |
| `prompts.rs` | LLM prompt templates |
| `watcher.rs` | Filesystem watcher |

## Conventions

### Commit messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
feat: add new command
fix: resolve crash on empty vault
docs: update README
refactor: extract vault resolution helper
```

### Code style

- Run `cargo fmt` before committing
- Run `cargo clippy -- -D warnings` and fix all warnings
- All modules accept `&ForgeConfig` — never read config internally
- Operations must be idempotent (running twice produces no extra output)
- If Ollama or git is unavailable, fail gracefully and log a warning

### Error handling

- Use `anyhow::Result` throughout
- Prefer `?` over `.unwrap()` / `.expect()` in library-style code
- Use `tracing::warn!` for recoverable errors, `tracing::error!` for unexpected failures

## Pull Request Process

1. Fork the repo and create a branch: `git checkout -b feat/my-feature`
2. Make your changes, add tests where applicable
3. Run `cargo fmt && cargo clippy -- -D warnings && cargo test`
4. Push and open a PR against `main`
5. Describe what changed and why in the PR body

## Reporting Bugs

Open an issue at [github.com/epicsagas/obsidian-forge/issues](https://github.com/epicsagas/obsidian-forge/issues) with:

- OS and Rust version (`rustc --version`)
- The command you ran
- Expected vs actual behavior
- Relevant log output (`~/.obsidian-forge/logs/obsidian-forge/forge.log`)

## Feature Requests

Open an issue with the `enhancement` label. Please describe the use case — not just the solution.

## License

By contributing, you agree that your contributions will be licensed under the [Apache 2.0 License](LICENSE).
