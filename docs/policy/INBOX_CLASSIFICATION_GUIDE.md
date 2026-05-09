# Inbox Classification Guide

This guide defines how AI should classify new notes arriving in `00-Inbox`.

## 1. Classification Axis: PARA + Concept

| Type | Target Folder | Criteria |
|------|---------------|----------|
| **Project** | `01-Projects/` | Active endeavors with a deadline/goal. |
| **Area** | `02-Areas/` | Ongoing responsibilities with high standards (e.g., Health, Finances). |
| **Resource** | `03-Resources/` | Topics of interest or reference material. |
| **Concept Seed** | `10-Zettelkasten/fleeting` | Raw ideas or atomic thoughts for the wiki. |

## 2. Decision Logic
1. **Does it belong to a project?** Check existing folder names in `01-Projects/`.
2. **Is it a general resource?** Check if it's a tutorial, paper, or tool.
3. **Is it a recurring theme?** Check `02-Areas/`.
4. **Is it an atomic idea?** If it's a short insight or "spark," mark as `Concept Seed`.

## 3. Metadata Requirements
Every classified note should have:
- `candidate_type`: The PARA/Concept category.
- `candidate_project`: Related project name (if any).
- `related_concepts`: Links to existing Zettel notes.
- `recommended_action`: `move`, `merge`, or `delete`.
