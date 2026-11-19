---
version: 1
slug: "scratch-feed-schedules"
primary_target: ".scratch/feed-schedules"
related_targets: []
---

## THESIS

A feeding schedule is a bench sheet: a company chart recreated as a
first-class object the grower owns, then bent onto the plant's real
calendar. The library is the drawer of sheets; the editor is the sheet
itself, ruled like the printed original; the This-week card is today's
column pulled out and answered. The grid absorbs every real-world chart
shape — BioBizz's product×week grid, the open-ended VEG* column, CalMag's
constant dose-range rows — because columns are a week plan, not a
calendar.

## OWN-WORLD

Inherited wholesale from the Cultivation Bench (DESIGN.md): flat tonal
depth, compact daisyUI-recipe components, `.facts` micro-labels, system
stack, two themes always. New surface, borrowed geometry: the dose grid
is chart context, so it takes the Gantt's language — hairline frame,
phase-tinted group rules at low alpha — and the current fitted week is
marked with the Gantt today-marker red. No shadows beyond card rest; no
new hues; phase purple stays inside the grid and the This-week position
chip only because both are chart/plan context.

## STORY

"BioBizz says WK 3 — my plant is 19 days into flowering. What goes in
the tank today?" Grower opens Schedules, the sheet is there with its
source label, opens the grow, and the This-week card answers with fitted
doses while the grid below shows WK 3 outlined in today-red. When the
grow finishes the sheet survives; the next grow inherits it. A grow
without a sheet shows nothing — until a quiet "Add feeding schedule…"
affordance offers the drawer.

## FIRST VIEWPORT

Screen 5 (grow with schedule): the This-week card in the first viewport
answering "what today" — position, due doses, flush countdown — with the
fitted grid's current column visibly marked.

## FORM

Static HTML/CSS/JS in .scratch/feed-schedules/, no build step, system
fonts, English copy, semantic table for the grid, one file per screen
state, injected prototype-index chip labelled as prototype chrome.
Mock data verbatim from BioBizz Nutrient Schedule 2026 EN [UK] (ALL·MIX
page + LIGHT·MIX/COCO·MIX page) and the UP Nutrients CalMag UP dosing
card (foliar 4 ml/L; irrigation 0.25–2 ml/L, any stage).

## FINISH

Five screens read as one app. The grid absorbs all three chart shapes.
Hide/affordance reads intentional, not forgotten. Both themes pass. The
reviewer can point at any settled grill decision and find it on screen.
