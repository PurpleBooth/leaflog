# GOAL: Leaflog – a cannabis grow diary that lives inside Home Assistant

## 1. Mission
Build **Leaflog**, a self-hosted grow diary packaged as a **Home Assistant app (formerly "add-on")** that appears as its own **sidebar tab** via ingress. Backend and frontend are both **Rust**. It must look polished enough to feel native to HA.

"Done" means a user can:
1. Install Leaflog from a local app repository (amd64 + aarch64) and open it from the HA sidebar.
2. Create a grow from a strain, substrate and equipment list, and see predicted dates for every phase.
3. Log actions and photos, with HA sensor data attached automatically.
4. See live "in range / out of range" feedback and **phase-, substrate- and strain-aware recommendations** for pH, EC/TDS, temperature, humidity, VPD, lamp height and equipment setup.
5. Follow a feeding schedule that is stretched or shrunk to that plant's real timeline.
All checks in §13 must pass, with evidence.

## 2. Non-negotiables
- **Rust only** for app code. No Node runtime in the final image. Build-time CSS tooling is fine.
- **Local-first.** No telemetry and no cloud calls at runtime. Strain/data imports are user-triggered only.
- **Ingress-correct** (the app is served under `/api/hassio_ingress/<token>/`):
  - Relative URLs everywhere. Never `/static/...` or a redirect to `/login`.
  - Read the `X-Ingress-Path` header. Inject it into `index.html` at runtime (`<base href>` plus a JS global) so the router base is set at runtime. Never bake a base path into the build. Build with a relative public URL.
  - Accept connections only from `172.30.32.2`. No login screen, because HA has already authenticated the user.
  - Large photo uploads must work: look at the `ingress_stream` option, and set an explicit Axum `DefaultBodyLimit` (the default is 2 MB).
- `panel_admin: true` by default. Grow data is private.
- **HA token hygiene.** The Supervisor token is administrator-level.
  - Never send it to the browser, never log it, and use read-only calls by default.
  - Any `call_service` (e.g. dimming a light) is gated behind an explicit opt-in setting.
- **Privacy.** No sharing, social features or public URLs. Strip EXIF/GPS from uploaded photos.
- Respect HA unit system and locale (°C/°F, in/cm, gal/L). Store SI internally. Display in the user's units. The presets below are written in imperial because that's the owner's preference, but you must also show metric.

## 3. Architecture (record decisions in `docs/ARCHITECTURE.md`)
- **Backend:** Axum + Tokio + SQLite (sqlx or rusqlite) in `/data`, with versioned migrations.
- **Shared crate** (`leaflog-core`): domain types plus the pure-Rust engines (forecast, feed-fit, VPD, lamp-height, recommendations). It's unit-testable and used by both backend and frontend.
- **Frontend: Leptos, CSR mode, built with Trunk** (WASM SPA served by Axum). Use the latest stable 0.8.x. Re-evaluate 0.9 when it goes final (0.9.0-rc1 was released 2026-10-31). Do not use SSR/hydration: the ingress path is dynamic, and plain CSR plus a runtime-injected base is the least fragile setup.
  - **M1 spike (time-box it):** prove that the router base can be set at runtime from the injected ingress path, that all asset URLs are relative, and that deep links and refresh work under an arbitrary prefix.
  - Only if the spike fails, write a one-page ADR with evidence and evaluate **Dioxus** (its `base_path` is documented as a build-time setting, so check for a runtime workaround). Do not switch frameworks for any other reason.
- **HA link (backend only):**
  - `homeassistant_api: true`. Use `SUPERVISOR_TOKEN` as a Bearer token.
  - REST at `http://supervisor/core/api/`, WebSocket at `ws://supervisor/core/websocket`.
  - Use `subscribe_entities` (filtered) for live state, `GET /api/states` for entity pickers, and the **documented** REST history endpoint (`/api/history/period/...` with `filter_entity_id`, `minimal_response`) for charts.
  - Don't depend on undocumented WebSocket commands. If you must, isolate them behind a trait with a fallback.
  - Reconnect with backoff and heartbeat. The diary must work fully with HA unreachable.
- **Browser ↔ backend:** JSON REST plus one WebSocket/SSE channel for compact live updates. The browser never talks to HA directly.
- Charts are rendered from Rust as SVG. Pick a crate (or hand-roll) and justify it in an ADR.
- Whole-grow JSON export/import, plus CSV for logs and sensor history.

## 4. Features

### 4.1 Equipment inventory
- Categories: light, tent/room, fan, carbon filter, humidifier, dehumidifier, heater/AC, pots/media, pumps/reservoir, meters (pH, EC/TDS, PPFD/PAR), sensors, controllers, misc.
- Fields: name, brand/model, **real wall wattage** (not the marketing "equivalent"), dimmable (Y/N), PPFD map (optional, see 4.7), purchase date/price, notes, photos.
- Optional **HA entity binding** per item (e.g. `light.*`, power sensor, dimmer `number.*`).
- Per-grow **assignment** that snapshots what was used, so later edits don't rewrite history.
- Energy estimate (watts × runtime) using bound power/energy sensors where available.

### 4.2 Action log
- Quick-add chips: Lights on/off or schedule change, Water, Feed (volume, input pH/EC/TDS, runoff pH/EC/TDS, recipe), Trim/defoliate, Train (LST/top), Transplant, Flip to 12/12, Pest/IPM, Env change, Light height/dimmer change, Measurement, Photo, Note, Trichome check, Harvest, Dry/Cure check.
- Each entry has: timestamp, plant(s), auto-filled phase and day number, structured fields, free text, appearance tags (colour, droop, taco, bleaching, tip burn, foxtailing, etc.) and photos.
- **Auto-capture from HA:** snapshot bound sensors (temp, RH, VPD, soil moisture, CO₂, reservoir pH/EC/TDS, light state/dim level) into each entry.
- **Auto-log from HA:** lights on/off transitions and bound-sensor excursions become log events, with opt-out.
- Timeline with filters, and a 3-tap mobile entry flow.
- Reminders (water/feed due, lamp height check, burp jars) show as UI badges, with an optional HA notification/event on opt-in.
- Some actions move the forecast: Flip, Transplant, Trichome check, Harvest.

### 4.3 Photo log
- Capture or upload from phone or desktop. Store originals in `/data/photos` and generate WebP thumbnails.
- Strip EXIF/GPS. Keep the capture timestamp as metadata.
- Link each photo to grow, plant, phase, grow day, log entry and tags.
- Views: gallery by plant/day, **side-by-side compare** (two dates, slider), a "same plant every N days" strip, and a **time-lapse player** (client-side slideshow with a day-number overlay).
- Optional: pull a still from a bound HA `camera.*` entity on a schedule or on demand (opt-in).
- Put thumbnails in `backup_exclude`, and regenerate them on demand. Warn when photo storage exceeds a configurable size.

### 4.4 Strain database
- Seeded from a small curated JSON in the repo, plus a user-triggered CSV/JSON importer. Store source and licence per record. Check licences before bundling anything. Many public datasets lack cultivation parameters.
- Fields: name, breeder, photoperiod/auto, feminized/regular, genetics (indica/sativa/hybrid % and landrace ancestry tags), **flower days (min/max)**, expected stretch factor, typical veg weeks, height class, auto total days from seed, yield notes, terpene/effect notes, **heat tolerance**, **cold tolerance**, **light tolerance**, user rating and notes, and an evidence tag per field (`breeder`, `grower-reported`, `inferred`).
- **Strain → recommendation modifiers.** Strain data adjusts:
  - the forecast (flower length, stretch, auto timing);
  - the temperature ceiling/floor alerts (heat/cold tolerance);
  - lamp-height caution (light tolerance);
  - the phase coach text.
- Heat/cold tolerance is a 3-level field (`sensitive`/`average`/`tolerant`). Default it from lineage when the user hasn't set it, and always show it as "inferred". Heuristics:
  - Autoflowers, short bushy broad-leaf indica-leaning plants and cold-climate lineages tend to be **more** heat-sensitive.
  - Sativa/Haze/African/Afghan/Hawaiian lineages tend to be **more** heat-tolerant, but can be shocked by cold.
  - There are exceptions (e.g. some stout indica or auto cultivars are reported heat-tolerant).
- Tolerance shifts the *alert thresholds*, not the species rules. Default shifts (editable): tolerant +2 °F ceiling, sensitive −3 °F ceiling. These are heuristics, so label them as such.
- Users can add, edit and clone a strain as "my phenotype". Per-grow actual results can be written back (flower days achieved, max stretch, what temps it tolerated).

### 4.5 Phase forecast
- Phases: germination → seedling → veg → (flip) → flower → harvest → dry → cure.
- Inputs: start date, strain, planned veg weeks (photoperiod) or auto age, overrides.
- Output per phase: start/end plus **earliest / likely / latest** window.
- Seed defaults (editable, sourced): germination 3–10 days, seedling 2–3 weeks, veg 3–8+ weeks (grower-chosen), flower 8–11 weeks (strain value wins; indica-leaning ~8–9, sativa-leaning at the top of the range or beyond), stretch weeks 1–3 adds about 50–100% height, dry ~7–14 days (until stem snap), cure ≥14 days (ideally ~30), autos start flowering ~3–5 weeks from seed and finish ~9–11 weeks from seed (some up to 13).
- Re-forecast from reality: logged events replace predictions. A "trichome check" entry nudges the harvest window.
- Gantt timeline with a "today" marker. Export phase dates to an `.ics` file or an HA calendar.

### 4.6 Feeding schedule stretch/shrink
- A feed template is week- or fraction-of-phase rows: EC target, pH range, nutrient doses (user-defined products and ml/L), watering frequency, notes. Ship generic soil, coco and hydro templates and an importer. Don't ship copyrighted brand charts.
- **Fit-to-timeline:** a piecewise time-warp per phase.
  - Map each template phase (seedling, early/late veg, early/mid/late flower, ripen, flush) onto the plant's actual planned phase length, and interpolate values linearly.
  - Never scale the whole template uniformly. Flower length is genetics-bound and veg length is the grower's choice.
  - Clamps: never drop ripen/flush; max EC step per feed is configurable.
  - Show a before/after chart and a diff table, with per-day pin/override.
  - Re-fit automatically when the forecast moves.
- Store EC canonically in µS/cm. See §5.3 for TDS scale handling.

### 4.7 Lamp height & intensity advisor
Inputs: fixture (type, real watts, dimmable, optional PPFD map), tent footprint and height, stage, current dim %, canopy temperature (HA), strain light tolerance, and optional measured PPFD.
Output: **recommended height range, dim %, target PPFD, DLI** (PPFD × hours × 0.0036), a confidence level, and a plain-English "why".
- Method, in order of preference:
  1. If the user has PPFD measurements or a manufacturer map, fit PPFD vs height and dim level. The inverse-square law is a rough guide only, so fit the exponent and clamp it.
  2. Otherwise use the wattage-class table in §5.1, with "low confidence".
- Rules (editable data):
  - Dimming ~50% lets the lamp hang roughly 30% closer.
  - Raise the lamp or dim if canopy temp is too high (see §5.1).
  - During the stretch, raise 2–4 inches as plants grow.
  - Never recommend below the fixture's minimum distance (default floor 10–12 in for dimmable LEDs).
  - On "bleaching / taco / crispy" tags, suggest +2–4 in or dim to 70–80%, then re-check after about a week.
- Prefer "set distance by stage, then fine-tune with the dimmer".
- Always show the units, say that the numbers are starting points, and recommend a PAR meter or a PPFD-meter log entry to calibrate.
- If a dimmer entity is bound **and** service calls are opted in, offer a one-tap "apply" with confirmation. Otherwise advise only.

### 4.8 Substrate-aware recommendations
Substrates: soil, coco, hydro (DWC/RDWC), plus user-defined (e.g. peat soilless, living soil). A grow's substrate modifies: pH target, EC/TDS band, watering/fertigation cadence, runoff checks, flush length, reminder text and coach tips. See §5.3 for the seed table. The coach must say *why*, e.g. "In coco, keep media wet and run 10–20% runoff to stop salt build-up".

### 4.9 Environment recommendations (pH, EC, TDS, humidity, temperature, VPD)
- Compute **VPD** from HA air temp + RH with a configurable **leaf-temperature offset** (default −2 °C / −3.6 °F; offer an IR-thermometer log entry to calibrate).
- Show a live VPD gauge and the target band for the current phase.
- Compute the **RH window** that would put VPD in band at the current temperature, rather than showing a fixed RH number only.
- Lights-on and lights-off targets differ, so use the bound light entity to know which applies.
- Alerts with hysteresis, quiet hours and per-strain thresholds. Examples: "Late flower, 83 °F, this strain is heat-sensitive: raise lamp or dim", "RH 62% in week 7: mould risk".
- Watering/feeding entries compare input vs runoff pH/EC and flag drift (§5.3).

### 4.10 Phase coach (suggested equipment setup)
A rules card per phase that lists targets, live status, and **recommended equipment categories matched to the user's own inventory first** (e.g. "You own a 150 W dehumidifier: use it in late flower"). Presets are stored as editable data, each with a short "why" and a source note.

### 4.11 Dashboard
Per grow: phase and day count, next milestone, today's feed target, lamp-height card, live sensor tiles, latest photo, last N log entries, and line charts (temp, RH, VPD, soil moisture, pH/EC/TDS) with log events overlaid as markers.

## 5. Grow Science Pack (seed data, editable, each row carries a source label)
**Rule:** the sources below disagree in places. Store ranges, never single magic numbers. Show "sources vary" where they do. Every preset is user-editable, with a "reset to default" button. Re-check each number against at least two sources before committing it. If you can't, mark it `unverified`.

### 5.1 Light & heat
| Stage | Generic LED height | Target PPFD |
|---|---|---|
| Seedling | 24–30 in (lower with a dimmer at 30–40%) | 200–300 |
| Veg | 18–24 in | 400–600 |
| Early flower | 16–20 in | 600–800 |
| Mature flower | 12–18 in | 800–1000 (some sources lower) |

Wattage class at full power (starting point):
| Real watts | Seedling | Veg | Early flower | Late flower |
|---|---|---|---|---|
| ~220 W | 24 in | 18–20 | 14–16 | 12–14 |
| ~330 W | 28–30 | 20–24 | 16–18 | 14–16 |
| ~420 W | 32–36 | 24–28 | 18–22 | 16–20 |
| ~750 W+ | 30–36 | 24–30 | 20–26 | 18–24 |

- Photoperiod: 18/6 veg, 12/12 flower. Autos: long schedule (commonly 18–20 h).
- Canopy temp above 85 °F → raise lamp ~2 in or dim to ~80%.
- Fixture height charts from different vendors differ by tent size and fixture design. Treat them as priors, not truth.

### 5.2 Temperature, humidity, VPD
| Phase | Lights-on temp | VPD (kPa) | Notes |
|---|---|---|---|
| Seedling/clone | ~70–80 °F | 0.4–0.8 | clones like 60–80% RH, avoid >80% |
| Early veg | 70–85 °F | 0.8–1.0 | mid-to-high RH |
| Late veg | 70–85 °F | 1.0–1.2 | |
| Early flower | 65–80 °F | 0.8–1.1 | keep ≤80 °F if possible |
| Mid flower | 65–80 °F | 1.0–1.4 | |
| Late flower | 65–80 °F | 1.2–1.6 | cooler nights, up to ~10 °F difference, lower RH to protect from mould |
| Dry | 60–68 °F | n/a | dark; RH 50% days 1–2 then ~55% until stems snap |
| Cure | room temp | n/a | jars at 58–62% RH; burp daily week 1, every 2–3 days week 2, weekly weeks 3–4 |

- Cold stress below ~60 °F. Up to ~85 °F is tolerable under strong light with low RH and good airflow. Above ~80 °F in flower can mean airy buds and lost aroma.
- Published VPD charts disagree mainly because of the **leaf-temperature offset** they assume, so expose the offset.
- RH ranges are derived from the VPD band (§4.9). A fixed sanity band is only a fallback.

### 5.3 Substrate, pH, EC/TDS
| | Soil | Coco | Hydro (DWC/RDWC) |
|---|---|---|---|
| Typical pH | 6.2–6.8 | 5.8–6.2 | 5.5–6.0 |
| Watering style | wet–dry cycle (water when top 1–2 in is dry) | keep ~90–100% saturated, fertigate ≥1×/day, 10–20% runoff every time | reservoir management, log pH/EC trend |
| Pre-harvest flush | ~7–10 days | ~5–7 days | ~3–5 days |
| Runoff check | occasional | every feed (compare in vs out) | n/a (reservoir) |

- Some sources give wider pH windows (e.g. coco 5.5–6.5). Let the user pick "tight" or "tolerant" profiles.
- Flushing is a debated practice among growers, so make "ripen/flush" configurable per grow.
- **EC/TDS.** Store EC in µS/cm. TDS (ppm) is *derived* using the user's meter scale: 0.5 (NaCl/"500"), 0.64 ("442"), or 0.7 ("700"). The same solution reads different ppm on different scales, so always label the scale and let the user set a default.
- Seed EC bands from one 500-scale source (ppm → EC):
  - Soil: seedling 100–200, early veg 300–500, late veg 600–900, early flower 800–1100, peak flower 1000–1400, late flower 700–1000, flush 0–50.
  - Coco: 250–350, 500–700, 800–1100, 1000–1300, 1200–1600, 900–1100, 0–50.
  - Hydro: 200–300, 400–600, 700–1000, 900–1200, 1100–1500, 800–1000, 0–50.
- **These are contested.** Another source gives only ~600–800 ppm for soil flowering, and growers commonly say published feed charts run high. Default the active preset to the **lower half** of each band. Make it very clear these are starting points and that product-label dosing for the user's own nutrients is what counts.
- Runoff-vs-input guidance (editable): runoff a few hundred ppm above input is normal in coco. If runoff is far above input (e.g. ≥1000 ppm), suggest a flush. Runoff below input suggests the plant is feeding hard.

## 6. UX / visual design
- Dark and light themes following `prefers-color-scheme`, with a manual override. Optional best-effort match to the HA theme.
- Calm botanical palette, cards, generous spacing, rounded corners, subtle motion, good empty states and skeletons. Phase accents (seedling green → flower purple/amber → cure warm brown).
- Phone-first for logging, wide layout for charts, timeline and photo compare. Large touch targets.
- WCAG AA, keyboard navigation, no colour-only signals.
- Screenshots in `docs/` (desktop + mobile, light + dark).

## 7. Home Assistant packaging
- Repo: `repository.yaml` plus an app folder with `config.yaml`, `Dockerfile`, `rootfs/etc/s6-overlay/` (s6 longrun service), `README.md`, `DOCS.md`, `CHANGELOG.md`, `icon.png`, `logo.png`, translations.
- `config.yaml`: `arch: [aarch64, amd64]`, `ingress: true`, `ingress_port: 8099`, `panel_icon: mdi:cannabis`, `panel_title: Leaflog`, `panel_admin: true`, `homeassistant_api: true`, `init: false`, `map` for `data`, options schema (units, TDS scale, entity bindings, opt-in service calls, alert thresholds, log level), and a multi-arch `image:` name.
- **Backups:** default is `backup: hot`, so checkpoint SQLite via `backup_pre`, or choose `cold`. Exclude thumbnail caches. Prove backup → restore keeps data and photos.
- **Dockerfile:** multi-stage with BuildKit, musl targets for both arches, and a **pinned** HA Alpine base (`ghcr.io/home-assistant/base:<tag>`), never `latest`. `BUILD_FROM` is no longer passed by default, so use a fixed `FROM` or give it a default. The musl cross-compilation happens inside the container build (document how: per-arch runners, or a pinned cross linker), so the Devbox environment doesn't need musl targets.
- GitHub Actions to build and publish a multi-arch image, plus a local-build path.
- Re-check the HA developer docs before each milestone. Naming and behaviour are changing ("add-ons" are now "apps").

## 8. Data model (starting point, refine)
`strain`, `equipment_item`, `fixture_ppfd_map`, `grow`, `plant`, `grow_equipment`, `substrate_profile`, `phase_plan`, `phase_event_actual`, `action_log`, `photo`, `log_attachment`, `sensor_binding`, `sensor_snapshot`, `feed_template`, `feed_template_row`, `feed_plan_day`, `preset_env`, `preset_light`, `preset_substrate`, `alert_rule`, `settings`, `schema_migrations`. ULIDs, UTC timestamps, SI units internally.

## 9. Compliance (built-in, informational)
The owner is in Germany. Private home cultivation is limited to **3 plants per adult** and must be **secured against access by third parties, especially minors**. Passing harvested cannabis to third parties is not allowed.
- Configurable per-jurisdiction plant-count warning (default off outside DE) with a disclaimer that this is informational and not legal advice.
- A "secured against third-party access" checklist reminder per grow.
- No sharing features. Verify any other statutory limits (e.g. possession amounts) from the current statute text before encoding them.

## 10. Code quality: mandatory lint policy
Every crate root (`lib.rs`/`main.rs` of `leaflog-core`, backend, frontend, mock-HA, xtask) starts with its `//!` crate doc, then this block **verbatim**:

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

Rules for living with it:
- **Never weaken the crate-level block.** If macro-generated code trips a lint (Leptos `#[component]` and prop structs, serde/sqlx derives, `#[derive(Store)]`), apply a narrowly scoped `#[allow(...)]` on that item or module, with a one-line reason comment. Log every exception in `docs/DECISIONS.md`.
- Expected friction points to handle properly, not suppress: `missing_docs` (document every public item, including components and props); `missing_debug_implementations` / `missing_copy_implementations` (derive `Debug`, and `Copy` where it's valid); `clippy::pedantic` items such as `must_use_candidate`, `missing_errors_doc`, `missing_panics_doc`, `module_name_repetitions`, `needless_pass_by_value` (Leptos components take owned props, so expect this one).
- `clippy::cargo` requires complete package metadata in every `Cargo.toml` (`description`, `license`, `repository`, `readme`, `keywords`, `categories`). Set them and inherit through `[workspace.package]` where possible.
- CI runs `cargo clippy --workspace --all-targets -- -D warnings` for the native target **and** for `wasm32-unknown-unknown` on the frontend crate, plus `cargo fmt --check`.
- Lint cleanliness is checked at every milestone commit, not just at the end.

## 11. Dev environment (Devbox)
- Use **Jetify Devbox** (Nix-based) for the toolchain. Commit `devbox.json`, `devbox.lock` and `rust-toolchain.toml` (pin stable ≥ 1.88; current Leptos 0.8 crates need it, and 0.9 moves to edition 2024).
- Follow Devbox's Rust pattern: `rustup` as a Devbox package, with the toolchain selected in `init_hook`. Draft:
  ```json
  {
    "packages": ["rustup@latest", "just@latest", "trunk@latest", "binaryen@latest", "sqlite@latest", "pkg-config@latest", "libiconv@latest"],
    "shell": {
      "init_hook": [
        "rustup show",
        "rustup target add wasm32-unknown-unknown",
        "rustup component add clippy rustfmt"
      ],
      "scripts": { "dev": "just dev", "ci": "just ci" }
    }
  }
  ```
  Verify each package name with `devbox search` and adjust. Let Trunk fetch its matching `wasm-bindgen` where it can, and document the fallback if it can't. Keep Podman on the host and out of Devbox.
- **Host note (Fedora Silverblue):** Nix needs `/nix`, which the immutable root doesn't provide. Do **not** automate this. Document the one-time host setup in `docs/DEV_SETUP.md` with the options and trade-offs (transient root + Nix installer in ostree mode, the Fedora `nix` package, or single-user Nix in a toolbox), then assume `devbox shell` works.
- Use **Podman** for image builds.
- Automation lives in a `justfile` (or an `xtask` crate). No bash-only scripts.
- `just dev` runs the backend against a **mock HA** (an Axum app that speaks the WebSocket protocol and replays fake sensor data) plus `trunk serve`. It also simulates ingress (random path prefix and `X-Ingress-Path`). Document how to test against a real HA.

## 12. Milestones (commit at each, lint-clean)
M1 skeleton + packaging + ingress "hello" tab + Leptos runtime-base spike → M2 DB, equipment, strains → M3 action log + photo log → M4 HA live data + history → M5 forecast + timeline → M6 feed fit + substrate profiles → M7 VPD/env engine, lamp advisor, phase coach, dashboard polish → M8 hardening, docs, CI, screenshots.

## 13. Acceptance criteria (show evidence for each)
- [ ] `just ci`: fmt, clippy with the §10 lint block and `-D warnings` (native + wasm), unit and integration tests, WASM build.
- [ ] Lint block present verbatim in every crate root; every `#[allow]` exception is listed in `docs/DECISIONS.md`.
- [ ] Forecast: property tests (phases contiguous and ordered, re-forecast from actuals consistent) plus golden tests for a photoperiod and an autoflower grow.
- [ ] Feed fit: a 9-week flower template fits 8 and 11 weeks with ripen/flush preserved and EC steps within clamps.
- [ ] VPD engine: golden values against a reference table; RH-window solver verified; leaf-offset respected.
- [ ] Lamp advisor: unit tests for dimming, temperature, stretch and floor rules; behaviour with and without a PPFD map.
- [ ] TDS/EC: round-trip conversion tests for all three scales.
- [ ] Substrate switch changes pH, EC, watering and flush recommendations (tested).
- [ ] Strain tolerance shifts alert thresholds (tested).
- [ ] Ingress: works under an arbitrary prefix with no absolute-path requests (automated network check). Rejects non-`172.30.32.2` in production mode. Photo upload works through ingress.
- [ ] Starts and works with HA unreachable, and recovers on reconnect.
- [ ] The token never appears in any browser-visible response or log (tested).
- [ ] EXIF/GPS stripped from stored photos (tested).
- [ ] Image builds for amd64 and aarch64 and installs as a local app. Sidebar tab shows the configured icon and title.
- [ ] Backup → restore keeps DB and photos.
- [ ] Screenshots committed. Lighthouse accessibility ≥ 90 on main views.

## 14. Working agreements
- Cite a source for every default number in the preset files. Where sources disagree, store the range and mark it.
- Pick the simplest option when ambiguous, log it in `docs/DECISIONS.md`, and keep going. Ask only about blocking choices.
- Out of scope for v1: multi-user accounts, cloud sync, native mobile app, sharing/selling features, automatic device control by default.
- Research snapshot date for all seed data and framework choices: 2026-11-08.
