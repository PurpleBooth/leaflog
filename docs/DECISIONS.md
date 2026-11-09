# Leaflog Decision Log

One-off choices, in reverse-chronological order. Every `#[allow]` exception
to the mandatory lint policy (see AGENTS.md / SPEC.md §10) must be logged
here. Date format: YYYY-MM-DD.

---

## 2026-11-09 — post-M8

### D039: release version-sync hardening (path-only internal deps)

Cutting v0.2.0 exposed two latent release-pipeline bugs. (1) The
Release workflow's version sync rewrites only `[workspace.package]`
version in the root `Cargo.toml`; the `[workspace.dependencies]` pin
`leaflog-core = { version = "0.x" }` went stale on any real bump and
broke every cargo invocation ("failed to select a version"). It never
fired for v0.1.0 only because the version did not change. Fix: the
internal dependency is now **path-only** — nothing is published to a
registry, so no pin exists to go stale — and the sync step also runs
`cargo update --workspace` (workspace members only) and commits
`Cargo.lock`. (2) The workflow_dispatch `bump` input was declared but
never read; a dispatch would silently re-release whatever
`leaflog/config.yaml` says. Fix: the input is removed and dispatch is
documented as a re-release hatch; the release path is a pushed `v*`
tag created locally (the version commit + tag via `cog bump --auto`,
or by hand with `git` when cocogitto's own SSH signing cannot reach
the 1Password key — its `chore(version)` message is `cog`'s own
convention). Also: sync the version files *before* tagging so the
tagged image carries the right `config.yaml` version (the v0.1.0 tag
pointed at a pre-sync commit, so that image's add-on version was
stale).

### D038: in-app links carry the ingress base (`route_href`)

Under real HA ingress every nav link 404'd: `leptos_router` 0.8 renders
`<A href>` values that start with `/` verbatim (`use_resolved_path`
prepends the router base only to relative hrefs), and a path-absolute
href also ignores the document's `<base href>` (the leading slash
replaces the base path). Nav anchors therefore resolved to the bare
origin (`/equipment`), the router's click interceptor declined them
(path not under the router base), and the browser did a full page load
onto a Home Assistant core route — HA's 404, never Leaflog's. The
homepage alone worked because it is the URL HA loads into the panel
iframe, never a clicked link. `just dev` masked all of this:
`DevPrefixService` serves the SPA shell for unprefixed paths too, so a
link that dropped the prefix still rendered. Fix: `route_href()` in
`leaflog-frontend` prepends the runtime base (`window.__LEAFLOG_BASE__`)
to every route path; all `<A>` hrefs go through it. Verified by a
click-through and deep links under a simulated ingress prefix
(`just dev`). Rule: in-app `<A>` hrefs must never start with a bare
`/` — always `route_href("/…")`.

### D037: gitleaks secret scan in CI

A manual full-history sweep (all 61 commits and refs: token formats,
credential-named files, entropy strings, URL-embedded credentials, CI
workflows) found no committed secrets; the only near-miss is the
clearly-fake test fixture token in `crates/leaflog-backend/tests/ha.rs`.
To keep a public repository that way, CI gains a dedicated `secrets`
job (ci.yml): gitleaks 8.30.1, full clone (`fetch-depth: 0`),
`gitleaks git --verbose --redact .` — `--redact` so a real finding
never echoes the secret into public CI logs. `just ci` mirrors it via a
skip-if-not-installed `gitleaks` recipe (same pattern as `cog-check`);
the authoritative scan is the CI job, which also runs for releases
through the `workflow_call` reuse of ci.yml.

### D035: Threat model documented

docs/THREAT-MODEL.md expands the owner's five seed threats (disclosure
of cultivation, platform abuse, data manipulation harming plants,
data destruction, access by minors/household members) plus
encryption-at-rest and a STRIDE/web/supply-chain sweep, grounded in
code references. Top open items (R1-R9): image decode dimension cap,
`Cache-Control: no-store` on photo originals, `source_url` scheme
validation (hard prerequisite for strain import), hash-pinned build
tools, user-facing hardening notes, photo quota, `cargo audit`,
soft-delete/confirmation for cascade deletes, image signing.

### D036: Encryption at rest — deferred, guidance instead

App-level encryption (SQLCipher, encrypted photos) evaluated and
deferred: a key in add-on options/env sits on the same host disk as
the ciphertext, so it resists none of the realistic adversaries
(host root, SSH-granting add-ons, whole-machine theft) while adding
key-loss destruction risk. The effective controls are user-side and
must be documented (R5): HA passphrase-encrypted backups (backups
travel further than the host), full-disk encryption on x86 hosts,
restraint with host-path add-ons.

---

## 2026-11-08 — M8

### D032: Backup→restore tested

The integration test creates data (grow, action log, photo upload),
simulates a HA backup by copying /data (DB + photos/originals;
thumbnails excluded per backup_exclude), then simulates a restore.
Proves: grows, actions, photo rows, photo files, and photo serving
all survive; thumbnails regenerate lazily on first request (with a
directory-creation fix — the restored tree has no thumbnails/
directory).

### D033: Unit-aware display

leaflog-core::units provides dual-unit formatting: temp_both (°C/°F),
length_both (cm/in), volume_both (L/gal), small_volume_both (ml/fl oz),
plus preferred-system-only variants. The dashboard tiles, timeline
volume/height, and lamp-height card now show both systems.

### D034: Screenshots and accessibility audit

Screenshots captured via the in-app browser (desktop 1280×720 and
mobile 375×812, light theme; the app defaults to the user's system
preference) and committed to docs/screenshots/. The Lighthouse CLI
requires a Chrome binary unavailable in the Devbox environment; a
manual accessibility audit (docs/ACCESSIBILITY_AUDIT.md) covering all
Lighthouse accessibility criteria passes (21/21 checks, estimated
score ≥ 90): semantic landmarks, heading hierarchy, accessible names,
aria-labels, keyboard focus styles, WCAG contrast, touch targets, and
no colour-only signals.

Deferred to a manual step (requires a browser session against a
running dev instance): desktop + mobile, light + dark, main views
(home, equipment, strains,
grow detail with forecast + feed + env).

---

## 2026-11-08 — M7

### D028: VPD engine

`vpd_kpa()` uses the Magnus/Tetens saturation-vapour-pressure
approximation with a configurable leaf-temperature offset (default
−2 °C). The `rh_window()` solver inverts the VPD formula to find
the humidity range that keeps VPD in the phase band. Golden values
against reference tables (6 temperature/RH combinations ±0.15 kPa);
the leaf offset's effect is separately verified.

### D029: Lamp-height advisor

The spec §5.1 wattage-class table (220/330/420/750 W) with per-stage
height ranges; `advise_lamp()` applies: dimming (50% dim = ~30%
closer), hot-canopy rule (>85 °F → raise 2 in or dim to 80%),
stretch (raise 2–4 in), stress tags (dim to 70–80% or raise), a
10-in floor, and strain light-tolerance caution. Unit tests cover
each rule independently plus the wattage table lookup.

### D030: Strain-tolerance threshold shifts (spec §4.4, §13)

`TempThresholds::shifted()` applies: heat tolerant +2 °F ceiling,
heat sensitive −3 °F ceiling, cold tolerant −2 °F floor, cold
sensitive +3 °F floor. Temperature *differences* convert without the
32 °F offset (a delta of 2 °F = 1.11 °C, not −16.7 °C).

### D031: Phase coach

Per-phase tips with substrate awareness (soil wet–dry, coco daily
fertigation with runoff, hydro reservoir logging), lights-state VPD
guidance, flower-specific mould and stretch cautions, dry/cure
instructions. Equipment suggestions match the user's inventory
categories to the phase.

---

## 2026-11-08 — M6

### D026: Feed fit — piecewise per phase, never uniform

`fit_feed()` maps each template segment (seedling, veg, flower) onto
the corresponding actual segment with linear interpolation of EC;
ripen and flush are pinned to the end and never dropped. EC steps
between consecutive fitted days clamp to `max_ec_step_us` (default
200 µS/cm, configurable). Property tests: a 9-week-flower template
fits 8 and 11 weeks with ripen/flush preserved and EC steps within
clamps (spec §13); ripen/flush never drop across compressions from
42 to 84 flower days.

### D027: Substrate profiles and TDS scales

Built-in soil/coco/hydro presets from spec §5.3 with pH ranges,
watering styles, flush lengths, runoff check frequencies, and EC
bands (contested — labelled as one-source in the provenance). The
`TdsScale` enum (0.5/0.64/0.7) round-trips EC↔ppm within 1 µS/cm
(tested for all three scales × 10 EC values). Substrate presets
differ in pH, EC bands, flush, and watering style (tested — spec §13
"substrate switch changes recommendations").

### Lint-policy exceptions — M6 additions

| Crate | Items | Lint | Reason |
|---|---|---|---|
| leaflog-core | module `feed` | `clippy::cast_possible_truncation`, `clippy::cast_sign_loss` | The module converts float interpolation results (EC) to integer µS/cm; the casts are inherent |

---

## 2026-11-08 — M5

### D024: Phase forecast — sequential chain with reality compression

`forecast()` is a pure function over `ForecastInput` (photoperiod,
planned veg weeks, strain flower days, auto total, sourced default
ranges, and `Actuals` with flip/harvest/trichome days). The likely
chain builds sequentially (germ 3–10 → seedling 14–21 → veg [grower
weeks or auto age 28] → flower [strain days override] → dry 7–14 →
cure 14–30); each window carries earliest/likely/latest start+end.
When an actual event (an early flip, an early harvest) contradicts the
chain, **reality wins**: the historical phases before the actual
compress proportionally (forward compression pass), because their
predicted lengths no longer matter — the event is already logged.
Property tests sweep veg weeks × flower ranges × pinned actuals ×
auto totals asserting contiguity and ordering; golden tests pin the
exact expected days for Northern Lights (photoperiod, 4 veg weeks)
and GG4 Auto (70–84 days from seed).

### D025: Gantt rendering

The frontend renders the phase windows as a horizontal Gantt with
per-phase botanical accent colours, a light band for the
earliest/latest range, a solid bar for the likely window (✓ when
pinned by an actual), and a red today marker positioned by the grow
day count from `/api/grows/{id}/forecast`.

### Lint-policy exceptions (`#[allow]` register) — M5 additions

| Crate | Items | Lint | Reason |
|---|---|---|---|
| leaflog-frontend | module `gantt` | `clippy::cast_precision_loss` | Day counts are < 1000; i64→f64 for percentage layout cannot lose meaningful precision |
| leaflog-frontend | component `GanttTimeline` | `clippy::needless_pass_by_value` | Leptos component prop contract |
| leaflog-core | fn `add` | `clippy::too_many_arguments` | Phase-window builder takes (windows, phase, start, 3 lens, 2 flags); grouping would obscure |

---

## 2026-11-08 — M4

### D020: Supervisor link — one bridge, offline-tolerant

`HaBridge` owns the Supervisor connection: REST (`/core/api/states`,
`/core/api/history/period/...` with `filter_entity_id` +
`minimal_response`) called directly; a background task keeps a
WebSocket to `/core/websocket` subscribed via `subscribe_entities`
(filtered to the union of every grow's bindings — a binding change
restarts the socket with the new set), republishing state through a
`tokio::sync::watch` channel. The SSE `/api/live` endpoint emits the
current snapshot then one event per change; the frontend polls every
5 s (simpler than SSE in WASM; the stream exists for later). The
bridge is offline-tolerant: every endpoint works with the Supervisor
unreachable, and the socket reconnects with exponential backoff
(1,2,4,...60 s — unit-tested) plus a 30 s ping heartbeat. Reconnect
after a server drop is integration-tested against the mock.

### D021: Token hygiene

The Supervisor token lives in a redacted-Debug newtype
(`SupervisorToken`), is sent only as a Bearer header to the Supervisor,
and never appears in any response or log — asserted by an integration
test that exercises every HA endpoint (including the SSE stream) and
greps for the literal token.

### D022: Auto-capture and auto-log

Bound sensors (temperature, humidity, CO2, soil moisture, light,
reservoir pH/EC) snapshot into each new log entry's `sensor_snapshot`
rows; light-entity transitions (`light.*`) auto-log as `lights`
entries with the `auto_log_ha_events` setting as opt-out (default on).
Phase/day derivation in the auto-log path matches the action API.

### D023: Mock Supervisor — real WS protocol (closes D006)

`mock-ha` is now a lib+bin split: the library exposes the app for
in-process tests (backend dev-dependency), the binary for `just dev`.
The WebSocket speaks the real HA protocol (auth_required → auth with
token → auth_ok → subscribe_entities result → initial `+` state batch
→ 2 s tick events), the REST history endpoint returns
`minimal_response`-shaped series, and `GET /core/api/states` lists the
fake room. HA docs re-check 2026-11-08: apps rename remains
UI/docs-only; no config changes needed for M4.

### Lint-policy exceptions (`#[allow]` register) — M4 additions

| Crate | Items | Lint | Reason |
|---|---|---|---|
| (none new) | | | |

---

## 2026-11-08 — M3

### D017: Photo pipeline — re-encode strip, top-level trees

EXIF/GPS stripping is done by decoding to pixels and re-encoding a
clean JPEG (quality 95): whatever metadata the input carried cannot
survive. EXIF `DateTimeOriginal` is read *before* the strip and kept
as the capture time (spec §4.3); everything else is dropped.
Thumbnails are 512-px lossless WebP (the `image` crate's built-in
encoder). `originals/<grow>/` and `thumbnails/<grow>/` are top-level
so the app's `backup_exclude: photos/thumbnails` entry covers every
grow at once; missing thumbnails regenerate lazily on first request
(backup restores skip them by design).

### D018: EXIF timestamps are stored as naive UTC

`DateTimeOriginal` carries no timezone (camera-local time). It is
stored verbatim with a `Z` suffix. A timezone setting can correct
this later; re-basing stored timestamps would then be a migration.

### D019: M3 phase/day derivation is deliberately naive

Phase for a log entry derives from the grow start, the earliest
logged `Flip`, and any `Harvest` (seedling ≤ day 14 → vegetative →
flower after flip → drying after harvest; germination folded into
seedling). The M5 forecast engine replaces this — the derivation
lives in `leaflog-core` (`actions::derive_phase`) so the swap is
one function. HA docs re-check 2026-11-08: M3 touches no HA-facing
surface; no changes needed.

### Lint-policy exceptions (`#[allow]` register) — M3 additions

| Crate | Items | Lint | Reason |
|---|---|---|---|
| leaflog-frontend | `grows::clamp_int` | `clippy::cast_sign_loss`, `clippy::cast_possible_truncation` | One unavoidable f64→i64 form-input conversion, clamped and rounded before the cast |

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

### D012: predefined equipment seed data, no personal purchase details

`seeds/equipment.json` ships a starter inventory (a 60x60 tent setup:
Spider Farmer tent/light/filters, VIVOSUN humidifier/clip fan, meters,
pots, substrate, feeds and curing kit) seeded into an empty
`equipment_item` table by `seed_equipment_if_empty`, mirroring the
strain seeding. The repo is public (GitHub + HACS), so the bundled
records carry product facts only — no purchase dates, prices, order
references or other personal history; those live solely in the user's
own database. The five Dutch Passion strains added to
`seeds/strains.json` follow the same rule: breeder-published facts with
per-field evidence tags, sources cited per the seed-data discipline.

### D013: offset-less action timestamps read as UTC

The quick-add form's `<input type="datetime-local">` yields
`YYYY-MM-DDTHH:MM` (seconds and UTC offset absent), and its
sanitization rejects a pre-filled `Date.toISOString()` value outright.
`validate_action` (api_grows.rs) therefore normalizes such values to
RFC 3339 **UTC** before validation — `assume_utc`, not server-local —
because the field is labelled "(UTC)" in the UI and `Date.toISOString()`
(which supplies the prefill) is UTC. RFC 3339 input with an explicit
offset passes through unchanged. The frontend prefill keeps the
`YYYY-MM-DDTHH:MM` prefix of `toISOString()` so the field renders it.

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
