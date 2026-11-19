# ADR 0004: Feeding schedules are product-dose grids, not EC plans

Status: accepted
Date: 2026-11-19
Decided in: Feeding-schedules prototype review (grill session, 2026-11-19)
Related: D128 (grid model and wire format), D129 (grow-side assignment integrity), ADR 0001 (charts stay SVG), GLOSSARY (Feeding schedule, Fitted schedule)

## Context

The shipped feed system modelled feeding as an EC/pH ramp: three builtin
"Generic soil/coco/hydro feed" templates stored per-phase EC targets that
were time-warped onto a grow's actual timeline. Real nutrient producers
do not publish EC ramps. Their dosing charts are product grids —
BioBizz's chart lists products (Bio·Grow, Bio·Bloom, Top·Max …) as rows
with dose ranges per stage column (propagation, veg, flowering week
1…8, a plain-water tail), and single-product supplements publish one
range for all stages. EC and pH do not appear on these charts at all,
and every company's flowering length differs, while strains in a real
grow flower for whatever length the plant chooses. A stretched generic
ramp can answer "what EC today" but not "what do I feed this week, and
how much of it" — the question the grow diary actually needs answered.

## Decision

Feeding becomes a **product-dose grid**; EC/pH targets leave the model
entirely:

- **Feeding schedule** (the reusable artefact): products as free-text
  rows owned by the schedule — name, unit (`ml_per_l` | `g_per_l`),
  free-text group (Rooting, Feeds, Boosters …), note. No product
  catalogue; schedules own their rows. Dose cells are min–max ranges
  (`Dose { min, max }` in SI micro-steps: µL/L, mg/L), stored per
  product as `ProductDoses { propagation, veg, flower: week-indexed,
  water }`. Display renders locale decimals and en-dash ranges
  ("0,25–2"); input accepts comma decimals, en/em dashes and inverted
  ranges.
- **Columns**: PROPAGATION (1 dose) · VEG* (1 dose, repeated for any
  veg length) · FLOWERING WK 1…N (user-picked N, clamped 1–24) · WATER
  tail (1 dose, pinned to the substrate flush profile, never dropped).
- **Medium ignored**: schedules are not bound to a substrate; the
  assignment select offers every schedule regardless of the grow's
  medium. Substrate profiles stay in the model for flush-day counts and
  VPD coaching — they no longer generate feed templates.
- **Fit**: week-granular piecewise time-warp onto the grow's actual
  timeline (seedling/veg/flower lengths from the phase forecast;
  `fit_schedule`). The result is the grow's **Fitted schedule**:
  per-day rows with phase, week number and that day's doses, frozen in
  `feed_plan`/`feed_plan_day` per grow. Finished grows keep their fitted
  plan; the schedule itself lives on in the library for the next run.
- **Refit hooks**: assigning a schedule on grow create/update fits
  immediately; timeline changes (phase pins, flip) refit through the
  same path; editing a schedule refits every grow that uses it
  (`/api/schedules/{id}/grows` + per-grow refit). "This week" and the
  fitted grid render from plan days alone, so a grow whose schedule was
  later deleted still shows its frozen plan.
- **Migration kills, no conversion**: V19 drops the builtin EC
  templates, their rows and all fitted EC plan days; `grow.feed_template_id`
  becomes `feed_schedule_id`. No EC→dose conversion is attempted — a
  converted guess would look authoritative while being fiction. The
  library ships empty; users enter (or duplicate) real charts.
- **Storage**: schedules, products and plan days live in SQLite with
  doses as JSON text columns (the V2 JSON-detail precedent); the
  grow→schedule reference is integrity-checked in Rust, not by a
  database foreign key (D129).

## Consequences

- The frontend feed section renders the grow's plan (this-week card +
  fitted grid) and hides itself with a quiet "No feeding schedule."
  row when none is assigned — no template picker mid-grow.
- The chart bundle gains no new dependency; the fitted grid is a plain
  table and the library/editor are forms (ADR 0001 untouched).
- EC/pH targets remain in substrate coaching (VPD, runoff) where they
  belong; feed-plan EC history is gone — yields by schedule aggregate
  by schedule name instead (D128).
- Seed data keeps only substrate profiles; the feed presets discipline
  (source labels per record) now applies to user-entered charts via the
  free-text source field.
