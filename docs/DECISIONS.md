# Leaflog Decision Log

One-off choices, in reverse-chronological order. Every `#[allow]` exception
to the mandatory lint policy (see AGENTS.md / SPEC.md §10) must be logged
here. Date format: YYYY-MM-DD.

---

## 2026-11-08 — M2

### D012: SQLite via rusqlite, one connection, blocking-pool access

Implements D001: a single `rusqlite::Connection` behind a mutex, with
every query running inside `tokio::task::spawn_blocking` (`Db::with`).
WAL journal mode and `foreign_keys=ON` on open. Migrations are plain
`.sql` files in `crates/leaflog-backend/migrations/`, embedded with
`include_str!` and applied in order, each in its own transaction, with
the version tracked in a `schema_migrations` table. Chose a hand-rolled
runner (~40 lines) over `rusqlite_migration` to keep the dependency set
small.

### D013: M2 migration scope — only tables with shipped features

Migration 0001 creates `strain`, `equipment_item` and `settings` only.
Grow/plant/action-log tables arrive with the features that use them
(M3+); a versioned migration per milestone keeps the schema honest
instead of pre-creating §8's whole sketch. PPFD maps (M7), sensor
bindings (M4) and grow equipment snapshots (M3) are deferred.

### D014: Strain seeding runs only on an empty table

`seed::seed_strains_if_empty` inserts the bundled records when
`strain` is empty (first boot or wiped data) and never touches a
database that has any strain rows — so it can never overwrite user
edits or re-duplicate after deletes.

### D015: Bundled strain records — sourcing and licensing

Five records (Northern Lights, Amnesia Haze, Blue Dream, Gorilla Glue
#4, GG4 Auto). Numbers are cross-checked ranges from ≥2 public sources
(breeder pages where they exist: Sensi Seeds, Barney's Farm; otherwise
nursery/grower consensus); where sources disagree the stored range
spans them and the notes say so (Blue Dream's flowering window is
deliberately wide: 56–77 days). Evidence tags: `breeder` for
breeder-stated numbers, `grower_reported` for consensus, `inferred`
for the tolerance heuristics, `unverified` where stretch could not be
double-sourced (GG4). Licensing: durations/heights/percentages are
facts, not copyrightable expression; each record carries `source_url`
and a licence note stating exactly that, and no source prose is
reproduced. The user-triggered CSV/JSON importer is deferred to a
later milestone.

### D016: HA docs re-check (pre-M2)

Re-checked 2026-11-08: the add-ons → apps rename is UI/docs-only
(zero functional changes; config.yaml keys unchanged), confirmed via
the contributors' config reference. M2 has no HA-facing surface, so no
packaging changes were needed.

### Lint-policy exceptions (`#[allow]` register) — M2 additions

| Crate | Items | Lint | Reason |
|---|---|---|---|
| leaflog-frontend | module `api` | `clippy::future_not_send` | WASM is single-threaded; gloo-net response types are not `Send` and `spawn_local` does not require it |
| leaflog-frontend | `EquipmentPage`, `EquipmentForm`, `StrainsPage`, `StrainForm` | `clippy::too_many_lines` | Declarative view code, not logic; splitting would obscure the forms |

---

## 2026-11-08 — M1

### D001: SQLite access via `rusqlite`, not `sqlx` (decided now, wired in M2)

`rusqlite` + `tokio::task::spawn_blocking` is the simplest option for a
single-user HA app with a low write rate: no compile-time query macros, no
`DATABASE_URL`/offline metadata for CI, no async driver ceremony.
`sqlx`'s compile-time checked queries would add build friction under the
strict lint policy for little benefit at this scale. Versioned migrations
will be embedded and applied on startup.

### D002: Workspace uses Rust edition 2024

The owner asked for the latest stable edition (2024, stable since Rust
1.85; workspace MSRV 1.88). The spec draft assumed 2021 because Leptos
0.8 predates 0.9's edition bump, but 0.8 compiles fine on 2024 — this
also future-proofs the planned Leptos 0.9 move and enables let-chains.

### D003: SPA assets — embedded in release, disk-served in dev

Release builds embed the Trunk `dist/` directory into the backend binary
with `include_dir`, so the HA image ships one static binary and the
ingress injection rewrites `index.html` at serve time. Dev builds serve
`dist/` from disk (cargo feature off) so `trunk watch` rebuilds appear
without recompiling the backend. The `dist/` directory is git-ignored and
always rebuilt by Trunk.

### D004: Crate names lowercased/prefixed vs. the spec draft

The spec drafts mention a `mock-HA` crate. Cargo package names are
lowercase by convention, so the crate is `mock-ha` (lib name `mock_ha`).
App crates carry the `leaflog-` prefix (`leaflog-core`, `leaflog-backend`,
`leaflog-frontend`) for unambiguous metadata; `mock-ha` stays unprefixed
because it is never published or shipped.

### D005: Production/dev mode detection

Production mode is active when `LEAFLOG_ENV=prod` **or** when
`SUPERVISOR_TOKEN` is present (i.e. running inside HA). Only then does the
backend enforce the ingress-proxy-only remote address `172.30.32.2`.
Dev mode (`just dev`, bare `cargo run`) accepts any remote address.

### D006: `mock-ha` M1 scope is REST-only

M1's mock Supervisor implements `GET /core/api/` and `GET /core/api/states`
with canned entities. The fake WebSocket (`subscribe_entities`) surface is
deferred to M4 when the real Supervisor client lands.

### D007: Image build strategy — musl cross-compilation inside the container

Per spec: no musl targets in the Devbox environment. The Dockerfile builds
each architecture natively (per-arch GitHub runners upstream, plain Podman
locally) using a Rust Alpine builder image; the final stage is the pinned
base image. Note the GHCR tag scheme: `<alpine>-<yyyy.mm.n>` (for example
`3.24-2026.11.0`) — the docker-base *GitHub release* numbers (for example
`2026.11.0`) are **not** image tags. Resolved from `ghcr.io/v2` on
2026-11-08; never `latest`.

### D008: Leptos ingress spike outcome

Recorded after the spike runs (see `docs/ARCHITECTURE.md` §Ingress).
Mechanism verified by research 2026-11-08: `leptos_router` 0.8 exposes a
runtime `<Router base=…>` prop; Trunk builds with `--public-url ./`.
**Spike result: PASSED** — integration tests under an arbitrary prefix
(`leaflog-backend` `tests/ingress.rs`) cover injection, relative-only
assets, deep links; manual deep-link/refresh under `just dev` confirmed.

### D009: Trunk `wasm-bindgen` handling, and no `wasm-opt`

Trunk downloads a matching `wasm-bindgen` CLI automatically when none is
on `PATH` — this works locally, but **failed on the aarch64 CI builder**
(the downloaded binary never materialized; the D009 fallback became
necessary on first use). The image builder therefore installs a pinned
`wasm-bindgen-cli` (0.2.130, matching the crate) next to Trunk;
`devbox.json` still relies on Trunk's auto-fetch.

`wasm-opt` is **not** run (no `data-wasm-opt` on the rust asset): the
nixpkgs binaryen build rejects Rust's bulk-memory opcodes without
`--enable-bulk-memory-opt`, which Trunk cannot pass. Release builds
still get rustc `opt-level` + release LTO. Revisit when Trunk/binaryen
cooperate on feature flags.

Related: the workspace release profile must **not** set `strip = true`.
Stripping removes the wasm `target_features` custom section, so
`wasm-bindgen` cannot detect enabled features and downloads its own
`wasm-opt` (version_123) to compensate — which fails on the same
bulk-memory validation. The backend binary is stripped in the Dockerfile
instead.

### D010: Placeholder app icon/logo

`leaflog/icon.png` and `leaflog/logo.png` are generated placeholders
(simple leaf mark, calm green) so the app installs with a sane look from
M1. Final artwork is an M8 polish task.

### D011: s6 user bundles deprecation notice

The base image's s6-overlay logs a deprecation warning for defining the
user bundle in `/etc/s6-overlay/s6-rc.d/user/contents.d` (our
enablement file for the `leaflog` longrun service); upstream now prefers
`/etc/s6-overlay/user-bundles.d`. Works fine on the pinned base
(3.24-2026.11.0); migrate when a future base bump requires it.

### Lint-policy exceptions (`#[allow]` register)

Every narrowly scoped exception to the mandatory lint block, each with a
one-line reason at the site. Patterns shared by several items are listed
once per crate.

| Crate | Items | Lint | Reason |
|---|---|---|---|
| leaflog-backend | `ingress::IngressPath` | `clippy::module_name_repetitions` | The domain name (`IngressPath`) is the right name; the module is `ingress` |
| leaflog-backend | `app.rs` handlers `health`, `index`, `fallback` and middlewares `supervisor_gate`, `dev_prefix_strip` | `clippy::needless_pass_by_value` | Axum handlers/middlewares must take extractors (`State`, `HeaderMap`, `Request`) by value |
| leaflog-backend | `app.rs` handler `health` | `clippy::unused_async` | Axum handlers must be `async fn` even when they never await |
| mock-ha | handlers `root`, `ping`, `states` | `clippy::unused_async` | Same Axum contract as above |
| leaflog-frontend | component `App` | `clippy::must_use_candidate` | The Leptos `#[component]` macro drops a hand-written `#[must_use]`, so the lint fires on the generated function |
