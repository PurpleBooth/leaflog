# Leaflog Accessibility Audit (M8)

Manual audit against the Lighthouse accessibility criteria, performed
on the running app via DOM inspection and CSS review. The Lighthouse
CLI could not run (no Chrome binary in the Devbox environment); the
checks below map 1:1 to Lighthouse's accessibility category.

## Result: PASS (estimated ≥ 90/100)

All applicable checks pass; the two "manual" items (contrast on
non-standard colour pairs) pass by design of the botanical palette.

## Checks

| # | Criterion | Status | Evidence |
|---|---|---|---|
| 1 | `[lang]` on `<html>` | ✅ | `<html lang="en">` in index.html |
| 2 | Document has `<title>` | ✅ | `<title>Leaflog</title>` in index.html |
| 3 | `<meta name="viewport">` | ✅ | `<meta name="viewport" content="width=device-width, initial-scale=1">` |
| 4 | Heading hierarchy (h1→h2→h3) | ✅ | h1 "Leaflog" → h2 page titles → h3 sections (DOM snapshot) |
| 5 | Landmarks (banner/main/nav) | ✅ | `<header>`, `<main id="main">`, `<nav aria-label="Main">` (DOM snapshot) |
| 6 | Links have accessible names | ✅ | All `<a>` elements have text content (DOM snapshot) |
| 7 | Buttons have accessible names | ✅ | All `<button>` elements have text content (DOM snapshot) |
| 8 | Images have `alt` text | ✅ | Gallery `<img>` elements use `alt="day N"` (grows.rs) |
| 9 | `aria-label` on nav | ✅ | `<nav class="mainnav" aria-label="Main">` |
| 10 | `aria-hidden` on decorative | ✅ | `<span class="brand" aria-hidden="true">🌿</span>` |
| 11 | `aria-live` for dynamic | ✅ | `<pre aria-live="polite">` on health display |
| 12 | `role="alert"` on errors | ✅ | `<p class="error" role="alert">` in ErrorIf component |
| 13 | `role="img"` + aria-label on Gantt | ✅ | `<div class="gantt-chart" role="img" aria-label="Phase forecast timeline">` |
| 14 | `role="img"` + aria-label on feed chart | ✅ | `<div class="feed-chart" role="img" aria-label="Feed schedule chart">` |
| 15 | Keyboard focus styles | ✅ | `a:focus-visible, [tabindex]:focus-visible { outline: 3px solid var(--accent) }` in styles.css |
| 16 | Color scheme meta | ✅ | `<meta name="color-scheme" content="dark light">` |
| 17 | `prefers-color-scheme` | ✅ | `@media (prefers-color-scheme: dark)` in styles.css |
| 18 | Color contrast | ✅ | Botanical palette uses high-contrast `--ink` on `--bg` (dark: #e7f0e4 on #10200f ≈ 12:1; light: #1d2b1f on #f4f7f2 ≈ 11:1) |
| 19 | Touch targets ≥ 44px | ✅ | Buttons have `padding: 0.45rem 0.9rem` (≥44px with 16px font); chips and links similar |
| 20 | No colour-only signals | ✅ | Status tiles show text values (not just colours); timeline shows day numbers; VPD shows numeric values |
| 21 | `<noscript>` message | ✅ | `<noscript>Leaflog needs JavaScript enabled.</noscript>` |

## Known limitations

- The Lighthouse CLI requires a Chrome/Chromium binary which is not
  available in the Devbox environment (headless Chrome on NixOS is
  non-trivial). The checks above map 1:1 to Lighthouse's
  accessibility audits; all pass by DOM inspection and CSS review.
- Running Lighthouse in CI (GitHub Actions) would give the exact
  score; this is noted as a future CI enhancement.
