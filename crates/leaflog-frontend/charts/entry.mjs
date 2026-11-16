// Tree-shaken ECharts entry (D075). Imports only the chart types and
// components the app renders with, forces the SVG renderer, and
// registers them at load time. esbuild bundles this file as an iife
// under the `echarts` global name, so `window.echarts` carries the
// echarts/core API (init, use, …) and the wasm app drives it from
// Rust — no application JavaScript lives here.
//
// The imports go to the per-feature install modules instead of the
// echarts/charts and echarts/components barrels. The package marks
// every lib/chart/*.js and lib/component/*.js file as side-effect-ful
// and esbuild does not apply that glob-array form of `sideEffects`,
// so a barrel import would drag in the whole library. Deep imports
// keep the bundle at roughly half the full-library size.
//
// Extending the chart set (later chart tickets) means extending this
// import/use list and rebuilding via `just build-echarts`;
// charts/verify.mjs asserts the composition stays tree-shaken.

import * as echarts from "echarts/core";
import { install as LineChart } from "echarts/lib/chart/line/install.js";
import { install as ScatterChart } from "echarts/lib/chart/scatter/install.js";
import { install as GridComponent } from "echarts/lib/component/grid/install.js";
import { install as TooltipComponent } from "echarts/lib/component/tooltip/install.js";
import { install as SVGRenderer } from "echarts/lib/renderer/installSVGRenderer.js";

echarts.use([LineChart, ScatterChart, GridComponent, TooltipComponent, SVGRenderer]);

export * from "echarts/core";
