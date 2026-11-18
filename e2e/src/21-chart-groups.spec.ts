// Chart groups with crosshair sync (ticket 02): the history charts
// render under Air and Root zone micro-label headers — Air above
// ground (temperature, humidity, VPD, CO₂), Root zone below
// (soil moisture, pH, EC) — at compact heights, two-up on wide
// screens. Every mounted chart joins one ECharts connect group, so
// one hover moves the crosshair on every chart and each chart's own
// tooltip reads its role at the same shared timestamp. The per-card
// window controls are gone (19-env-section-window.spec.ts owns that
// half); the shared window control stays the only one. The Light
// card that used to sit outside the group is gone too (ticket 05
// folded it into the Air charts' night shading).

import { expect, test, type Locator, type Page } from '@playwright/test';
import { api, createGrow, dismissOnboarding, revealRootZone, url } from './helpers';

test.beforeAll(async ({ request }) => {
  await dismissOnboarding(request);
});

/** Binds the mock-fed roles and waits for all of them to be live. */
async function bindAndAwaitLive(
  request: import('@playwright/test').APIRequestContext,
  growId: string,
  extra: Record<string, string> = {},
) {
  await api(request, 'PUT', `api/grows/${growId}/bindings`, {
    bindings: {
      temperature: 'sensor.grow_temperature',
      humidity: 'sensor.grow_humidity',
      soil_moisture: 'sensor.grow_soil_moisture',
      light: 'light.grow_led',
      ...extra,
    },
  });
  await expect
    .poll(async () => {
      const env = await api<{
        bindings: Array<{ role: string; available: boolean }>;
      }>(request, 'GET', `api/grows/${growId}/env`);
      return env!.bindings.every((binding) => binding.available);
    }, { timeout: 10_000 })
    .toBe(true);
}

/** One chart's svg by role. */
function chartSvg(page: Page, role: string): Locator {
  return page.locator(`svg[aria-label="${role} history"]`);
}

/** Hovers the middle of one chart's svg so the axis tooltip snaps. */
async function hoverCenter(page: Page, svg: Locator) {
  await svg.scrollIntoViewIfNeeded();
  const box = await svg.boundingBox();
  expect(box, 'chart laid out for hover').not.toBeNull();
  await page.mouse.move(box!.x + box!.width * 0.5, box!.y + box!.height * 0.5, {
    steps: 5,
  });
}

/** The chart card that holds one role's svg. */
function cardOf(page: Page, role: string): Locator {
  return page.locator('.card', {
    has: page.locator(`svg[aria-label="${role} history"]`),
  });
}

/** The visible tooltip text on one role's chart card, if any. */
async function tooltipText(page: Page, role: string): Promise<string> {
  const tip = cardOf(page, role).getByText(/UTC/).first();
  if (!(await tip.isVisible().catch(() => false))) {
    return '';
  }
  return (await tip.textContent()) ?? '';
}

/** The UTC timestamp the tooltip shows. */
const stampOf = (text: string): string =>
  text.match(/\d{4}-\d{2}-\d{2} \d{2}:\d{2} UTC/)?.[0] ?? 'no-timestamp';

test('Air and Root zone group their charts under micro-label headers', async ({
  page,
  request,
}) => {
  const growId = await createGrow(request, 'E2E Chart Groups');
  // CO₂ stays unbound: its card must not render at all, group or no
  // group. Light binds, feeding the Air charts' night shading.
  await bindAndAwaitLive(request, growId);
  await page.goto(url(`/grows/${growId}`));
  await expect(chartSvg(page, 'temperature')).toBeVisible();
  // The Root zone grid mounts lazily (ticket 07): reveal it before
  // asserting anything about its charts.
  await revealRootZone(page);
  await expect(chartSvg(page, 'soil_moisture')).toBeVisible();

  // Exactly one Air and one Root zone heading, each uppercase in
  // styling only (the source text stays the plain group name).
  const air = page.getByRole('heading', { name: 'Air', exact: true });
  const root = page.getByRole('heading', { name: 'Root zone', exact: true });
  await expect(air).toHaveCount(1);
  await expect(root).toHaveCount(1);
  expect(await air.evaluate((el) => getComputedStyle(el).textTransform)).toBe('uppercase');
  expect(await root.evaluate((el) => getComputedStyle(el).textTransform)).toBe('uppercase');

  // Document order: Air, its charts, Root zone, its charts. The
  // card titles step down to h5 under the h4 group labels.
  const sequence = await page.evaluate(() =>
    Array.from(document.querySelectorAll('h4, h5, svg[aria-label$=" history"]')).map((el) =>
      el instanceof SVGElement ? el.getAttribute('aria-label') : el.textContent?.trim(),
    ),
  );
  const at = (needle: string) => sequence.indexOf(needle);
  const seq = sequence.join(' | ');
  expect(at('Air'), seq).toBeGreaterThanOrEqual(0);
  expect(at('Air'), seq).toBeLessThan(at('temperature history'));
  expect(at('temperature history'), seq).toBeLessThan(at('humidity history'));
  expect(at('humidity history'), seq).toBeLessThan(at('Root zone'));
  expect(at('Root zone'), seq).toBeLessThan(at('soil_moisture history'));

  // Ticket 05: no standalone Light card anywhere in the section.
  await expect(page.locator('svg[aria-label="light history"]')).toHaveCount(0);

  // The Air card titles sit under their group label, not beside it.
  expect(at('Air'), seq).toBeLessThan(at('Temperature'));

  // CO₂ unbound: no card, in either group.
  await expect(chartSvg(page, 'co2')).toHaveCount(0);
});

test('one hover moves every chart at the same shared timestamp', async ({
  page,
  request,
}) => {
  const growId = await createGrow(request, 'E2E Crosshair Sync');
  await bindAndAwaitLive(request, growId);
  // Three note snapshots a beat apart give the charts fresh points
  // to snap the crosshair to (each note auto-captures the bound
  // roles); the charts must be mounted only after the mock entities
  // are live, or the first history fetch races the backend's sensor
  // integration and no tooltip ever renders.
  for (const note of ['first', 'second', 'third']) {
    await api(request, 'POST', `api/grows/${growId}/actions`, {
      plant_ids: [],
      occurred_at: new Date().toISOString(),
      action_type: 'note',
      details: {},
      notes: note,
      appearance_tags: [],
    });
    await page.waitForTimeout(1_200);
  }
  await page.goto(url(`/grows/${growId}`));
  await revealRootZone(page);
  for (const role of ['temperature', 'humidity', 'soil_moisture']) {
    await expect(chartSvg(page, role)).toBeVisible();
  }

  // Hover the temperature chart: every chart's own tooltip must
  // appear — the connect group carried the pointer — each reading its
  // own role's value format at one shared timestamp.
  await hoverCenter(page, chartSvg(page, 'temperature'));
  const tooltips: Record<string, string> = {};
  for (const role of ['temperature', 'humidity', 'soil_moisture']) {
    await expect
      .poll(() => tooltipText(page, role), { timeout: 5_000 })
      .toMatch(/UTC/);
    tooltips[role] = await tooltipText(page, role);
  }
  expect(tooltips.temperature, 'role-scoped tooltip text').toContain('°C');
  expect(tooltips.humidity, 'role-scoped tooltip text').toContain('%');
  expect(tooltips.soil_moisture, 'role-scoped tooltip text').toContain('%');
  const stamps = Object.values(tooltips).map(stampOf);
  expect(new Set(stamps).size, `timestamps: ${stamps.join(', ')}`).toBe(1);

  // Hover a Root zone chart instead: the Air charts follow it too.
  await hoverCenter(page, chartSvg(page, 'soil_moisture'));
  for (const role of ['temperature', 'humidity', 'soil_moisture']) {
    await expect
      .poll(() => tooltipText(page, role), { timeout: 5_000 })
      .toMatch(/UTC/);
  }
  const followStamps = (
    await Promise.all(
      ['temperature', 'humidity', 'soil_moisture'].map((role) =>
        tooltipText(page, role).then(stampOf),
      ),
    )
  ).filter((stamp) => stamp !== 'no-timestamp');
  expect(followStamps.length, 'every chart shows a tooltip').toBe(3);
  expect(new Set(followStamps).size, `timestamps: ${followStamps.join(', ')}`).toBe(1);
});

test('compact chart heights stack one-up narrow and pair two-up wide', async ({
  page,
  request,
}) => {
  const growId = await createGrow(request, 'E2E Chart Heights');
  await bindAndAwaitLive(request, growId);
  await page.goto(url(`/grows/${growId}`));
  await expect(chartSvg(page, 'temperature')).toBeVisible();

  // Wide screen: the Air group pairs two-up and the charts stand
  // ~176 px tall (the compact instrument-panel height).
  const tempWide = await chartSvg(page, 'temperature').boundingBox();
  const humidWide = await chartSvg(page, 'humidity').boundingBox();
  expect(tempWide, 'wide temperature chart laid out').not.toBeNull();
  expect(humidWide, 'wide humidity chart laid out').not.toBeNull();
  expect(tempWide!.height, 'desktop height ~176px').toBeGreaterThan(170);
  expect(tempWide!.height).toBeLessThan(182);
  expect(humidWide!.x, 'two-up: humidity beside temperature').toBeGreaterThan(
    tempWide!.x,
  );
  expect(Math.abs(humidWide!.y - tempWide!.y), 'two-up: same row').toBeLessThan(4);

  // Narrow screen: one column, ~144 px tall.
  await page.setViewportSize({ width: 375, height: 812 });
  await expect(chartSvg(page, 'temperature')).toBeVisible();
  const tempNarrow = await chartSvg(page, 'temperature').boundingBox();
  const humidNarrow = await chartSvg(page, 'humidity').boundingBox();
  expect(tempNarrow, 'narrow temperature chart laid out').not.toBeNull();
  expect(humidNarrow, 'narrow humidity chart laid out').not.toBeNull();
  expect(tempNarrow!.height, 'mobile height ~144px').toBeGreaterThan(138);
  expect(tempNarrow!.height).toBeLessThan(150);
  expect(humidNarrow!.y, 'one column: humidity under temperature').toBeGreaterThan(
    tempNarrow!.y,
  );
});
