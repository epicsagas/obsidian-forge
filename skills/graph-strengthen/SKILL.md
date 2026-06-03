---
name: graph-strengthen
description: "Knowledge graph strengthening pipeline — runs `of graph health` then `of strengthen-graph` to add backlinks, bridge notes, and auto-tags. Trigger: strengthen graph, graph health, fix orphans, improve connections."
---

# Graph Strengthening Pipeline

Analyze and strengthen the vault's knowledge graph by adding backlinks, bridge notes, and auto-tags.

## Process

### Step 1: Baseline metrics

```bash
of graph health --vault <name>
```

Capture baseline: total files, total links, orphan count, average links per note.

### Step 2: Strengthen the graph

```bash
of strengthen-graph --vault <name>
```

This runs the full pipeline: backlinks, bridge notes, related-project links, auto-tags.

### Step 3: Post-strengthening metrics

```bash
of graph health --vault <name>
```

Compare with baseline.

### Step 4: Report

Report in Korean:

1. **이전 상태** — baseline file count, link count, orphan count
2. **강화 작업** — backlinks added, bridge notes created, tags applied
3. **이후 상태** — new metrics, delta from baseline
4. **남은 고아 노트** — list remaining orphans (if any)

### Optional: Orphan handling

If orphans remain after strengthening:

```bash
of graph orphans --vault <name>
```

For AI-assisted auto-linking of remaining orphans:

```bash
of graph orphans --vault <name> --auto-link
```

### Optional: Agent index generation

Generate `index.md` for AI agent navigation:

```bash
of graph index --vault <name>
```

## Anti-Rationalization

| Excuse | Rebuttal | What to do instead |
|--------|----------|-------------------|
| "Skip baseline, just strengthen" | Cannot measure improvement without baseline | Always run `of graph health` before and after |
| "All orphans will be resolved" | Some are seed files or intentionally disconnected | Report remaining orphans honestly |
| "Run --auto-link automatically" | It modifies content without user review | Get user consent before --auto-link |

## Evidence Required

- [ ] Before/after `of graph health` output (exact numbers)
- [ ] List of bridge notes created (file paths)
- [ ] Count of orphans remaining after strengthening

**No evidence = not done.**

## Red Flags

- Graph health shows 0 links → vault may have no wikilinks at all
- Strengthening adds 0 links → `vault.toml` graph settings may be disabled
- Auto-link fails → AI provider may be misconfigured
- Orphan count > 100 → vault may need structural reorganization first
