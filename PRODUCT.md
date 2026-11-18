# Product

<!-- impeccable:product-schema 1 -->

## Platform

web

## Users

The instance owner: a home grower who already runs Home Assistant. Single
user per install (`panel_admin: true`; multi-user is explicitly out of scope
for v1). Usage scene is phone-first at the tent or grow space — quick
logging, checking alerts and live ranges — with wide desktop layouts for
charts, timeline and photo compare.

Leaflog is a **published community product in its early stages**: any HA
self-hoster can install it via HACS or the add-on store, so onboarding,
defaults and robustness must hold up for strangers, not just the owner. The
owner is in Germany; the app ships English and German.

## Product Purpose

Leaflog is a self-hosted cannabis grow diary packaged as a Home Assistant
app (sidebar tab via ingress). It records grows, plants, actions and photos
with automatic sensor capture from bound HA entities, then turns that
record into guidance: phase forecasting (earliest/likely/latest), feed
schedules fitted to the plant's real timeline, and VPD / lamp-height /
substrate-aware coaching. Harvest records and cross-grow history close the
loop from one grow to the next.

Success means the owner **learns to grow better**: every recorded
experience carries forward into future grows (history, forecast accuracy,
one-tap "start next run"). Success also means the app **shortens the
unhappy-plant loop** — from "why does my plant look unhappy" to "how do I
fix that" to "I need to do X" in as few steps as possible. The
`docs/SPEC.md` §1 done-definition remains the acceptance-level truth.

## Positioning

The grow diary that is native to Home Assistant: automatic environmental
capture from sensors the user already has, pure-Rust grow-science engines
(forecast, feed-fit, VPD, lamp height, recommendations), and fully local,
private operation. Cloud grow journals and generic note apps cannot
truthfully claim the HA sensor integration or the local-first guarantee.

## Operating Context

- Installed as an HA app via ingress; no login screen — HA has already
  authenticated the user.
- Must work fully with HA unreachable and reconnect with backoff; live
  updates over one WebSocket/SSE channel (browser never talks to HA).
- SQLite in `/data` with versioned migrations; photos in `/data/photos`
  with WebP thumbnails (EXIF/GPS stripped); whole-grow JSON export/import
  plus CSV for logs and sensor history.
- SI units stored internally; display follows the HA unit system and
  locale (°C/°F, in/cm, gal/L). EC canonical in µS/cm; TDS always derived
  via the user's meter scale and labelled.
- UTC internally; HA's timezone (manual override possible) is canonical
  for display and grow-day numbering — local calendar days, DST-safe.
- Domain vocabulary (phases, appearance traits, grow-day) per root
  `GLOSSARY.md` and `docs/SPEC.md` §4; the spec is the source of truth.

## Capabilities and Constraints

Confirmed shipped (v0.1.0, 2026-11-08; milestones M1–M8 complete, feature
strip D075 applied — no strain catalogue, seed bank, grow spaces or
equipment catalogue):

- Grows carry strain label, photoperiod toggle, heat/cold/light tolerances
  and lamp facts; plants, action log (quick-add chips, 3-tap mobile entry)
  and photo log (gallery, side-by-side compare, time-lapse player).
- Per-grow sensor bindings to HA entities; auto-capture into log entries,
  auto-logging of light transitions and excursions (opt-out), snapshot
  retention pruning.
- Phase forecast with re-forecast from logged events; phase picks are
  events; feed schedules stretched/shrunk to the real timeline.
- VPD, lamp-height advisor and substrate-aware coaching with alert
  thresholds shifted by the grow's tolerances; entity drift degrades to
  explicit "no data" with rebind affordance, never silent "in range".
- Harvest wet/dry/trimmed per plant; cross-grow history: predicted-vs-
  actual accuracy, yield by strain label and substrate, "start next run".
- Onboarding wizard (bind sensors → plants, strain label inline); EN + DE
  localisation (CI-gated key parity).

Non-negotiable constraints (from AGENTS.md / SPEC §2):

- Rust only for app code; no Node runtime in the final image.
- Local-first: no telemetry, no cloud calls at runtime; user-triggered
  imports only. No sharing/social/public URLs.
- Ingress-correct: relative URLs everywhere, runtime `X-Ingress-Path`
  injection, accept connections only from `172.30.32.2` in production.
- Supervisor token is admin-level: never sent to the browser, never
  logged, read-only calls by default; `call_service` behind explicit
  opt-in.
- Presets are sourced ranges, never single magic numbers; every preset row
  carries a source label; bundled preset data is third-party and licence-
  checked before bundling.
- Out of scope for v1: multi-user accounts, cloud sync, native mobile app,
  sharing/selling features, automatic device control by default.
- Owner in Germany: plant-count limits and access-securing reminders are
  informational only (default off outside DE).

## Brand Commitments

- Name **Leaflog**; panel icon `mdi:cannabis`; panel title "Leaflog".
- Voice: lean professional horticulture — calm, precise, credible. Never
  a stoner-cliché register or joke-y drug references.
- The UI must adopt the user's Home Assistant settings' stylings (HA
  theme; spec §6 currently words this as best-effort match — the owner
  has reaffirmed HA-theme adoption as the goal, so treat it as the
  direction product work pushes toward).
- Dark and light themes following `prefers-color-scheme` with manual
  override.
- The spec pins a calm botanical palette with phase accents (seedling
  green → flower purple/amber → cure warm brown); visual execution
  belongs to DESIGN.md, not here.

## Evidence on Hand

- `docs/SPEC.md` — full product spec, source of truth (research snapshot
  2026-11-08).
- `docs/ARCHITECTURE.md`, `docs/DECISIONS.md` (D-numbered decisions),
  `docs/THREAT-MODEL.md`, `GLOSSARY.md`.
- `CHANGELOG.md` / `leaflog/CHANGELOG.md` — release history via
  conventional commits.
- `docs/ACCESSIBILITY_AUDIT.md` — manual Lighthouse-mapped audit, PASS
  (estimated ≥ 90).
- `docs/screenshots/` — desktop + mobile, light theme only;
  `desktop-equipment-light.png` and `desktop-strains-light.png` show
  pre-D075 surfaces (equipment/strain screens since removed) — refresh
  before using as current-state evidence.
- No users, testimonials, adoption numbers, benchmarks or pricing exist
  yet; future work must not fabricate any.

## Product Principles

1. **The diary teaches.** Every recorded grow must carry forward —
   cross-grow history, forecast accuracy and one-tap "start next run" are
   core, not extras.
2. **Shorten the unhappy-plant loop.** From observed symptom to diagnosis
   to next action in as few taps as possible.
3. **Home Assistant native.** Sensors, theme, units, timezone and
   authentication come from HA; the app stays local-first and private.
4. **Professional register.** Every string, state and empty-screen moment
   reads as serious horticulture, never stoner cliché.
5. **Ranges with sources over magic numbers.** Presets are editable,
   sourced and marked where the science disagrees.

## Accessibility & Inclusion

- WCAG AA: keyboard navigation, visible focus, no colour-only signals;
  touch targets ≥ 44 px; Lighthouse accessibility ≥ 90 on main views.
- EN + DE UI following the browser language, manual override in settings;
  locale-aware number and date formatting (German decimal commas).
- Phone-first: core logging flows work one-handed on mobile; analysis
  surfaces use the wide layout.
