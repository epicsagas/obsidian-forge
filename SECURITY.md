# Security Policy

## Supported Versions

| Version | Supported |
| ------- | --------- |
| 0.1.x   | ✅        |

## Sensitive Data Handling

### API Keys

`obsidian-forge` supports multiple AI providers. API keys are **never** required in vault files.

**Recommended — environment variables (never committed to git):**
```bash
export OPENAI_API_KEY="sk-..."
export OPENROUTER_API_KEY="sk-or-..."
```

**Alternatively — global config (`~/.obsidian-forge/config.toml`):**
```toml
[ai]
api_key = "sk-..."  # This file lives outside any vault repository
```

**Do NOT store API keys in `vault.toml`** if that file is committed to a shared or public git repository.

The lookup order is: `vault.toml` → environment variable. If both are absent, AI features are skipped silently.

### vault.toml

`vault.toml` is created inside each vault directory. If the vault is tracked in git:

- Keep `api_key` commented out (it is by default)
- Verify `.gitignore` includes `.env` (added automatically by `obsidian-forge init`)
- Consider adding `vault.toml` to `.gitignore` if it contains sensitive overrides

### Log Files

Daemon logs are written to `~/.obsidian-forge/logs/forge.log`. They contain file paths and processing summaries but **never** API keys or note content.

## File System Access

`obsidian-forge` only reads and writes within:
- The vault directories you explicitly register
- `~/.obsidian-forge/` (global config and templates)

It does not access the network except for configured AI provider endpoints.

## Reporting a Vulnerability

If you discover a security vulnerability, please **do not** open a public GitHub issue.

Instead, report it via [GitHub Security Advisories](https://github.com/epicsagas/obsidian-forge/security/advisories/new).

Please include:
- A description of the vulnerability
- Steps to reproduce
- Potential impact
- Suggested fix (if any)

You can expect an initial response within **72 hours**.
