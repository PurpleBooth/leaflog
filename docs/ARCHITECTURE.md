# Leaflog Architecture

This file records the structural decisions behind Leaflog. One-off choices
live in [`DECISIONS.md`](DECISIONS.md); the product spec is
[`SPEC.md`](SPEC.md); the threat model is
[`THREAT-MODEL.md`](THREAT-MODEL.md).

## Crate graph

```
┌────────────────────────────────────────────────────────────┐
│                     leaflog-frontend (wasm)                │
│          Leptos 0.8 CSR SPA, built with Trunk              │
│              (browser, talks only to backend)              │
└───────────────┬────────────────────────────────────────────┘
                │ JSON REST + one WS/SSE channel (relative URLs)
┌───────────────▼────────────────────────────────────────────┐
│                     leaflog-backend (native)                │
│   Axum + Tokio; serves the SPA; SQLite in /data (M2);      │
│   the only component allowed to talk to HA Supervisor      │
└───────┬──────────────────────────────────┬─────────────────┘
        │ depends on                       │ SUPERVISOR_TOKEN (Bearer)
┌───────▼──────────────┐        ┌──────────▼─────────────────┐
│     leaflog-core     │        │  Home Assistant Supervisor │
│ domain types + pure  │        │  REST + WebSocket          │
│ engines (no I/O)     │        │  (mock-ha fakes this in    │
│ used by both sides   │        │   development)             │
└──────────────────────┘        └────────────────────────────┘
```

- **`leaflog-core`** — shared domain types and pure engines (forecast,
  feed-fit, VPD, lamp-height, recommendations, chart series merge,
  appearance vocabulary). No I/O, unit-testable, compiles for native and
  `wasm32-unknown-unknown`.
- **`leaflog-backend`** — Axum server. Serves the compiled SPA, the JSON
  API, and (from M2) SQLite persistence in `/data`. The only component
  that ever holds `SUPERVISOR_TOKEN`; it must work fully with HA
  unreachable.
- **`leaflog-frontend`** — Leptos CSR mode SPA built with Trunk. Never
  talks to Home Assistant directly.
- **`mock-ha`** — an Axum app that fakes the Supervisor API surface
  (`/core/api/...`) with canned sensor data so `just dev` needs no real HA.

## Ingress: how serving works

Home Assistant serves the app under
`/api/hassio_ingress/<token>/`. Everything below follows from that:

1. **Build time** — Trunk builds the SPA with a *relative* public URL
   (`public_url = "./"` in `Trunk.toml`), so every asset reference in
   `index.html` is relative (`./leaflog-frontend-*.js` and friends,
   content-hashed at the dist root). No base path is ever baked into
   the build — Trunk emits no `<base>` tag at all under a relative
   public URL.
2. **Serve time** — the backend reads the `X-Ingress-Path` request header
   (set by HA's ingress proxy) and injects, into the served `index.html`:
   - `<base href="{path}/">` so the browser resolves relative asset and
     route URLs under the ingress prefix, and
   - `window.__LEAFLOG_BASE__ = "{path}"` so the Leptos router can set
     its base at runtime (`<Router base=…>`, leptos_router 0.8).
3. **Asset embedding** — release builds embed `dist/` in the backend
   binary (`include_dir`), so the container ships a single static
   binary. Dev builds serve `dist/` from disk so `trunk watch` rebuilds
   are picked up without recompiling the backend.
4. **Security** — in production mode (a Supervisor environment is
   detected) the backend accepts connections only from HA's ingress
   proxy address `172.30.32.2`; there is no login screen because HA has
   already authenticated the user.

```
Browser                    Backend                      HA Supervisor
  │  GET {base}/             │                              │
  │─────────────────────────►│ read X-Ingress-Path          │
  │                          │ inject <base> + JS global    │
  │◄─────── index.html ──────│                              │
  │  GET {base}/pkg/app.wasm │                              │
  │◄─────── embedded asset ──│                              │
  │  GET {base}/api/...      │      (read-only REST/WS)     │
  │─────────────────────────►│─────────────────────────────►│
```

## Development flow (`just dev`)

```
mock-ha (fake Supervisor on :8098)
backend (dev mode, disk assets, random ingress prefix)
trunk watch (rebuilds dist/ on change)
```

The dev backend prints a URL like
`http://127.0.0.1:8099/api/hassio_ingress/<random>/` that behaves like
real ingress (arbitrary prefix, header injection, deep links) without a
Home Assistant instance.

## Milestone map

| Milestone | Adds |
|---|---|
| M1 | workspace, ingress-correct SPA serving, HA packaging, CI |
| M2 | SQLite schema + migrations, equipment, strains |
| M3 | action log, photo pipeline (EXIF strip, WebP thumbs) |
| M4 | Supervisor client (REST + WS), live data, history charts |
| M5 | phase forecast engine + Gantt timeline |
| M6 | feed templates + fit-to-timeline, substrate profiles |
| M7 | VPD/env engine, lamp advisor, phase coach, dashboard |
| M8 | hardening, docs, screenshots, accessibility |

## Grow overview redesign (post-M8, spec 2fe2dcc)

Three subsystems and one rule, per D073/D074:

- **Phases are events.** The phase picker records Flip / Harvest /
  Phase-change actions; the forecast pins its boundaries from the same
  event stream (latest event per boundary wins) and re-anchors — bands,
  coach, lamp advice and the day-count target follow. The design keeps
  no override state anywhere.
- **Charts are one endpoint plus a pure core merge.**
  `GET /api/grows/{id}/history` merges HA history with Leaflog's own
  log-time snapshots per role; the corridor (temperature, RH window,
  VPD, soil band) comes from phase-keyed presets in core. The
  frontend renders charts with ECharts' SVG renderer, driven from
  Rust — the bundle is a build-time artifact (ADR 0001, D088). HA
  outages degrade to snapshot-only, per the Dead binding rule.
- **The app lands on the active grow's overview**, not a marketing
  page; fresh installs still meet the onboarding wizard.

## Localisation (D112, ADR 0003)

The app speaks English and German. Strings live in compile-time
catalogs (`leptos_i18n` 0.6.3): `crates/leaflog-frontend/locales/en.json`
is the key source of truth, `de.json` mirrors it key-for-key in du-form.
The generated i18n module lands in `OUT_DIR`; `build.rs` panics on
EN↔DE key drift, so a missing key never ships. Nothing is fetched at
runtime.

The locale chain: Home Assistant user language first (`GET /api/locale`
carries the `language` field the backend captures from the documented
config endpoint on every reconnect), then the browser locale, then
English. Tags map by prefix (`de*` → de, `en*` → en); anything else
falls through. No picker, no persistence.

The backend has its own small catalog (D113) for the strings it owns:
the environment-alert notification templates and the four standard
error wrapper phrases. `crates/leaflog-backend/src/catalog.rs` holds
an exhaustive `(locale, message)` match — key parity is a build error,
not a runtime check. The alert path follows the bridge's cached
language; the wrappers follow the language the config fetch publishes
process-wide, because a response conversion has no state access.
Interpolated detail text (store errors, entity and category names,
alert kinds) stays English.

### Keying a string (the pattern for the keying batches)

An English literal becomes a key, one catalog entry per locale, and a
`t!()` call. Worked example — the Grows empty state:

```rust
// 1. The literal in the view:
<p class="py-6 text-center text-base-content/60">"No grows yet — start your first one."</p>

// 2. The key path mirrors the page and purpose (grows.empty).
//    Add it to BOTH catalogs in the same edit; the build fails
//    otherwise (key parity).
{
  "grows": {
    "empty": "No grows yet — start your first one."
  }
}
{
  "grows": {
    "empty": "Noch keine Anbauten — starte deinen ersten."
  }
}

// 3. The call: grab the context once per component, use it in the
//    view. Interpolated values use {{ name }} in both catalogs and
//    named arguments at the call site.
let i18n = use_i18n();
<p class="py-6 text-center text-base-content/60">{t!(i18n, grows.empty)}</p>
<p>{t!(i18n, grows.started, date = grow.started_on)}</p>
```

German drafts du-form ("starte deinen ersten", never "starten Sie").
Seed data, evidence tags and breeder names stay untranslated.
