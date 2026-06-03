---
name: vault-sync
description: "Full vault sync cycle — runs `of sync` to rebuild MOCs, strengthen graph, and commit to git. Trigger: sync vault, update mocs and graph, commit vault changes."
---

# Vault Sync Cycle

Run `of sync` to execute the full sync pipeline: MOC rebuild → graph strengthening → git commit/push.

## Process

### Step 1: Pre-sync check

Run a quick health check first:

```bash
of doctor --vault <name> --no-ping
```

Verify:
- Vault is registered and enabled
- No critical configuration errors

### Step 2: Execute sync

```bash
of sync --vault <name>
```

For verbose output (debugging):

```bash
RUST_LOG=debug of sync --vault <name>
```

### Step 3: Verify and report

After sync completes, check git status:

```bash
git -C <vault-path> log -1 --format="%h %s (%cs)"
```

Report in Korean:

1. **MOC 업데이트** — which hub files were regenerated
2. **그래프 강화** — backlinks added, bridge notes created, tags applied
3. **Git 커밋** — commit hash, message, timestamp
4. **소요 시간** — total time taken

### Step 4: Flag issues

- Sync failed mid-pipeline → report which stage failed and the error
- No changes detected → inform user vault is already up to date
- Git push failed → check remote connectivity

## Anti-Rationalization

| Excuse | Rebuttal | What to do instead |
|--------|----------|-------------------|
| "Skip the pre-sync check, just run it" | Silent failures waste user time | Always run `of doctor --no-ping` first |
| "Sync is idempotent so it must have worked" | Exit code 1 means something failed | Check exit code, not assumptions |
| "It succeeded" without evidence | Partial completion looks like success | Show MOC count, graph ops, git hash |

## Evidence Required

- [ ] Exit code from `of sync`
- [ ] Git log showing the new commit (or absence thereof)
- [ ] Specific counts of MOCs updated, graph operations performed

**No evidence = not done.**

## Red Flags

- Sync hangs → daemon may be running and holding a lock; check `of daemon status`
- MOC rebuild produces empty files → `vault.toml` MOC paths may be misconfigured
- Graph strengthening finds 0 new links → vault may need more cross-linked content
- Git commit fails → check for merge conflicts or detached HEAD
