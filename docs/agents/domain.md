# Domain Docs

How the engineering skills should consume this repo's domain documentation when exploring the codebase.

## Before exploring, read these

- **`GLOSSARY.md`** at the repo root — created lazily by `/domain-modeling`; its absence is fine.
- **`docs/DECISIONS.md`** — this repo's decision log (numbered `D0xx` entries). Read entries that touch the area you're about to work in.
- **`docs/SPEC.md`** is the source of truth for features and seed data; **`docs/ARCHITECTURE.md`** for structure.

If any of these don't exist, **proceed silently** — don't flag their absence or suggest creating them upfront. The `/domain-modeling` skill (reached via `/grill-with-docs` and `/improve-codebase-architecture`) creates the glossary lazily when terms actually get resolved.

This repo is **single-context**: one root `GLOSSARY.md`; all decisions in `docs/DECISIONS.md` (no `docs/adr/`).

## Use the glossary's vocabulary

When your output names a domain concept (in an issue title, a refactor proposal, a hypothesis, a test name), use the term as defined in `GLOSSARY.md`. Don't drift to synonyms the glossary explicitly avoids.

If the concept you need isn't in the glossary yet, that's a signal: either you're inventing language the project doesn't use (reconsider) or there's a real gap (note it for `/domain-modeling`).

## Flag decision conflicts

If your output contradicts an existing decision, surface it explicitly rather than silently overriding — cite the `D0xx` number:

> _Contradicts D065 (prose lint via vale), but worth reopening because…_
