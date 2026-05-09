# AI Design Principles: "The Assistant, Not the Decider"

This document defines the core philosophy and constraints for AI integration within `obsidian-forge`.

## 1. Core Mandate
> **"AI organizes raw data, humans finalize the ontology."**

AI is a **Classifier, Extractor, and Assistant**, not an autonomous creator or a decision-maker for the system's structure.

## 2. Roles of AI

### A. Inbox Intake (Classification)
- **Role:** Propose classification (Project, Area, Resource, Concept Seed).
- **Output:** Summary, candidate type, related project/concept links.
- **Principle:** AI suggests metadata in frontmatter; it does **not** move files without a secondary rule-based confirmation or human approval.

### B. Resource Extraction
- **Role:** Extract key claims and generalizable principles from external resources (03-Resources).
- **Output:** Claims, reusable principles, identification of relevant Zettels.
- **Principle:** Resources are preserved as truth; AI only performs extraction.

### C. Concept Promotion Review
- **Role:** Judge if a note (report, spec, etc.) is worthy of being promoted to a Permanent Note (Zettel).
- **Output:** Promotion recommendation (Yes/No), reasoning, target name, deduplication check.
- **Principle:** AI acts as a "Reviewer." New Zettel creation is restricted and merge-heavy.

## 3. The "Never" List (Constraints)
AI MUST NOT:
- Overwrite "Source of Truth" documents (PRD, Architecture, Decisions) without approval.
- Arbitrarily decide project document locations.
- Create mass Zettels or infinite tags.
- Force naming conventions that conflict with the existing ontology.

## 4. Automation Rules
- **Rule A (Inbox):** Auto-classify, manual confirm.
- **Rule B (Resource):** Auto-summarize, manual promote.
- **Rule C (Project Docs):** Assistant role only (drafts/links).
- **Rule D (Zettelkasten):** "Merge" priority over "Add." Check existing concepts before creating new ones.

## 5. Policy Layer (Source of Truth for AI)
AI in `obsidian-forge` must use the following documents as its policy layer:
- `policy/INBOX_CLASSIFICATION_GUIDE.md`: Standards for classification.
- `policy/RESOURCE_TO_ZETTEL_WORKFLOW.md`: Standards for extraction.
- `policy/ZETTEL_PROMOTION_DASHBOARD.md`: List of candidates for review.
- `policy/ONTOLOGY_TAGGING_STANDARD.md`: Constraints on tag generation.
- `policy/ZETTEL_TEMPLATE.md`: Output format for promoted notes.
