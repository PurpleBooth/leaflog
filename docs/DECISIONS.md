# Leaflog Decision Log

One-off choices, in reverse-chronological order. Every `#[allow]` exception
to the mandatory lint policy (see AGENTS.md / SPEC.md §10) must be logged
here. Date format: YYYY-MM-DD.

---

## 2026-11-20 — adopted-theme colour serialization + chart tooltip/series fixes (D135)

### D135: adopted theme variables serialize as chart-safe sRGB; the chart probes reject colours the renderer cannot parse; the main line travels under a wire name

Three defects found together on an adopted-theme install (the built-in
hex themes never reproduced either one, so the chart e2e stayed green
while the product was broken):

- **Adoption format.** D100 pinned the inline overrides as
  `oklch(...)` custom properties. The CSS engine wears oklch happily,
  but the ECharts/zrender colour parser handles only named colours,
  hex and `rgb()/rgba()/hsl()/hsla()` — everything else silently
  paints nothing, so under every adopted theme the accent-stroked
  marks (data line, Target Band, scatter dots) vanished while
  shading, grid and tooltips kept rendering. The overrides are now
  written as comma-syntax `rgb()`/`rgba()` (`rgb_string` in
  `leaflog-core::theme`, channels clamped and rounded to CSS
  integers): sRGB is the lowest-common-denominator every consumer
  understands. `rgb_to_oklch` stays — the scheme detection still
  derives dark/light from the page background's oklch lightness.
  This amends D100's serialization sentence in place; everything else
  in D100 stands.
- **Probe hardening.** `theme_look`'s computed-style probes now
  reject anything outside the renderer's colour grammar
  (`chart_safe_color`: hex or rgb/rgba/hsl/hsla prefixes) and fall
  back to the built-in palette values instead of handing an
  unparseable string to the option builder. This also catches
  Tailwind's opacity-modifier classes (`text-base-content/50`
  computes to a `color-mix()` result the renderer cannot read),
  which previously reached zrender raw and silently misrendered.
- **Tooltip wire name.** The single-line charts were the only unnamed
  series, and the renderer fills synthetic `seriesN` names for
  unnamed series, so the tooltip leaked a bogus `series0:` prefix.
  The main line now travels under the `MAIN_SERIES_NAME` wire token
  ("Readings", alongside the existing `MARKER_SERIES_NAME`), which
  the tooltip renders plain — value — time — because the card title
  already names the role; the pair charts' locale names keep their
  prefixes.

Verified end-to-end against a hand-rolled HA-style hex parent page:
the adopted root reads `--color-primary: rgb(124, 77, 255)` for a
`#7c4dff` parent, the idle line stroke is that exact string, and the
crosshair sync is untouched. New e2e `27-adopted-theme-charts.spec.ts`
pins the regression; spec 15 additionally pins the stroked line and
the series-name absence on the plain (non-ingress) path.

## 2026-11-20 — overview and dashboard cleanup (D131–D134)

### D131: the overview timeline groups manual entries under day headings; Home-Assistant light rows collapse into a quiet auto tier keyed on the `auto:` note prefix

- `day_groups` sorts manual entries by descending grow day and keeps
  arrival order inside a day; each section heading is the German/English
  day label plus the phase badge of its first entry. Entry cards keep
  the day badge so the phase-day e2e pins stay valid.
- The auto tier renders only when at least one entry has a note
  starting with `auto:` — the same convention the retention sweeper
  uses to recognise machine-written rows. No schema change; if a user
  hand-writes an `auto:` note it simply lands in the quiet tier, which
  is the correct reading of such a note anyway.
- Auto rows are one-line (label, truncated note, timestamp, small
  delete) under an "Automatic (Home Assistant)" heading; they stay
  undoable like any other entry (D126).

### D132: the root-zone lazy reservation is conditional — reserved while bindings are unknown, dropped only when nothing can render there

- The six-hundred-pixel `min-h` reservation for the lazy root-zone
  charts was the page's largest dead void on grows without soil
  sensors. `EnvironmentSection` now reserves the block only while the
  bindings fetch is in flight, or when something can render there:
  any bound root-zone role (soil moisture, pH, EC), or a feed log
  with pH/EC values. The pH and EC charts read the log, not
  bindings, so a feed-only grow still has a chart to show.
- The ROOT ZONE micro-label always renders; only the reserved chart
  grid disappears. In-flight keeps reserving to avoid layout jump;
  a failed bindings fetch degrades to "reserved" (banner path).

### D133: the Verlauf history page becomes a dashboard — summary band first, three carded aggregates, no trailing back-link

- A `statband` card leads the page: grow count, average **absolute**
  flip/harvest deviation in days (always plural, one decimal, `—`
  until a boundary exists) and dry yield with wet/trim subtotals. The
  yield total prefers the first non-empty grouping of label →
  substrate → feeding schedule, matching the aggregate cards below.
- The three aggregate tables are proper cards in a responsive
  `lg:grid-cols-3` grid; the trailing "back to home" link is removed —
  the header navigation owns movement between pages.
- Numbers use tabular numerals via a `.table` rule so columns align.

### D134: grow list and form surfaces localize substrate and photoperiod words; raw wire values never render

- `substrate_label` / photoperiod options map through the locale
  catalogues (Erde/Coco/Hydro, Fotoperiode/Autoflower in the form;
  the card shows the adjective Fotoperiodisch/Autoflower). The card
  second row is strain label · photoperiod · substrate, third row the
  keyed "started" date; the glyph stays (established vocabulary).
- The schedule library dose units render as `ml/L` / `g/L` through the
  same echo-arm pattern instead of the raw `ml_per_l` serde label.

## 2026-11-19 — feeding schedules replace EC feed templates (D128–D130)

### D128: a feeding schedule is a product-dose grid; EC/pH targets leave the feed model and the three builtin EC templates die unconverted

- **Model** (ADR 0004): `FeedSchedule` carries `name`, free-text
  `source`, `flower_weeks` (clamped 1–24) and its own `ScheduleProduct`
  rows (free-text name/group/note, unit `ml_per_l` | `g_per_l`,
  `ProductDoses { propagation, veg, flower: week-indexed, water }`).
  `Dose { min, max }` stores SI micro-steps (µL/L, mg/L, ×1000 steps);
  display and input run through `format_dose`/`parse_dose_text` with
  locale decimals (en-dash ranges, comma decimals, inverted ranges
  tolerated). No product catalogue — schedules own their rows, free-text
  groups do the visual grouping (≤ 64 products, client-validated).
- **Fit**: `fit_schedule` time-warps the grid onto the grow's actual
  timeline at week granularity; the WATER column pins to the substrate
  flush and is never dropped. The result freezes into `feed_plan` +
  `feed_plan_day` (doses as JSON) per grow; the overview renders from
  plan days alone so finished grows and deleted schedules still show
  their frozen plan. Editing a schedule refits every grow that uses it.
- **Migration V19**: drops `feed_template`, `feed_template_row` and all
  EC `feed_plan_day` rows, renames `grow.feed_template_id` →
  `feed_schedule_id`. **No EC→dose conversion**: a converted guess would
  read as authoritative while being fiction. The library ships empty by
  design; `tests/upgrade.rs` pins the empty-library invariant.
- **History**: `yield_by_feed_template` becomes
  `yield_by_feed_schedule`, aggregated by schedule name (unassigned
  grows under "none") — schedule names are user-owned and may collide;
  accepted for a single-user diary.

### D129: the grow→schedule reference has no database foreign key; integrity lives in the store layer
- **Constraint**: SQLite cannot retarget or add an FK column in place,
  and a table rebuild inside a refinery migration runs with
  `PRAGMA foreign_keys=ON` (set before `migrate()` in `db.rs`; the
  pragma is a no-op inside the per-migration transaction, so a
  migration cannot turn it off). Rebuilding `grow` under FK-ON would
  fire its cascades; `writable_schema` hacks are disqualified. V19
  therefore adds `feed_schedule_id TEXT` without a REFERENCES clause.
- **Integrity in Rust**: grow create/update 404 on an unknown schedule
  id (`try_get_schedule`, the same pattern the template validation
  used); schedule delete nulls every referencing `grow.feed_schedule_id`
  in the same transaction before dropping the schedule's rows. Orphaned
  ids can only appear by direct database edits; the reader treats an
  unresolvable id as unassigned.
- **Semantics**: schedules outlive grows. Deleting a grow never touches
  the schedule; deleting a schedule leaves finished grows with their
  frozen plan (rendered from `feed_plan` alone).

### D130: an assigned schedule with no forecast flowering window shows a pending card, not a fabricated plan

- **Constraint**: `fit_schedule` needs the grow's actual timeline,
  and the forecast produces a flowering window only once the grow has
  one — an autoflower from photoperiod + planned veg weeks, a
  photoperiod grow only after the flip is logged. Fitting "something"
  pre-flip would invent a flip day: the this-week card would print
  flowering week numbers while the plant is still in veg.
- **Decision**: `fit_grow` returns `None` when the forecast has no
  flowering window (the grow keeps its previous plan, if any) and the
  overview renders the assignment as a **pending card** (schedule
  name, "the plan appears once the grow's flowering window is known",
  link to the schedule) instead of the quiet row. Create and update
  still fit immediately when the window exists; flip, phase-pin and
  harvest writes re-fit (best-effort — an incomplete forecast leaves
  the stored plan untouched).
- **Cost**: a photoperiod grow shows propagation/veg guidance only
  after the flip. Accepted: honest gaps beat invented weeks.


## 2026-11-19 — recents in the quick-add dialog picker (D127)

### D127: the quick-add dialog remembers the last three picks in localStorage, most recent first, and degrades to no recents when storage is unavailable

- **Storage**: `localStorage` key `leaflog.quick-add-recents`, namespaced
  like `leaflog.history-window` in env_section.rs. Values are the
  actions' stable `snake_case` keys (`ActionType::label`) — the same
  encoding the database and API use, stored as a JSON array; no second
  key mapping exists. The list is capped at 3 entries, most recent
  first, and a re-pick moves the entry back to the front. The pure
  rule lives in quick_add.rs (`update_recents`) with unit tests;
  unknown or stale keys degrade to "not recent" on load.
- **Degradation**: private mode or disabled storage yields "no
  recents" — the dialog renders only the taxonomy groups and never
  errors, following the history-window load/save pattern's error
  handling. A failed write keeps the session's in-memory recents.
- **Scope**: only picks made through the dialog are remembered
  (`PhaseChange` never reaches it); the list is read once when the
  dialog opens, not reactively. Recents render through the same group
  renderer as the taxonomy chips, so a recent pick behaves exactly
  like its taxonomy chip — same draft reset, close and focus
  semantics (D069).

## 2026-11-19 — undoable entry-level destructive actions (D126)

### D126: entry deletes and light-cycle resets are undoable within 8 s instead of confirmed; the whole-grow delete keeps its confirmation

- **Undo policy**: a per-entry delete on the timeline and the light-cycle
  reset no longer ask for confirmation — the destructive step happens
  immediately and an undo banner offers the reversal for
  `ui::UNDO_WINDOW_MS` (8 s). The whole-grow delete on the edit page
  **keeps** its confirmation dialog: its cascade removes photos and
  photo files, which no client-side capture can bring back. Entry-level
  deletes only ever lose one log row, and the data needed to rebuild it
  travels with the banner.
- **What undo captures**: the timeline's undo holds the deleted entry's
  `ActionEntry` reduced by `entry_to_input` to the exact `ActionInput`
  a fresh create would carry — `ActionEntry` is a strict superset of
  `ActionInput` (id, grow id, phase, grow day and timestamps are all
  backend-filled), so the reconstruction is faithful and has **no
  gap**. Undo re-POSTs that payload to the ordinary create endpoint:
  the backend mints a **new id** and re-derives phase and grow day from
  the same `occurred_at` (deterministic), so the restored row reads
  identically. Ids are internal, so the new id is acceptable; the e2e
  spec asserts the new id explicitly. The light-cycle reset captures
  the form's current `lights_on`/hours before deleting and undo re-PUTs
  them through the ordinary save endpoint.
- **One banner, last delete wins**: rapid deletes push onto a pending
  stack; the banner always names the **newest** record
  (`ui::live_undo`). Undo or dismissal clears the whole stack, and a
  lapsed window hides it — an earlier delete never resurfaces after a
  newer one, and its undo expires with the banner. The pure
  recency/expiry rule lives in `ui.rs` (`undo_is_live`, `live_undo`)
  with unit tests; the banner component (`ui::UndoBanner`) re-arms its
  8 s auto-dismiss on every new record, is `role=status` /
  `aria-live=polite`, and animates nothing.
- **Failed deletes roll back their own capture**: each pending record
  is keyed by the deleted entry's id, so a rejected DELETE drops
  exactly its own record — Undo must never re-create a duplicate of an
  entry that is still on the timeline. Keying by id rather than
  timestamp keeps this exact even if two rapid deletes share a
  millisecond.
- **Cast exceptions**: `ui::now_ms` narrows `js_sys::Date::now()`'s
  `f64` to `u64` under
  `#[allow(clippy::cast_sign_loss, clippy::cast_possible_truncation)]`
  — the value is a positive, finite millisecond count. The banner's
  timer converts `UNDO_WINDOW_MS` with `u32::try_from(...).unwrap_or(0)`
  (gloo's `Timeout::new` takes `u32`), following the env_section arming
  timer's idiom, so no cast allow is needed there.

## 2026-11-19 — the unsaved-changes guard on the grow edit page (D125)

### D125: dirty-form guard without a router blocker — capture-phase click guard, pure snapshot seam, native beforeunload

- **Mechanism**: leptos_router 0.8 ships no navigation blocker API, so
  the edit page answers for itself while its form is dirty. A
  document-level capture-phase click listener runs before the router's
  global anchor handler (bubble phase), which honours
  `default_prevented`: preventing the event suppresses the SPA
  navigation, the page asks `window.confirm` with the localized
  `grow_edit.unsaved_confirm` once, and a confirmed leave re-navigates
  through the router with the plain route path (the runtime ingress
  base is stripped back off the anchor's `href` — a base-prefixed path
  handed to `navigate` would double the prefix). The in-page Cancel
  button is a button, not an anchor, and runs the same dirty check
  explicitly. Modified clicks (ctrl/meta/shift/alt) and anchors with
  an explicit `target` open elsewhere and never abandon the page, so
  they are not intercepted; `route_path_from_href` returns `None` for
  anything that is not an in-app route path (foreign URLs, hash
  links) and the guard leaves those alone too.
- **Snapshot seam**: `FormSnapshot` + `edits_dirty(before, after)` in
  edit.rs — a plain `PartialEq` struct over exactly the form-editable
  fields (the fitted feed template is not form-editable and stays
  out). Text fields trim at snapshot time and blank notes read as
  `None`, matching the form's `non_empty` write path, so whitespace
  churn is never an edit worth a confirm — but blanking previously
  set notes is one. The baseline (`StoredValue`) anchors when the grow
  loads and re-anchors after a successful save, before that save
  navigates, so the guard never fires on the path home after a
  write. Save failures leave the baseline untouched: the form stays
  dirty, exactly what the guard should report.
- **beforeunload scope**: while dirty, a window `beforeunload`
  listener prevent-defaults and sets `returnValue`, giving the
  browser's native prompt for reloads and tab closes. It is
  deliberately not localized (browsers render it themselves) and not
  duplicated in-page; it is not covered by e2e (headless Chromium
  cannot surface it) — the two listener paths share the same dirty
  check the anchor guard exercises.
- The delete flow keeps its own confirmation: the danger-zone button
  navigates only via the `deleted` signal's `<Redirect>` after the
  API call succeeded, and no anchor is involved, so the guard never
  fires on it.

---

## 2026-11-19 — debug-only spike route and the status-first health card (D124)

### D124: the `/spike` route is cfg-gated to debug builds; the health card leads with a status line

- **Spike gating mechanism**: `#[cfg]` cannot sit on a single view
  element inside `view!`, and an interpolated `{expr}` child of
  `<Routes>` is forced through `IntoRender` by the view macro, so a
  helper returning the `<Route>` tuple cannot satisfy the typed
  `RouteChildren` bound. The whole routes segment is therefore
  cfg-paired as two `routes()` functions in leaflog-frontend/src/lib.rs:
  the debug arm registers all seven routes (including `/spike`), the
  release arm the same six without the spike, and AppShell renders
  `{routes()}` as a plain view child. `trunk build --release` compiles
  without the `Spike` component or its imports; the `page.spike_*`
  i18n keys stay in the catalog for debug builds. The e2e locale sweep
  (25-locale.spec.ts) drops its spike assertions and instead pins the
  new health status line.
- **Health card design**: the landing card showed the raw
  `api/health` JSON in a `<pre aria-live="polite">`. It now leads with
  a status line — localized `page.health_ok` / `page.health_unreachable`
  beside a small dot in the existing palette roles (`bg-primary` /
  `bg-error`, no new colors), still `aria-live="polite"` — and keeps the
  raw payload for honest diagnosis inside a `<details>` labelled
  `page.health_details` (`page.health_read_error` /
  `page.health_network_error` text lands in the details block and reads
  as unreachable). Healthiness is decided by the pure helper
  `api::health_ok`, which tolerantly parses `{ "status": "ok", ... }`
  and errs toward unreachable on any garbage (unit-tested).

---

## 2026-11-19 — locale-aware decimal formatting and the German copy pass (D123)

### D123: pure decimal formatter, one Locale→style mapping, backend-compat note, German grow terms

- **Mechanism**: `leaflog_core::format::format_decimal(value, decimals,
  DecimalStyle)` — a pure, no-`Intl` formatter (local-first; wasm-safe).
  Rust's `{:.N}` stays the rounding authority (half-to-even on exact
  binary ties); the module only swaps separators and inserts thousands
  grouping, so a rounded carry regroups for free (`999.995` at one
  decimal → `1,000.0` / `1.000,0`). `DecimalStyle` is `Dot`/`Comma`;
  the frontend maps `Locale`→style in exactly one place
  (`decimal_style(locale)` in leaflog-frontend/src/lib.rs). The chart
  axis/tooltip closures and the dashboard/timeline formatters receive
  the style at their existing locale read, so a locale swap rebuilds
  with the right separators.
- **Backend-compat choice**: `units.rs` helpers (`temp_both`,
  `temp_preferred`, `length_both`, `length_preferred`, `volume_both`,
  `volume_preferred`, `small_volume_both`) gained a `DecimalStyle`
  parameter in place — no dot-style shims kept. Verified first that the
  backend has **zero** consumers of these helpers (they are
  display-only; the API ships raw numbers), so the smallest non-breaking
  shape is the breaking-to-nobody one.
- **German terms**: the curing phase is now **„Aushärten"**
  (`phases.curing` was the literal English "Curing"; register matches
  the existing „Trocknung"). The same concept is spelled one way
  everywhere: `actions.dry_cure_check` „Trocknen/Curing" →
  „Trocknen/Aushärten", `env.coach_curing_jars_why` leading "Curing" →
  „Aushärten". The praying leaf sign is **„Aufgerichtet"** (was
  „Betend" — a literal rendering of "praying"; the new label parallels
  „Hängend" and stays chip-compact). `form.when_utc` is now
  „Zeitpunkt (UTC) *". Reviewed and deliberately left: the de/en
  identical abbreviations (CO₂, EC, pH, VPD, Temp), „Autoflower",
  „Dimmer (%)", „Status", „Tolerant", „Flip 12/12" — established
  German grower usage, not leaks.
- **Gantt caption**: the caption sentence, day count and ` / total`
  suffix were three adjacent view nodes, which Leptos renders with no
  separator (`…Zeitpunkt.65 / 141`). Composed in a pure
  `forecast_caption()` helper (pinned by a test) so the space is
  built in, in both locales. The unused `gantt.day_count` de key was
  left alone (dead key, separate cleanup).

---

## 2026-11-19 — the quick-add picker is a grouped native dialog (D122)

### D122: the quick-add picker is a grouped native `<dialog>`, not a dropdown

The flat dropdown put sixteen actions on one row-order list with small
targets — wrong for the tent-first grower working one-handed in a
browser sidebar. The picker is now a native `<dialog>` with daisyUI
`modal` chrome:

- **Dialog pattern**: a real `<dialog class="modal">` opened with
  `showModal()`, so the browser owns the focus trap, the Escape cancel
  and the focus restore to the trigger — no manual key handling. The
  backdrop is daisyUI's `modal-backdrop` form (`method="dialog"`), so a
  backdrop click closes natively; `aria-modal` + `aria-labelledby` keep
  the semantics honest and `aria-expanded` tracks the `close` event.
  Chosen over a manual daisyUI-div modal because the native element is
  less code and cannot drift from browser focus semantics.
- **Taxonomy** (`action_groups()` in quick_add.rs, the testable seam):
  Watering & feeding (water, feed) · Plant care (trim, train,
  transplant, pest/IPM) · Environment (lights, env change, light
  change) · Observations (measure, photo, note, trichomes) ·
  Milestones (flip, harvest, dry/cure). Within a group the chip order
  of the old row is preserved; all 16 actions stay reachable
  (`PhaseChange` is still created only by the forecast/phase UIs).
- **Targets**: action buttons carry `min-h-11` (44px) and wrap in an
  auto-fill grid sized for a 390px viewport.
- **Draft semantics unchanged** (per D069's found-in-e2e rule): pick
  resets via `empty_action()` with the chosen type, sets chip + draft,
  closes the dialog and focuses `#quick-add-form` next frame.

The `.dropdown.quick-add` overrides in tailwind.css died with the
dropdown and were removed. Supersession note for the lint register:
the D099-era row for `quick_add::QuickAddMenu`
(`clippy::too_many_lines`) gives "menu markup plus its keydown
handlers" as the reason; the dialog version has no keydown handlers —
the allow now covers the dialog and grouped-grid markup, the old
reason text no longer applies.

## 2026-11-19 — api failure classification and dismissable error banners

### D121: `ApiFailure` is classified before display; raw transport/parse detail goes to the browser console, never the banner

`failure_text` used to return `ApiFailure.message` verbatim, so a status-0
serde/transport error string reached the `alert-error` banner as user copy.
The display path now classifies first (`classify_failure` in api.rs):

- `BackendMessage` — status above 0 and a non-empty message. The message
  was decoded from the backend's localized error catalog (D113) and passes
  through unchanged, as D116 already mandated.
- `StatusOnly` — status above 0 with no usable message. Renders the
  localized generic `errors.request_failed` with the status interpolated.
- `TransportOrParse` — status 0. Renders the new calm copy
  `errors.request_interrupted` (en + de) and logs the raw detail via
  `web_sys::console` so diagnosis survives; the raw string never enters
  the DOM (copy hygiene and privacy in one move).

The classification is a pure function pinned by unit tests with
representative payloads (serde noise at status 0, a German catalog
sentence at 4xx/5xx, an empty message at 500). `failure_text` keeps its
`(&ApiFailure, Locale) -> String` signature, so every banner call site
(chart, grows, history, live) inherits the fix without touching them.
The console logging is inlined into the match arm rather than extracted:
the nursery const-fn lint would fire on a helper that only exists to
carry a wasm-only body, and no `#[allow]` register row is warranted for
a two-line block.

The `ErrorIf` banner gains dismiss (clears the error signal) and retry
(a bare `window.location().reload()` — no URL construction, so the
ingress prefix is untouched). Both buttons are `min-h-11` touch targets
with `aria-label`s from the new `errors.dismiss` / `errors.retry` keys;
the `p[role=alert]` contract that the e2e specs pin is unchanged.

## 2026-11-19 — two surface tiers, ui primitives, gap consolidation (design re-anchor)

### D120: the frontend adopts exactly two surface tiers — full card (`px-5 py-4`) and tile (`px-4 py-3`) — with the class recipes extracted to `crates/leaflog-frontend/src/ui.rs`

The design spec names exactly two surface tiers; the code drifted (small
`px-4 py-3` cards on chart/lamp/coach/timeline/plant/not-measured surfaces,
`px-3 py-2` env-strip tiles, scattered `gap-3`/`gap-6` rhythm gaps). The
spec is the truth, so the code moved to it:

- `ui::card()` returns the full-card recipe `card bg-base-100 shadow-sm
  px-5 py-4`; `ui::tile()` returns the tile recipe `rounded-box border
  border-base-300 bg-base-100 px-4 py-3`. Both are pinned by unit tests.
  Rhythm margins (`my-4`) stay at call sites, so cards inside gap-managed
  grids do not double the spacing; Leptos rejects a second `class`
  attribute, so call sites compose with `format!("{} my-4", ui::card())` —
  every Tailwind token stays a source literal.
- Small cards upgraded to the full tier: chart card, Grow overview
  lamp/coach cards, timeline entries, plant card, not-measured card, and
  the edit page's danger card. The env-strip tiles and the tinted
  `live::tile_class` variants moved to the tile padding; the tinted
  variants keep full literal strings rather than composing over
  `ui::tile()` — the border-*color* utility would then depend on CSS
  order between two literals (double-applied `border`), and the existing
  test pins those strings.
- `CARD_GRID` moved from `env_section.rs` into `ui.rs` and is used by the
  environment section and the Grow overview lamp/coach grid alike.
- Components extracted alongside: `EntityIdChip` (see below),
  `EmptyState` (the `py-6 text-center text-base-content/60` idiom, six
  call sites; the message stays with the caller as children so it keeps
  its reactive translation), and `SectionHeader` (generalized from the
  Air/Root zone group labels; title and hint arrive as `Signal<String>`
  props so a locale swap re-translates the label, same contract as the
  history `Aggregate` title).

**Gap consolidation rule**: rhythm-level gaps use only `gap-2` (content
clusters) and `gap-4` (section splits); `gap-0.5`/`1`/`1.5` stay legal
only inside content clusters (chip rows, `.facts`, the feed bar list).
Converted accordingly: the page stacks, the 14-rem form-field grids, the
timeline/plants split (`gap-6` → `gap-4`), the history-window control,
feed inputs, the navbar cluster. Heading/card rhythm margins moved
`mt-3` → `mt-4` (photo grid, prediction-card footer); `mt-3` stays only
on margins directly attached to a control inside its cluster (the edit
danger button, the photo fieldset).

**Entity chip root cause**: the long-id overflow (ticket 10) was treated
with `overflow-wrap: anywhere`, which trades layout overflow for
collapsed min-content inside the daisyUI alert grid. The fix is
controlled truncation: `.entity-id` is an `inline-grid` whose single
`minmax(0, 1fr)` track pins the chip's min-content to zero (grid and
flex ancestors can shrink it — no overflow by construction), and the
text ellipsizes on one line while the DOM text stays complete for
screen readers and tests; the full id rides on the `title` tooltip.
The dead-binding banner renders the chips in one `flex flex-wrap gap-2`
row via `EntityIdChip`.

**Feed grid fix**: the before/after chart pair auto-placed into
`lg:grid-cols-[minmax(10rem,1fr)_minmax(0,3fr)]`, so an absent before
list dropped the after chart into the narrow track. `.feed-after` now
carries explicit `lg:col-start-2` placement — it occupies the `3fr`
track whether or not the before list renders; mobile stays stacked and
the `.feed-chart` horizontal scroll is untouched.

**Judgment calls logged**: the danger card takes the standard `my-4`
>rhythm (it is a full card, not a sub-block); the timeline/plant list
items drop their margins — the surrounding `gap-2`/`gap-4` grids own
the rhythm; the not-measured card keeps its hairline border over the
full-card tier because it flags "nothing feeds these charts", not a
primary surface.

**Lint exceptions**: the three new `#[component]` items in `ui.rs` carry
the established narrow `clippy::must_use_candidate` allow (register
below; same macro behaviour as `ErrorIf`).

## 2026-11-19 — the subscribe seed is not a state change (light auto-log)

### D119: `subscribe_entities` seed events populate the live map but never emit auto-log light flips; only `c`-diff state updates do

The "Keine Daten" bug fix (real HA sends its initial batch as full
states under `event.a`, which the live-map parser dropped) raised the
question of whether a seed should count as a light flip for the
auto-capture log. It must not: a seed is HA replaying the current
truth after every (re)connect and rebind, so counting it would write
a `lights` entry per grow on every app boot — a reconnect log-spam
channel, not a diary. The flip record stays reserved for observed
transitions, which arrive as `c` diffs carrying a state. Unverified
in reverse: no test or feature depends on auto-log entries appearing
on connect.

The mock replays real HA's shapes (`a` for the batch, `+`-wrapped
`c` diffs for ticks) so the integration suite exercises the parser
that ships; its older abridged shapes remain tolerated in the parser
for forwards compatibility.

## 2026-11-19 — deny: build-only unsafe-libyaml advisory ignored (CI repair)

### D118: `RUSTSEC-2024-0449` is ignored in `deny.toml` — the advisory rides a build-time-only path of the localisation mechanism

CI went red on the first post-localisation push: `cargo deny` fails on
`unsafe-libyaml 0.2.11` (RUSTSEC-2024-0449, unmaintained, repo archived).
The crate enters the graph solely via `serde_yaml` ← `leptos_i18n_parser`
0.6.3 ← `leptos_i18n_build` / `leptos_i18n_codegen` / `leptos_i18n_macro`
— all **build dependencies** of the frontend's localisation mechanism
(D112). It compiles the locale catalogs at build time and never ships:
neither the WASM bundle nor the backend binary contain it, so there is no
runtime exposure.

No fix exists inside the `leptos_i18n` 0.6 line (0.6.3 is the latest
release; `serde_yaml` is its parser's pinned choice), and switching the
catalogs' own format would not help — the advisory is inside the parser
crate, not our usage. Follows the D046 precedent set for `paste`
(RUSTSEC-2024-0436): ignore with a reason and a re-check trigger. The
ignore names the `leptos_i18n` upgrade (its 0.7 line or the Leptos 0.9
stack) as the point where the entry must be re-evaluated or dropped.

`just deny` still gates everything else: licences, bans, sources, and
all other advisories. The advisory would fail CI again loudly if the
ignore were stale.

## 2026-11-19 — prose gate narrowing (locale ticket 06, git-bug cbfde49)

### D117: the prose gate covers markdown and Rust comments only; the UI-string extractor left `lint-prose`

Tickets 02–04 keyed every frontend prose literal into the
compile-time catalogs (D112, 307 keys per locale). The D065
extractor therefore polices nothing: Rust keeps keys, not literals.
The prose it could still see sits where the gate must never scan —
locale text lives in `crates/leaflog-frontend/locales/*.json`, and
the German catalog would trip the English prose rules.

The UI-string extraction block is gone from the `lint-prose`
recipe. Markdown and Rust comments stay gated exactly as before;
the Rust-comment paragraphing awk is untouched.

Wording follows the mechanism, history stays put: AGENTS.md's
`just ci` line now reads "markdown and Rust comments", and the
`.vale.ini` header's scope note drops the UI strings. D065's text
is a dated record and keeps its "frontend's UI strings" wording;
this entry is the note of that change. D112's "Prose gate
narrowing" paragraph recorded the intent ahead of the keying
batches — this is its implementation.

## 2026-11-19 — keying the remaining surfaces (locale ticket 04, git-bug 394f2c8)

### D116: the frontend's residual English is an inventoried set of non-prose exceptions, and error banners substitute a localised status line only when the backend sends none

The sweep closes the keying: nav landmark, Home (intro, privacy,
backend-health block), the ingress spike, the route fallback, the
history report (predicted-vs-actual phrasing, aggregate titles as
signals, table headers) and every error banner are catalog-driven.
What deliberately stays unkeyed — each judged in the ticket 04 grep
sweep (quoted literals with a letter and a space across
`crates/leaflog-frontend/src`, minus classes, styles and URLs):

- Chart/SVG wire anchors already on the D114 list (`{role} history`
  aria suffix, `No data: ` prefix, the "Events" series token).
- `{date} UTC` in the chart tooltip: the ISO timestamp is
  locale-neutral and UTC is the zone designation, not prose.
- Numeric and unit compositions (`{value:.0} kPa`, `{grams:.0} g`,
  `{ec} µS/cm`, ` / {total_days}`, `{name} ({id})`): numbers, SI
  units and user data joined by punctuation — nothing to translate
  (SPEC story 22 keeps units out of the catalogs).
- `role_style`'s axis names (chart_option.rs): `°C / °F` is unit
  text; `on / off` belongs to a light-role chart that ticket D110's
  night shading retired — the match arm is dead at runtime and stays
  only for shape completeness.
- Rust doc comments, `expect`/`assert` panic strings, and test
  fixtures: developer-facing, not UI prose (the vale gate covers the
  comments).

For the banners: `ApiFailure` now distinguishes "the backend said
something" (`message: Some(_)`, displayed as-is — deep detail stays
English per D113, and backend messages already arrive localised)
from "only a status came back" (`message: None`).
`api::failure_text` substitutes the catalog's
`errors.request_failed` ("Request failed (status N).") for `None`,
so no set site keeps a baked English fallback; transport errors
(gloo's text) remain library deep detail under the same rule.

## 2026-11-19 — keying the remaining form flows (locale ticket 03, git-bug d32e7c5)

### D115: the plural keys need ICU's compiled data; it is baked at build time

`plant.photos_linked` (ticket 03) was the first plural key actually
rendered by the app and it panicked with "No DataProvider provided":
leptos_i18n's `plurals` feature selects plural variants through ICU
plural rules, and without `icu_compiled_data` the data provider is
`None` until the app installs one. The pre-existing `feed.days` keys
(ticket 02) carry the same latent defect — no e2e case renders a
feed-days line, so it never fired. The fix is the feature flag:
`icu_compiled_data` on `leaflog-frontend`'s `leptos_i18n` dependency
bakes the plural-rule data into the binary at compile time — zero
runtime fetches (SPEC story 23), native tests and wasm behave alike,
and the bundle stays inside the verify.mjs size window. The
alternative (installing a runtime provider or hand-rolling plural
selection) buys nothing and adds code.

## 2026-11-18 — keying the grow pages (locale ticket 02, git-bug 390fbde)

### D114: core-generated coach and lamp-why strings travel as wire codes, not text

The backend and `leaflog-core` compose English coaching prose from
interpolated numbers (`"Keep VPD 0.75–0.95 kPa …"`, lamp-why sentences).
Matching rendered English against a catalog to translate it is brittle,
so the generators now emit a stable **code alongside the English text**:
`CoachTip` gains `code: &'static str` (ten tip codes, `vpd_band` …
`curing_jars`), `LampAdvice` gains `why_codes: Vec<&'static str>` (eight
sentence codes plus `default_starting_points` for the empty-why case),
and the coach response carries `light_on` so the frontend can pick the
VPD when-on/when-off phrasing. The English strings stay on the wire
unchanged (older consumers, tests, and the unknown-code fallback all
keep working). The frontend maps code → catalog key (`env.coach_*`,
`env.lamp_*`) in the active locale, reads the VPD band numbers straight
from `leaflog_core::env::vpd_band` instead of scraping them out of the
English title, and falls back to the wire text for unknown codes.
Deliberately **not** localised: the chart SVG `aria-label` suffix
(`"{role} history"` — the e2e suite and the connect-group contract
match on it), the `"No data: "` banner prefix and the
`" — rebind it on the grow edit page"` link phrasing (same e2e anchors),
the ECharts debug/tooltip series token `"Events"` (never displayed),
and appearance-tag `meaning` prose (core documentation text shown as a
hover title).

## 2026-11-18 — backend message catalog (locale ticket 05, git-bug a0d6639)

### D113: the backend speaks through a hand-rolled catalog; wrappers read a process-global language

`leaflog-backend` gained `src/catalog.rs`: an exhaustive
`(locale, message)` `match` with `format!` interpolation covering the
environment-alert templates (title + body) and the four standard error
wrapper phrases (not found, bad request, conflict, internal error). No
new dependencies, nothing fetched at runtime. EN↔DE key parity is
compile-time here: the match must cover both locales, so a message
missing in one locale fails the build — the backend analogue of the
frontend catalogs' parity panic (D112).

Locale source: the HA config `language` (D112), English fallback for
absent or unmapped tags. The alert path reads the bridge's cached
language. The API error wrappers cannot: axum's `IntoResponse` for
`ApiError` runs without state access. They read a process-global
`RwLock<Option<String>>` in the catalog that the same config fetch
publishes next to the bridge's language watch. One HA instance per
process makes that sound: the language is app-wide, and a fetch failure
keeps the previous value. The plain-text unknown-API 404 in `app.rs`
joins the same wrapper.

Deep interpolated detail text stays English by design: store errors,
alert kinds, grow names ride inside the templates as-is. The German
templates are du-form in Home Assistant register ("Ungültige Anfrage:",
"schau im Panel nach").

## 2026-11-18 — EN+DE localisation plumbing (locale ticket 01, git-bug b67691f)

### D112: leptos_i18n 0.6.3 compile-time catalogs; the chain is HA language → browser locale → English

The frontend localises through **leptos_i18n 0.6.3** (CSR-compatible,
compile-time macro) with JSON catalogs under `crates/leaflog-frontend/locales/`:
`en.json` is the default locale and the **key source of truth**; `de.json`
mirrors it key-for-key in **du-form**. `build.rs` generates the i18n module
into `OUT_DIR` and additionally **panics on EN↔DE key drift** — the crate's
own codegen only warns and falls back, but the ticket requires the build to
fail (the parity proof is a one-key removal; the panic names the missing
keys). Nothing is fetched at runtime.

**Locale chain (frontend)**: `en`/`de` catalogs negotiate from the browser
at first paint (leptos_i18n's navigator matching, cookie disabled — no
persistence), then the backend half applies: the bridge captures the HA
config `language` on every reconnect alongside the timezone
(`GET /core/api/config`, documented surface), `GET /api/locale` carries it
to the frontend, and `LocaleChain` sets the locale only when the tag maps
(`de*` → de, `en*` → en — an explicit English from HA must beat a German
browser). Any other tag (or HA unreachable, or before the first fetch —
the endpoint reports `null`) falls through to the browser locale, then
English. The backend itself never translates in this ticket — it forwards
the observed field only.

**Ship shape**: big-bang — one release where German exists fully;
incremental per-page mixes are rejected. This ticket seeds nav labels, page
headings and the Grows page; everything unseeded stays English, acceptable
only within ticket 01's batch.

**Prose gate narrowing (noted, not implemented here)**: once the keying
batches land, the D065 UI-string extractor leaves `lint-prose` — locale
text lives in catalog files the extractor never reads and Rust keeps keys,
not literals. The narrowing (markdown and Rust comments stay gated) is
ticket 06 (cbfde49).

**Lint exceptions**: the generated i18n module is the one codegen allow in
the frontend crate — `#[allow(missing_docs, clippy::pedantic,
clippy::nursery)]` on the `generated_i18n` include module (macro output
trips the crate's pedantic/nursery denies and omits item docs). The
`#[component]` `must_use_candidate` pattern extends to the new `Home`,
`LocaleChain` and `AppShell` components, and the Axum
`needless_pass_by_value` pattern to the `locale` handler (register below).

**Addendum 2026-11-18**: the owner removed the spec § citations from UI
strings during keying — the `" (spec §5.2)"`-style suffixes several
seeded entries carried are gone from the English catalog and were never
carried into the German one (citations live in the docs, not the UI).

## 2026-11-18 — lazy root zone, empty-window honesty, verification sweep (env-section ticket 07, git-bug 70f3303)

### D111: the lazy watch arms late; the hint keys on readings, not bands

The Root-zone grid mounts through an `IntersectionObserver` on a
wrapper that reserves the block's height (`min-h-[620px]
lg:min-h-[480px]`). Two details keep the first intersection honest.
First, the Air cards above render only after their first history
fetches, so the page grows by hundreds of pixels after first paint —
a watch armed at mount reports the block "in view" on that short
page. The watch therefore arms after a 500 ms delay
(`LAZY_WATCH_ARM_DELAY_MS`); on a network so slow the growth
outlasts it, the block mounts a little early — once, as always — so
the failure mode degrades towards eager, never towards broken.
Second, the reservation holds the block's place so the mount does
not jump the page under the visitor. The mount latches once: the
observer disconnects on the first intersection and never re-runs
the work (scroll-away-and-back is still one fetch per role, and a
window switch adds exactly one more). Browsers without
`IntersectionObserver` — or a throwing constructor — take the
immediate path; the group is never held hostage to the API. The
group label stays outside the lazy block so the section's outline
never flickers, and it is the scroll target the e2e suite uses
(`revealRootZone`).

The empty-window hint now keys on `series_is_empty` (series and
compare points; Target Bands deliberately excluded) instead of
`window_is_empty` (which also ignores nights and markers). Since
ticket 06, temperature, VPD and soil always carry bands, so the old
key could never fire for them and a bound-but-dataless window drew
silent bands with no hint. Against the e2e mock the sensor variant
of that state is unreachable — the mock serves fresh points for
every bound sensor — so the chart_option unit tests carry the
banded sensor case and the spec asserts the feed variant (one entry
just outside the selected window). No new user-visible strings: the
hints and labels are the earlier tickets' inline English (the D089
i18n gap, as D110).

The verification sweep (spec 24) pins the quietness mechanically
where it can: marker ticks single-glyph across every window switch,
the Markers switch clearing and restoring them, and the svg root
surviving a switch via a dataset probe. The same sweep captures one
screenshot per viewport × colour-scheme × markers × window as the
pass artefact for the visual review. The DESIGN.md palette verdict
is refreshed to the owner's endorsement (2026-11-18): the palette
tracks the Home Assistant theme and revision stays a deliberate
redesign, not drift. No new `#[allow]` exceptions.

## 2026-11-18 — night shading behind the Air charts (env-section ticket 05, git-bug 114a85c)

### D110: the night tint is history payload + a silent markArea back layer; the Light card retires

The backend resolves the light timeline for the whole Air group
(temperature, humidity, VPD, CO₂): observed light-entity states
win, with gaps backfilled from the Light Cycle schedule — the
existing resolve precedence. It emits the window's dark stretches
as a `nights` array on the history response; every other role gets
an empty array, so the Root-zone and log-detail payloads never pay
the light fetch. The frontend draws that array as its own series
appended after the marker ticks: no data, `z` 0, and a `markArea`
of xAxis-only region pairs, so `ECharts` spans each region over
the full plot height whatever the value range. The tint uses the
look's muted ink (already 50 % alpha) at opacity 0.16 — roughly
8 % effective, tonal depth rather than a new hue, and the same
numbers hold in both themes. Regions clip to the window, drop
unparseable bounds, and merge butt-joined stretches so one
continuous night shows no seam. Nights are context, not data:
`window_is_empty` ignores them and the value range never widens
for them.

The standalone Light card leaves the section — the shading tells
the same story in place — and the Air group label carries the legend
hint "Shaded areas mark night periods." (inline English; the German
string rides the future i18n work, the D089 gap). Light also leaves
the not-measured chip list: the edit page still takes the binding,
but the section no longer has a card to chip. The now-unused on/off
axis formatter retires. Light Cycle edits change the shading on the
next load/refetch because the timeline resolves per request. No new
`#[allow]` exceptions. Hygiene while here: the ticket-06 entry below
was mis-numbered `D106` (a collision with the ticket-02 entry, which
the decision-id gate rejects) and is renumbered `D109`; a stray
merge-marker fragment left in this file by the base merge is
deleted.

---

## 2026-11-18 — Air and Root zone chart groups with crosshair sync (env-section ticket 02)

### D106: one connect group, one shared plot rectangle

The Environment section's charts join a single `ECharts` connect
group (`leaflog-env-history`): each mounted instance sets its `group`
property and one `echarts.connect` call registers the group — both
idempotent, so per-mount repetition is safe. A connected hover
broadcasts `updateAxisPointer` actions in *pixels*, so every chart
must draw the same plot rectangle for the same pixels to land on the
same timestamp. The option builder therefore pins the grid insets
explicitly (left 110 px, top 36, bottom 24, right 16) and keeps
`containLabel` off: `containLabel` measures each role's *own* label
text, and `yAxis.axisLabel.width` does not change what it measures —
the live cards' grid lefts drifted 61 px apart (temperature 106,
humidity 45, VPD 65) before the fix. Labels sit inside the fixed
gutter in a 96 px box with truncation overflow; 96 px fits the widest
label the cards render — the temperature card's dual-unit "18.0 °C /
64.4 °F" measures ~89 px at the 12 px label font, negatives ~93 px
(canvas-measured in headless Chromium). The Light card sits in the
full-width slot between the groups, so its pixel scale differs and it
opts out of the connect group (`joined_crosshair=false`) — the later
night-shading ticket retires the card.

Correction (review b1): with the Light card retired (D110) the opt-out
prop is gone — every remaining chart joins the group unconditionally.

Chart heights land at ~144 px mobile / ~176 px desktop
(`h-36 lg:h-44`) — the ticket's compact instrument-panel heights,
with the two-up grid per group on wide screens. The group headers
are `h4` micro-labels (uppercase in CSS, source text "Air" /
"Root zone") and the card titles step down to `h5` so the outline
follows the grouping. The per-card
window toggles are gone: the section's shared control is the only
one (spec 19's per-card assertions updated to match).

## 2026-11-18 — log-event markers on the environment charts (env-section ticket 04, git-bug 5a8eb6a)

### D107: markers split by zone — root roles get water/feed/transplant, air roles get light and phase events, as one muted tick series

The history response carries a `markers` array beside its series:
the action-log events inside the window that matter to the role.
`marker_kinds` decides: root-zone roles (`soil_moisture`, `ph`,
`ec`) get water/feed/transplant; air roles (`temperature`,
`humidity`, `vpd`, `co2`) get lights/light-change/flip/phase-change;
every other role (light, the reservoir roles) gets none — a light
log entry marking the light chart says nothing the chart does not.
Backend selection is one small read of the newest 1000 action-log
rows beside the sensor pass (`MARKER_SCAN_LIMIT`); a log page
fetches 200, so no window in practice truncates. Correction
(review a2): the scan is window-bound — it reads the newest 1000
rows inside the requested window, so out-of-window entries can
never crowd an in-window marker out.

Each marker draws as one quiet vertical tick: a 2×12 rect in the
muted gridline colour, anchored just above the axis floor. The
ticks are geometry, not colour — no new hue enters either theme
(the Phase Accent Rule), and the option builder appends the
`Events` scatter series last so the reading series keep their
indices. The tooltip names the event (`Lights — 2026-11-18 16:41
UTC`); the tick's y position is placement, never a reading.

### D108: hiding markers strips the set and re-renders with a full option replace; the switch is per visit

One `Markers` checkbox beside the section's window toggles hides
and shows every chart's ticks at once: the shared signal rides the
section header beside the shared window, and `EChartsHistory`
strips the marker set before the builder sees it (a hidden set
renders no series). Because the toggle changes the series list,
`setOption` now passes `notMerge: true` — a merged update would
keep the stale `Events` series, and a full replace is always
correct since these charts carry nothing stateful (no dataZoom).
The choice is not persisted: only the window is a stored
preference, the tick glance is not. No new `#[allow]` exceptions.

---

## 2026-11-18 — one quiet card for unbound roles (env-section ticket 03)

### D105: the not-measured card chips only the five charted sensor roles

The Environment section closes with one quiet "not measured" card:
for every sensor role whose chart card the section renders but whose
binding row is missing, a chip names the role and is itself the bind
affordance — an in-app `<A>` to the grow edit page's Entity bindings
flow (the existing flow; nothing new is invented). The card appears
only once the bindings response names at least one unbound role:
while the fetch is in flight, or after it failed, it renders nothing,
so it never flashes and never claims a gap it cannot see.

Which roles chip: `temperature`, `humidity`, `co2`, `light`,
`soil_moisture` — exactly the roles a missing binding can hide here,
with the chart cards' own titles for labels. VPD needs no chip (it
derives from temperature + humidity, so one of those chips explains
its absence); pH and EC are log-detail roles whose bound state comes
from log entries, not the bindings map; the reservoir roles are
bindable but carry no chart card in this section. Unbound means key
absence in the bindings map and nothing else — a dead binding is not
unbound: its chart card stays with the degraded banner, so it must
not chip. The in-card hiding and pH/EC fallback rules are untouched
(ticket 03 preserves them). No new `#[allow]` exceptions.

Correction (review c1): `light` left the chip list when the Light
card retired (D110) — the shipped list is the four remaining roles.

---

## 2026-11-18 — temperature and VPD Target Bands (env-section ticket 06, git-bug 74cba42)

### D109: temperature's Target Band is the flat tolerance band; VPD keeps the light-stepped band; CO₂ goes bare

The temperature chart's shaded band is the grow's shifted
`TempThresholds` pair (`temperature_target_band(heat, cold)` in
leaflog-core — the same edges the tiles' `temp_ok` colouring uses),
drawn through the phase-span `build_bands` path with no light
stepping. The ticket's "the same values the section's sensor tiles
display as highs/lows" names those thresholds, and they carry no
phase or light dependency. VPD keeps the existing day/night stepping
(`vpd_band` / `night_vpd_band`), so the stepped-band contract now
rides the VPD and humidity charts alone, and `is_stepped` no longer
matches temperature or CO₂ (their histories skip the light-timeline
fetch). CO₂ renders bare: the old CO₂ day/night band assertions are
gone from the Rust and e2e suites, replaced by bare counter-cases.

`temperature_corridor`, `night_temperature_corridor` and the two
`CO2_*_PPM` presets stay in leaflog-core as sourced seed data with no
chart consumer; their docs no longer claim the chart draws them.
Deleting public presets mid-integration-branch would churn siblings
for nothing — removal can ride a later cleanup if nothing reclaims
them.

---

## 2026-11-18 — the Environment section owns its window (env-section ticket 01)

### D103: shared history window — wire labels `day`/`week`/`month`, default `week`, localStorage key `leaflog.history-window`

The Environment section renders the live tiles as its top row, then
the history charts, under one heading with no separate History
heading, and moves above the Forecast on the grow page. One segmented
24h/7d/30d control in the section header drives every chart card:
`HistoryChart` takes an optional `shared_window: RwSignal<String>`
and an effect syncs the card's local window on shared changes (the
equality guard keeps the mount-time agreement from refetching).

The choice persists per browser in localStorage under the
Leaflog-namespaced key `leaflog.history-window`, stored as the API's
wire label. Missing/locked-down/stale or foreign values all fall
back to the 7-day default (`week`) — the day/night rhythm stays
visible without touching anything; nothing breaks on a bad value.
The first visit's default is persisted too, so storage never
disagrees with what the page shows.

The control reserves the fourth position's width with an
`aria-hidden` empty span — a whole-grow window joins later without
shifting the row, and no disabled teaser is drawn. Per-card toggles
stay in this ticket (the grouping ticket retires them); while both
exist, a card's own toggle retargets that card alone and the next
shared change wins on every card again. `RwSignal::set` notifies even
on equal values, so re-clicking an already-active window still
refetches — the abort-path test relies on that.

New web-sys feature: `Storage` (the `Window::local_storage` getter
needs it). No new `#[allow]` exceptions.

---

## 2026-11-18 — the Light Cycle editor shows reality (ticket 03)

### D102: observed light pattern — span-quotient hours, per-day circular-mean on-time, optional block on the existing GET

The learned pattern comes from one pure core function,
`leaflog_core::light_cycle::observed_cycle`, over the same staircase
the charts already fetch. Hours-on is a span quotient: total lit
seconds over the observed span times 24, rounded to a whole hour,
clamped 1–24. Absolute durations make it DST-honest — a fall-back
Sunday simply contributes its real 25 hours.

Lights-on is the circular mean over each local day's first
on-transition minute (atan2 of summed sines and cosines), so a
crossing pattern like 18:00→12:00 averages near midnight without
wrapping, and per-day-first-on ignores mid-day interruptions. An
unresolvable zone name falls back to UTC, mirroring the chart code.

The wire change is additive: the existing light-cycle GET now may
carry an `observed` block (bound entity id, learned on-time, learned
hours-on), flattened beside the stored fields and omitted when the
grow is unbound, the history is dataless, or HA is unreachable —
fail closed, the editor shows only the editable defaults.

Derivation is lazy: computed on page loads and binding changes, never
a background job. A 60 s per-grow cache absorbs repeated loads; a
bindings PUT or grow delete invalidates it, so a rebind shows its
new entity immediately. The editor renders the block as a display-only
caption — the per-phase fields stay editable defaults with
reset-to-default, and nothing is ever written back from reality
(ADR 0002).

Lint-policy exceptions (`#[allow]` register) — ticket 03 addition:

| Crate | Items | Lint | Reason |
|---|---|---|---|
| leaflog-core | fn `observed_cycle` | `clippy::cast_possible_truncation`, `clippy::cast_precision_loss`, `clippy::cast_sign_loss` | Epoch seconds sit far below f64's exact-integer range; results are rounded and clamped before narrowing |

---

## 2026-11-18 — quick add collapses into a dropdown menu (ticket 06)

### D099: quick-add icons via leptos_icons + Lucide (icondata_lu)

The sixteen-button quick-add row became one daisyUI dropdown at the
same spot (spec 95b2e66, "Quick add" implementation decision) — the
app's first dropdown. Icons come from a single family — Lucide via
`icondata_lu` — rendered by `leptos_icons::Icon` (all MIT, real
crates.io dependencies, nothing vendored). Every quick-add action maps
to one glyph; actions without a fitting glyph render none rather than
a wrong one (`PhaseChange` is not offered by the menu at all, D073's
phase picker owns it). The trigger carries a plus and a chevron icon.
No emoji anywhere in the menu.

Opening is controlled by the `dropdown-open` class only: daisyUI's
`:focus-within` auto-open is suppressed for `.dropdown.quick-add` in
`tailwind.css`, because Escape hands focus back to the trigger and the
auto-open would visually reopen a menu whose `aria-expanded` says
`false`. Keyboard semantics follow the ARIA APG menu-button pattern
(arrows, Home/End, Escape, Tab-out and outside-mousedown close);
picking an action opens the inline logging form card with the same
draft semantics the chip row had (D069) and moves focus onto the card.

| Crate | Item | Allow | Reason |
|---|---|---|---|
| leaflog-frontend | component `quick_add::QuickAddMenu` | `clippy::too_many_lines` | Declarative view code (menu markup plus its keydown handlers), not logic; same exception as the other large `view!` components |

---

## 2026-11-18 — HA theme matching (ticket 07, grow-page rework)

### D100: the grow app wears the parent HA theme — computed-style read, daisyUI oklch overrides, silent fallback

Leaflog adopts the grower's Home Assistant frontend theme inside the
ingress iframe. Mechanism: HA's own `applyThemesOnElement` writes the
live theme as inline custom properties on the parent
`document.documentElement`, so the app reads the parent root's
**computed** style (that resolves HA's `var()` indirections like
`--primary-color: var(--ha-color-primary-40)`) and maps:

- `--primary-color`/`--accent-color` → `--color-primary`/`--color-accent`,
- `--card-background-color` → `--color-base-100`,
- `--primary-background-color` → `--color-base-200`,
- `--divider-color` (alpha flattened over the page background) →
  `--color-base-300`,
- `--primary-text-color` → `--color-base-content`,
- `--success/-warning/-error/-info-color` → the same-named daisyUI slots,
  each individually optional — a missing one keeps the built-in value.

Content (foreground) colours for primary use HA's `--text-primary-color`
when present, otherwise HA's own contrast rule (white vs `#212121` at a
6:1 threshold against the fill); state and accent contents always use the
contrast rule. HA's `--secondary-background-color` has no clean daisyUI
counterpart (the secondary/neutral slots are barely used in the app, one
chart legend probe) and stays unmapped — leftover built-ins are accepted.

The overrides are written as inline custom properties on the app's own
root element (`body` sits outside the mount div, so the root is the
only override point that covers it) plus a `color-scheme` derived from
the page background's lightness; dark/light follows the parent
implicitly, and there is no theme picker or toggle anywhere. (D135
amends the original serialization: the overrides were first written as
`oklch(...)` — chart-invisible, see D135 — and are now comma-syntax
`rgb()`/`rgba()` values.) A `MutationObserver` on the parent root's
`style` attribute re-runs the read+map, so a theme or dark-mode flip in
HA re-dresses the app live without a reload. Colour math
(hex/rgb/rgba parsing, alpha flattening, WCAG contrast, Ottosson
rgb→oklch) lives in `leaflog-core::theme` as pure unit-tested
functions; the frontend only bridges DOM and applies.

The fallback to the built-in theme is mandatory and silent: no parent
(standalone), a cross-origin or sandboxed parent, or any missing or
unparsable member of the six core variables (primary, accent, primary
background, card background, divider, primary text) leaves the built-in
palette untouched — including clearing previously applied overrides when
a re-read degrades. Fixing that fallback surfaced a latent wiring gap
from D064: the built-in light theme had never actually applied, because
the `--default` flag sat on the daisyUI `themes:` line where custom
themes ignore it — the light palette only existed under
`[data-theme="leaflog"]`, which nothing sets, so light-preference
browsers rendered with no theme variables at all (dark worked via its
`prefersdark` media block). The fix is `default: true;` on the light
custom-theme block in `tailwind.css`, emitting it on `:where(:root)`;
the `prefersdark` dark block still wins for dark-preference browsers
(higher specificity, later in the file).

### D101: the parent-theme read rides an undocumented HA surface — sandbox or realm changes break adoption silently

Everything D100 reads is undocumented surface. The HA frontend docs
describe theming (variables, `applyThemesOnElement`) but nothing
promises that the ingress iframe's parent stays same-origin-readable:
the current `ha-panel-app.ts` (re-checked 2026-11-18 at
https://contributors.home-assistant.io/frontend/architecture/theming/ and
the panel source) mounts ingress apps in a plain same-origin iframe with
no sandbox attribute — the ticket's "sandboxed allow-same-origin" claim
is already outdated — while an in-source comment notes "CSS variables
don't cross the iframe boundary", which is why a JS computed-style read
is needed at all. Any future HA release could add `sandbox` without
`allow-same-origin`, split origins, or reshuffle the `--*-color` names,
and every one of those degrades to exactly the documented fallback:
built-in palette, no user-visible state, no console noise. The same
fragility guard covers the DOM detail the implementation leans on: the
parent document lives in another realm, so `instanceof`-based casting
(wasm-bindgen's `dyn_into`) rejects it even same-origin — the read walks
`document`→`documentElement` reflectively and unchecked-casts, which
stays correct because `getComputedStyle` and `MutationObserver.observe`
dispatch on internal slots, not realm. The dev-only `/theme-harness`
page (never in `dist`, gated to `Mode::Dev`) is the regression seam for
all three paths — adoption, live re-adoption, fallback — and any HA-side
breakage shows up there as a red e2e, not as a silently unthemed app.

---

## 2026-11-18 — the Germination pick owns the start date (ticket 04)

### D098: recompute, not migrate — the pick re-dates stored grow days in one transaction

A Germination pick now writes the grow's `started_on`: the anchor is
the pick's local calendar date in the canonical timezone (a new
`leaflog_core::tz::local_date`, mirroring `day_number_local`), so the
pick row itself lands on day 0 whatever the wall clock. Every stored
grow-day value is then recomputed from its own timestamp —
`action_log.occurred_at` and `photo.captured_at` — under the new
anchor, inside the same SQLite transaction that moves the anchor and
inserts the pick (the `unchecked_transaction` pattern, D012). A
re-dating step that fails aborts the whole pick; nothing half-lands.

No migration: recomputation beat a schema change because the stored
columns already carry the timestamps they need, and `feed_plan_day` is
index-based with no timestamp, so its day numbers keep their
day-index meaning untouched. A stored timestamp that does not parse
fails the transaction as `StoreError::Unparsable` rather than
silently keeping the old day. Scope notes: photos keep their phase
labels (only day values move); editing a germination row through
`PUT /api/actions/{id}` does not re-anchor — a pick is a create; and
the forecast pin mechanism (D073) is unchanged, it just reads the
re-dated rows. Other picks and plain actions never touch
`started_on`.

---

## 2026-11-18 — Phase day badge (grow page rework, ticket 05)

### D104: Phase day anchors on the raw transition event, not the clamped window start

_(Numbered D104 per the ticket's instructions; D097 was the last
number in use on this branch.)_

The spec pins the Phase day to "each phase's transition event" but
leaves open which number the badge reads when a pick is backdated
hard: the raw event day, or the forecast window start. The two
differ. The engine clamps projected windows so they stay contiguous
for the Gantt: a flip on day 3 under a projected seedling window of
6–23 pushes the flower window start to day 6, while the timeline
still records the flip on day 3.

The badge reads the raw event day — `leaflog_core::forecast::phase_day`
over `Actuals`, with day 0 being the transition day — so late diary
entries stay truthful about how long the phase has run. The Gantt
and the charts stay on the Day count; re-anchoring them to Phase day
is explicitly out of scope for the rework.

---

## 2026-11-17 — review fixes and retro hardening

### D083: decision ids are unique — the `decision-ids` gate

The decision log's numbering failed twice in the same way: parallel
tickets numbered from different bases and claimed the same D-number.
D012 and D013 each headed two decisions — the M2-era storage pair
(SQLite via rusqlite, migration scope) and a later pair (bundled
equipment seed data, offset-less action timestamps) — and the
feature-strip tickets re-created the failure mode, with D078 claimed
twice and sorted by hand mid-run. The `just decision-ids` recipe
makes the collision mechanical: every `### D<3digits>:` heading in
this file must be unique, or `just ci` fails naming the offenders.

The one-time cleanup renumbers the later-chronological pair onto the
next free numbers — equipment seed data D012 → D081, action
timestamps D013 → D082 — keeping the entries in their dated sections
with a note, and repoints D075's supersede list, which meant the
equipment decision. The earlier M2 pair keeps D012/D013; every other
inbound reference (the single-connection notes in the store, the
THREAT-MODEL migration row) already meant that pair.

### D084: the ci gate self-serializes on a host flock

Two full `just ci` gates running at once on the 14 Gi dev host were
OOM-killed, so the gate now self-serializes: `just ci` runs its
implementation (`_ci-impl`, the same recipe list as before) under
`flock /tmp/leaflog-fs/ci.lock`, and a second concurrent gate waits
instead of racing. CI is unaffected — its jobs call `just e2e` and
`just lint-prose` directly on their own runners, and `just ci` is the
only gate in its job. A host without `flock` runs unlocked (the
skip-if-not-installed pattern). Do not wrap `just ci` in an outer
flock of the same file: the gate holds the lock itself, so a second
flock of that path would wait on it forever.

### D085: e2e fails fast on busy ports

The e2e stack binds two fixed ports (app and mock Supervisor). When a
stale stack held one, the run surfaced it only after the full
frontend build — as a dead backend or a health poll answered by
someone else's process. The `e2e` recipe now probes both ports before
any build work and fails immediately, naming the busy port and the
remedy: stop the stale stack, or pick free ports with
`just e2e_port=<port> e2e_mock_port=<port> e2e`. The post-launch bind
check stays: it catches a backend that dies between the probe and its
own bind.

### Lint-policy exceptions (`#[allow]` register) — review additions

| Crate | Items | Lint | Reason |
|---|---|---|---|
| leaflog-backend | store fn `grows::map_grow` | `clippy::too_many_lines` | Flat row-to-struct mapping of the grow's columns, not logic; splitting would obscure the column list |
| leaflog-core | fn `env::advisor_watts` | `clippy::cast_possible_truncation`, `clippy::cast_sign_loss` | The value is rounded and floored at 1 (positive) before the cast, and f64→u32 casts saturate rather than wrap |

---

## 2026-11-16 — feature strip: strains, seed bank, spaces, equipment catalogue

### D075: the grow carries strain, tolerance and lamp facts; catalogue features are removed

Stripping the app back to a single-grower diary removes four features
outright: the strain database (and its 15 bundled records), the seed
bank (batches + germination log), grow spaces, and the equipment
catalogue (lights/tents/fans plus the bundled equipment seed data).
What replaced them lives on the grow itself:

- `grow` gains `strain_label` (free text), a photoperiod toggle
  (Photoperiod/Autoflower), `heat/cold/light_tolerance`
  (Sensitive/Average/Tolerant, default Average), `real_watts` and
  `dimmable` — the last five presented as an Advanced Settings section
  in the UI.
- Forecast, feed-fit and the phase picker read the photoperiod from
  the grow. The tolerance threshold shifts (D030's `shifted` logic)
  and the lamp advisor (D044's 330 W fallback) keep their logic and
  change only their source: the grow, not a strain record or the
  space's strongest light.
- History's yield aggregate keys on `strain_label`. The yield
  write-back (D055, SPEC §4.14) has no destination and is deleted.
- HA entities enter the app only as per-grow sensor bindings (the
  existing `sensor_binding` rows). The space-seeded binding defaults
  (D067) disappear; sensors are bound on the grow page, and onboarding
  step 1 becomes exactly that.

Supersedes D081, D014, D015, D043, D044, D045, D049, D050, D051,
D052, D053, D056, D067 — their subject matter is deleted. THREAT-MODEL
R3 (the strain `source_url` link) closes with the catalogue, and the
third-party licence burden of the bundled strain records goes with it.
Migration V14 hard-drops `strain`, `seed_batch`, `germination_log`,
`grow_space`, `grow_space_equipment`, `equipment_item` and the dead
`grow` columns, deriving each surviving grow's `strain_label` and
photoperiod from its former strain link (tolerances default to
Average); dropped rows are unrecoverable, so `/data` is backed up
before upgrading. SPEC §4.1/§4.4/§4.13/§5.4/§13 and AGENTS.md's
strain/equipment clauses are rewritten to match; the grow-science
seed-data discipline stays for whatever preset data remains.

### D076: V14 grow-field choices — float watts, inline UI strings

Ticket 01 (the additive expand step) settles three small shapes the
spec leaves open. The grow's `real_watts` mirrors the equipment
catalogue's field of the same name: `REAL` in SQLite, `Option<f64>`
in Rust, negatives rejected with the same message — same concept,
same representation. Because `f64` carries no `Eq`, `Grow` and
`GrowInput` drop their `Eq` derive (nothing keyed on a grow needs
it); the later ticket that wires the lamp advisor converts to the
`u32` its input wants. "Start next run" copies the new fields along
with substrate and the feed template, so a rerun keeps the strain
label, photoperiod, tolerances and lamp facts of the grow it comes
from. The grow form's new strings ("Strain label", "Advanced
settings", the tolerance and lamp labels) are inline English like
every other form string: the locale-catalog system with compile-time
key parity is still future work (the `leptos_i18n` milestone), so
there is no catalog to join yet. V14 as shipped only *adds* the
nullable columns — the label/photoperiod backfill from the strain
links stays in the later drop migration, exactly as D075 describes.

### D077: grow-sourced engine defaults — the auto total window, the watt conversion, the type relocation

Ticket 02 re-sources the forecast, feed-fit, threshold shifts, lamp
advisor and phase picker from the grow (D075). Three shapes it
settles. First, the auto default: the SPEC's autoflower window
(~9–11 weeks from seed, §4.5) was previously only reachable through a
strain record; a strain-less auto actually finished at flip + default
flower days (~day 94). The grow's toggle now carries the default
directly: `forecast::AUTO_TOTAL_DAYS = (63, 77)` from seed, anchored
on `forecast::AUTO_FLIP_DAY = 28`, with `input_for` deriving the
auto's flower window as the total minus the age flip — so a default
auto's forecast (harvest day 70, band 63–77) finally matches the
documented default. Second, the watt conversion the D076 note
promised: `env::advisor_watts` maps the grow's `Option<f64>
real_watts` to the advisor's whole-watt `u32` — `None` falls back to
the D044 330 W fixture (`env::DEFAULT_FIXTURE_WATTS`), a recorded
value rounds to the nearest watt and never lands below 1. Third,
`PhotoperiodType` and `Tolerance` move from `strain.rs` to
`grows.rs` — grow attributes per the glossary — with `strain.rs`
re-exporting them until ticket 05 deletes the catalogue, so
recommendation modules no longer name the strain module at all.
The advisor's caution text says "plant", not "strain", to match.

### D078: the onboarding bind step defers its binding writes to the created grow

Ticket 04 (D075's spaces/equipment removal) collapses the wizard's
equipment and space steps into one "Bind sensors" step. A binding
hangs off a grow (`sensor_binding.grow_id`), and the wizard binds
before any grow exists — so step 1 only collects role → entity picks
locally (the same pickers the grow page renders, driven by the
unchanged `GET /api/ha/entities`), and the finish handler applies
them with one `PUT /api/grows/{id}/bindings` right after it creates
the grow and its plants. The visible order matches the spec's
"bind sensors, then create the grow" while the storage keeps its one
owner of bindings; skipping step 1 writes nothing. The wizard state
endpoint (`GET /api/onboarding`) shrank with the same removal: it
reports only `show_wizard`/`dismissed`/`has_grows` — spaces and
equipment no longer keep the wizard alive. One small side effect of
the equipment page's deletion: the grow page's dead-binding drift
banner is now where a bound entity id renders as text, so the
wrap-anywhere `entity-id` treatment (the long-id overflow fix, and
its e2e regression) moved there from the equipment card.

### D079: V17 backfill never clobbers; unlabelled grows aggregate under one key

Ticket 05 (the catalogue drop) settles three small shapes. First, the
V17 backfill is a *fill*, not an overwrite: a grow linked to a strain
adopts the strain's name and photoperiod only for the fields it has
not set itself — between V14 and V17 the grower could already edit
`strain_label` and the toggle directly, and D075's "the migration does
not rewrite my history" applies to those edits too. Second, the
history aggregate's key for grows without a label changes from the old
`"unknown"` sentinel (which named a missing catalogue lookup) to
`"unlabelled"`, which names the actual state after D075: a grow whose
free-text label is empty. Third, the substrate-profile and generic
feed-template seeding moves out of the strain seeder (it used to run
as `seed_strains`'s side effect) into its own `seed_feed_presets`
boot step, so first boot keeps seeding feed presets after the strain
bundle — and the seeder itself — are gone.

### D080: docs sweep — the flower override loses its name, SPEC keeps its numbering

Ticket 06 (the docs sweep) settles two small shapes. First, the
forecast seam's `strain_flower_days` input becomes
`flower_days_override`: no strain feeds it any more, and the old name
said otherwise. Call sites and tests follow; `input_for` still fills
it only for the auto default window. The lamp advisor's `confidence`
doc no longer promises a "high with a PPFD map" tier — the engine has
always emitted "low", and no fixture map exists after D075. Second,
SPEC.md keeps its section numbering while rewriting the removed
sections in place (§4.1/§4.4/§4.13/§5.4 stay as headings marked
"removed, D075"), so the cross-references from code comments, tests
and the surviving sections survive the strip. THREAT-MODEL's stray
references to bundled strain records and the CSV/JSON strain import
are reworded to the surviving feed/substrate presets and a generic
future data import.

---

## 2026-11-17 — the coach sees the real light state; the spec closes out (ticket 08)

### D096: the coach resolves the light state live map → snapshot → Light Cycle

_(Numbered D096 in the 2026-11-17 merge with the released
feature-strip stream: the entry first shipped as D085 on this
branch, a number the stream's own v0.7.0 decision already holds;
that stream keeps its number.)_

`grow_coach` no longer hardcodes `light_on = true`. At call time it resolves:

1. **The bound light entity's latest known state.** The live entity map first —
   it is the WebSocket subscription's current snapshot and costs one in-memory
   read. When the map has nothing (HA unreachable, entity not yet subscribed),
   the newest auto-captured `light` snapshot row speaks.
2. **The grow's Light Cycle, evaluated for "now".** The same expansion the bands
   use, through the new core point query `schedule_light_on_at`: expand a window
   straddling the instant, read off the covering interval. Interval ends are
   exclusive, so an instant exactly at lights-off is already dark. This is also
   the whole answer when nothing is bound, and the fallback for a bound entity
   with no known state.
3. **No source at all keeps the lights-on assumption** — the corridor's
   convention for an unknown state (D094), and the only reading the unlit
   phases admit.

The auto-logged `lights` entries were considered as a source and rejected.
Their state lives in free-text notes ("auto: light on"), not structured data;
parsing notes would freeze the wording for users. The live map plus snapshots
cover the realistic cases.

The night VPD tip now reads "lights off: keep close to daytime; watch the RH
spike". The old "(lights off: run higher)" contradicted the research: measured
VPD falls on its own as the room cools (research caveat C2; Pulse Grow and
Black Dog LED night sections). The tip keeps the day band's numbers on
purpose — night guidance is "hold near the day band" — and the RH spike stays
the watch item. Core tests pin the tip selection by state.

Supporting fix in `mock-ha`: the session doc claimed `toggle_light` "forces a
state-changed push on the next tick", but `tick_changes` never included the
light. The flip never reached subscribers. The session now tracks the light
state it has pushed and includes the entity only on an actual change — HA
pushes deltas, and re-sending an unchanged binary state every 2 s would trip
the auto-log into writing a `lights` entry per tick.

### D097: the Light Cycle spec closes out — obligations verified, glossary aligned

_(Numbered D097 in the 2026-11-17 merge with the released
feature-strip stream: the entry first shipped as D086 on this
branch, a number the stream's own v0.7.0 decision already holds;
that stream keeps its number.)_

Closeout state of the "Light Cycle stepped bands and ECharts chart overhaul"
spec:

- SPEC §4.9 describes the implemented precedence: lights-on and lights-off
  targets exist per phase, the bound light entity's history wins, the Light
  Cycle fills the gaps, neither source keeps a single band, and the coach
  resolves the same way. The phantom `preset_env` tables in SPEC §8 stay,
  deliberately — separate cleanup, out of scope.
- GLOSSARY.md coherence pass: added the missing agreed terms Lights-On Period,
  Lights-Off Period and Target Band. The Light Cycle entry lost its
  storage-speak ("nullable"). No duplicates, no drift, definitions stay
  implementation-free.
- Obligations discharged: the AGENTS.md architecture line names npm + esbuild
  for the chart bundle (D088); `docs/adr/0001-echarts-rendering.md` exists;
  the decisions doc carries the night leaf offset (D086), the VPD clamp
  (D087), the ECharts tooling (D088), the Light Cycle model (D089), the band
  precedence and corridors (D093–D094), and everything since.
- Story 22's binding-UI obligation verified end to end: the Light role's
  picker lists every entity (no domain filter anywhere in the chain), and a
  `switch.*` on/off entity bound to the Light role drives the chart
  staircase exactly like a `light.*` fixture (the mock now serves
  `switch.grow_panel`; integration test
  `grow_history_steps_follow_any_on_off_entity_bound_to_light`). The
  binary_sensor.* domain reads the same `"on"`/`"off"` states through the
  same parse, so it is covered by the same rule rather than its own mock
  entity.

Open ideas, none blocking:

- **Value-axis tick labels in the HA unit system.** The temperature axis
  renders "20.0 °C / 68.0 °F" per tick (ticket 02 called it cluttered).
  Per-system ticks need plumbing that does not exist yet: nothing exposes the
  HA unit system to the frontend (the bridge caches only the config
  timezone), and the card's formatter is a plain `fn(f64) -> String`, not a
  unit-aware closure. Judged more feature than polish, so skipped. The axis
  name already declares both units, and the tooltip keeps dual-unit text.
- **Alert evaluation against night bands** (research caveat C3) and
  **suspending germination-dome VPD band checks** (C4) — future work, as the
  spec's out-of-scope list records.

---

## 2026-11-17 — the Target Bands step between day and night (ticket 07)

### D093: the light-state timeline wins by source — sensor staircase first, schedule fills the gaps

_(Numbered D093 in the 2026-11-17 merge with the released
feature-strip stream: the entry first shipped as D082 on this
branch, a number the stream's own v0.7.0 decision already holds;
that stream keeps its number.)_

The history endpoint resolves one lights-on/off timeline per Chart
window, by precedence:

1. **A bound light entity's real history wins wherever it has
   in-window readings.** The staircase semantics match the light
   chart: each reading's state holds until the next reading, and the
   last reading holds until the window end. Any positive value counts
   as lit (a dimmer at 42 % is a lit fixture); exactly zero is dark.
2. **The stretch before the first reading belongs to the Light
   Cycle.** A reading's state cannot be projected backwards, so the
   schedule covers that prefix (clipped at the first reading). When
   no light entity is bound, the schedule owns the whole window.
3. **Neither source → no light state.** The band stays single, as
   before ticket 07.

Every grow has a Light Cycle (stored default = cited presets), so
unbound grows step day/night out of the box; the single-band path
survives only for the unlit phases. The schedule anchors at the
lights-on wall-clock time in the grow's canonical timezone and lasts
`hours_on` **absolute** hours, so the on-length survives a DST
transition while the anchor stays local — the convention
`phase_spans` already uses for phase boundaries.

A failed HA fetch of the *light* entity never degrades the chart:
the schedule covers the window instead, and the `degraded` flag
stays bound to the charted role's own fetch. The default Light Cycle
(06:00, strain-aware hours) is a grower-preference anchor marked
unverified in `light_cycle.rs`.

### D094: band segments grow a light-state dimension; CO₂ gets its first corridor

_(Numbered D094 in the 2026-11-17 merge with the released
feature-strip stream: the entry first shipped as D083 on this
branch, a number the stream's own v0.7.0 decision already holds;
that stream keeps its number.)_

`BandSegment` (and the frontend's `BandSegmentData`) carries
`light_on: Option<bool>`. `None` covers the unlit phases (drying and
curing never split) and the no-light-state case. The pure builder
`build_light_bands` intersects phase spans with the light timeline,
computes each piece's corridor, and merges neighbours that agree on
`(min, max, phase, light_on)` — a month of steady nights is one
region, not 60. The comparison is exact on purpose: range edges are
preset constants copied bit-identical (the D074 rationale again), so
the merge carries a scoped `#[allow(clippy::float_cmp)]` with the
reason inline, and the butt-join check `last.end == segment.start`
carries `#[allow(clippy::suspicious_operation_groupings)]` the same
way. The endpoint's response-reduce helper keeps a
`#[allow(clippy::type_complexity)]`: its four-tuple mirrors the
`GrowHistory` fields one to one.

Corridors per state: temperature uses the lights-on §5.2 range by
day and the researched lights-off range by night. VPD swaps to the
night band. The humidity band solves the RH window at the segment's
actual **temperature** mean with the matching leaf offset (−2 °C
day, −1 °C night) — the humidity-as-temperature bug is fixed at the
root, with the temperature series fetched for the humidity chart.
Segments without in-segment points have no honest mean and emit no
humidity band. CO₂ gains its first corridor: 800–1,500 ppm while
lit, ambient 400–450 ppm in the dark, the lights-on target kept
when the state is unknown.

The frontend needs no new drawing logic — the option builder
already rendered one `markArea` region per segment, so varying
ranges step for free. The e2e seam (`16-stepped-bands.spec.ts`)
asserts the visible half: multiple filled regions of differing
geometry inside the chart SVG, fed by a grow whose Light Cycle is
set and, separately, by the bound light's fixture staircase.

### D095: the mock's light history is a request-anchored on/off staircase

_(Numbered D095 in the 2026-11-17 merge with the released
feature-strip stream: the entry first shipped as D084 on this
branch, a number the stream's own v0.7.0 decision already holds;
that stream keeps its number.)_

`GET /core/api/history/period` for `light.grow_led` now returns a
deterministic off→on→off staircase stamped relative to request time
(off at now−23 h, on at now−20 h, off at now−18 h), in the
documented `minimal_response` shape. Sensor histories keep their
canned 2026-11-08 points — nothing asserted on those timestamps.
The relative stamping keeps the staircase inside any rolling window
on every run date, which is what the bound-light integration test
and `16-stepped-bands.spec.ts` both lean on.

## 2026-11-17 — the pair scatters move onto ECharts; unbound cards stop rendering (ticket 04)

### D092: every chart is an option-built chart; the card-visibility rule lands

_(Numbered D092 in the 2026-11-17 merge with the released
feature-strip stream: the entry first shipped as D081 on this branch,
a number the stream's own decision already holds; that stream keeps
its number.)_

The pH/EC pair cards now render through the same pure option builder as
every other chart (D090): `role_style` maps the pair roles to a
`SeriesKind::Scatter` drawing mode, and the builder emits two named dot
series — `Input` in the theme accent (`text-primary` probe, the old
`fill-primary`) and `Runoff` in a second, distinct colour probed from
`text-secondary` (the old `fill-secondary`). The runoff values join the
value-axis range so the compare dots can never sit off-axis. With the
pair migrated, the hand-rolled SVG renderer is gone entirely: ticket 03
kept `ChartSvg` alive for pH/EC behind a `TEMPORARY` marker, and its
small `padded_value_range` helper dies with it. No hand-rolled chart
code remains; the Gantt and feed-fit bars stay inline-styled by design
(out of the charts overhaul's scope).

Choices worth remembering:

- **`step: bool` became a `SeriesKind` enum.** A third drawing mode
  (scatter) arrived, and two booleans with a never-both invariant is
  worse than an honest enum: `Line`, `StepLine` (the light staircase),
  `Scatter` (the pair roles).
- **The scatter series carry names; the line series stay unnamed.** The
  tooltip closure now renders one line per axis-trigger entry and
  prefixes the series name when there is one, so the pair tooltip reads
  `Input: 6.2 — 2026-11-16 14:03 UTC` over `Runoff: 6.8 — …`, while the
  continuous charts keep the exact `value — time` line they always had.
- **The compare colour's probe class lives in a string literal.** The
  Tailwind scan picks tokens out of the Rust sources, and
  `text-secondary` appears nowhere else, so the probe call itself is
  what gets the utility compiled (the same accident `text-primary`
  already relies on through its doc comment).
- **A black probe result now means "styles not ready".** Writing the
  distinct-colour e2e exposed a latent bug from ticket 02: if the chart
  effect runs before the stylesheet applies, `getComputedStyle` returns
  the unstyled initial `rgb(0, 0, 0)` and the option baked black chrome
  (dots and axes) into every first paint. Black is never a leaflog
  theme colour, so the probe now rejects it and the chart draws with
  the light-palette fallbacks instead; once styles are loaded the
  probes resolve the real theme colours as before.
- **The card-visibility rule: `bound` decides, nothing else renders.**
  `HistoryChart` gates the whole card on the loaded response's
  `bound` flag — no sensor, or no pH/EC entries, means no card, no
  placeholder. A dead binding is `bound: true` (the binding exists), so
  its card stays and the env dashboard's No-data banner names it. A
  bound role whose history fetch fails keeps its card too: a failed
  fetch consults the bindings endpoint, and a bound role renders a
  degraded banner naming the failure instead of vanishing. The
  glossary's rule holds — a dead binding is the only state the page
  may call "no data". Unbound roles, and the pH/EC log roles a failed
  first fetch cannot classify, stay hidden.
- **The visibility gate is a reactive block, not `Show`.** A `Show`
  children closure must be `Fn`, and the card carries the children slot
  (the soil band editor, D091) — building it inside a `Fn` closure does
  not type-check. The gate is `{move || data.get().filter(bound)}`,
  the same declarative pattern the env dashboard uses; the editor
  re-mounts with its card and its signals re-run.
- **11-soil-band e2e runs on a dedicated bound grow.** The landing
  page's grow pick is not deterministic under the parallel suite, and
  an unbound grow would now legitimately hide the soil card, so the
  flow binds `sensor.grow_soil_moisture` and navigates to its own grow
  page instead of `/`.

---

## 2026-11-16 — every continuous-role chart on ECharts (ticket 03)

### D091: the option builder serves every continuous role; the band editor moved into the soil card

_(Numbered D091 in the 2026-11-17 merge with the released
feature-strip stream: the entry first shipped as D080 on this branch,
a number the stream's own decision already holds; that stream keeps
its number.)_

All six continuous-role cards (temperature, humidity, VPD, CO₂, light,
soil moisture) render through the same pure option builder
(ticket 02's seam, D090). `role_style` maps the history response's
role to the value-axis unit name (`°C / °F`, `%`, `kPa`, `ppm`, and
`on / off` for the binary light), so a card's mount passes only its
formatter; the axis-name choice and the formatter can no longer
disagree. The per-role value formatting is unchanged: `58 %`,
`0.85 kPa`, `900 ppm`, `on`/`off`.

Choices worth remembering:

- **The light staircase is a native step series.** The option sets
  `step: "end"`, which holds each value until the next reading's x,
  then jumps — the exact semantics of the deleted `step_points`
  vertex expansion. No points are invented; the series data stay the
  raw `[epoch ms, value]` pairs.
- **CO₂ keeps no bands, and that renders cleanly.** The backend ships
  no CO₂ band source today, so the builder emits zero `markArea`
  regions and frames the axis on the readings alone (unit-tested).
  Ticket 07 adds the stepped day/night bands.
- **The soil band editor lives inside the soil card.** `HistoryChart`
  grew an optional `children` slot rendered under the chart, and the
  soil mount passes `<BandEditor />`. The accessible field labels
  ("Band min %" / "Band max %") are untouched; the e2e flow scopes to
  the card by its heading, so the relocation is what the test pins.
- **The hand-rolled SVG is gone except the pH/EC scatter.**
  `geometry()`, `ChartGeometry`, `step_points` and their tests are
  deleted. The coordinator's boundary note keeps the event-dot
  component alive until ticket 04 swaps pH/EC to an `ECharts` scatter
  and removes the last SVG code; it is marked as temporary at the
  component. The scatter's dot scale keeps the same padding rule in a
  small `padded_value_range` helper that dies with the rest.

## 2026-11-16 — temperature chart on ECharts (ticket 02)

### D090: the option-builder seam replaces the SVG geometry seam; the chart window vocabulary changes

Renumbered twice: from a first draft D078 in the ticket 02 branch
(ticket 05's Light Cycle decision had landed as D078 while this ticket
ran), then from D079 in the 2026-11-17 merge — the released
feature-strip stream claims D079 — so D090 is the authoritative number
for this entry.

The temperature card now renders through `ECharts` (ADR 0001, D088).
A pure function in `chart_option.rs` turns the history response into
the option JSON: axes, the dual °C/°F unit label, gridlines, the
hover tooltip, explicit padded axis bounds (the headroom rule,
carried over from the old `geometry()`), and the Target Band regions
as `markArea` pairs clipped to the window. The Leptos side only
mounts: it probes the active daisyUI theme's computed colours for the
option, passes the JSON with `serde-wasm-bindgen`, and drives
`window.echarts` with `js_sys` calls — no hand-written application
JavaScript. The value-axis and tooltip formatters are wasm closures
handed over with `Closure::into_js_value`, so the JS GC owns their
lifetime after the handoff.

Choices worth remembering:

- **The option crosses the wasm boundary as plain JSON objects.**
  serde-wasm-bindgen's default serializes a `serde_json::Value::Object`
  as a JS `Map`, and `setOption` reads plain objects only — a Map
  option renders a bare frame with no complaint. The option serializes
  with maps-as-objects, the axis/tooltip formatter closures attach to
  the builder's single `yAxis` object (an array-indexing attach would
  silently skip), and the time axis is pinned to the Chart window's
  bounds so sparse data cannot zoom the axis in. The e2e suite
  therefore asserts the rendered DOM — axis name, tick labels, line
  path, hover tooltip — not just the Rust-side JSON.
- **The aria-label contract moved to the SVG root.** The hand-rolled
  chart was one `<svg role="img" aria-label="<role> history">`; the
  e2e suite locates that element. `ECharts` renders its own root, so
  the component stamps `role="img"` and the same label onto the
  rendered root after init — the contract survives the migration.
- **Colours are probed, not hard-coded.** Both daisyUI themes are
  active by `prefers-color-scheme`; the option's colours come from
  `getComputedStyle` on throwaway probes carrying `text-primary` /
  `text-base-content` classes, so the chart follows the theme. The
  band tint rides on `opacity` instead of an alpha colour, because
  SVG attributes (where zrender paints) do not resolve `var()`.
- **Chart window toggles read 24h / 7d / 30d everywhere.** The old
  "Day" collided with the Light Cycle's day (new GLOSSARY entry
  "Chart window"). API labels stay `day` / `week` / `month`.
- **The i18n gap is real but empty.** The ticket's "locale catalogs
  keep key parity" criterion has no runtime target: the locale-catalog
  effort is still spec-side, and the app ships inline English strings
  (the BandEditor precedent, also noted in D089). The renamed
  captions are plain English strings; when catalogs land, these keys
  migrate with the rest.

## 2026-11-16 — the per-grow Light Cycle (ticket 05)

### D089: the Light Cycle is grow columns, not a settings key

_Renumbered twice: from a first draft D075 (while this ticket ran,
D075–D077 went to the night leaf offset, the VPD zero clamp and the
ECharts bundle) and again from D078 in the 2026-11-17 merge — the
released feature-strip stream claims D078 — so D089 is the
authoritative number for this entry._

The Light Cycle (one lights-on local time plus hours-on per lit
phase) belongs to one grow, like the strain or the start date, so it
is stored as a nullable JSON column on `grow`
(`light_cycle`, V18) — not as a key under `settings`, which is
global. The soil-moisture band override kept its settings key
because it is app-wide; a Light Cycle per grow under settings would
have meant grown-up key names (`light_cycle.<grow-id>`) and no
foreign-key integrity. `NULL` means "the preset defaults apply",
which makes reset-to-default a `DELETE` on the cycle endpoint that
writes `NULL` — the same cleared-override shape the soil band uses.

The whole document is validated in Rust
(`leaflog-core::light_cycle`): a bad time or out-of-range hours
answers a clear 400. The defaults are pure functions of the phase
and the strain's photoperiod type, shared by backend, editor
placeholders and the phase picker's derived signal. Photoperiod
defaults are 18/6 from germination through veg and 12/12 flower
(SPEC §5.1; GrowWeedEasy's light-schedules guide — seedlings start
in the vegetative stage on the same 18/6). Autoflowers get 18 h long
days throughout (SPEC §5.1 "commonly 18–20 h", low end kept so a
dark period remains; research caveat C6). The 06:00 default
lights-on time is marked unverified in its source label: no
published standard anchors a wall-clock on-time — it is a grower
preference.

Editor strings are plain English inline, like every existing form:
the locale catalogs the spec's Language section describes do not
exist in the codebase yet, so key parity has nothing to compile
against. When the localization layer lands, these labels convert
with the rest of the forms.

## 2026-11-16 — night presets in core (light cycle ticket 06)

### D086: lights-off maths get their own leaf offset; the night presets land as sourced constants

_(Numbered D086 in the 2026-11-17 merge with the released
feature-strip stream: the entry first shipped as D075 on this
branch, a number the stream's own v0.7.0 decision already holds;
that stream keeps its number.)_

The −2 °C default leaf offset is a lights-on assumption: under the
lamps, leaves run measurably below the air sensor's reading. In
darkness the stomata close, transpiration cooling stops, and the leaf
sits close to air temperature; Black Dog LED's VPD guide puts the
lights-off span at 0 to −1 °C. Core now carries `NIGHT_LEAF_OFFSET_C`
(−1 °C, the conservative edge) beside the default. Ticket 07's
lights-off band segments pass it in place of the default, so night VPD
and RH numbers stop carrying a cooling assumption that belongs to a
lit room.

The presets landed beside it, each row with its source label. Night
temperature corridors: seedling 20.0–23.3 °C, veg 18.3–23.9 °C,
flower 15.6–23.3 °C. Night VPD bands: germination/seedling
0.5–0.9 kPa (sources span 0.3–1.0 and the label says so), veg
0.8–1.2 kPa, flower 1.0–1.4 kPa. Both are Option-returning accessors:
drying and curing rooms run without lights, their single day bands
apply 24/7, and the accessors return `None` for them. Night CO2 is
ambient 400–450 ppm with enrichment off; the lights-on band
(800–1,500 ppm) comes from the two enrichment articles, which disagree
on the ceiling (1,200 vs 1,500 ppm), so the encoded union keeps that
disagreement visible in its label.

Two open items travel with the numbers. Inside a germination dome at
75–90 % RH the computed VPD sits far below any band, so band checks
for dome-bound grows stay suspended until a ticket takes them up. And
no university extension publishes cannabis lights-off setpoints — the
strongest anchor is the peer-reviewed hemp study (Agronomy
13(10):2636), which disagrees with hobby guides on ideal night
temperature; the ranges keep both.

### D087: the VPD derivation clamps at zero, as a tested contract

_(Numbered D087 in the 2026-11-17 merge with the released
feature-strip stream: the entry first shipped as D076 on this
branch, a number the stream's own v0.7.0 decision already holds;
that stream keeps its number.)_

The Magnus derivation in `vpd_kpa` has always applied `.max(0.0)`, so
no consumer has seen a negative value. Nothing guaranteed it: the
clamp was an implementation detail, one refactor away from vanishing,
and a saturated dark period (100 % RH, leaf cooler than air) computes
a raw difference around −0.4 kPa. The clamp is now part of the
derivation's contract: golden-value cases pin the saturation clamp, a
grid over realistic temperatures, humidities and offsets asserts
VPD ≥ 0, and the chart's derived-VPD test covers both leaf offsets at
saturation. Alert evaluation against night bands (an open item, not
yet built) inherits the guarantee.

---

## 2026-11-16 — charts move to ECharts (ticket 01, Light Cycle spec)

### D088: Charts render through a build-time ECharts bundle; npm + esbuild join the toolchain

Renumbered twice: from a first draft D075 in the ticket 01 merge
(ticket 06's night-preset decisions claimed D075/D076 on the
integration branch first), then from D077 in the 2026-11-17 merge —
the released feature-strip stream claims D077 — so D088 is the
authoritative number; branch commits from before the merges still say
D075 or D077.

The history charts leave the hand-rolled SVG path (ADR 0001 covers the
rendering trade-off). The bundle lands as a build artifact:
`crates/leaflog-frontend/charts/` is an npm project with exact pins
(echarts 6.1.0, esbuild 0.28.2, committed lockfile), and
`just build-echarts` bundles an entry module that registers line +
scatter charts, grid + tooltip components, and the SVG renderer onto
the `window.echarts` global. index.html loads it before the wasm
boot; no application JavaScript is or will be hand-written — the wasm
app will drive `echarts.init` from Rust in the chart tickets.

Choices worth remembering:

- **Barrels bypassed on purpose.** `echarts/charts` and
  `echarts/components` re-export every chart type, and the package
  marks `lib/chart/*.js` as side-effect-ful — a glob-array
  `sideEffects` form esbuild does not apply, so a barrel import pulls
  the whole library into the graph. The entry imports per-feature
  install modules instead, and the bundle sits at 517 KiB minified
  (roughly half a full-library dump).
- **Composition checks read the output graph.** The esbuild metafile
  lists every parsed file under `inputs`, but `outputs[...].inputs`
  with `bytesInOutput` says what actually landed in the bundle;
  `charts/verify.mjs` judges tree-shaking there, plus size bounds, no
  CDN host, no Node constructs, and the index.html/dist wiring. The
  checks run in `just build-echarts`, `just build-frontend`, `just
  e2e`, and the Docker build.
- **Node stays build-time.** The Dockerfile's frontend-builder stage
  installs Debian's nodejs 18 (esbuild needs >=18) for the bundle
  step; the app stage still copies only the backend binary with the
  embedded dist, so the release image keeps no Node. The AGENTS.md
  "no Node anywhere" stance is amended to "no Node in the image",
  matching the Tailwind CLI precedent (D064).
- **Out of scope, deliberately:** zoom/pan/brush components are not
  in the bundle; the forecast Gantt and feed-fit bar chart stay
  hand-rolled for now.

---

## 2026-11-16 — grow overview redesign (D073 follow-up)

### D074: `float_cmp` allowance in the chart's step builder

`step_points` (`leaflog-frontend/src/chart.rs`) turns a plotted line
into a staircase: each new value holds until the next point's x. The
step fires only when the new value differs from the held one, and
that comparison is exact on purpose — the held value is the same
copied `f64` bit pattern, not a computed float, so there is nothing
for a margin to absorb. `clippy::float_cmp` cannot see that, so the
match carries `#[allow(clippy::float_cmp)]` with the reason inline.
The paired test asserts the staircase shape without comparing
computed floats.

Also carries the planned-vegetative-weeks wiring (a nullable `grow`
column, form field and forecast input — the missing piece ticket 01
flagged), and the chart role extensions' small inline casts, whose
bounds are documented at each site.

---

## 2026-11-15 — phases become events (grow overview redesign)

### D073: Phase picks record transition events; the forecast re-anchors from them

Until now the derived forecast was the only source of the grow's
current phase: `Phase` follows from the start date, the flip and
harvest events, and the default windows, and a grower who flipped
late, harvested early, or simply disagreed had no way to say so. The
fix keeps one answer to that question: there is no override state at
all. Setting the phase from the interface records the same transition
events the forecast already consumes — Flowering records a Flip,
Drying records a Harvest, and boundaries with no canonical action
(into Seedling, Vegetative, Curing, or back to Germination) record a
new Phase-change action. The forecast re-anchors its windows from
those pins, and the environment bands, phase coach, lamp advice, and
the day-count target follow.

An override layer on top of the derived phase was the runner-up and
was rejected: it creates two answers to "what phase is this grow in"
that can disagree, and every consumer would have to pick a winner.
It also needs its own store, its own backdating rules, and its own
clearing rules — the event stream already has all three.

With picks being events, the silent four-week vegetative default for
photoperiods goes too. A photoperiod grow stays in vegetative until a
pin or a planned veg length says otherwise; the default made the
forecast show flower windows nobody planned. Autoflowers keep their
age-based flip estimate. GLOSSARY.md now carries Phase pick,
Photoperiod, Autoflower, and Grow overview.

---

## 2026-11-14 — issue tracker moves to git-bug

### D072: git-bug as the issue tracker, synced to origin

The tracker moves from local markdown under `.scratch/` to
[git-bug](https://github.com/MichaelMure/git-bug) (v0.11.0, already installed
via devbox): bugs are git objects under `refs/bugs/*`, shared between clones
with explicit `git bug pull` / `git bug push` against `origin`. The motivating
cost of the machine-local tracker was coordination between parallel sessions
(this machine and Tangled): the same D-number got claimed twice (D061) and a
release landed under us (v0.4.1). The `git fetch` discipline covers commits,
but not working state. A tracker stored as plain git refs shares that state
without new infrastructure — no GitHub token, no Issues, no bridge (the
git-bug GitHub bridge exists if mirroring is ever wanted; unused for now).

Once pushed, bug content is public on GitHub. That matches the repo's existing
posture (SPEC.md and THREAT-MODEL.md are already public), and ordinary
`git push` never carries `refs/bugs/*`, so syncing the tracker is always an
explicit step and nothing leaks by accident.

Two things deliberately stay local. Wayfinder's map/child model does not fit a
flat bug store and its files are ephemeral working state, so it keeps
`.scratch/<effort>/` (gitignored), unchanged. The released
`grow-page-correctness` batch also stays in `.scratch/` as history. The live
`localisation` effort was migrated: spec + 7 tickets, statuses became labels,
original numbers kept in titles with shortId resolution recorded in comments.

The five canonical triage roles become git-bug labels verbatim; done work is
the built-in closed status, `wontfix` is label + close. The pinned build has
the old-style CLI with three sharp edges, all recorded in
`docs/agents/issue-tracker.md`: verbs precede the bug id; `git bug bug new -t`
does not persist the title, so skills create-then-`title edit`; and every
`-F` file body silently strips `#`-led lines (commit-comment behaviour), so
heading-bearing bodies are set verbatim with
`comment edit <comment-id> -m "$(cat body.md)"`. GitHub Issues was rejected:
it needs `gh` and network for every read and abandons the local-first working
posture.

---

## 2026-11-12 — grow-page correctness (v0.5.1)

### D071: Crate-level `empty_enums` allowance on Leptos-generated code

Rust stable moved under the project between releases: devbox resolved
the floating `channel = "stable"` to 1.100.0 (2026-11-12), and that
clippy newly fires the nursery lint `empty_enums` on the builder
marker enums the `#[component]` macro generates — fourteen
pre-existing sites across nine files, none of them hand-written. With
the mandatory `-D warnings` gate that broke `just ci` on a clean
checkout, proven by a stashed-tree run.

The carve-out is one crate-root line in `leaflog-frontend`,
`#![allow(clippy::empty_enums)]` beside the existing
`multiple_crate_versions` allowance, with the reason inline. It
exists to avoid fighting things outside our control: the offending
code is generated by an upstream macro, and the toolchain level that
activates the lint is whichever `stable` devbox resolves today.
Per-item allows were rejected: they would spread across
macro-generated items in files owned by concurrent tickets, and
every Leptos upgrade would force a re-audit. The crate-root scope
is static. Pinning an exact toolchain
version was also rejected: floating `stable` is the existing policy,
and a pin only relocates the next surprise. Cost accepted: the lint
is also silenced for a hand-written empty enum in this one crate.
The backend needed no allowance; the workspace native gate passes
with this single line.

---

## 2026-11-11 — agent tooling

### D070: Engineering-agent skills use a local-markdown issue tracker

The mattpocock engineering skills (`to-tickets`, `triage`, `to-spec`,
`wayfinder`) need to know where issues live. Chose local markdown under
`.scratch/<feature-slug>/` over GitHub Issues: agent-driven spec and ticket
workflows produce working notes (specs, ticket drafts, wayfinder maps) that
belong next to the checkout, not in the public tracker. `/.scratch` is
gitignored like `/data` — machine-local state, never pushed.

Wiring lives in `docs/agents/`: `issue-tracker.md` (file conventions and
wayfinding), `triage-labels.md` (the five canonical triage roles kept verbatim
as `Status:` values), and `domain.md` (single-context domain docs — a
lazily-created root `GLOSSARY.md`; decision conflicts cite `D0xx` entries in
this file rather than a separate `docs/adr/`). `AGENTS.md` points at the three
via its `## Agent skills` section.

---

### D069: Frontend E2E — Playwright under a simulated ingress prefix

D061 (every form silently doing a native GET submit) and D066 (edit UI
that called nothing) both shipped to a release because the compiler
cannot see them: a listener for a custom event name compiles fine. Both
were found by hand-run playwright sessions that then evaporated. The
kept answer is a Playwright suite in `e2e/`. Node there is test-time
tooling only, like the build-time Tailwind CLI (D064) — nothing Node
enters the shipped image.

Harness: the `e2e` justfile recipe serves the built `dist/` through the
real backend against the mock Supervisor, on ports 8199/8198 (so `just
dev` can keep running), under the fixed prefix `/api/hassio_ingress/e2e`
and with a throwaway SQLite DB and photo dir. Every navigation and API
call in the suite goes through that prefix.

The D061 guard: each form case arms a `window` marker before submit and
asserts it survived — a native submit reloads the page and loses the
marker. Each case then reloads and asserts the data persisted. All ten
forms are covered (equipment, strain, space ×2, seed batch, grow ×2,
wizard, quick-add log, plant edit), plus the ingress/deep-link checks
and the photo upload.

Rules: one shared stack per run (`workers: 1`, numeric file order —
`01-onboarding` needs the fresh DB and dismisses the wizard for the
rest), and every new form gets a case in `e2e/src/02-forms.spec.ts`.
`just e2e` skips when node is absent, the gitleaks/deny pattern; CI's
dedicated `e2e` job installs node + chromium so the gate is real, and
release runs it through the ci workflow call.

---

## 2026-11-11 — post-M8

### D068: `cog check` ignores merge commits

The vale-gate session merged its branch with git's default "Merge
branch '…'" message, which is not a conventional commit — and `cog
check` (the first `just ci` recipe) then failed for every later
checkout, including the Image workflow on main. Merge commits carry no
changelog content, so the `cog-check` recipe passes
`--ignore-merge-commits`; branches should still land as conventional
commits.

### D067: A space's sensor items are the default env bindings; seeding only fills unbound roles

Spec §4.1 says a space's temperature + humidity sensor items "are
offered as the default env bindings for grows in it (per-grow overrides
stay)" — but nothing implemented the bridge, so a sensor added on the
equipment page was write-only inventory while the live tiles read the
per-grow `sensor_binding` rows only the grow page set. Growers could
not discover the split (reported: "can't work out how to add a sensor
to equipment").

`store::sensors::seed_from_space` maps a sensor item's device class to
its binding role and inserts bindings **only for roles the grow has not
bound** — an explicit pick is never overwritten, and with several items
of one class in a space the first in list order wins. It runs from
three hooks: grow create, grow update (a grow moved into a space picks
its defaults) and sensor-item-added-to-space (existing grows catch up),
plus `seed_sensor_defaults` once at startup as an idempotent healing
pass for installs whose data predates the bridge. Numbered D067: while
this work was in flight the Tailwind, UI-widening and vale sessions
landed D063-D065 on main.

### D066: Grow and plant editing in the UI (the PUT endpoints existed, nothing called them)

`PUT /api/grows/{id}` and `PUT /api/plants/{id}` shipped in M2/M11, but
no page offered them: grows were create-only and plants could not be
renamed or re-weighed after the fact. The grows page and grow detail
page now share one `GrowForm` component (name, strain, space,
substrate, started-on, plus the previously UI-less `status` and `notes`
fields); `GrowInput::from_grow` in leaflog-core mirrors
`EquipmentInput::from_item` for the pre-fill. The detail page also
gains "Delete grow" — destructive (plants, log entries and photos go
with it), so it asks for a `confirm()` first and redirects via
`<Redirect>` rather than `use_navigate`, whose non-`Copy` closure
cannot be captured from a reactive header render (the E0525 lesson:
handlers must stay `FnMut`; payloads are cloned per render like the
equipment page's `for_edit`). `<Redirect>` takes the **plain route
path** (`/grows`), not `route_href("/grows")`: unlike an `<A href>`,
it routes through `navigate()`, which resolves its argument against
the router base — a base-prefixed path doubles the ingress prefix and
the router falls to its fallback view. Found with the playwright smoke
(`just dev` + mock HA), the same class of catch as D061.

---

## 2026-11-11 — prose gate

### D065: Vale prose gate — LLM-tell vocabulary, 50-word sentence cap, curated write-good

`just lint-prose` (in the `just ci` chain, enforced by a dedicated
CI job) runs [vale](https://vale.sh) over every piece of prose in the
repo: all markdown (changelogs and the licence are generated or legal
text, not prose), all Rust comments, and the frontend's UI strings.
The goal is narrow: the repo must not grow LLM-sounding text —
telltale vocabulary or walls of text. Two style packages, curated:

- **`Prose`** (repo-owned, `.vale/styles/Prose/`): an existence rule
  with a surgical LLM-tell word list (`delve`, `seamlessly`,
  `plethora`, `in conclusion`, `it's worth noting`, …) at error
  level, and the language-complexity gate — no sentence past 50
  words (`occurrence` rule, `scope: sentence`, warning). The word
  list deliberately omits domain words: `harness` (test harness),
  `unlock` (LUKS), `elevate` (lamp height), `elevation` (STRIDE).
- **`write-good`** (vendored verbatim from
  [errata-ai/write-good](https://github.com/errata-ai/write-good),
  MIT — license text in its README): the standard grammar lints.
  Calibrated against the existing corpus, data-driven: E-Prime off
  (be-verbs are the right voice for describing state), Passive and
  TooWordy demoted to suggestion (380 legitimate "is stored"-style
  passives; TooWordy's business-prose list collides with the domain
  — "minimum"/"maximum" presets, "purchase" equipment log,
  "equivalent" EC↔TDS). `Cliches`, `So`, `ThereIs`, `Weasel` and
  `Illusions` gate as warnings/errors. **Vendoring exception** (the
  no-vendoring rule, AGENTS.md): vale styles have no dependency
  mechanism — `vale sync` fetches the latest release zip unpinned,
  which would make the gate drift. Removal plan: re-sync from
  upstream and re-pin when vale gains version-pinned packages.

The 50-word cap was calibrated, not assumed: 30 words flagged 175
existing sentences, 50 flagged 20 — all fixed in this change as
genuine walls. Baseline cleanup also fixed every weasel word and
`There is` opener the gate surfaced.

Extraction (bash inside the recipe, per the no-scripts rule):
comments strip `//`, `///`, `//!` and join consecutive lines into
paragraphs; rustdoc headings and list items start their own
paragraph; every paragraph gets terminal punctuation and a blank
line (vale's plain-text format runs sentences across paragraphs
until it meets a period). UI strings are frontend string literals
that contain whitespace, minus Leptos attribute fragments and
pure format-spec strings. The registry's `AiTells` package was
considered and skipped: the repo-owned list is smaller and
domain-aware.

## 2026-11-10 — post-M8

### D064: Tailwind v4 + daisyUI 5 replace the hand-rolled stylesheet

Rust-native Leptos component libraries were ruled out first: thaw 0.4.8
pins `leptos ^0.7.7` (we are on 0.8), Leptonic last shipped Feb 2024,
no Mantine port exists. Adopted instead: **Tailwind CSS 4.3.5 standalone
CLI + daisyUI 5.7.50**, both build-time only (no Node anywhere, per the
non-negotiables — Trunk fetches the pinned tailwind binary via
`[tools]` in Trunk.toml and runs it off `<link data-trunk
rel="tailwind-css">`).

- Per the no-vendoring agreement, the daisyUI plugin bundles are
  **fetched at build time, not committed**: `just tailwind-daisyui`
  downloads `daisyui.mjs` + `daisyui-theme.mjs` (release v5.7.50, MIT)
  into the gitignored `tailwind/` dir and refuses to proceed on any
  sha256 mismatch (`d69de07c…6be472`, `e48a35d8…f251`) — the same
  pinned-fetch pattern Trunk itself uses for the tailwindcss binary.
  CSS entry is `tailwind.css` (`@source "./src/**/*.rs"`);
  `styles.css` is deleted.
- The green identity survives as two custom daisyUI themes (`leaflog`
  light, `leaflogdark` dark via `prefersdark`) carrying the old palette:
  base-100 = card surface, base-200 = page, base-300 = hairline.
- Stayed hand-rolled by design: gantt + feed-chart geometry (inline
  styles from Rust), the `.facts` dl grid, `.mainnav a[aria-current]`
  (leptos_router flags active via aria-current, not a class), and
  element defaults preflight strips (headings, `p` rhythm, `a`, `code`).
- Leptos 0.8 gotchas found: an element cannot mix `class="…"` with
  `class:name=` directives (two `Class` attributes fail `RenderHtml`);
  reactive whole-class values use `class=move || …` (or a helper
  returning the closure, because the view! parser rejects inline
  `move || if {} else {}`). Every class token must appear as a literal
  in the sources for Tailwind's scanner — `format!` fragments produce
  no CSS, so conditional classes pick between full literal strings.
- `steps-sm` was dropped: daisyUI 5 has no size modifier for `steps`.

### D063: app shell widened to 72rem

The UI shell (`styles.css` `.shell`) was capped at `46rem` — a reading
column, narrow for a dashboard-style app with split grids, galleries and
charts. Widened to `72rem`, still centred with side padding. The 48rem
media queries are viewport-based and unchanged; all inner layouts
(3fr/2fr split, auto-fill gallery, percentage-positioned gantt bars,
flexbox feed bars) are fluid, so no other rules needed adjusting.
### D062: Home Assistant serves `leaflog/CHANGELOG.md`; version headings are bare `## <version>`

The HA update dialog kept showing the 0.1.0 changelog for every
update. Two stacked causes:

- Supervisor does not read git tags or GitHub releases for the
  changelog — it serves `<addon-dir>/CHANGELOG.md` verbatim from its
  shallow clone of the repository's **default branch**
  (`supervisor/apps/model.py::path_changelog`). All release tooling
  maintained only the repo-root `CHANGELOG.md` (D042), which Supervisor
  ignores for a subdirectory app; `leaflog/CHANGELOG.md` had been frozen
  at its hand-written 0.1.0 content since the initial packaging commit.
- HA Core then slices that file per version in
  `hassio/update.py::async_release_notes`: a heading must be exactly
  `#* <version>` (e.g. `## 0.4.0` — the string from
  `leaflog/config.yaml`, no `v` prefix, date, or brackets) or Core
  falls back to the whole file. Cocogitto's `## vX.Y.Z - date`
  headings can never match.

`just changelog` now writes both files from one `cog changelog` run:
the root file stays cocogitto output verbatim (D042 unchanged);
`leaflog/CHANGELOG.md` is the HA-facing artifact with headings
rewritten (`## X.Y.Z` plus a `_Released date._` line) by a `sed` step
in the recipe — chosen over a cocogitto custom template because CI
pins cog 6.2.0 and the transform is verifiable locally. The release
workflow commits both files with the version sync, and pr-check
regenerates and diffs both, asserts every version heading is bare
semver with the top one equal to `config.yaml`'s version, and
simulates Core's slice regex — so a future cocogitto template change
cannot silently re-break it. Because Supervisor reads the default
branch only, the fix went live on merge + store reload; no image
re-release was needed.
### D061: Form submit handlers call `prevent_default` themselves — `on:submit:prevent` is not a Leptos modifier

Every form used `on:submit:prevent=save` — but `leptos_macro` 0.8 only
knows the `:undelegated`, `:target` and `:capture` suffixes
(`parse_event`, `view/mod.rs`). Anything else stays part of the event
name: the attribute compiled to a listener for a *custom* event named
`submit:prevent`, which the DOM never dispatches. Compiles fine, runs
never — so every save button (all 8 forms, including the onboarding
wizard) performed a **native** GET submission: page reload, all entered
data lost, nothing persisted. Found via a user's HAR capture (a
`GET …/?` plus a full asset reload where the POSTs should have been);
CI never saw it because the wrong listener name is not a compile error.

Fix: the canonical idiom — handlers take the event and call
`ev.prevent_default()` as their first statement, forms bind plain
`on:submit`. While fixing the wizard, its equipment-attach call moved
from `api::post_empty` (which tried to JSON-parse the `204 No Content`
body of `POST /api/spaces/{id}/equipment/{id}` and always failed,
swallowed by `let _ =`) to `api::post_action`, and the per-plant POST
failures now surface in the wizard's error line instead of being
dropped silently.

### D060: Environment alerts route to Home Assistant, gated three ways

Implements §4.9's notification routing:

- **Opt-in** (`env_alert_notifications`, default off): a background
  evaluator ticks every minute, computes each active grow's
  environment snapshot (the same shared `snapshot` the env route
  serves) and routes out-of-band verdicts — temperature and VPD, the
  two verdicts the snapshot carries — as persistent notifications via
  the documented `notify/persistent_notification` service.
- **Rate limit**: at most one notification per grow and alert kind
  per hour, in-memory (a restart clears it, which still honours the
  bound).
- **Quiet hours**: the `quiet_hours` setting (`HH:MM-HH:MM`, local
  wall clock in the canonical D059 zone, midnight-wrapping) suppresses
  sends; an unknown zone never blocks.
- The mock Supervisor accepts and records the notify service, so the
  integration test drives the evaluator directly (its own rate-limit
  map — no ticker sleeps) and proves the opt-in, the rate limit and a
  minute-precise quiet window end to end. Mould-risk stays un-routed:
  it needs the phase-conditional RH rule §4.9 describes, which the
  two verdicts do not carry yet.

### D059: Canonical timezone — HA's zone, local calendar days

Implements the §2 timezone canon (spec §13 "Timezone" criterion):

- **`jiff` 0.2** provides the IANA database embedded in the binary —
  the HA image has no system tzdata to read, and HA reports zones by
  name (`Europe/Berlin`).
- **`leaflog-core::tz`**: `day_number_local` counts whole local
  calendar days between the grow's start date and a UTC instant's
  local date in the zone; unknown zones return `None` rather than
  guessing (callers fall back to UTC math). Golden tests pin the
  2026-03-29 EU spring-forward and 2026-10-25 fall-back (the
  naive-UTC answers differ — day 3 vs 2 and day 5 vs 4). A
  `QuietHours` wall-clock window helper (midnight-wrapping,
  half-open) lands with it for the alert routing that consumes it.
- **Canonical resolution**: the `display_timezone` settings override
  wins, then the zone from the documented `GET /core/api/config`
  (fetched by the bridge on every (re)connect into a watch channel;
  unreachable HA keeps the last known zone), then UTC. mock-HA serves
  the config route with `Europe/Berlin`.
- **Every day-math site is converted**: action logging and updates,
  snapshot capture, forecast today, env/lamp/coach phase derivation,
  and the auto-log path. The end-to-end test logs an entry at
  23:30Z that is 00:30 the next local day in Berlin and asserts
  grow day 1, not the UTC day 0.

### D058: Snapshot retention and entity-drift status

First M12 pieces (spec §4.2, §4.1, §13):

- **Retention**: `store::retention::prune` thins `sensor_snapshot`
  rows captured before the cutoff to at most one per (entity, hour)
  — the earliest — always keeping each entity-day's first and last
  row and every excursion-linked row (a snapshot attached to an
  auto-logged entry, matched by the `auto:` notes marker). Rows
  inside the window are untouched. The window is the
  `snapshot_retention_months` setting (default 24; 31-day months — a
  retention horizon is a coarse thing). A task in `serve` runs it at
  startup and weekly (`MissedTickBehavior::Delay`, so a sleeping box
  does not fire a burst).
- **Entity drift**: `GET /api/grows/{id}/env` reports per-binding
  `BindingStatus` — available means tracked and not HA's
  `unavailable`/`unknown`. The dashboard shows a "No data … rebind in
  Entity bindings" chip for dead bindings; values and range verdicts
  were already `Option`-shaped and degrade to null — never "in
  range". Rebinding is the existing bindings editor
  (`live.rs::EntityBindings`). Env-alert HA notification routing
  (§4.9) deliberately waits: it needs a persistent evaluation loop
  and is not one of the §13 gates.

### D057: Cross-grow history — plan-based accuracy, aggregates, next-run

The §4.15 section, computed read-only over existing tables:

- **Accuracy compares the *plan*, not the live forecast.** The live
  forecast pins boundaries to logged events, so feeding actuals into
  it would zero every delta. `GET /api/history` therefore forecasts
  each grow with empty `Actuals` and compares the flip against the
  predicted flower start and the harvest against the predicted flower
  end (likely path), reporting `actual − predicted` per boundary —
  only for boundaries actually logged. The engine behind the predicted
  numbers is the M5-golden-verified one.
- **Yield aggregates** group per-grow totals by strain name,
  substrate and fitted feed-template name (D055's link), keeping only
  grows that produced a weight. Input-vs-runoff drift and alert
  frequency views are deferred until M12's alert routing persists the
  events they read.
- **Per-plant view**: `GET /api/plants/{id}/view` returns the plant,
  the log entries naming it (`plant_ids` is a JSON array — matched on
  the id between its quotes), its photos and, via the plant's
  weights, its yield. Grow-detail plant names link to it.
- **Start next run**: `POST /api/grows/{id}/next-run` creates an
  active grow copying strain, substrate, grow space and feed template
  by reference (seed batch deliberately not copied — §4.15 does not
  list it), started today, named `X (next run)`.

### D056: Germination-test log — observed evidence beside the preset

The §4.13 germination log (V12): `germination_log` rows per batch
(date, sown, sprouted, notes), created and listed under
`/api/seed-batches/{id}/germination-tests`. The batch view carries the
aggregated `observed` rate (total sprouted / total sown across tests,
`None` until the first test) next to the §5.4 `viability` window — two
separate fields, so the user's evidence can never overwrite the preset.
Validation: `sprouted ≤ sown`, well-formed `tested_on`, unknown batch
is a 404. Deleting a batch cascades its tests.

### D055: Harvest weights on plants, yield summary, strain write-back

The §4.14 foundation of M11:

- **Weights live on `plant`** (V11: nullable `wet_g`/`dry_g`/
  `trimmed_g`, grams = SI internal): unset means "not weighed",
  never zero. `PUT /api/plants/{id}` replaces name + weights
  (negatives and blank names are 400s). Display units stay the
  frontend's job, like every other quantity.
- **`GET /api/grows/{id}/yield`** aggregates via
  `leaflog_core::grows::YieldSummary`: per-plant breakdown, totals
  (a total is `None` when no plant carries that weight), and the
  wet→dry ratio when both exist and dry is positive.
- **Write-back** (`POST /api/grows/{id}/writeback`) copies the
  achieved totals into the grow's strain's `yield_notes` as one line
  per grow — `[grow:<id>]` marked, so re-writing replaces instead of
  piling up. User-owned strains only: bundled strains answer 409
  with the clone-redirect; a strainless grow is a 400.
- **`grow.feed_template_id`** (V11, nullable, SET NULL): grows record
  which template was fitted so §4.15 analytics can group by template
  and "start next run" can copy it by reference. Fits predating the
  column stay unattributed. Validated like the space and batch links.

### D054: Onboarding wizard — state-derived, dismissal as a settings flag

Implements the onboarding half of M10 (spec §4.12, §13):

- **State is derived, not stored**: `GET /api/onboarding` reports
  `show_wizard = no spaces && no grows && !dismissed`. Progress
  "persists across restarts" for free — the wizard reappears exactly
  while there is nothing to show, and its steps pre-fill from whatever
  already exists. Dismissal is the `onboarding_dismissed` settings
  flag; the empty-state "set up" card clears it to restart the wizard.
- **The wizard gates nothing**: every screen stays reachable; each
  step has an explicit skip; finishing with everything skipped only
  dismisses. Step 1 picks bundled presets; own gear and HA sensors
  link out to the equipment page — the wizard waits, it does not
  clone those forms. Step 2 names the space built from the picks.
  Step 3 picks a strain (or skips). Step 4 creates the grow — plants
  named by prefix, germination defaulting to today, optional seed
  batch link — and navigates to its dashboard.
- The acceptance test drives the wizard's exact call sequence against
  a seeded, otherwise-fresh database and checks the appear → run →
  never-again transitions plus dismissal and restart.

### D053: Seedbank — batches, §5.4 windows in core, oldest-first, link-decrement

Implements the seedbank half of M10 (spec §4.13, §13):

- **`leaflog-core::seedbank`** owns the viability math. Storage
  methods carry the §5.4 window table as sourced preset ranges
  (6–12 / 12–36 / 60–120 / 120+ months, each row labelled
  "spec §5.4 table (research snapshot 2026-11-08); sources
  disagree"). Ageing runs in whole calendar months: a mid-month
  acquisition is one month old only after its day-of-month passes.
  The arithmetic is signed — the bare month difference underflows
  across year boundaries. Past-prime = age beyond the window's upper
  bound; frozen is open-ended (`120, None`): "10+ years reported"
  never earns a past-prime badge. Past prime is a hint, never a
  block (§5.4).
- **API** (`/api/seed-batches`): CRUD with the viability window
  computed per response (`SeedBatchView` lives in core, shared by
  frontend). Listing orders oldest acquisition first within each
  strain (use the oldest seeds first), unlinked batches trailing.
- **The plant-start link**: `grow.seed_batch_id` (nullable FK, ON
  DELETE SET NULL — the strain_id pattern). Adding a plant to a linked
  grow decrements the batch count (clamped at zero, never raised);
  manual edits always win, so the decrement simply continues from the
  edited value. Deleting a batch keeps its grows running, unlinked.
- **UI**: a Seedbank tab listing batches with storage, window, age and
  the past-prime badge, plus a create/edit form (strain picker,
  storage defaulting to the §5.4 cool/dark/dry).

### D052: Sensor items are picked HA entities

Implements the HA-sensor half of D045 (spec §4.1, §13): a sensor item
is created by picking a Home Assistant entity, and the entity is the
item's identity.

- **Schema (V9).** `equipment_item` gains `ha_entity_id` (the sensor's
  identity; an optional binding for other categories — a `light.*`,
  power sensor or dimmer `number.*`) and `ha_device_class`
  (temperature/humidity, stored so a space's env sensors resolve
  without HA being reachable).
- **The API is the gate.** A sensor item without a `sensor.*` entity id
  or without one of the two supported device classes is a 400; a
  device class on a non-sensor item is a 400; a malformed entity id of
  any kind is a 400. Sensor items are therefore never hand-entered
  text and never bundled — only picked.
- **The picker.** `GET /api/ha/entities` gains `?device_class=`,
  filtering both the Supervisor list and the offline live-map fallback
  (mock-HA now carries `device_class` on its grow sensors). The
  equipment page's "Add sensor from HA" panel lists
  temperature/humidity entities; picking one pre-fills the form. With
  HA unreachable the panel shows an explicit skip-for-now notice and
  the rest of the app is unaffected — verified by the sensor API test
  running against an offline bridge.

### D051: Bundled-data upgrade rule — stable ids, insert-new-only seeding

Implements the §4.1 upgrade rule (spec §13 "bundled-data upgrade"
criterion). Every bundled record carries a stable id from its source
dataset (`seed-equip-<slug>`, `seed-strain-<slug>`). Upgrades insert
bundled rows that are new and never modify or delete existing ones;
user entries and clones are never touched. Corrections ship as new
revisions, and removals require an explicit migration (V6 did that).

- **V8 adopts pre-stable-id databases.** Existing databases were seeded
  with random ULIDs, so keying by stable id alone would duplicate every
  bundled row on upgrade. V8 re-keys old bundled rows onto their stable
  ids with the FK-safe insert → repoint children → delete-old pattern
  (foreign keys stay ON through refinery migrations; a plain PK update
  would violate them). Children follow: `grow_space_equipment` for
  equipment, `grow.strain_id` and clone parents (`strain.is_phenotype_of`)
  for strains. Adoption matches by (name, category) / (name, breeder)
  and only when exactly one row matches — zero matches means the row is
  "new" under its stable id (the seeder inserts it fresh), and an
  ambiguous match is left untouched rather than guessed at. Adopted
  rows are marked `is_builtin = 1` whatever the old flag said.
- **User edits survive by construction.** A renamed bundled row no
  longer matches its adoption block, keeps its identity untouched, and
  the pristine record inserts fresh beside it. Verified against the
  real dev database: the tent and light (unrenamed) were adopted onto
  stable ids while thirteen renamed rows kept their ids, and the
  renamed fans will get the pristine fan presets re-inserted next boot.
- **The seeder is idempotent and insert-only.** `seed_strains` /
  `seed_equipment` skip any stable id already present (user-edited
  bundled rows keep their values), so seeding runs on every boot
  instead of the old empty-table guard. Feed templates keep their
  count-based guard for now — they are outside the §4.1 criterion.

### D050: Bundled records are read-only; immutability is a 409

Implements the read-only-preset half of D045 (spec §4.1, §4.4, §13):

- `strain` gains the explicit `is_builtin` flag the data model
  reserves for it (V7), backfilled from provenance — only the seeder
  ever wrote `source_url`, so every row carrying one is bundled. The
  core `Strain` field parses as `false` when absent
  (`#[serde(default)]`) so pre-flag fixtures keep deserializing.
- PUT and DELETE on a bundled equipment entry or strain return **409
  Conflict** with a clone-redirecting message. 409 over 403: the
  resource exists and the client may mutate it — just not this
  representation of it; the fix is a new resource (the clone), which
  is what a conflict response points at.
- `POST /api/equipment/{id}/clone` joins the strain clone: any entry
  (bundled or user) copies into a user entry named `X (copy)`. For
  user entries this is also the "two identical lights are two entries"
  flow from §4.1.
- UI: bundled cards show the existing badge and only the clone action
  (edit/delete hidden); user cards keep edit/delete and gain clone.

### D049: Catalogue trim to four categories, with a legacy `misc` bucket

Implements D045's catalogue decision (spec §4.1): the equipment
catalogue covers durable hardware only — `light`, `tent_room`, `fan`
(incl. carbon filters) and `sensor`. `EquipmentCategory::catalogue()`
is exactly those four and is what the API validates and the UI offers.

- **The legacy bucket.** Retiring nine categories from a type that
  round-trips stored rows would make old rows undeserializable.
  Instead the enum keeps a fifth variant, `Misc`, documented as
  legacy-only: V6 parks user rows in retired categories there, they
  keep listing and displaying, but no create/update payload may say
  `misc` (an update must pick a catalogue category — which is also the
  only thing the UI's category select can send). No new row can ever
  become misc.
- **V6 (`V6__catalogue_trim.sql`).** `carbon_filter` → `fan`; the
  sixteen bundled rows whose categories left the catalogue are deleted
  by exact `(name, category)` match — the explicit, documented removal
  spec §4.1 requires (space references cascade); everything else in a
  retired category is parked in `misc` rather than deleted, because
  silently dropping user data is worse than a legacy bucket. The
  bundled seed set shrinks 21 → 5 entries (tent, light, clip fan, two
  carbon filters as fans).
- `EquipmentCategory::all()` became `catalogue()` (four first-class
  categories); row parsing moved to `from_label`, which also parses
  `misc` so parked rows deserialize.

### D048: Migrations run on refinery — auto-discovered files, checksum-verified history

The hand-rolled runner from D001 (an embedded `&[&str]` array plus a
`schema_migrations` table) is replaced by [refinery] 0.10, satisfying
the first-class migration tooling of spec §3 / D045:

- **Why refinery over the leading candidate `rusqlite_migration`:**
  the spec wants the applied history *checksum-verified* with a
  refusal on divergent or out-of-order histories. `rusqlite_migration`
  (2.x) tracks state in SQLite's single `user_version` integer — there
  is no per-migration record to checksum against, so the requirement
  is unimplementable on it without a parallel table that duplicates
  the runner's job. Refinery keeps a `refinery_schema_history` table
  (version, name, applied_on, checksum), verifies every applied row
  against the embedded set on every run (`abort_divergent`, default
  on), and aborts on gaps, unknown versions, or repeated versions
  (`abort_missing`, default on). Each migration applies in its own
  transaction (the default; `set_grouped` stays untouched).
- **File discovery is the workflow:** `embed_migrations!` collects
  `crates/leaflog-backend/migrations/V<n>__<name>.sql` at compile
  time — adding a file is the whole change. Files were renamed from
  `0001_init.sql` to `V1__init.sql` (content untouched, so checksums
  describe every database ever built). `just migrate new <name>`
  creates the next numbered file and pokes the crate root so cargo's
  change detection cannot miss the new file.
- **Adopting pre-refinery databases:** on open, a legacy
  `schema_migrations` table (the D001 format) is adopted once — its
  max version is marked applied via `Target::FakeVersion` (inserting
  the embedded names/checksums without running anything) and the
  legacy table is dropped. A legacy version newer than the embedded
  set refuses to start, and an adoption interrupted between marking
  and dropping completes on the next open.
- **Tooling:** the binary gains `migrate apply` / `migrate status`
  subcommands (boot-time apply stays the normal path); `just migrate
  new/status/apply` wrap them, plus a dev-only `just migrate redo`
  that rebuilds `data/leaflog.db` from scratch. Refinery pairs with
  rusqlite 0.41 (via the D047 bump) and needs Rust ≥ 1.92 — already
  the workspace floor.

Supersedes the runner half of D001 (the `rusqlite` + blocking-pool
choice stands).

[refinery]: https://crates.io/crates/refinery

### Lint-policy exceptions (`#[allow]` register) — refinery macros

| Crate | Items | Lint | Reason |
|---|---|---|---|
| leaflog-backend | `db::embedded` module | `missing_docs` | The refinery `embed_migrations!` macro generates the inner module and `runner()` without doc comments; the wrapper module documents the contract |

### D047: `cargo deny` gate — allowlist, one advisory ignore, and the rusqlite bump it forced

`cargo deny check` (config in `deny.toml`) joins `just ci` per D046.
Choices the first run surfaced:

- **Licence allowlist**: the usual permissive set plus `CC0-1.0`
  (`base16`, via the wasm stack) and `BSL-1.0` (`xxhash-rust`, via
  Leptos). Both are FSF-free; `cargo deny` had flagged them as
  unlisted, not as problematic.
- **One advisory ignore**: RUSTSEC-2024-0436 (`paste` is unmaintained)
  — `paste` is pulled by the Leptos 0.8 internals (`either_of`,
  `tachys`, `reactive_graph`) with no upgrade inside the 0.8 line, and
  the advisory is informational. The ignore carries a reason inline
  and is dropped at the Leptos 0.9 upgrade.
- **`bans.wildcards = "allow"`**: the only wildcard version
  requirements in the graph are the path-only workspace members
  (`leaflog-core`, `mock-ha`), which deliberately carry no version.
  `multiple-versions` stays a warning, matching
  `#![allow(clippy::multiple_crate_versions)]`.
- **rusqlite 0.37 → 0.41**: the gate's first real finding,
  RUSTSEC-2026-0459 — bundled SQLite before 3.54.0 can return
  uninitialized page-cache memory from a crafted db+wal pair
  (CVE-2026-119375). The fix ships in `libsqlite3-sys` 0.39, which
  requires rusqlite 0.41, which declares Rust 1.89; the workspace
  MSRV floor moves 1.88 → 1.92 accordingly (the toolchain itself
  already floats on stable, currently 1.99). Backend code needed no
  changes; the full backend suite passes on 0.41.

### D046: Spec amendment — harvest & yield, cross-grow history, longevity rules, EN+DE, timezone canon

Post-v0.3 roadmap additions recorded in SPEC.md (§1, §2, §4.1, §4.2,
§4.4, §4.9, §4.13–§4.15, §5, §6, §7, §8, §10, §12, §13; M11/M12):

- **Harvest & yield (M11):** structured wet/dry/trimmed weights on the
  Harvest action (per plant, grams internally, HA display units), a
  yield summary with wet→dry ratio, and achieved yield added to the
  strain write-back. Dry/cure stay action-log-driven; no jar
  inventory in v1.
- **Cross-grow history (M11):** predicted-vs-actual phase accuracy,
  yield by strain/substrate/feed template, input-vs-runoff drift and
  alert-frequency views, per-plant views, and a one-tap "start next
  run" that copies by reference. Computed read-only over existing
  tables.
- **Germination-test log (M11):** sown/sprouted per seed batch shown
  as observed evidence beside the §5.4 preset window, never
  overwriting it.
- **Longevity (M12, one part amends M9):** the bundled-data upgrade
  rule — stable IDs, insert-new-only on upgrade, corrections ship as
  new revisions, user clones never touched — folds into M9's
  read-only-preset work. Sensor-snapshot retention pruning (default
  24 months, ≤1/hour after, first/last-of-day and excursion rows
  kept). Entity-drift rebinding with "no data" degradation for
  dependent alerts. Env-alert HA notification routing under the
  existing §4.2 opt-in. A bootstrap-only options schema: in-app
  settings win, options are initial seeds.
- **Localisation EN+DE (M12):** `leptos_i18n` 0.6 (the line targeting
  Leptos 0.8, needs ≥ 0.8.15; re-check at the 0.9 upgrade), language
  follows the browser with a manual override, CI gate on missing
  keys. Decided bilingual over English-only so new UI strings are
  externalised as they are written from M9 onward, not retrofitted.
- **Timezone canon (§2):** UTC stored; HA's timezone (documented
  `GET /api/config`) is canonical for display and grow-day numbering —
  local calendar days, DST-safe — overridable in settings. This
  supersedes any implicit browser-timezone rendering for day math.
- **CI:** `cargo deny` (RustSec advisories + licence allowlist,
  `deny.toml`) joins the `just ci` gate immediately, not gated on a
  milestone.

### D045: Spec amendment — trimmed catalogue, HA sensors, read-only presets, onboarding, seedbank, migration tooling

Product decisions recorded in SPEC.md (§4.1, §4.4, §4.12, §4.13, §5.4,
§13; milestones M9/M10) for the post-v0.3 roadmap. Two of them amend
D043, whose implementation is in flight:

- The equipment catalogue shrinks to durable hardware: lights,
  tents/rooms, fans (incl. carbon filters) and sensors. Consumables and
  instruments leave the catalogue — nutrients already live in feed
  templates, pH/EC/TDS readings in action-log fields. The bundled
  21-item seed set is trimmed to lights/tents/fans accordingly.
- Sensor items are never bundled: each is created from an HA entity
  (temperature + humidity device classes for now), replacing the
  hygrometer-style hardware presets.
- **Bundled equipment and strains become immutable** (preset badge,
  edit/delete rejected, clone-to-edit). This reverses D043's "builtins
  are deletable" and the flag being merely cosmetic; the API must
  enforce it. The §5 "presets are user-editable" rule is clarified to
  cover only the numeric grow-science presets.
- New features added to the spec: a first-run onboarding wizard
  (equipment → space → strain → plants) and a seedbank log with §5.4
  viability windows and oldest-first use. First-class migration
  tooling: automatically discovered files, checksum-verified history,
  a `migrate` subcommand and `just migrate` recipes. A maintained
  runner — `rusqlite_migration` preferred — replaces the hand-rolled
  embedded array in `db.rs`.

### D044: Grow spaces — grows live inside a space

`grow_space` (name, optional interior W/D/H in centimetres, notes) sits
above grows: `grow.grow_space_id` is a nullable FK with
`ON DELETE SET NULL` (the `strain_id` pattern), so grows may run
outside any space and deleting a space keeps its grows. Spaces relate
to the equipment catalogue by reference (D043). Sensor bindings stay
per-grow — moving them up to the space is deferred until a real need
shows up. The lamp advisor (`GET /api/grows/{id}/lamp`) now takes the
most powerful light with a known real wattage from the grow's space
(and whether any light is dimmable); the previous hard-coded 330 W /
dimmable default remains the fallback when there is no space, no
light, or no wattage. Space dimensions are stored in cm and displayed
as-is (per-unit-system display is deferred; only lamp heights are
inch-based today, in the advisor output).

### D043: Equipment entries are catalogue templates with bundled defaults

`equipment_item` is the equipment catalogue (the strain-database
pattern), not a per-grow inventory. The seeder stamps fresh seeds with
`is_builtin = 1` plus a `source` provenance note (the feed-template
pattern — per-record URLs are not worth re-researching for product
facts). Rows already present in upgraded databases stay unflagged:
the flag is cosmetic (a "bundled" badge; builtins are deletable, like
bundled strains). API-created items are always user entries, and
updates preserve both flags. Spaces reference entries many-to-many
(`grow_space_equipment`): a shared item (a meter carried between
tents) may sit in more than one space, and two identical lights are
two entries — chosen over per-space instances for simplicity (fewer
entities, fewer forms). The spec's per-grow equipment snapshot
(`grow_equipment`) is deferred until a history-preserving need shows
up.

### Lint-policy exceptions (`#[allow]` register) — grow spaces

| Crate | Items | Lint | Reason |
|---|---|---|---|
| leaflog-backend | `api_env::space_light` | `clippy::cast_possible_truncation`, `clippy::cast_sign_loss` | **Historical, superseded by D075**: the spaces and this handler are gone; the wattage cast now lives in leaflog-core's `env::advisor_watts` (2026-11-17 register) |

### D042: CHANGELOG.md is cocogitto output, regenerated in CI

The committed changelog had drifted from what cocogitto produces. The
v0.1.0 section was missing entirely — cocogitto's `cog bump` only
*prepends* a new section into the existing file, so history before the
file first existed is never backfilled. Release dates showed bump-day
instead of tag-day, and scope styling matched the host's cargo-installed
cog 7.0.0 while devbox/CI pin 6.2.0. Nothing verified the file, so it
could equally have been edited by hand.

Chosen setup, replacing `cog bump`'s prepend as the maintainer:

- **Canonical form**: `cog changelog <root-commit>..<latest-tag>`
  (released tags only, never an Unreleased section, deterministic
  wherever main is) wrapped in cocogitto's own `DEFAULT_HEADER` /
  `DEFAULT_FOOTER` file constants. Verified byte-identical to the
  format `cog bump` writes.
- **`just changelog`** is the only writer of the file (the header/footer
  constants live in exactly this one place). CI installs the same
  pinned `just` 1.59.0 as devbox.lock.
- **The release pipeline rewrites the file wholesale** via that recipe
  and commits it with the version sync, so local bump-time drift (or a
  hand edit) never survives a release. The GitHub Release body uses the
  same `<root>..<tag>` range, which also fixes dispatch re-runs
  picking up an Unreleased section once main moved past the tag.
- **PR guard** (pr-check.yml): when a PR touches CHANGELOG.md it must
  be byte-identical to `just changelog` output; anything else fails.

`cog bump --auto` still creates the local version commit + tag (and its
prepend write is harmless), but the file's content is defined by CI
regeneration, not by the bump. Local bumps should use devbox's pinned
cog (`devbox run -- cog bump --auto`) — the style skew came from host
cog 7.0.0.

## 2026-11-09 — post-M8

### D041: multi-part photo upload is all-or-nothing

The frontend appends every selected photo under one `file` field, and
the upload handler answers with one row per part. Storing per part as
the loop went meant a failure on part k (one corrupt image in a
multi-file selection, a full disk mid-request) left parts 1..k-1
persisted as rows *and* files while the request returned an error —
and a retry (there is no idempotency key) duplicated them. Same
partial-state class the sensors/feeds/seed transactions already fix.
`upload_photo` (api_grows.rs) now runs the file pipeline for **every**
part before any row exists, inserts all rows through one
`unchecked_transaction`, and on any failure removes the files of every
part that reached the disk; `PhotoStore::store` equally removes its
own original when the thumbnail write fails, so a failed `store`
leaves nothing. Verified by `failed_multi_part_upload_stores_nothing`
(tests/grows.rs): 400 → zero rows, zero files, retry stores exactly
one.

### D040: `nosniff` on every response, hash-pinned CSP on the SPA shell

Serving findings (backlog `add-nosniff-and-csp-headers`): no response
carried a single security header. Two changes, both deliberately
scoped:

1. **`X-Content-Type-Options: nosniff`** via an Axum middleware
   (`security_headers` in `app.rs`) layered *outside* the Supervisor
   gate, so every response — API JSON, photo bytes, assets, the SPA
   shell, 403/404/405 paths — carries it. The browser never
   content-sniffs a photo or asset response into an executable type.
2. **`Content-Security-Policy`** on the served `index.html`
   (`ingress::content_security_policy`), bounding the SPA if any XSS
   ever reaches the DOM — worth doing because the app lives on HA's
   origin, where Leaflog XSS is HA XSS (THREAT-MODEL §2.2).

The CSP shape is dictated by what the Trunk-built Leptos SPA actually
loads (all same-origin, per the ingress rules). `script-src 'self'`
for the bundle, plus `'wasm-unsafe-eval'` because Chromium gates wasm
compilation on script-src and Leptos CSR *is* wasm.
`style-src 'self' 'unsafe-inline'` covers the linked stylesheet and
the inline styles Leptos emits. Directives beyond that: `'self'` for
img/connect/form-action, `object-src 'none'`, `base-uri 'self'` (the
injected `<base href>` is a same-origin path). Inline scripts are
allowed by **per-response `'sha256-…'` hashes of the exact script
texts**, never `'unsafe-inline'`. One hash covers the
`window.__LEAFLOG_BASE__` global injected at serve time, computed
where the base is known, from the same builder that renders the
script — `ingress.rs` keeps one source of truth and unit tests pin
the digest. A second hash covers each inline `<script>` found in
the served template. That group exists because Trunk 0.21 emits
its wasm-bindgen bootstrap **inline** into `index.html`, as
`import init … window.wasmBindings = bindings …`. The bootstrap
text names per-build hashed files, so no hash baked into the
policy could survive a
rebuild. The first version of this change hashed only the injected
base script, so the CSP silently blocked the bootstrap and the SPA
never booted (blank panel, `#app` empty, in production and `just dev`
alike). The tests could not see it because the fixture used an
external `<script src>`. Fix: hashes are computed from the actual
template at serve time, staying correct on every rebuild. The test
fixture now reproduces the real Trunk inline-bootstrap shape.
`csp_allows_every_inline_script_in_the_served_html` (tests/ingress.rs)
re-derives the hash rule over every inline script in the served HTML
and demands a matching hash for each. That rule is **exact bytes, no
whitespace normalization**: browsers hash inline scripts
byte-for-byte between the tags. Verified experimentally in Chromium
152 — a padded script whose trimmed hash is listed stays blocked —
and per MDN. A CSP3-style "strip leading/trailing whitespace"
hash silently re-breaks the app; a real-browser click-through is the
only check that catches this class. `'unsafe-inline'` would have
neutralized the
CSP's entire point. `frame-ancestors` is
deliberately omitted: HA's ingress panel frames the app from the HA
page (same origin), and remote-UI proxy setups must not be bricked by
guesswork — revisit with a real ingress click-through if it is ever
wanted. Costs `sha2` + `base64` (RustCrypto/standard, dev-only
compile-time cost). Not an R-item: R2 is Cache-Control, separate.

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
dummy test fixture token in `crates/leaflog-backend/tests/ha.rs`.
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
| leaflog-frontend | `GrowsPage::fetch_list`, `GrowsPage::timed_get` | `clippy::future_not_send` | Same rationale as module `api`: the list fetch races a timeout future and the wasm types are not `Send` |
| leaflog-frontend | `EquipmentPage`, `EquipmentForm`, `StrainsPage`, `StrainForm` | `clippy::too_many_lines` | **Historical**: the pages left with the D075 strip; kept as register history (the declarative-view reason now covers the surviving pages) |
| leaflog-backend | store `photos`, `feeds`, `actions`, `grows` serde writes | `clippy::missing_panics_doc` | Serialising plain data with serde_json cannot fail |
| leaflog-frontend | `OnboardingState` in `onboarding.rs` | `clippy::struct_excessive_bools` | **Historical**: D078 shrank the state to three booleans, under the lint's threshold; kept as register history |

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

### D081: predefined equipment seed data, no personal purchase details

*(Numbered D081 in 2026-11-17, D083's cleanup: the entry first
shipped as D012, a number the earlier M2 storage decision already
held; the original number stays with that decision. D075's supersede
list now points here.)*

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

### D082: offset-less action timestamps read as UTC

*(Numbered D082 in 2026-11-17, D083's cleanup: the entry first
shipped as D013, a number the earlier M2 migration-scope decision
already held; the original number stays with that decision.)*

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
one-line reason at the site. Patterns shared by more than one item are listed
once per crate.

| Crate | Items | Lint | Reason |
|---|---|---|---|
| leaflog-backend | `ingress::IngressPath` | `clippy::module_name_repetitions` | The domain name (`IngressPath`) is the right name; the module is `ingress` |
| leaflog-backend | `app.rs` handlers `health`, `index`, `fallback` and middlewares `supervisor_gate`, `security_headers`, `dev_prefix_strip` | `clippy::needless_pass_by_value` | Axum handlers/middlewares must take extractors (`State`, `HeaderMap`, `Request`) by value |
| leaflog-backend | `api_locale.rs` handler `locale` | `clippy::needless_pass_by_value` | Same Axum handler contract as above (D112) |
| leaflog-backend | `app.rs` handler `health` | `clippy::unused_async` | Axum handlers must be `async fn` even when they never await |
| mock-ha | handlers `root`, `ping`, `states` | `clippy::unused_async` | Same Axum contract as above |
| leaflog-frontend | component `App` | `clippy::must_use_candidate` | The Leptos `#[component]` macro drops a hand-written `#[must_use]`, so the lint fires on the generated function |
| leaflog-frontend | components `Home`, `LocaleChain`, `AppShell` | `clippy::must_use_candidate` | Same `#[component]` macro behaviour as `App` (`Home` predates the register row; `LocaleChain`/`AppShell` arrive with D112) |
| leaflog-frontend | component `ErrorIf` | `clippy::must_use_candidate` | Same `#[component]` macro behaviour as `App` |
| leaflog-frontend | components `EntityIdChip`, `EmptyState`, `SectionHeader`, `UndoBanner` in `ui.rs` | `clippy::must_use_candidate` | Same `#[component]` macro behaviour as `App` (arrive with D120; `UndoBanner` with D126) |
| leaflog-frontend | component `LightCycleEditor` in `light_cycle.rs` | `clippy::too_many_lines` | Declarative view code, not logic; splitting would obscure the form (same pattern as the M2 register row) |
| leaflog-frontend | components `FeedSection` in `feed.rs`, `ScheduleEditorPage`/`EditorGrid` in `schedules.rs` | `clippy::needless_pass_by_value` | Leptos component props are taken by value by the `#[component]` macro contract |
| leaflog-frontend | components `FeedSection` in `feed.rs`, `SchedulesPage`/`ScheduleEditorPage`/`EditorGrid` in `schedules.rs` | `clippy::too_many_lines` | Declarative view code, not logic; splitting would obscure the form (same pattern as `LightCycleEditor`) |
| leaflog-backend | `api_feeds.rs` handlers | `clippy::needless_pass_by_value` | Same Axum handler contract as the `app.rs` row above |
| leaflog-backend | `api_feeds.rs` handlers | `clippy::unused_async` | Same Axum contract as the `app.rs` row above |
| leaflog-core | `feed.rs` module + `TdsScale` conversions | `clippy::cast_possible_truncation`, `clippy::cast_sign_loss` | SI scaling rounds to whole µL/L·mg/L steps before casting; grafted substrate/TDS arithmetic (ADR 0004) |
| leaflog-frontend | `history.rs` fn `summary` | `clippy::cast_precision_loss` | u16/f64 day and gram tallies fit a diary's scales far below float precision; casts are exact in practice (D133) |
| leaflog-frontend | `history.rs` test `yield_totals_come_from_the_first_nonempty_grouping` | `clippy::float_cmp` | Test re-adds the same f64 constants bit-identically; exact comparison is the point |
