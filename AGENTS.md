# Leaflog — Agent Instructions

Self-hosted cannabis grow diary packaged as a **Home Assistant app** (sidebar tab via ingress).
Backend and frontend are both **Rust**. Hosted on **GitHub** (CI via GitHub Actions).
Licensed under the **Unlicense** (`UNLICENSE` file at root; SPDX `Unlicense` in Cargo.toml).
Greenfield project; the full spec is the source of truth
for features, seed data, and acceptance criteria.

## Status

Current release and version history: CHANGELOG.md (generated from the
conventional commits, so it cannot drift). 

## Non-negotiables (violating these breaks the product)

- **Local-first**: no telemetry, no cloud calls at runtime. Imports are user-triggered only.
- **Privacy**: no sharing/social/public URLs. Strip EXIF/GPS from uploaded photos.

## Architecture

Record architectural decisions as ADRs in `docs/adr/`, referenced from
`docs/ARCHITECTURE.md`; one-off choices go in `docs/DECISIONS.md`.

## Mandatory lint policy (never weaken)

Every crate root (`lib.rs`/`main.rs`) starts with its `//!` crate doc, then this block verbatim:

```rust
#![warn(clippy::nursery)]
#![deny(
    unused,
    nonstandard_style,
    future_incompatible,
    missing_copy_implementations,
    missing_debug_implementations,
    missing_docs,
    clippy::cargo,
    clippy::complexity,
    clippy::correctness,
    clippy::perf,
    clippy::style,
    clippy::suspicious,
    clippy::pedantic,
    non_fmt_panics
)]
#![allow(clippy::multiple_crate_versions)]
```

- Macro-generated code (Leptos `#[component]`, serde/sqlx derives) gets a narrowly scoped
  `#[allow(...)]` on that item with a one-line reason comment; log every exception in
  `docs/DECISIONS.md`.

## Commands

Automation lives in a `justfile` (or `xtask` crate) — no bash-only scripts.
- `just dev` — Start a dev server
- `just ci` - Tests, lints, etc
- Cutting a release — `docs/RELEASE.md`.

## Dev environment

- **Devbox** (Nix-based): commit `devbox.json`, `devbox.lock`, `rust-toolchain.toml`
  (stable ≥ 1.88). `rustup` as a Devbox package; wasm32 target + clippy/rustfmt via init_hook.
- **GitHub Actions** builds and publishes the multi-arch image, plus a local-build path via
  Podman. CI (`just ci` gate: fmt, clippy native + wasm, tests, WASM build) also runs on
  GitHub Actions.

## Working agreements

- **No vendoring**: manage third-party code as real dependencies (crates.io / git), not
  copies in the repo. Vendoring is not preferred — avoid it, and remove existing vendored
  code when a proper dependency can replace it.

## Agent skills

Most important for the UI:

/impeccable audit

Most important for code:

/ponytail-review

Most important for architecture

/improve-codebase-architecture

### Issue tracker

Issues live in git-bug (`refs/bugs/*`, synced to origin via `git bug pull` /
`git bug push`); reference tickets by shortId. Wayfinder maps stay local under
`.scratch/`. See `docs/agents/issue-tracker.md`.

### Triage labels

Default five-role vocabulary (`needs-triage`, `needs-info`, `ready-for-agent`,
`ready-for-human`, `wontfix`), applied as git-bug labels. See
`docs/agents/triage-labels.md`.

### Domain docs

Single-context: root `GLOSSARY.md` (created lazily) + decisions in
`docs/DECISIONS.md`. See `docs/agents/domain.md`.
