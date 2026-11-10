# Leaflog — Agent Instructions

Self-hosted cannabis grow diary packaged as a **Home Assistant app** (sidebar tab via ingress).
Backend and frontend are both **Rust**. Hosted on **GitHub** (CI via GitHub Actions).
Licensed under the **Unlicense** (`UNLICENSE` file at root; SPDX `Unlicense` in Cargo.toml).
Greenfield project; the full spec is the source of truth
for features, seed data, and acceptance criteria (research snapshot 2026-11-08).

## Status

v0.1.0 released (tagged 2026-11-08); M1-M8 complete, all docs/SPEC.md
§13 acceptance criteria verified. Post-release hardening: keep commits
lint-clean; open items R1-R9 in docs/THREAT-MODEL.md, one-off choices
in docs/DECISIONS.md.

## Non-negotiables (violating these breaks the product)

- **Rust only** for app code. No Node runtime in the final image (build-time CSS tooling is fine).
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
- Frontend — Leptos **0.8.x, CSR mode**, built with Trunk. No SSR/hydration. Charts as SVG
  rendered from Rust.
- `mock-HA` — Axum app speaking the WS protocol with fake sensor data for `just dev`.
- Browser ↔ backend: JSON REST + one WebSocket/SSE channel. The browser never talks to HA.

Record architectural decisions in `docs/ARCHITECTURE.md` and one-off choices in
`docs/DECISIONS.md`.

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
- Handle friction properly, don't suppress: document all public items (`missing_docs`),
  derive `Debug`/`Copy`, expect pedantic lints like `needless_pass_by_value` on Leptos props.
- `clippy::cargo` requires complete package metadata in every `Cargo.toml` (`description`,
  `license`, `repository`, `readme`, `keywords`, `categories`) — inherit via
  `[workspace.package]`. Use SPDX `Unlicense` for `license`.
- CI gate: `cargo clippy --workspace --all-targets -- -D warnings` for native **and**
  `wasm32-unknown-unknown` (frontend crate), plus `cargo fmt --check`.

## Commands

Automation lives in a `justfile` (or `xtask` crate) — no bash-only scripts.
- `just dev` — backend against mock HA + `trunk serve`; simulates ingress (random path
  prefix + `X-Ingress-Path`).
- `just ci` — fmt, clippy (native + wasm, `-D warnings`), unit/integration tests, WASM build,
  prose lint (`just lint-prose`, vale: no LLM-sounding vocabulary or wall-of-text sentences in
  markdown, Rust comments or UI strings; config in `.vale.ini`, D065).

## Dev environment

- **Devbox** (Nix-based): commit `devbox.json`, `devbox.lock`, `rust-toolchain.toml`
  (stable ≥ 1.88). `rustup` as a Devbox package; wasm32 target + clippy/rustfmt via init_hook.
- **Host is Fedora Silverblue (immutable)**: `/nix` setup is a one-time *manual* host step —
  do not automate it; options are documented in `docs/DEV_SETUP.md`. Keep Podman on the host,
  out of Devbox.
- Image builds use **Podman** (locally), multi-stage BuildKit, musl targets for aarch64 + amd64,
  pinned HA Alpine base (`ghcr.io/home-assistant/base:<tag>`, never `latest`). Cross-compilation
  happens inside the container build.
- **GitHub Actions** builds and publishes the multi-arch image, plus a local-build path via
  Podman. CI (`just ci` gate: fmt, clippy native + wasm, tests, WASM build) also runs on
  GitHub Actions.

## Seed-data & grow-science discipline

- Cite a source for every default number in preset files. Where sources disagree, store the
  **range** and mark it. Re-check each number against ≥2 sources; if impossible, mark `unverified`.
- Presets are ranges, never single magic numbers. All user-editable with reset-to-default.
- Every preset row carries a source label; strain fields carry an evidence tag
  (`breeder` / `grower-reported` / `inferred`).
- The code is Unlicense, but **bundled strain/seed data is third-party**: store source and
  licence per record and check licences before bundling anything.

## Working agreements

- **No vendoring**: manage third-party code as real dependencies (crates.io / git), not
  copies in the repo. Vendoring is not preferred — avoid it, and remove existing vendored
  code when a proper dependency can replace it. Log any unavoidable exception (and its
  removal plan) in `docs/DECISIONS.md`. Bundled seed data is data, not vendored code.
- Pick the simplest option when ambiguous, log it in `docs/DECISIONS.md`, keep going.
  Ask only about blocking choices.
- Re-check the HA developer docs before each milestone ("add-ons" are now "apps"; naming is
  in flux).
- Out of scope for v1: multi-user, cloud sync, native mobile app, sharing/selling features,
  automatic device control by default.
- Owner is in Germany: plant-count limits and access-securing reminders are informational
  only (default off outside DE); verify statutory limits from current statute text before
  encoding.
