// Bundle verification for the ECharts build pipeline (D077).
//
// Checks the built bundle at the build-artifact seam: the tree-shaken
// module composition (metafile), local-only loading (no CDN hosts, no
// Node constructs), and the index.html wiring that Trunk copies into
// dist. Run standalone after `just build-echarts`, or pass a dist
// directory (built by trunk) to also check the wired result:
//
//   node charts/verify.mjs [distDir]

import { readFileSync, existsSync, statSync } from "node:fs";
import { createHash } from "node:crypto";
import path from "node:path";
import { fileURLToPath } from "node:url";
import process from "node:process";

const chartsDir = fileURLToPath(new URL(".", import.meta.url));
const bundlePath = path.join(chartsDir, "echarts.js");
const metafilePath = path.join(chartsDir, "metafile.json");
const indexPath = path.join(chartsDir, "..", "index.html");

// Build-time-only Node marker: must never survive into the bundle.
const nodeMarkers = ["require(", "node:", "import("];

// Public CDNs the app must never reference (local-first). The Apache
// licence URL in the bundle's trailing legal comment is legitimate.
const cdnHosts = [
  "unpkg.com",
  "cdn.jsdelivr.net",
  "cdnjs.cloudflare.com",
  "ajax.googleapis.com",
  "bootcdn.net",
  "staticfile.org",
];

// Size guard for the tree-shaken bundle (line + scatter + grid +
// tooltip + SVG renderer), measured at 517 KiB. Floor: the bundle
// keeps the chart machinery, not a stub. Ceiling: a silent regression
// to the full library dump (~1.1 MB minified) fails instead of
// shipping.
const minBundleBytes = 400 * 1024;
const maxBundleBytes = 900 * 1024;

// The SVG painter pulls three zrender/lib/canvas files as shared draw
// utilities (dash patterns, text/graphic helpers). Any other canvas
// file means the canvas painter sneaked back in.
const allowedZrenderCanvasFiles = ["helper.js", "dashStyle.js", "graphic.js"];

// Chart directories allowed to contribute to the output. `custom` is
// registered by echarts core itself (customSeriesRegister), `helper`
// holds line/scatter shared helpers; neither can be tree-shaken away.
const allowedChartDirs = new Set(["line", "scatter", "custom", "helper"]);

const failures = [];
function check(name, ok, detail = "") {
  if (ok) {
    console.log(`ok   ${name}`);
  } else {
    failures.push(name);
    console.error(`FAIL ${name}${detail ? ` — ${detail}` : ""}`);
  }
}

function verifyBundle() {
  const exists = existsSync(bundlePath);
  check("bundle exists", exists, "run just build-echarts first");
  if (!exists) return;

  const bytes = statSync(bundlePath).size;
  const text = readFileSync(bundlePath, "utf8");

  check(
    "bundle installs the window.echarts global",
    text.startsWith("var echarts"),
    "expected esbuild's iife globalName prologue",
  );
  check(
    `bundle size within ${minBundleBytes}..${maxBundleBytes} bytes`,
    bytes >= minBundleBytes && bytes <= maxBundleBytes,
    `got ${bytes} bytes`,
  );
  for (const marker of nodeMarkers) {
    check(
      `bundle free of Node construct ${JSON.stringify(marker)}`,
      !text.includes(marker),
    );
  }
  const cdnHit = cdnHosts.find((host) => text.includes(host));
  check("bundle references no CDN host", cdnHit === undefined, cdnHit ?? "");
}

function verifyComposition() {
  const exists = existsSync(metafilePath);
  check("esbuild metafile exists", exists, "run just build-echarts first");
  if (!exists) return;

  const metafile = JSON.parse(readFileSync(metafilePath, "utf8"));
  const output = metafile.outputs["echarts.js"];
  if (output === undefined) {
    check("esbuild metafile names the echarts.js output", false);
    return;
  }
  // Composition must be judged on the OUTPUT graph: files whose code
  // actually landed in the bundle (bytesInOutput > 0), never on the
  // input graph, which lists every file esbuild merely parsed.
  const contributing = Object.entries(output.inputs)
    .filter(([, info]) => info.bytesInOutput > 0)
    .map(([p]) => p);

  const chartDirs = new Set(
    contributing
      .filter((p) => p.includes("/echarts/lib/chart/"))
      .map((p) => p.split("/echarts/lib/chart/")[1].split("/")[0]),
  );
  const unexpectedCharts = [...chartDirs].filter((c) => !allowedChartDirs.has(c));
  const missingCharts = ["line", "scatter"].filter((c) => !chartDirs.has(c));
  check(
    "output tree-shaken to the line and scatter chart types",
    unexpectedCharts.length === 0 && missingCharts.length === 0,
    `included: [${[...chartDirs].sort().join(", ")}]`,
  );

  const contributingComponent = (dir) =>
    contributing.some((p) => p.includes(`/echarts/lib/component/${dir}/`));
  check("grid component included", contributingComponent("grid"));
  check("tooltip component included", contributingComponent("tooltip"));

  const contributingCanvas = contributing
    .filter((p) => p.includes("/zrender/lib/canvas/"))
    .map((p) => p.split("/zrender/lib/canvas/")[1]);
  const strayCanvas = contributingCanvas.filter(
    (f) => !allowedZrenderCanvasFiles.includes(f),
  );
  check(
    "zrender svg renderer included",
    contributing.some((p) => p.includes("/zrender/lib/svg/")),
    "SVG rendering is the only supported renderer (local-first, theme-following)",
  );
  check(
    "zrender canvas renderer excluded",
    strayCanvas.length === 0,
    `unexpected canvas files: [${strayCanvas.join(", ")}]`,
  );
  check(
    "echarts canvas renderer excluded",
    !contributing.some((p) => p.includes("installCanvasRenderer")),
    "the bundle must keep only installSVGRenderer",
  );
}

function verifyWiring() {
  const exists = existsSync(indexPath);
  check("index.html exists", exists);
  if (!exists) return;

  const index = readFileSync(indexPath, "utf8");
  check(
    "index.html copies the bundle through Trunk (copy-file)",
    index.includes('rel="copy-file"') && index.includes("charts/echarts.js"),
  );
  check(
    'index.html loads the bundle locally (<script src="echarts.js">)',
    index.includes('<script src="echarts.js"></script>'),
    "the tag must stay relative: the backend injects <base href> at serve time",
  );
}

function verifyDist(distDir) {
  const distBundle = path.join(distDir, "echarts.js");
  const distIndex = path.join(distDir, "index.html");

  const exists = existsSync(distBundle);
  check("dist contains the bundle", exists);
  if (exists) {
    const sha = (p) => createHash("sha256").update(readFileSync(p)).digest("hex");
    check(
      "dist bundle is byte-identical to the built bundle",
      sha(distBundle) === sha(bundlePath),
    );
  }

  if (existsSync(distIndex)) {
    const index = readFileSync(distIndex, "utf8");
    check(
      'dist index.html loads the bundle locally (<script src="echarts.js">)',
      index.includes('src="echarts.js"'),
    );
    const cdnHit = cdnHosts.find((host) => index.includes(host));
    check("dist index.html references no CDN host", cdnHit === undefined, cdnHit ?? "");
  } else {
    check("dist index.html exists", false, `${distIndex} not found`);
  }
}

verifyBundle();
verifyComposition();
verifyWiring();

const distDir = process.argv[2];
if (distDir !== undefined) verifyDist(path.resolve(distDir));

if (failures.length > 0) {
  console.error(`\n${failures.length} bundle check(s) failed`);
  process.exit(1);
}
console.log("\nall bundle checks passed");
