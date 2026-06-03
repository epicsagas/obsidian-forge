---
name: vault-fix
description: "Batch vault repair — runs `of check-tags --fix`, `of check-links --fix`, and `of normalize-frontmatter --fix` to repair tag health, broken wikilinks, and YAML malformations. Trigger: fix vault, repair tags, fix links, fix frontmatter, vault maintenance."
---

# Batch Vault Repair

Run all three repair commands sequentially to fix tag health, broken wikilinks, and YAML frontmatter malformations.

## Process

### Step 1: Pre-fix audit (dry run)

Run each command without `--fix` first to assess scope:

```bash
of check-tags --vault <name>
of check-links --vault <name>
of normalize-frontmatter --vault <name>
```

### Step 2: Report findings to user

Present the dry-run results in Korean:

1. **태그 이슈** — missing layer/type/project tags, tag count violations
2. **링크 이슈** — broken wikilinks, filename mismatches
3. **프론매터 이슈** — YAML malformations, missing fields

Ask user to confirm before applying fixes (unless user explicitly said "just fix everything").

### Step 3: Apply fixes

Run all three with `--fix`:

```bash
of check-tags --fix --vault <name>
of check-links --fix --vault <name>
of normalize-frontmatter --fix --vault <name>
```

### Step 4: Post-fix verification

Re-run without `--fix` to confirm remaining issues:

```bash
of check-tags --vault <name>
of check-links --vault <name>
of normalize-frontmatter --vault <name>
```

### Step 5: Report results

Report in Korean:

1. **수정된 태그** — count and examples of tags fixed
2. **수정된 링크** — count and file paths of links repaired
3. **수정된 프론매터** — count and types of frontmatter fixes
4. **남은 이슈** — issues that could not be auto-fixed
5. **사용자 확인** — ask if they want to commit changes

### Optional: Scope control

- Tag scope: `--scope project` (project docs only) or `--scope vault` (includes Resources, default)
- Individual fixes: user may request only one type (tags, links, or frontmatter only)

## Anti-Rationalization

| Excuse | Rebuttal | What to do instead |
|--------|----------|-------------------|
| "Just apply fixes without dry run" | Some fixes need manual review | Always show dry-run results first |
| "All issues can be auto-fixed" | Some require manual intervention | Report remaining issues honestly |
| "Skip post-fix verification" | Fixes can introduce new issues | Always verify after fixing |

## Evidence Required

- [ ] Dry-run output showing issue counts and categories
- [ ] Post-fix output showing what was resolved
- [ ] Specific file paths for any remaining manual-fix items

**No evidence = not done.**

## Red Flags

- Dry run finds 0 issues → vault is clean, inform user
- Fix count > 200 → may indicate systemic config problem, review before committing
- Post-fix shows new issues → fixes may have introduced regressions
- check-links --fix renames files → may break existing wikilinks from other notes (verify with another check-links pass)
