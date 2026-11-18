---
name: Leaflog
description: Self-hosted cannabis grow diary, native to Home Assistant
colors:
  # Light theme "leaflog" (default) — values are the incumbent palette
  paper: "#ffffff"
  greenhouse-paper: "#f4f7f2"
  hairline: "#dbe5da"
  ink: "#1d2b1f"
  cultivar-green: "#2e7d4f"
  sage-grey: "#4c5f4e"
  fresh-mint: "#6fcf97"
  sky-loop: "#5b8def"
  vigor-green: "#4caf50"
  amber-caution: "#ff9800"
  rust-alert: "#b3392f"
  # Dark theme "leaflogdark" (prefers-color-scheme)
  loam-surface: "#182a17"
  deep-loam: "#10200f"
  canopy-line: "#2c422a"
  morning-mist: "#e7f0e4"
  night-canopy: "#6fcf97"
  sage-mist: "#a9bda6"
  dusk-loop: "#7ca9f7"
  leaf-warn: "#66bb6a"
  amber-soft: "#ffb74d"
  dawn-alert: "#ef5350"
  # Shared chart signal
  marker-red: "#e53935"
typography:
  display:
    fontFamily: "system-ui, -apple-system, Segoe UI, sans-serif"
    fontSize: "1.35rem"
    fontWeight: 700
    lineHeight: 1.55
  headline:
    fontFamily: "system-ui, -apple-system, Segoe UI, sans-serif"
    fontSize: "1.25rem"
    fontWeight: 600
    lineHeight: 1.55
  title:
    fontFamily: "system-ui, -apple-system, Segoe UI, sans-serif"
    fontSize: "1.05rem"
    fontWeight: 600
    lineHeight: 1.55
  body:
    fontFamily: "system-ui, -apple-system, Segoe UI, sans-serif"
    fontSize: "1rem"
    fontWeight: 400
    lineHeight: 1.55
  label:
    fontFamily: "system-ui, -apple-system, Segoe UI, sans-serif"
    fontSize: "0.75rem"
    fontWeight: 400
    letterSpacing: "0.06em"
rounded:
  field: "0.5rem"
  selector: "0.5rem"
  box: "0.875rem"
  chart-bar: "6px"
  focus: "4px"
spacing:
  xs: "0.25rem"
  sm: "0.5rem"
  md: "1rem"
  card-x: "1.25rem"
  card-y: "1rem"
components:
  card:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.ink}"
    rounded: "{rounded.box}"
    padding: "16px 20px"
  tile:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.ink}"
    rounded: "{rounded.box}"
    padding: "12px 16px"
  button-primary:
    backgroundColor: "{colors.cultivar-green}"
    textColor: "#ffffff"
    rounded: "{rounded.field}"
    height: "2rem"
    padding: "4px 12px"
  button-outline:
    backgroundColor: "transparent"
    textColor: "{colors.cultivar-green}"
    rounded: "{rounded.field}"
    height: "2rem"
    padding: "4px 12px"
  input:
    backgroundColor: "{colors.paper}"
    textColor: "{colors.ink}"
    rounded: "{rounded.field}"
    height: "2rem"
  badge-ghost:
    backgroundColor: "transparent"
    textColor: "{colors.sage-grey}"
    rounded: "{rounded.field}"
---

# Design System: Leaflog

## Overview

**Creative North Star: "The Cultivation Bench"**

Leaflog looks like a tidy workbench in a professional growing operation: a
flat, matte surface where evidence is laid out — dates, ranges, sensor
readings, phase windows — and every tool sits within reach. Nothing glows,
nothing floats, nothing performs. The system is quietly utilitarian:
compact controls, plain labels, small unadorned cards that stack like
index cards on the bench. Precision is the aesthetic; the personality
lives in exact values, uppercase micro-labels and the phase-colour
vocabulary of the charts.

The register is professional horticulture, never stoner cliché, neon-leaf
branding or a dark "grow-op" mood. It is also not an ops dashboard: density
stays low, cards breathe, and one concern occupies one card. Two complete
themes — `leaflog` (light, default) and `leaflogdark` (dark, via
`prefers-color-scheme`) — carry the same layout and the same component
recipes; both must always work.

**Key Characteristics:**

- Flat tonal depth: card surface (`base-100`) on page ground (`base-200`),
  separated by `base-300` hairlines; `shadow-sm` is the only shadow.
- Compact daisyUI component vocabulary (`btn-sm`, `input-sm`, `badge-sm`,
  `text-sm`) — sized for one-handed phone use.
- Two hand-tuned daisyUI themes carrying an incumbent botanical palette.
- Uppercase letter-spaced micro-labels for data facts (the `.facts dt`
  pattern).
- Phase accents reserved for charts: germination `#8bc34a` → seedling
  `#66bb6a` → vegetative `#4caf50` → flower `#ab47bc` → drying `#ff9800`
  → curing `#a1887f`, fallback `#90a4ae`.
- System font stack; no downloaded or CDN fonts (local-first).

## Colors

An incumbent calm-botanical palette in two mirrored themes; desaturated
greens do the structural work, status hues and chart phase accents carry
the colour. **Owner verdict on record: these hex values are tolerated, not
loved — treat them as incumbent fact, not endorsement. A palette revision
is a deliberate redesign decision (new-work), not drift.**

### Primary
- **Cultivar Green** (`#2e7d4f`, light `--color-primary`): primary
  buttons, links, active states, focus outlines. Swaps with the accent in
  the dark theme — **Night Canopy** (`#6fcf97`) takes the primary role
  there.
- **Fresh Mint** (`#6fcf97`): light-theme accent (dark-theme primary is
  this same hue; in the dark theme the accent role falls back to
  **Cultivar Green** `#2e7d4f`).

### Secondary
- **Sage Grey** (`#4c5f4e`, also `--color-secondary` and `--color-neutral`
  in light): secondary/ghost surfaces, muted structure. Dark counterpart
  **Sage Mist** (`#a9bda6`).

### Tertiary
- **Fresh Mint** (above) doubles as the tertiary voice where a second
  accent is needed; no separate tertiary hue exists. Do not invent one.

### Neutral
- **Paper White** (`#ffffff`, `base-100` light): every card and chart
  ground. Dark: **Loam Surface** (`#182a17`).
- **Greenhouse Paper** (`#f4f7f2`, `base-200` light): the page background
  — the tonal step that makes white cards read as raised without shadows.
  Dark: **Deep Loam** (`#10200f`).
- **Hairline** (`#dbe5da`, `base-300` light): 1px borders, chart frames,
  code/pre borders. Dark: **Canopy Line** (`#2c422a`).
- **Ink** (`#1d2b1f`, `base-content` light): all body text. Dark:
  **Morning Mist** (`#e7f0e4`).
- Muted text is Ink/Sage at reduced opacity (`text-base-content/60` /
  `/70`), never a separate grey token.

### Status
- Info **Sky Loop** `#5b8def` / dark **Dusk Loop** `#7ca9f7`; success
  **Vigor Green** `#4caf50` / dark **Leaf Warn** `#66bb6a`; warning
  **Amber Caution** `#ff9800` / dark **Amber Soft** `#ffb74d`; error
  **Rust Alert** `#b3392f` / dark **Dawn Alert** `#ef5350`. Used via
  daisyUI `alert-*`, `btn-error`, badge roles — semantic roles, not raw
  hex in markup.

### Named Rules
**The Two-Theme Rule.** Every colour decision lands in both `leaflog` and
`leaflogdark` at the same moment. A choice that only works in one theme is
a bug, not a style.

**The Phase Accent Rule.** Strong hue is chart data, never chrome. The six
phase colours and the today-marker **Marker Red** (`#e53935`) appear in
Gantt/feed/chart contexts only; UI chrome stays green-and-neutral.

## Typography

**Display / Body Font:** system stack — `system-ui, -apple-system,
"Segoe UI", sans-serif` — no webfonts, no CDN font loads (local-first;
the platform's UI voice is the brand voice).

**Character:** native, plain-spoken, utilitarian. Hierarchy is built from
size and weight steps on one family; no decorative face anywhere.

### Hierarchy
- **Display** (700, 1.35rem): the brand word "Leaflog" in the header and
  page-level `h1` — the largest type in the app, deliberately modest.
- **Headline** (600, 1.25rem): page `h2` section titles.
- **Title** (600, 1.05rem): `h3` card and group titles; `h4` (600,
  0.95rem) for nested subdivisions.
- **Body** (400, 1rem, line-height 1.55): default text; `text-sm`
  (0.875rem) is the standard density for data rows, table cells and tile
  content.
- **Label** (400, 0.75rem, letter-spacing 0.06em, uppercase): fact-grid
  captions (`.facts dt`) and Gantt phase labels (0.04em), always in
  base-content at 65% opacity. Chart's today-label: 0.65rem/700.

### Named Rules
**The System Stack Rule.** No downloaded fonts, ever. If a future design
wants a distinct face, that is a redesign decision with a local,
self-hosted file — never a CDN.

## Layout

Single-column phone-first flow: full-width cards stack vertically with
`my-4` rhythm; on `lg` screens two-column splits (`lg:grid-cols-2`) and
auto-fill tile grids take over. The recurring grid is
`grid-cols-[repeat(auto-fill,minmax(14rem,1fr))]` for card/tile mosaics;
inside cards, the `.facts` definition grid runs
`repeat(auto-fill,minmax(10rem,1fr))` with `0.5rem 1rem` gaps. Spacing
vocabulary is the Tailwind scale with `gap-2` (0.5rem) as the default
gap, `px-5 py-4` (20px/16px) as the card recipe and `px-4 py-3` for
smaller tiles. The header is a wrapping flex bar: brand left, nav links
right (`ml-auto`), links wrap on narrow phones; the active page is
marked by a 2px underline offset 4px (`aria-current="page"`), never by
colour alone. Density is compact by default (`btn-sm`, `input-sm`,
`text-sm`); nothing is taller than it needs to be.

## Elevation & Depth

Flat by default; depth is tonal, not cast. `--depth: 0` in both themes.
A card reads as raised because Paper White sits on Greenhouse Paper
bounded by a Hairline — the same trick the dark theme plays with Loam
Surface on Deep Loam. The single shadow is daisyUI's `shadow-sm` on the
resting card recipe; there are no hover elevations, no overlays, no
glows. Charts are flat frames: 1px Hairline border, box radius, card
background. The Gantt today-marker is a 2px Marker Red line drawn over
the track — depth cues come from geometry (gutter, label column, today
overlay), not from shadow.

### Shadow Vocabulary
- **Card rest** (`shadow-sm` ≈ `0 1px 2px rgba(0,0,0,0.05)`): the one
  permitted shadow, part of the card recipe, both themes, no hover
  variant.

### Named Rules
**The Hairline Rule.** Separation is `base-300` borders and tonal steps.
If a layout needs more shadow to read, the tonal structure is wrong — fix
the steps, don't add depth.

## Shapes

Rounded but disciplined: daisyUI radius tokens are `--radius-selector`
and `--radius-field` at 0.5rem (buttons, inputs, badges, chips) and
`--radius-box` at 0.875rem (cards, charts, `pre` blocks) — corners are
noticeably rounder on containers than on controls. Chart internals are
tighter: Gantt bars/ranges at 6px, feed bars at 2px, feed containers
inheriting the box radius. Borders are always 1px (`--border: 1px`).
Focus is a 3px primary-colour outline offset 2px with a 4px corner —
high-visibility, keyboard-first. Inline code gets a small 6px radius
chip treatment (base-200 fill, hairline border).

## Components

The component vocabulary is daisyUI 5 class recipes over the two Leaflog
themes; all styling flows through semantic roles (`bg-base-100`,
`btn-primary`), never raw hex in markup.

### Buttons
- **Shape:** field radius (0.5rem), compact `btn-sm` (≈2rem tall) as the
  default size across the app.
- **Primary:** `btn btn-primary` — Cultivar Green fill, white text
  (light); Night Canopy fill, near-black text (dark).
- **Variants in use:** `btn-outline` (primary border/text on card
  ground), `btn-ghost` (text-only), `btn-error` for destructive actions.
- **Hover / Focus:** daisyUI state tints; keyboard focus is the global
  3px primary outline. Motion is a ≤200ms functional fade, never
  decoration.

### Chips / Badges
- **Style:** `badge badge-ghost` — transparent ground, hairline border,
  Sage-toned text; `badge-sm` in data rows. Used for tags, appearance
  traits and sensor-role chips.

### Cards / Containers
- **Corner Style:** box radius (0.875rem).
- **Background:** `base-100` on the `base-200` page, `shadow-sm`.
- **Border:** none on cards (the tonal step does the work); 1px
  hairlines on charts and code blocks.
- **Internal Padding:** `px-5 py-4` standard, `px-4 py-3` for dashboard
  tiles.
- **Content pattern:** `h3` title, then `.facts` grid of uppercase
  micro-labelled values, then compact action row.

### Inputs / Fields
- **Style:** field radius (0.5rem), compact `input-sm`, card-ground fill,
  1px base-300 border (daisyUI default border treatment).
- **Focus:** global primary outline; no glow, no border-colour shift
  beyond daisyUI defaults.

### Navigation
- **Style:** wrapping flex header, no background of its own (page ground
  shows through), text-sm links in primary colour, right-aligned.
- **Active:** 2px underline, 4px offset, via `aria-current="page"` —
  never colour-only. No hover background; links brighten slightly
  (`brightness(1.1)`).

### Fact Grid (`.facts`) — signature pattern
The app's characteristic data presentation: a responsive definition grid
where each value carries a 0.75rem uppercase letter-spaced caption at
65% opacity above a normal-weight value. Every card that reports state
(grow facts, environment tiles, sensor bindings) uses it. New data
displays should reach for it before inventing a layout.

### Gantt Forecast — signature chart
Phase rows with a 6rem right-aligned uppercase label column, range
bands (phase colour at 18% fill, 35% border) and likely-bars (55%
fill), geometry computed in Rust and set inline; a 2px Marker Red
today-line overlays the track. Flat, framed, no gridlines beyond the
frame itself.

## Do's and Don'ts

### Do:
- **Do** style everything through daisyUI semantic roles and theme
  variables (`bg-base-100`, `text-base-content/60`) — both themes come
  free; raw hex belongs only in chart geometry (phase colours, marker
  red).
- **Do** check every visual choice in both themes before calling it
  done (The Two-Theme Rule).
- **Do** use the `.facts` grid for state display and `btn-sm` /
  `text-sm` density for data-heavy surfaces.
- **Do** keep phase colours for phase data; reuse the exact `phase_color`
  values when a new chart needs phase meaning.
- **Do** mark the active nav state with the `aria-current` underline
  pattern and keep focus outlines at 3px/offset 2px.

### Don't:
- **Don't** add shadows beyond the card-rest `shadow-sm`; if depth
  doesn't read, fix the tonal steps (The Hairline Rule).
- **Don't** load webfonts or any CDN asset at runtime — local-first is
  a product constraint, not a preference.
- **Don't** let chrome borrow chart hues: no phase purple outside
  charts, no Marker Red outside the today-marker and destructive
  feedback.
- **Don't** treat the incumbent palette as endorsed — it is documented
  fact the owner has not fallen in love with; propose a deliberate
  redesign instead of drifting hue-by-hue.
- **Don't** build dark-only or light-only styling; a `prefers-color-scheme`
  media query is a smell — the daisyUI `--prefersdark` theme switch is
  the mechanism.
