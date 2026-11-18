// Shared helpers for the Leaflog end-to-end suite.
//
// Everything runs under the simulated HA ingress prefix, exactly like
// `just dev` and production: `url('/grows')` is
// `/api/hassio_ingress/e2e/grows`. No test may talk to the bare
// origin — ingress-relative URLs are a non-negotiable, and the whole
// suite exercises them on every navigation and API call.

import { expect, type APIRequestContext, type Locator, type Page } from '@playwright/test';

/** The simulated ingress prefix the e2e stack serves under. */
export const INGRESS_PREFIX =
  process.env.LEAFLOG_E2E_PREFIX ?? '/api/hassio_ingress/e2e';

/** A route or API path under the ingress prefix. */
export function url(path: string): string {
  const suffix = path.startsWith('/') ? path : `/${path}`;
  return `${INGRESS_PREFIX}${suffix}`;
}

let counter = 0;

/** A unique per-run name, so repeated runs can never collide. */
export function uniqueName(prefix: string): string {
  counter += 1;
  return `${prefix} ${Date.now().toString(36)}-${counter}`;
}

// The D061 regression guard. A native form submit (the bug: a listener
// name the DOM never dispatches, so the browser submits and reloads)
// wipes this marker with the document. Arm it right before clicking a
// submit button; assert it right after the expected UI change and
// before any explicit reload.
const MARKER = '__leaflogE2eAlive';

/** Arms the no-native-submit marker on the current document. */
export async function armNoReloadGuard(page: Page): Promise<void> {
  await page.evaluate(`window.${MARKER} = true`);
}

/** Asserts the marker survived: no native form submission happened. */
export async function expectNoReload(page: Page): Promise<void> {
  expect(await page.evaluate(`window.${MARKER} === true`), [
    'the page reloaded — the form did a native submit (the D061 bug class)',
  ]).toBe(true);
}

/** One JSON REST call against the backend, under the ingress prefix. */
export async function api<T = unknown>(
  request: APIRequestContext,
  method: 'GET' | 'POST' | 'PUT' | 'DELETE',
  path: string,
  body?: unknown,
): Promise<T | undefined> {
  const response = await request.fetch(url(path), {
    method,
    data: body === undefined ? undefined : JSON.stringify(body),
    headers:
      body === undefined ? undefined : { 'content-type': 'application/json' },
  });
  const text = await response.text();
  expect(
    response.ok(),
    `${method} ${path} -> ${response.status()}: ${text}`,
  ).toBeTruthy();
  return text === '' ? undefined : (JSON.parse(text) as T);
}

/** Creates a grow via the API (day one today) and returns its id. */
export async function createGrow(
  request: APIRequestContext,
  name: string,
  overrides: Record<string, unknown> = {},
): Promise<string> {
  const today = new Date().toISOString().slice(0, 10);
  const grow = await api<{ id: string }>(request, 'POST', 'api/grows', {
    name,
    feed_template_id: null,
    substrate: 'soil',
    started_on: today,
    status: 'active',
    notes: null,
    ...overrides,
  });
  return grow!.id;
}

/** Creates a plant in a grow via the API and returns its id. */
export async function createPlant(
  request: APIRequestContext,
  growId: string,
  name: string,
): Promise<string> {
  const plant = await api<{ id: string }>(
    request,
    'POST',
    `api/grows/${growId}/plants`,
    { name },
  );
  return plant!.id;
}

/** Dismisses the onboarding wizard for this database. */
export async function dismissOnboarding(
  request: APIRequestContext,
): Promise<void> {
  await api(request, 'PUT', 'api/settings', { onboarding_dismissed: true });
}

/** One Target Band segment of a chart history payload. */
export interface BandSegment {
  start: string;
  end: string;
  min: number;
  max: number;
  phase: string;
  light_on: boolean | null;
}

/** The log-event tick glyphs on one chart's ECharts SVG (ticket
 *  04): narrow filled rect paths, 2px wide and 12px tall, in the
 *  muted gridline colour — geometry, not a new hue. Width at most
 *  4px separates them from scatter dots (8px) and Target Band shading
 *  (viewport-wide); the 8–16px height window keeps grid strokes
 *  (1–2px) out. zrender's clip shapes are excluded as with the
 *  bands. Returns viewport boxes so specs can count and locate them. */
export async function markerTicks(
  svg: Locator,
): Promise<{ x: number; y: number; height: number }[]> {
  return svg.evaluate((root) => {
    const ticks: { x: number; y: number; height: number }[] = [];
    for (const path of root.querySelectorAll<SVGPathElement>(
      'path[fill]:not([fill="none"])',
    )) {
      if (path.closest('clipPath')) {
        continue;
      }
      const box = path.getBoundingClientRect();
      if (box.width <= 4 && box.height >= 8 && box.height <= 16) {
        ticks.push({ x: box.x, y: box.y, height: box.height });
      }
    }
    return ticks;
  });
}

/** The night-shading regions of one chart's ECharts SVG (ticket
 *  05): filled paths spanning the plot's full height — the plot
 *  rectangle is the SVG minus the grid insets (top 36, bottom 24),
 *  and a region qualifies at ≥ 95 % of that height. Height is the
 *  discriminator: Target Band shading sits inside the padded value
 *  range (≤ ~80 % of the plot) and the log-event ticks are narrow.
 *  zrender's clip shapes are excluded as with the bands. Returns
 *  viewport boxes so specs can count and locate them. */
export async function nightShades(
  svg: Locator,
): Promise<{ x: number; width: number; height: number }[]> {
  return svg.evaluate((root) => {
    const plotHeight = root.getBoundingClientRect().height - 60;
    const shades: { x: number; width: number; height: number }[] = [];
    for (const path of root.querySelectorAll<SVGPathElement>(
      'path[fill]:not([fill="none"])',
    )) {
      if (path.closest('clipPath')) {
        continue;
      }
      const box = path.getBoundingClientRect();
      if (box.width > 4 && box.height >= plotHeight * 0.95) {
        shades.push({ x: box.x, width: box.width, height: box.height });
      }
    }
    return shades;
  });
}

/** The filled (non-line) paths of one chart's ECharts SVG: the
 *  rendered Target Band regions, as top/height boxes (viewport
 *  coordinates — comparisons only). zrender's clip shapes carry a
 *  fill but live inside `<clipPath>` and never paint, so they are
 *  excluded: what remains is real shading. The night layer (ticket
 *  05) also paints filled paths, but spans the full plot height
 *  (the SVG minus the grid insets, top 36 bottom 24) — bands sit
 *  inside the padded value range and never reach it, so the
 *  full-height paths are filtered out here and counted by
 *  nightShades instead. */
export async function bandRegions(
  svg: Locator,
): Promise<{ top: number; height: number }[]> {
  return svg.evaluate((root) => {
    const plotHeight = root.getBoundingClientRect().height - 60;
    const regions: { top: number; height: number }[] = [];
    for (const path of root.querySelectorAll<SVGPathElement>(
      'path[fill]:not([fill="none"])',
    )) {
      if (path.closest('clipPath')) {
        continue;
      }
      const box = path.getBoundingClientRect();
      if (box.height > 1 && box.height < plotHeight * 0.95) {
        regions.push({ top: box.top, height: box.height });
      }
    }
    return regions;
  });
}

/** Scroll the Root zone group into view so its lazily mounted chart
 *  grid renders (ticket 07): the group waits below the fold for its
 *  first intersection before fetching or drawing anything. The label
 *  is the group's structural scroll target — it always renders. */
export async function revealRootZone(page: Page): Promise<void> {
  await page
    .getByRole('heading', { name: 'Root zone', exact: true })
    .scrollIntoViewIfNeeded();
}
