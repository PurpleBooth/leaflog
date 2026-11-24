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

- **Rust only** for app code. No Node runtime in the final image (build-time
  tooling for CSS and the chart bundle is fine — Tailwind CLI, npm + esbuild).
- **Local-first**: no telemetry, no cloud calls at runtime. Imports are user-triggered only.
- **Ingress-correct** (app is served under `/api/hassio_ingress/<token>/`):
  - Relative URLs everywhere. Never `/static/...`, never redirect to `/login`.
  - Read `X-Ingress-Path`; inject into `index.html` at runtime (`<base href>` + JS global).
    Never bake a base path into the build; build with a relative public URL.
  - Accept connections only from `172.30.32.2` in production. No login screen (HA authenticates).
  - Set an explicit Axum `DefaultBodyLimit` (default is 2 MB) for photo uploads; use `ingress_stream`.
- **Supervisor token is admin-level**: never send to browser, never log, read-only calls by
  default. Any `call_service` is gated behind an explicit opt-in setting.
- **Privacy**: no sharing/social/public URLs. Strip EXIF/GPS from uploaded photos.
- **Units**: store SI internally; display per HA unit system and locale (°C/°F, in/cm, gal/L).
  EC is canonical in µS/cm; TDS (ppm) is always derived via the user's meter scale
  (0.5 / 0.64 / 0.7) and labelled.
- `panel_admin: true`. Grow data is private.

## Architecture

Cargo workspace with:
- `leaflog-core` — shared domain types + pure engines (forecast, feed-fit, VPD, lamp-height,
  recommendations). No I/O; unit-testable; used by both backend and frontend.
- Backend — Axum + Tokio + SQLite (sqlx or rusqlite) in `/data`, versioned migrations.
  Only the backend talks to HA (`SUPERVISOR_TOKEN` as Bearer; REST
  `http://supervisor/core/api/`, WebSocket `ws://supervisor/core/websocket`; documented
  endpoints only — isolate undocumented ones behind a trait with fallback). Must work fully
  with HA unreachable and reconnect with backoff.
- Frontend — Leptos **0.8.x, CSR mode**, built with Trunk. No SSR/hydration. Charts render
  with the ECharts SVG renderer, driven from Rust (js_sys/serde-wasm-bindgen builds the
  option payload); all domain logic stays in Rust. The ECharts bundle is a build-time
  artifact produced by npm + esbuild — no CDN, and Node never enters the image
  (docs/adr/0001-echarts-rendering.md, D088).
- `mock-HA` — Axum app speaking the WS protocol with fake sensor data for `just dev`.
- Browser ↔ backend: JSON REST + one WebSocket/SSE channel. The browser never talks to HA.

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
- `just dev` — backend against mock HA + `trunk serve`; simulates ingress (random path
  prefix + `X-Ingress-Path`).
- `just ci` — fmt, clippy (native + wasm, `-D warnings`), unit/integration tests, WASM build,
  prose lint (`just lint-prose`, vale: no LLM-sounding vocabulary or wall-of-text sentences in
  markdown and Rust comments; config in `.vale.ini`, D065, D117).
- Cutting a release — `docs/RELEASE.md`. Push a signed `v*` tag on the version commit and
  let the Release workflow own the changelog commits; never hand-commit `just changelog`
  output (D062).

## Dev environment

- **Devbox** (Nix-based): commit `devbox.json`, `devbox.lock`, `rust-toolchain.toml`
  (stable ≥ 1.88). `rustup` as a Devbox package; wasm32 target + clippy/rustfmt via init_hook.
- **GitHub Actions** builds and publishes the multi-arch image, plus a local-build path via
  Podman. CI (`just ci` gate: fmt, clippy native + wasm, tests, WASM build) also runs on
  GitHub Actions.

## Seed-data & grow-science discipline

- Cite a source for every default number in preset files. Where sources disagree, store the
  **range** and mark it. Re-check each number against ≥2 sources; if impossible, mark `unverified`.
- Presets are ranges, never single magic numbers. All user-editable with reset-to-default.
- Every preset row carries a source label. The feed and substrate presets are the only
  bundled data left (D075 removed the bundled strain and equipment records).
- The code is Unlicense, but **bundled preset data is third-party**: store source and
  licence per record and check licences before bundling anything.

## Working agreements

- **No vendoring**: manage third-party code as real dependencies (crates.io / git), not
  copies in the repo. Vendoring is not preferred — avoid it, and remove existing vendored
  code when a proper dependency can replace it. Log any unavoidable exception (and its
  removal plan) in `docs/DECISIONS.md`. Bundled seed data is data, not vendored code.
- Pick the simplest option when ambiguous, log it in `docs/DECISIONS.md`, keep going.
  Ask only about blocking choices.
- Re-check the HA developer docs before each milestone

## Agent skills

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
