// Theme matching (ticket 07): the app must wear the parent Home
// Assistant frontend's theme — adoption inside the harness iframe,
// live re-adoption when the parent theme flips without a reload, and
// the mandatory silent built-in fallback when there is no readable
// parent (the plain, top-level app load).

import { test, expect, type Frame, type Page } from '@playwright/test';
import { url } from './helpers';

/** Harness palette as HA hex/rgba strings (theme-harness.rs). */
const HARNESS = {
  light: {
    primary: '#3f51b5',
    accent: '#ff9800',
    background: '#fafafa',
    card: '#ffffff',
    text: '#212121',
  },
  dark: {
    primary: '#b39ddb',
    accent: '#ffb74d',
    background: '#111111',
    card: '#1c1c1c',
    text: '#e1e1e1',
  },
} as const;

/** Minimal oklch of a hex colour, mirroring leaflog-core's math. */
function oklchOfHex(hex: string): [number, number, number] {
  const full = hex.replace('#', '');
  const [r, g, b] = [0, 2, 4].map((at) => parseInt(full.slice(at, at + 2), 16) / 255);
  const lin = (c: number) => (c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4));
  const [rl, gl, bl] = [r, g, b].map(lin);
  const l = Math.cbrt(0.4122214708 * rl + 0.5363325363 * gl + 0.0514459929 * bl);
  const m = Math.cbrt(0.2119034982 * rl + 0.6806995451 * gl + 0.1073969566 * bl);
  const s = Math.cbrt(0.0883024619 * rl + 0.2817188376 * gl + 0.6299787005 * bl);
  const L = 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s;
  const a = 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s;
  const bb = 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s;
  return [L, Math.hypot(a, bb), (Math.atan2(bb, a) * 180) / Math.PI];
}

/** Parses a computed colour — "oklch(L C H)" when the app set it,
 *  "rgb(r, g, b)" when it comes from the built-in hex palette — and
 *  returns it as a lightness/chroma/hue triple. */
function parseColor(value: string): [number, number, number] {
  const oklch = value.match(/oklch\(([\d.]+)\s+([\d.]+)\s+([\d.]+)/);
  if (oklch) {
    return [Number(oklch[1]), Number(oklch[2]), Number(oklch[3])];
  }
  const rgb = value.match(/rgba?\(\s*([\d.]+)[,\s]+([\d.]+)[,\s]+([\d.]+(?:[,\s]+[\d.]+)?)\s*\)/);
  if (rgb) {
    const channels = [rgb[1], rgb[2], rgb[3]].map(Number);
    const hex = channels
      .slice(0, 3)
      .map((channel) => Math.round(channel).toString(16).padStart(2, '0'))
      .join('');
    return oklchOfHex(`#${hex}`);
  }
  throw new Error(`expected an oklch() or rgb() computed value, got ${JSON.stringify(value)}`);
}

/** Asserts two oklch triples match within display tolerance. Hue is
 *  compared on the circle: the app emits [0,360), the helper here
 *  mirrors atan2 in (-180,180]. */
function expectCloseTo(actual: [number, number, number], expected: [number, number, number]) {
  const tolerance = 0.01;
  for (const axis of [0, 1, 2]) {
    const raw = Math.abs(actual[axis] - expected[axis]) % 360;
    const delta = axis === 2 && raw > 180 ? 360 - raw : raw;
    expect(delta, `oklch axis ${axis}: ${actual} vs ${expected}`).toBeLessThan(tolerance);
  }
}

/** The app frame inside the harness page (it loads `./`). */
async function appFrame(page: Page): Promise<Frame> {
  const frame = page.frame({ url: (u) => u.pathname === url('/') });
  expect(frame, 'the harness must frame the app under the ingress prefix').toBeTruthy();
  return frame as Frame;
}

/** Reads a resolved colour inside the app frame: the computed value of
 *  `property` on an element with `classes` (probe-span pattern, since
 *  not every semantic colour is rendered on every page). */
async function probeColor(frame: Frame, classes: string, property: string): Promise<string> {
  return frame.evaluate(
    ({ classes, property }) => {
      const span = document.createElement('span');
      span.className = classes;
      document.body.appendChild(span);
      const value = getComputedStyle(span).getPropertyValue(property);
      span.remove();
      return value.trim();
    },
    { classes, property },
  );
}

test('adopts the harness theme inside the iframe', async ({ page }) => {
  await page.goto(url('/theme-harness'));
  const frame = await appFrame(page);

  // The wasm adoption runs at app boot; poll until it has applied.
  await expect
    .poll(async () => frame.evaluate('document.documentElement.style.colorScheme'))
    .toBe('light');

  // The page background follows the parent's primary background.
  const bodyBg = await frame.evaluate(
    'getComputedStyle(document.body).backgroundColor',
  );
  expectCloseTo(parseColor(bodyBg), oklchOfHex(HARNESS.light.background));

  // Cards adopt the parent card background, text the parent text
  // colour, and the semantic primary the parent primary (which reaches
  // the app through the var() indirection on the harness root).
  const card = await probeColor(frame, 'bg-base-100', 'background-color');
  expectCloseTo(parseColor(card), oklchOfHex(HARNESS.light.card));
  const text = await probeColor(frame, 'text-base-content', 'color');
  expectCloseTo(parseColor(text), oklchOfHex(HARNESS.light.text));
  const primary = await probeColor(frame, 'text-primary', 'color');
  expectCloseTo(parseColor(primary), oklchOfHex(HARNESS.light.primary));

  // Dark/light follows the parent implicitly: a light parent page sets
  // the colour scheme, not a picker.
  expect(await frame.evaluate('document.documentElement.style.colorScheme')).toBe('light');
});
test('re-adopts live when the parent theme flips', async ({ page }) => {
  await page.goto(url('/theme-harness'));
  const frame = await appFrame(page);

  // Flip the parent root the way HA re-applies a theme — no reload of
  // the app document, so only the MutationObserver path can carry it.
  await page.evaluate('window.setHaTheme("dark")');

  // Await stability, then assert the dark palette end-to-end.
  await expect
    .poll(async () => frame.evaluate('document.documentElement.style.colorScheme'))
    .toBe('dark');
  const bodyBg = await frame.evaluate('getComputedStyle(document.body).backgroundColor');
  expectCloseTo(parseColor(bodyBg), oklchOfHex(HARNESS.dark.background));
  const primary = await probeColor(frame, 'text-primary', 'color');
  expectCloseTo(parseColor(primary), oklchOfHex(HARNESS.dark.primary));
  const text = await probeColor(frame, 'text-base-content', 'color');
  expectCloseTo(parseColor(text), oklchOfHex(HARNESS.dark.text));
});

test('falls back to the built-in theme without a parent', async ({ page }) => {
  // A plain top-level load: no parent theme to read (the harness is
  // not involved), so the app must silently wear its own palette.
  // The light palette is pinned so the assertion names real colours.
  await page.emulateMedia({ colorScheme: 'light' });
  await page.goto(url('/'));
  await expect(page.getByRole('heading', { name: 'Leaflog' })).toBeVisible();

  const overrides = await page.evaluate(
    `document.documentElement.style.getPropertyValue('--color-base-200')`,
  );
  expect(overrides, 'no adoption means no inline override').toBe('');
  const bodyBg = await page.evaluate('getComputedStyle(document.body).backgroundColor');
  expectCloseTo(parseColor(bodyBg), oklchOfHex('#f4f7f2'));
});
