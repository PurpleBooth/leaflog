// Stepped Target Bands (ticket 07): the shaded band on the
// environment charts steps between a day range and a night range
// following the Light Cycle — or, when a light entity is bound, the
// entity's real on/off history (timer overrides included). The
// backend contract is pinned by the Rust integration tests; these
// cases prove the visible behaviour: several filled band regions of
// differing geometry inside the rendered chart SVG.

import { expect, test, type Locator } from '@playwright/test';
import { api, createGrow, dismissOnboarding, url } from './helpers';

test.beforeAll(async ({ request }) => {
  await dismissOnboarding(request);
});

/** Today minus three days, so germination covers any day window. */
const threeDaysAgo = () =>
  new Date(Date.now() - 3 * 86_400_000).toISOString().slice(0, 10);

interface BandSegment {
  start: string;
  end: string;
  min: number;
  max: number;
  phase: string;
  light_on: boolean | null;
}

/** The filled (non-line) paths of one chart's ECharts SVG. */
async function bandRegions(svg: Locator): Promise<{ top: number; height: number }[]> {
  const paths = svg.locator('path[fill]:not([fill="none"])');
  const regions: { top: number; height: number }[] = [];
  const count = await paths.count();
  for (let index = 0; index < count; index += 1) {
    const box = await paths.nth(index).boundingBox();
    if (box && box.height > 1) {
      regions.push({ top: box.y, height: box.height });
    }
  }
  return regions;
}

test('bands step day/night per the Light Cycle without a bound light', async ({
  page,
  request,
}) => {
  const growId = await createGrow(request, 'E2E Stepped Schedule', {
    started_on: threeDaysAgo(),
  });
  // 12 h on from 08:00: a day window always holds both stretches.
  await api(request, 'PUT', `api/grows/${growId}/light-cycle`, {
    lights_on: '08:00',
    hours_on: { germination: 12 },
  });
  await api(request, 'PUT', `api/grows/${growId}/bindings`, {
    bindings: {
      temperature: 'sensor.grow_temperature',
      humidity: 'sensor.grow_humidity',
    },
  });
  // One log entry auto-captures the bound sensors, so the line draws.
  await api(request, 'POST', `api/grows/${growId}/actions`, {
    plant_ids: [],
    occurred_at: new Date().toISOString(),
    action_type: 'note',
    details: {},
    notes: 'reading',
    appearance_tags: [],
  });

  // The API half of the contract: alternating day/night segments with
  // their own corridors (germination day 21.1–26.7, night 20.0–23.3).
  const history = await api<{ bands: BandSegment[] }>(
    request,
    'GET',
    `api/grows/${growId}/history?role=temperature&window=day`,
  );
  const bands = history!.bands.filter((band) => band.light_on !== null);
  expect(bands.length, 'day and night segments in the window').toBeGreaterThanOrEqual(2);
  for (let index = 1; index < bands.length; index += 1) {
    expect(
      bands[index].light_on,
      'identical neighbours merge, so states alternate',
    ).not.toBe(bands[index - 1].light_on);
  }
  for (const band of bands) {
    expect(band.phase, 'the grow is germinating').toBe('germination');
    const expected = band.light_on ? [21.1, 26.7] : [20.0, 23.3];
    expect(band.min, 'corridor floor follows the light state').toBeCloseTo(expected[0]);
    expect(band.max, 'corridor ceiling follows the light state').toBeCloseTo(expected[1]);
  }

  // The visible half: the temperature chart draws several filled band
  // regions whose heights differ — the day band is taller than the
  // night band.
  await page.goto(url(`/grows/${growId}`));
  const svg = page.locator('svg[aria-label="temperature history"]');
  await expect(svg).toBeVisible();
  const regions = await bandRegions(svg);
  expect(
    regions.length,
    'several band regions render, not one uniform corridor',
  ).toBeGreaterThanOrEqual(2);
  const heights = new Set(regions.map((region) => Math.round(region.height)));
  expect(heights.size, 'the day and night regions differ in height').toBeGreaterThanOrEqual(2);
});

test('band steps follow the bound light entity on/off history', async ({
  page,
  request,
}) => {
  const growId = await createGrow(request, 'E2E Stepped Fixture', {
    started_on: threeDaysAgo(),
  });
  await api(request, 'PUT', `api/grows/${growId}/bindings`, {
    bindings: {
      temperature: 'sensor.grow_temperature',
      light: 'light.grow_led',
    },
  });
  // A 24 h schedule: on its own it would render a single always-lit
  // band. The mock's light staircase (off at now−23 h, on at now−20 h,
  // off at now−18 h) overrides it with dark stretches.
  await api(request, 'PUT', `api/grows/${growId}/light-cycle`, {
    lights_on: '06:00',
    hours_on: { germination: 24 },
  });
  await api(request, 'POST', `api/grows/${growId}/actions`, {
    plant_ids: [],
    occurred_at: new Date().toISOString(),
    action_type: 'note',
    details: {},
    notes: 'reading',
    appearance_tags: [],
  });

  const history = await api<{ bands: BandSegment[] }>(
    request,
    'GET',
    `api/grows/${growId}/history?role=temperature&window=day`,
  );
  const bands = history!.bands;
  expect(bands.length, 'the staircase splits the always-on schedule').toBeGreaterThanOrEqual(3);
  expect(
    bands.some((band) => band.light_on === false),
    'a dark stretch exists that the 24 h schedule cannot explain',
  ).toBe(true);
  for (const band of bands) {
    const expected = band.light_on ? [21.1, 26.7] : [20.0, 23.3];
    expect(band.min).toBeCloseTo(expected[0]);
    expect(band.max).toBeCloseTo(expected[1]);
  }

  // Visible: the chart shows the fixture's steps as separate regions
  // with differing geometry, not the schedule's one uniform band.
  await page.goto(url(`/grows/${growId}`));
  const svg = page.locator('svg[aria-label="temperature history"]');
  await expect(svg).toBeVisible();
  const regions = await bandRegions(svg);
  expect(regions.length, 'the fixture steps are visible as regions').toBeGreaterThanOrEqual(3);
  const heights = new Set(regions.map((region) => Math.round(region.height)));
  expect(heights.size, 'the dark and lit regions differ in height').toBeGreaterThanOrEqual(2);
});
