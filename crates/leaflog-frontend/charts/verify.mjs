// Bundle verification for the ECharts build pipeline (D075).
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
// tooltip + SVG renderer). Floor: the bundle is populated, not an
// empty stub. Ceiling: a silent regression to the full library dump
// (~1.1 MB minified) fails instead of shipping.
const minBundleBytes = 100 * 1024;
const maxBundleBytes = 900 * 1024;

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
  const inputs = Object.keys(metafile.inputs);

  const chartDirs = new Set(
    inputs
      .filter((p) => p.includes("/echarts/lib/chart/"))
      .map((p) => p.split("/echarts/lib/chart/")[1].split("/")[0]),
  );
  const expectedCharts = new Set(["line", "scatter"]);
  const unexpectedCharts = [...chartDirs].filter((c) => !expectedCharts.has(c));
  const missingCharts = [...expectedCharts].filter((c) => !chartDirs.has(c));
  check(
    "tree-shaken to exactly the line and scatter chart types",
    unexpectedCharts.length === 0 && missingCharts.length === 0,
    `included: [${[...chartDirs].join(", ")}]`,
  );

  const hasInput = (needle) => inputs.some((p) => p.includes(needle));
  check("grid component included", hasInput("/echarts/lib/component/grid/"));
  check(
    "tooltip component included",
    hasInput("/echarts/lib/component/tooltip/"),
  );
  check("svg renderer included", hasInput("/echarts/lib/renderer/"));
  check("echarts core included", hasInput("/echarts/lib/"));

  check(
    "zrender svg renderer included",
    hasInput("/zrender/lib/svg/"),
    "SVG rendering is the only supported renderer (local-first, theme-following)",
  );
  check(
    "zrender canvas renderer excluded",
    !hasInput("/zrender/lib/canvas/"),
    "canvas doubles the bundle for a renderer the app never selects",
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
