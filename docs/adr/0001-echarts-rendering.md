# ADR 0001: ECharts (SVG renderer) as the chart engine

Status: accepted
Date: 2026-11-16
Decided in: ticket 01 of the Light Cycle + ECharts spec (git-bug b251a31)
Related: D077 (build tooling entry in docs/DECISIONS.md)

## Context

The grow overview's history charts are hand-rolled SVG drawn from Rust
Leptos components. They have no axes, no tick labels, no gridlines,
and no hover feedback — only two corner numbers. The Light Cycle spec
adds stepped day/night Target Bands, hover tooltips, labelled axes,
and preserved accessibility labels. Hand-rolled SVG would turn each of
those into more bespoke geometry code per chart, with each interaction
rewritten by hand.

Constraints that bound the choice:

- Local-first: the app must load everything from its own ingress
  origin. No CDN references, anywhere, ever.
- Rust-only app code: the domain logic (bands, windows, units,
  phases) lives in `leaflog-core` and the backend. Whatever draws the
  charts must not grow an application JavaScript layer.
- No Node in the release image: build-time tooling is fine (the
  Tailwind standalone CLI set that precedent), but nothing that ships
  may need Node at runtime.
- Ingress-correct serving: relative URLs only; the backend injects
  `<base href>` at serve time.
- The renderer must follow the app theme and stay crisp in dark mode,
  and keep text labels reachable for screen readers.

## Decision

Charts render with [ECharts](https://echarts.apache.org) 6.1.0, forced
to its SVG renderer. The bundle is produced at build time:

- A small npm project at `crates/leaflog-frontend/charts/` pins exact
  versions (`echarts` 6.1.0, `esbuild` 0.28.2) with a committed
  lockfile. `just build-echarts` runs `npm ci` and esbuild.
- `charts/entry.mjs` imports only the line and scatter chart types,
  the grid and tooltip components, and the SVG renderer, registering
  them through `echarts.use`. esbuild bundles it minified as an iife
  under the `echarts` global name, so `window.echarts` exposes the
  `echarts/core` API.
- `index.html` loads `echarts.js` with a relative `src` before the
  wasm bootstrap, so the global exists before any chart code runs.
  Trunk copies the bundle into dist via `copy-file`, and the backend
  embeds dist like the rest of the frontend assets.
- The imports bypass the `echarts/charts` and `echarts/components`
  barrels and use the per-feature install modules. The package marks
  `lib/chart/*.js` as side-effect-ful and esbuild does not apply that
  glob-array form of `sideEffects`; barrel imports drag the whole
  library into the module graph.
- `charts/verify.mjs` checks the result at the build-artifact seam:
  the esbuild metafile's output composition (line + scatter only, no
  canvas renderer, no stray chart types), size bounds, no CDN host,
  no Node constructs, and the index.html/dist wiring. `just ci`, the
  e2e recipe, and the Docker build all run it.
- ECharts draws; Rust decides. The wasm app builds each chart's
  option payload from `GrowHistory` data with
  `js_sys`/`serde-wasm-bindgen` (chart tickets 02-04). No application
  JavaScript exists or will be hand-written — the only JS in the repo
  is this build pipeline and the e2e suite.

## Alternatives considered

- Keep hand-rolled SVG. Rejected: the spec's axes, tooltips, stepped
  bands, and accessibility work would all be per-chart geometry code
  in Leptos components, re-tested per chart, with no interaction
  story to build on.
- D3. Rejected: a low-level toolkit. It would make Leaflog write and
  maintain its own chart layer — the cost this ADR exists to remove.
- Chart.js. Rejected: canvas-first; SVG support is weak, and the
  DOM-tree renderer is what keeps labels accessible and the theme
  simple to follow.
- uPlot. Rejected: canvas-only, and it needs a wrapper for axes and
  tooltips that recreates much of what ECharts ships.
- ECharts with the canvas renderer. Rejected for now: SVG keeps the
  drawing in DOM nodes that match the theme and screen readers; the
  canvas painter is excluded at bundle level and the checks fail the
  build if it sneaks back.

## Consequences

- Charts gain labelled value axes, time tick labels, gridlines, and a
  hover tooltip as option JSON instead of SVG geometry code. Later
  chart tickets extend the import list, rebuild, and the checks
  assert the composition stays tree-shaken.
- The frontend dist grows by about 517 KiB minified (echarts.js).
  The wasm bundle is about 3.0 MB, so the total download grows by
  roughly a sixth. No further per-chart cost is expected.
- Developers and CI need npm + node at build time. The release image
  stays Node-free: the Dockerfile's frontend-builder stage installs
  Debian's node 18 for the bundle step, and the app stage copies only
  the backend binary with the embedded dist.
- Upgrades are a pinned-version bump plus `npm install` to refresh
  the lockfile; the verification asserts the bundle composition after
  the bump, so a regression fails `just ci` instead of shipping.
- Zoom, pan, and brush interactions stay out of scope; their
  components are not in the bundle, so adding them later means an
  import-list change plus a rebuild, not a redesign.
