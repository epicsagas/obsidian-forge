---
name: vault-sync
description: "Full vault sync cycle — runs `of sync` for a graph health check and git commit/push. Trigger: sync vault, graph health check, commit vault changes."
---

# Vault Sync Cycle

Run `of sync` to execute the sync cycle: graph health check → git commit/push.

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

1. **그래프 상태** — note/link/orphan/broken-link counts from the health check
2. **Git 커밋** — commit hash, message, timestamp (or "no changes")
3. **소요 시간** — total time taken

### Step 4: Flag issues

- Sync failed mid-pipeline → report which stage failed and the error
- No changes detected → inform user vault is already up to date
- Git push failed → check remote connectivity

## Anti-Rationalization

| Excuse | Rebuttal | What to do instead |
|--------|----------|-------------------|
| "Skip the pre-sync check, just run it" | Silent failures waste user time | Always run `of doctor --no-ping` first |
| "Sync is idempotent so it must have worked" | Exit code 1 means something failed | Check exit code, not assumptions |
| "It succeeded" without evidence | Partial completion looks like success | Show health counts and git hash |

## Evidence Required

- [ ] Exit code from `of sync`
- [ ] Git log showing the new commit (or absence thereof)
- [ ] Health metrics from the sync log (notes, links, orphans, broken)

**No evidence = not done.**

## Red Flags

- Sync hangs → daemon may be running and holding a lock; check `of daemon status`
- Broken-link count spikes → a recent edit may have introduced bad wikilinks; run `of check-links`
- Git commit fails → check for merge conflicts or detached HEAD
