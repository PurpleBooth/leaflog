# GOAL: Leaflog – a cannabis grow diary that lives inside Home Assistant

## 1. Mission
Build **Leaflog**, a self-hosted grow diary packaged as a **Home Assistant app (formerly "add-on")** that appears as its own **sidebar tab** via ingress. Backend and frontend are both **Rust**. It must look polished enough to feel native to HA.

"Done" means a user can:
1. Install Leaflog from a local app repository (amd64 + aarch64) and open it from the HA sidebar.
2. On first start, run a short onboarding wizard: log equipment basics, create a grow space, pick a strain, add plants (§4.12).
3. Create a grow from a strain, substrate and equipment list, and see predicted dates for every phase.
4. Log actions and photos, with HA sensor data attached automatically.
5. See live "in range / out of range" feedback and **phase-, substrate- and strain-aware recommendations** for pH, EC/TDS, temperature, humidity, VPD, lamp height and equipment setup.
6. Follow a feeding schedule that is stretched or shrunk to that plant's real timeline.
7. Record the harvest (wet/dry/trimmed weight per plant) and later see how each grow actually went against its forecast.
8. Look back across grows: yield by strain and substrate, forecast accuracy, per-plant timelines, and a one-tap "start next run".
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
- **Timezone-aware.** Store UTC internally. The canonical timezone for display and grow-day numbering is Home Assistant's (read from the documented `GET /api/config`; manual override in settings), so day numbers match the grower's wall clock even when the panel is opened remotely. Grow-day numbers count **local calendar days** (DST-safe), never UTC day flips. Quiet hours and `.ics` phase milestones (all-day events) follow the same timezone.

## 3. Architecture (record decisions in `docs/ARCHITECTURE.md`)
- **Backend:** Axum + Tokio + SQLite (sqlx or rusqlite) in `/data`, with versioned migrations.
- **Migrations are first-class tooling, not a hand-maintained list.** Files in `crates/leaflog-backend/migrations/` are discovered automatically (adding a file is the whole workflow); each applies in its own transaction; the applied history is checksum-verified against the embedded set, and a divergent or out-of-order history refuses to start. The backend binary gains a `migrate` subcommand (apply, status) for standalone runs while boot-time apply stays the normal path, and `just migrate new <name>` / `just migrate status` (plus a dev-only `redo`) wrap it for development. Use a maintained runner crate (`rusqlite_migration` is the leading candidate given rusqlite); log the pick in `docs/DECISIONS.md`. No external CLI in the app image.
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

### 4.1 Equipment catalogue and grow spaces
- The equipment **catalogue** covers durable hardware only — **lights, tents/rooms, fans (incl. carbon filters) and sensors**. Consumables and instruments aren't catalogued: nutrient products live in feed templates (§4.6), pH/EC/TDS readings are action-log fields (§4.2), media and pots are notes.
- The catalogue follows the strain-database pattern: bundled presets (product facts only) plus user-added entries. **Bundled presets are read-only** — edit and delete are rejected by the API, the UI marks them with a preset badge, and "clone" copies a preset into an editable user entry. Presets ship for lights, tents and fans only, never sensors.
- **Bundled data across app upgrades:** every bundled record (equipment presets, strains) carries a stable ID from its source dataset. On upgrade the app inserts bundled rows that are new and never modifies or deletes existing ones; user entries and clones of bundled records are never touched. A corrected bundled record ships as a new revision (new stable ID) with the old row left in place, so clones keep working; removing a bundled row entirely requires an explicit, documented migration.
- **Sensors are populated from Home Assistant.** A sensor item is created by picking an HA entity in an entity picker (`GET /api/states`, filtered by device class); v1 supports **temperature** and **humidity**. The entity is the item's identity, and a space's temperature + humidity items are offered as the default env bindings for grows in it (per-grow overrides stay, §4.9). With HA unreachable the picker defers — skip now, pick later — and nothing else blocks.
- Fields: name, brand/model, **real wall wattage** (not the marketing "equivalent"), dimmable (Y/N), PPFD map (lights only, optional, see 4.7), purchase date/price, notes, photos.
- Spaces *reference* catalogue entries — there is no per-item instance layer: a shared item (a clip fan carried between tents) may appear in more than one space, and two identical lights are two entries.
- A **grow space** is a named physical setup (for example a tent): optional interior dimensions (SI cm), notes, and a set of catalogue references. Grows run inside a space (optional); deleting a space keeps its grows. The space's most powerful light with a known wattage feeds the §4.7 advisor.
- Optional **HA entity binding** per item (e.g. `light.*`, power sensor, dimmer `number.*`); for sensor items the bound entity is the item itself (above).
- **Entity drift:** HA entity IDs change (renames, re-added devices). A binding whose entity is unavailable shows a status chip and a **rebind** affordance instead of silently reading stale or missing data. Alerts that depend on a dead binding degrade to an explicit "no data" state — never "in range" (§4.9).
- Future: a per-grow snapshot of the space's equipment ("what was used"), so later catalogue edits don't rewrite history (supersedes the earlier `grow_equipment` idea).
- Energy estimate (watts × runtime) using bound power/energy sensors where available.

### 4.2 Action log
- Quick-add chips: Lights on/off or schedule change, Water, Feed (volume, input pH/EC/TDS, runoff pH/EC/TDS, recipe), Trim/defoliate, Train (LST/top), Transplant, Flip to 12/12, Pest/IPM, Env change, Light height/dimmer change, Measurement, Photo, Note, Trichome check, Harvest, Dry/Cure check.
- Each entry has: timestamp, plant(s), auto-filled phase and day number, structured fields, free text, appearance tags (colour, droop, taco, bleaching, tip burn, foxtailing, etc.) and photos.
- **Auto-capture from HA:** snapshot bound sensors (temp, RH, VPD, soil moisture, CO₂, reservoir pH/EC/TDS, light state/dim level) into each entry.
- **Auto-log from HA:** lights on/off transitions and bound-sensor excursions become log events, with opt-out.
- **Snapshot retention:** auto-captured snapshots and auto-logged excursions would otherwise accumulate forever. A settings-configurable retention job (default 24 months) prunes old `sensor_snapshot` rows down to at most one per hour, always keeping the first and last row of each day and every excursion-linked row; it runs at startup and weekly. A database-size indicator sits beside the §4.3 photo-storage warning. Charts needing longer history read HA's history API on demand, so pruning costs no visible data.
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
- Fields: name, breeder, photoperiod/auto, feminized/regular, genetics (indica/sativa/hybrid % and landrace ancestry tags), **flower days (min/max)**, expected stretch factor, typical veg weeks, height class, auto total days from seed. Also yield notes, terpene/effect notes, **heat tolerance**, **cold tolerance**, **light tolerance**, user rating and notes, and an evidence tag per field (`breeder`, `grower-reported`, `inferred`).
- **Strain → recommendation modifiers.** Strain data adjusts:
  - the forecast (flower length, stretch, auto timing);
  - the temperature ceiling/floor alerts (heat/cold tolerance);
  - lamp-height caution (light tolerance);
  - the phase coach text.
- Heat/cold tolerance is a 3-level field (`sensitive`/`average`/`tolerant`). Default it from lineage when the user hasn't set it, and always show it as "inferred". Heuristics:
  - Autoflowers, short bushy broad-leaf indica-leaning plants and cold-climate lineages tend to be **more** heat-sensitive.
  - Sativa/Haze/African/Afghan/Hawaiian lineages tend to be **more** heat-tolerant, but can be shocked by cold.
  - Exceptions exist — some stout indica or auto cultivars are reported heat-tolerant.
- Tolerance shifts the *alert thresholds*, not the species rules. Default shifts (editable): tolerant +2 °F ceiling, sensitive −3 °F ceiling. These are heuristics, so label them as such.
- Users can add their own strains and clone any strain (bundled or own) as an editable "my phenotype" copy. **Bundled strains are read-only** — preset badge, edit/delete rejected, clone-to-modify — the same contract as bundled equipment (§4.1). Per-grow actual results (flower days achieved, max stretch, what temps it tolerated, achieved yield per §4.14) can be written back to user-owned strains only. Bundled strains follow the §4.1 upgrade rule.

### 4.5 Phase forecast
- Phases: germination → seedling → veg → (flip) → flower → harvest → dry → cure.
- Inputs: start date, strain, planned veg weeks (photoperiod) or auto age, overrides.
- Output per phase: start/end plus **earliest / likely / latest** window.
- Seed defaults (editable, sourced): germination 3–10 days, seedling 2–3 weeks, veg 3–8+ weeks (grower-chosen), flower 8–11 weeks (strain value wins; indica-leaning ~8–9, sativa-leaning at the top of the range or beyond). Stretch weeks 1–3 add about 50–100% height. Dry ~7–14 days (until stem snap), cure ≥14 days (ideally ~30). Autos start flowering ~3–5 weeks from seed and finish ~9–11 weeks from seed (some up to 13).
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
- When the grow has a space, the advising fixture is the most powerful light in that space with a known real wattage (plus whether any light is dimmable); otherwise a 330 W-class default applies (D021).
- Always show the units, say that the numbers are starting points, and recommend a PAR meter or a PPFD-meter log entry to calibrate.
- If a dimmer entity is bound **and** service calls are opted in, offer a one-tap "apply" with confirmation. Otherwise advise only.

### 4.8 Substrate-aware recommendations
Substrates: soil, coco, hydro (DWC/RDWC), plus user-defined (e.g. peat soilless, living soil). A grow's substrate modifies: pH target, EC/TDS band, watering/fertigation cadence, runoff checks, flush length, reminder text and coach tips. See §5.3 for the seed table. The coach must say *why*, e.g. "In coco, keep media wet and run 10–20% runoff to stop salt build-up".

### 4.9 Environment recommendations (pH, EC, TDS, humidity, temperature, VPD)
- Compute **VPD** from HA air temp + RH with a configurable **leaf-temperature offset** (default −2 °C / −3.6 °F; offer an IR-thermometer log entry to calibrate).
- Show a live VPD gauge and the target band for the current phase.
- Compute the **RH window** that would put VPD in band at the current temperature, rather than showing a fixed RH number only.
- Lights-on and lights-off targets differ, so use the bound light entity to know which applies.
- Alerts with hysteresis, quiet hours and per-strain thresholds. Examples: "Late flower, 83 °F, this strain is heat-sensitive: raise lamp or dim", "RH 62% in week 7: mould risk". An alert whose sensor binding is unavailable shows **"no data"**, never "in range".
- Env excursions can go out of band: under the §4.2 opt-in, temperature/RH/VPD and mould-risk alerts may also raise an HA notification — rate-limited (default: at most one per alert type per hour) and always inside quiet hours.
- Watering/feeding entries compare input vs runoff pH/EC and flag drift (§5.3).

### 4.10 Phase coach (suggested equipment setup)
A rules card per phase that lists targets, live status, and **recommended equipment matched to the user's own inventory first** (e.g. "Your 6 in inline fan: raise its speed in late flower for airflow and mould control"). Recommendations may name gear outside the four catalogue categories (a humidifier, a dehumidifier) as advice to acquire — such items are never catalogue entries (§4.1). Presets are stored as editable data, each with a short "why" and a source note.

### 4.11 Dashboard
Per grow: phase and day count, next milestone, today's feed target, lamp-height card, live sensor tiles, latest photo, last N log entries, and line charts (temp, RH, VPD, soil moisture, pH/EC/TDS) with log events overlaid as markers.

### 4.12 Onboarding wizard
- A slick first-run flow for a fresh install: when there are no grow spaces, no grows and no prior dismissal, the app opens a mobile-friendly wizard that ends with a first grow running. Every step can be skipped, progress persists across restarts, and the empty-state "set up" card restarts it.
- Steps, in order:
  1. **Equipment basics** — pick bundled presets (light, tent, fans), add own entries or both; bind HA sensors, temperature + humidity at minimum (picker per §4.1; deferred when HA is unreachable).
  2. **Grow space** — name it, optional dimensions; created from the equipment just picked.
  3. **Strain** — pick a bundled strain, an existing own strain, or add a new one.
  4. **Plants** — name and count the plants; finishing creates a grow inside the space (germination defaults to today, editable) and lands on its dashboard.
- The wizard gates nothing: every screen stays reachable, and dismissing it leaves the normal empty dashboard.

### 4.13 Seedbank
- A basic log of owned seeds, per batch: strain (linked to the strain database), quantity, acquired from (bank / breeder / own harvest), acquisition date, storage method (default cool/dark/dry per §5.4), notes.
- Batches list **oldest first** within a strain (use the oldest seeds first) and show the §5.4 viability window, with a "past prime" badge once it has passed.
- Starting plants from a batch (onboarding step 4 or normal grow creation) may link to it and decrement the count; manual edits always win. Informational only — no reminders beyond the badge.
- **Germination-test log** per batch: date, number sown, number sprouted (paper-towel or direct). The batch then shows the observed germination rate next to the §5.4 window — the rate is user evidence that sits beside the preset, it never overwrites it. The §5.4 germination-test reminder links to this log.

### 4.14 Harvest, dry & cure
- The Harvest action gains structured fields: wet weight, dry weight and trimmed weight — per plant, all optional, grams internally, displayed per HA units (g/oz). Logging it completes the forecast and freezes the actual timeline.
- A **yield summary** on the finished grow: totals, per-plant breakdown and the wet→dry ratio.
- Dry and cure stay action-log-driven (the Dry/Cure check chips) with the §4.2 reminders (burp jars); dry-room conditions come from the auto-captured sensors bound to the space. No jar inventory in v1.
- Weights feed the §4.15 analytics and the §4.4 write-back (user-owned strains only).

### 4.15 Cross-grow history & analytics
- A **History** section over all grows: predicted-vs-actual phase accuracy (per grow and in aggregate), yield by strain, substrate and feed template, input-vs-runoff pH/EC drift over time, and alert frequency over time.
- **Per-plant views**: timeline, photos, actions and final yield for one plant within a grow.
- **Start next run**: one tap creates a new grow copying strain, substrate, grow space (with its equipment references) and feed template from an existing grow. References are copied as references — bundled entries are never duplicated.
- Everything here is computed read-only from existing tables; no new capture flows.

## 5. Grow Science Pack (seed data, editable, each row carries a source label)
**Rule:** the sources below disagree in places. Store ranges, never single magic numbers. Show "sources vary" where they do. Every preset is user-editable, with a "reset to default" button. Re-check each number against at least two sources before committing it. If you can't, mark it `unverified`. (User-editability applies to the numeric grow-science presets; bundled catalogue records — equipment and strains — are read-only, see §4.1 and §4.4. On app upgrade, user-edited presets keep the user's values; "reset to default" pulls the newly shipped defaults — the §4.1 upgrade rule.)

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
- **These are contested.** Another source gives only ~600–800 ppm for soil flowering, and growers commonly say published feed charts run high. Default the active preset to the **lower half** of each band. Make explicit that these are starting points and that product-label dosing for the user's own nutrients is what counts.
- Runoff-vs-input guidance (editable): runoff a few hundred ppm above input is normal in coco. If runoff is far above input (e.g. ≥1000 ppm), suggest a flush. Runoff below input suggests the plant is feeding hard.

### 5.4 Seed viability (seedbank windows)
Ranges from acquisition date, by storage method — sources disagree, so re-check against at least two before encoding and mark anything unresolved `unverified`:
| Storage | Full-viability window (typical) |
|---|---|
| Room temperature, dark drawer | ~6–12 months, then germination declines noticeably |
| Cool, dark, dry cupboard (default) | ~1–3 years |
| Refrigerated, airtight (~4–8 °C) | ~5 years; reports up to ~10 |
| Frozen, dried, airtight | 10+ years reported |

- The window counts from acquisition (bank packing date when known, else purchase/receipt date). Past the window means *falling germination odds*, not an expiry: old batches get a "past prime" hint and a germination-test reminder, never a hard block.
- Storage conditions dominate; seed type barely moves the window. Don't encode per-strain viability numbers without sources.

## 6. UX / visual design
- Dark and light themes following `prefers-color-scheme`, with a manual override. Optional best-effort match to the HA theme.
- Calm botanical palette, cards, generous spacing, rounded corners, subtle motion, good empty states and skeletons. Phase accents (seedling green → flower purple/amber → cure warm brown).
- Phone-first for logging, wide layout for charts, timeline and photo compare. Large touch targets.
- WCAG AA, keyboard navigation, no colour-only signals.
- **Localisation: English and German.** Language follows the browser, with a manual override in settings. Externalise every UI string through `leptos_i18n` 0.6 (the line that targets Leptos 0.8, needing ≥ 0.8.15; re-check on the 0.9 upgrade), with locale-aware number and date formatting — German renders decimal commas. App-store metadata translations (§7) ship both languages too. CI fails on a key missing in either locale.
- Screenshots in `docs/` (desktop + mobile, light + dark).

## 7. Home Assistant packaging
- Repo: `repository.yaml` plus an app folder with `config.yaml`, `Dockerfile`, `rootfs/etc/s6-overlay/` (s6 longrun service), `README.md`, `DOCS.md`, `CHANGELOG.md`, `icon.png`, `logo.png`, translations.
- `config.yaml`: `arch: [aarch64, amd64]`, `ingress: true`, `ingress_port: 8099`, `panel_icon: mdi:cannabis`, `panel_title: Leaflog`, `panel_admin: true`, `homeassistant_api: true`, `init: false`, `map` for `data`, options schema — **bootstrap-level only**: units, TDS scale, log level and the service-call opt-in; entity bindings, alert thresholds and presets live in-app, and where both could apply the options are initial seeds that in-app settings override — and a multi-arch `image:` name.
- **Backups:** default is `backup: hot`, so checkpoint SQLite via `backup_pre`, or choose `cold`. Exclude thumbnail caches. Prove backup → restore keeps data and photos.
- **Dockerfile:** multi-stage with BuildKit, musl targets for both arches, and a **pinned** HA Alpine base (`ghcr.io/home-assistant/base:<tag>`), never `latest`. `BUILD_FROM` is no longer passed by default, so use a fixed `FROM` or give it a default. The musl cross-compilation happens inside the container build (document how: per-arch runners, or a pinned cross linker), so the Devbox environment doesn't need musl targets.
- GitHub Actions to build and publish a multi-arch image, plus a local-build path.
- Re-check the HA developer docs before each milestone. Naming and behaviour are changing ("add-ons" are now "apps").

## 8. Data model (starting point, refine)
`strain` (+ `is_builtin`), `equipment_item` (+ `source`, `is_builtin`; sensor items carry their HA entity), `seed_batch`, `fixture_ppfd_map`, `grow` (+ `grow_space_id`), `plant`, `grow_space`, `grow_space_equipment`, `substrate_profile`, `phase_plan`, `phase_event_actual`, `action_log`, `photo`, `log_attachment`, `sensor_binding`, `sensor_snapshot`, `feed_template`, `feed_template_row`, `feed_plan_day`, `preset_env`, `preset_light`, `preset_substrate`, `alert_rule`, `germination_log`, `settings`, `schema_migrations`. ULIDs, UTC timestamps, SI units internally; harvest weights are nullable columns on `plant` (`wet_g`, `dry_g`, `trimmed_g`). `settings` also carries: language and display-timezone overrides, and the snapshot-retention window. Spaces reference catalogue entries (many-to-many `grow_space_equipment`); the planned `grow_equipment` snapshot is superseded by spaces for now (see 4.1). Onboarding dismissal is a `settings` flag.

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
- CI runs `cargo clippy --workspace --all-targets -- -D warnings` for the native target **and** for `wasm32-unknown-unknown` on the frontend crate, plus `cargo fmt --check`. Dependency hygiene is part of the same gate: `cargo deny` (config in `deny.toml`) fails the build on RustSec advisories and on licences outside the allowlist.
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
- M1 — skeleton + packaging + ingress "hello" tab + Leptos runtime-base spike
- M2 — DB, equipment, strains
- M3 — action log + photo log
- M4 — HA live data + history
- M5 — forecast + timeline
- M6 — feed fit + substrate profiles
- M7 — VPD/env engine, lamp advisor, phase coach, dashboard polish
- M8 — hardening, docs, CI, screenshots
- M9 — catalogue trim, HA sensor items, read-only presets and bundled-data upgrade rules, migration tooling
- M10 — onboarding wizard + seedbank
- M11 — harvest & yield, germination logging, cross-grow history/analytics, per-plant views, start-next-run
- M12 — snapshot retention, entity-drift rebinding, env-alert HA routing, EN+DE localisation

(`cargo deny` joins CI immediately, not gated on M12.)

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
- [ ] Bundled equipment and strains reject edit and delete at the API (tested) and carry a preset badge in the UI; clone-to-edit yields a user entry.
- [ ] Sensor items come only from the HA entity picker (temperature + humidity device classes); with HA unreachable the step defers instead of blocking.
- [ ] Onboarding: on a fresh database the wizard appears and equipment → space → strain → plants produces a grow on the dashboard; skip and resume both behave (tested).
- [ ] Seedbank: oldest-first ordering, §5.4 window shown, linked plant starts decrement the count (tested).
- [ ] Migration tooling: `just migrate new/status` and the `migrate` subcommand work; boot applies pending files and refuses a divergent or out-of-order history (tested).
- [ ] Harvest: wet/dry/trimmed weights per plant round-trip (SI internally, correct display units); yield summary and wet→dry ratio are correct; write-back of achieved yield reaches user-owned strains only (tested).
- [ ] Germination log: recorded sown/sprouted shows as an observed rate beside the §5.4 window (tested).
- [ ] History: predicted-vs-actual accuracy correct on a golden grow; "start next run" copies strain, substrate, space and feed template by reference without duplicating bundled entries (tested).
- [ ] Bundled-data upgrade: on a simulated upgrade, new bundled rows are inserted, existing bundled rows untouched, user clones and user-edited presets keep their values (tested).
- [ ] Retention: the prune keeps at most one snapshot per hour, the first and last row of each day, and every excursion-linked row inside the window (tested).
- [ ] Entity drift: an unavailable bound entity renders "no data" — never "in range" — and offers rebinding (tested).
- [ ] Timezone: grow-day numbering across a DST boundary matches local calendar days; quiet hours follow local wall-clock (golden test).
- [ ] Localisation: no message key missing from EN or DE (CI gate); German decimal-comma rendering spot-checked.

## 14. Working agreements
- Cite a source for every default number in the preset files. Where sources disagree, store the range and mark it.
- Pick the simplest option when ambiguous, log it in `docs/DECISIONS.md`, and keep going. Ask only about blocking choices.
- Out of scope for v1: multi-user accounts, cloud sync, native mobile app, sharing/selling features, automatic device control by default.
- Research snapshot date for all seed data and framework choices: 2026-11-08.
