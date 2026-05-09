# Ontology Tagging Standard

## 1. Hierarchical Prefixing (Mandatory)
AI must use the following prefixes for all generated tags:
- `topics/`: Subject matter (e.g., `topics/rust`, `topics/ai/llm`).
- `status/`: Maturity (e.g., `status/seed`, `status/evergreen`).
- `type/`: Document role (e.g., `type/prd`, `type/report`).

## 2. Constraints
- **Max 7 tags** per file.
- **No spaces**: Use hyphens (e.g., `machine-learning`).
- **No duplicates**: Check existing tags in `src/templates/TAGGING.md`.
- **Project Tag First**: The first tag for project docs must match the project name.

## 3. Layer Architecture
- `layer/raw`: For notes in Projects, Areas, Resources.
- `layer/wiki`: For Zettelkasten notes.
