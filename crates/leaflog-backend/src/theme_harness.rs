//! The theme harness page (ticket 07): a development-only host that
//! frames the app the way Home Assistant's ingress panel does and
//! carries HA-like theme variables on its root element.
//!
//! It exists so the e2e suite can drive real rendered-DOM assertions
//! for parent-theme adoption (D079's boundary rule for the
//! `js_sys`/`wasm-bindgen` parent reads) without a Home Assistant
//! install. The route is registered only in [`crate::app::Mode::Dev`],
//! so it never ships in the production image.

/// `GET /theme-harness` — the harness document.
///
/// `setHaTheme("light" | "dark")` rewrites the root variables the way
/// HA's `applyThemesOnElement` does, exercising the live re-adoption
/// path. The `--primary-color: var(--ha-color-primary-40)` indirection
/// mirrors the default HA theme's structure: custom-property
/// computed values substitute `var()` references, which is what the
/// frontend's computed-style read relies on.
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
pub async fn harness_page() -> axum::response::Html<&'static str> {
    axum::response::Html(HARNESS_HTML)
}

/// The harness document, inline so no dist asset is needed.
const HARNESS_HTML: &str = r##"<!doctype html>
<html lang="en">
<head>
<meta charset="utf-8">
<title>Leaflog theme harness</title>
<style>
  body { margin: 0; font: 14px/1.4 system-ui, sans-serif; }
  .bar { padding: 8px 12px; display: flex; gap: 16px; align-items: center; }
  #mode { font-weight: 600; }
  iframe { display: block; width: 100%; height: 82vh; border: 0; }
</style>
<script>
  // HA-like palettes: the same variables the default HA frontend sets
  // on its document root, in HA's own value formats.
  const PALETTES = {
    light: {
      "--ha-color-primary-40": "#3f51b5",
      "--primary-color": "var(--ha-color-primary-40)",
      "--accent-color": "#ff9800",
      "--primary-background-color": "#fafafa",
      "--card-background-color": "#ffffff",
      "--divider-color": "rgba(0, 0, 0, 0.12)",
      "--primary-text-color": "#212121",
      "--text-primary-color": "#ffffff",
      "--success-color": "#43a047",
      "--warning-color": "#ffa600",
      "--error-color": "#db4437",
      "--info-color": "#039be5",
    },
    dark: {
      "--ha-color-primary-40": "#b39ddb",
      "--primary-color": "var(--ha-color-primary-40)",
      "--accent-color": "#ffb74d",
      "--primary-background-color": "#111111",
      "--card-background-color": "#1c1c1c",
      "--divider-color": "rgba(225, 225, 225, 0.12)",
      "--primary-text-color": "#e1e1e1",
      "--text-primary-color": "#212121",
      "--success-color": "#66bb6a",
      "--warning-color": "#ff9800",
      "--error-color": "#ef5350",
      "--info-color": "#4fc3f7",
    },
  };

  function setHaTheme(mode) {
    const vars = PALETTES[mode];
    const root = document.documentElement;
    for (const name of Object.keys(vars)) {
      root.style.setProperty(name, vars[name]);
    }
    const label = document.getElementById("mode");
    if (label) { label.textContent = mode; }
  }
  window.setHaTheme = setHaTheme;
  setHaTheme("light");
</script>
</head>
<body>
  <div class="bar">
    Leaflog theme harness — HA-like variables live on this page's
    root element. Mode: <span id="mode">light</span>.
    Switch with <code>setHaTheme("dark")</code>.
  </div>
  <iframe id="app" title="Leaflog app under the harness" src="./"></iframe>
</body>
</html>
"##;
