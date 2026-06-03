---
name: vault-health
description: "Obsidian vault diagnostics — runs `of doctor` and reports vault status, AI connectivity, inbox state, and git health. Trigger: vault health check, diagnose vault, vault status."
---

# Vault Health Diagnostics

Run `of doctor` to diagnose the current vault's health.

## Process

### Step 1: Determine the vault

- If the user specifies a vault name, use `--vault <name>`.
- If working inside a vault directory (contains `vault.toml`), omit the flag (auto-detect).
- If unsure, run `of vault list` first to show registered vaults.

### Step 2: Run diagnostics

```bash
of doctor --vault <name>
```

Skip the AI connectivity test if the user only wants structural checks:

```bash
of doctor --vault <name> --no-ping
```

### Step 3: Analyze and report

Parse the output. Report in Korean:

1. **볼트 상태** — vault name, path, note count, last sync time
2. **인박스** — inbox directory existence and item count
3. **AI 연결** — provider status (if checked)
4. **Git 상태** — branch, uncommitted changes
5. **설정** — config file path, daemon status (macOS)

### Step 4: Recommend actions

Based on findings:
- Inbox has items → suggest `inbox-process` skill
- Graph health unknown → suggest `graph-strengthen` skill
- Uncommitted changes → suggest `vault-sync` skill
- Tag/link issues detected → suggest `vault-fix` skill

## Anti-Rationalization

| Excuse | Rebuttal | What to do instead |
|--------|----------|-------------------|
| "I know the vault state already" | Vault state changes between sessions | Run `of doctor` every time |
| "AI provider is definitely fine" | Config drift, key rotation happen | Check actual output, don't assume |
| "Everything looks fine" | No evidence = not checked | Show concrete metrics from command output |

## Evidence Required

- [ ] Raw command output from `of doctor`
- [ ] Specific file paths for any issues found
- [ ] Concrete counts (notes, inbox items, uncommitted changes)

**No evidence = not done.**

## Red Flags

- `of doctor` fails entirely → vault may not be registered or `vault.toml` is missing
- AI connectivity failure → check `~/.obsidian-forge/.env` for API keys
- Git not initialized → vault was created without git, needs `of init` repair
- Inbox count > 50 → backlog needs triage before sync
