// esbuild driver for the ECharts chart bundle (D075). Bundles
// entry.mjs minified as an iife under the `echarts` global name —
// the bundle assigns the echarts/core API to window.echarts at load
// — and writes the esbuild metafile next to it for the composition
// checks in verify.mjs.
//
// No source map: the bundle is a versioned artifact the wasm app
// loads wholesale, and a map would double the dist weight for a
// file nothing debugs line-by-line.

import { build } from "esbuild";
import { writeFileSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const chartsDir = fileURLToPath(new URL(".", import.meta.url));

const result = await build({
  absWorkingDir: chartsDir,
  entryPoints: ["entry.mjs"],
  outfile: "echarts.js",
  metafile: true,
  bundle: true,
  minify: true,
  format: "iife",
  globalName: "echarts",
  target: ["es2020"],
  sourcemap: false,
  logLevel: "info",
});

writeFileSync(path.join(chartsDir, "metafile.json"), JSON.stringify(result.metafile, null, 2));

const bytes = result.metafile.outputs["echarts.js"].bytes;
console.log(`echarts.js: ${bytes} bytes (${(bytes / 1024).toFixed(0)} KiB)`);
