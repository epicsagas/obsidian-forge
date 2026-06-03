---
name: inbox-process
description: "AI inbox classification pipeline — runs `of process-all` to classify inbox notes with AI, inject frontmatter, and route to PARA folders. Trigger: process inbox, classify notes, empty inbox, PARA routing."
---

# Inbox Processing Pipeline

Process all notes in the vault's Inbox directory using AI classification, frontmatter injection, and PARA routing.

## Process

### Step 1: Check prerequisites

Verify AI provider is configured:

```bash
of doctor --vault <name>
```

Check the AI section of the output. If AI provider is not configured, the user needs to set up `[ai]` in `vault.toml` and add API keys to `~/.obsidian-forge/.env`.

### Step 2: Count inbox items

```bash
of doctor --vault <name>
```

Note the inbox item count from the output.

### Step 3: Process inbox

```bash
of process-all --vault <name>
```

This reads all files from the Inbox directory, calls the configured AI provider for classification, injects frontmatter, and moves each file to the appropriate PARA folder.

### Step 4: Verify and report

```bash
of doctor --vault <name>
```

Check that inbox count is now 0 (or reduced).

Report in Korean:

1. **처리 전 인박스** — number of items before processing
2. **분류 결과** — how many went to each PARA folder
3. **처리 후 인박스** — remaining items (failures or partial)
4. **실패 항목** — any notes that could not be classified (with reasons)

## Anti-Rationalization

| Excuse | Rebuttal | What to do instead |
|--------|----------|-------------------|
| "AI provider should be fine" | Config drift, key rotation happen silently | Verify with `of doctor` before processing |
| "100% success is guaranteed" | API limits, malformed notes can cause failures | Check before/after inbox counts |
| "Just re-run if some fail" | Repeated failures indicate systemic issue | Investigate failures before re-running |

## Evidence Required

- [ ] AI provider status from `of doctor`
- [ ] Before/after inbox item counts
- [ ] Specific file paths for any failures
- [ ] PARA folder destination for each processed note

**No evidence = not done.**

## Red Flags

- AI provider error → check `~/.obsidian-forge/.env` and `[ai]` config in `vault.toml`
- All notes fail → API key may be invalid or provider endpoint is down
- Notes routed to wrong folders → AI model may need prompt tuning
- Inbox is empty before processing → nothing to do, inform user
