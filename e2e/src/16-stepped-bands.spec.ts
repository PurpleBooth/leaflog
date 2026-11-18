// Stepped Target Bands (ticket 07; env-section ticket 06): the VPD
// chart's shaded band steps between a day range and a night range
// following the Light Cycle — or, when a light entity is bound, the
// entity's real on/off history (timer overrides included). The
// temperature band is flat (tolerance-derived) and CO₂ stays bare, so
// VPD carries the stepping contract. The backend contract is pinned
// by the Rust integration tests; these cases prove the visible
// behaviour: several filled band regions of differing geometry inside
// the rendered chart SVG.

import { expect, test } from '@playwright/test';
import {
  api,
  bandRegions,
  createGrow,
  dismissOnboarding,
  url,
  type BandSegment,
} from './helpers';

test.beforeAll(async ({ request }) => {
  await dismissOnboarding(request);
});

/** Today minus three days, so germination covers any day window. */
const threeDaysAgo = () =>
  new Date(Date.now() - 3 * 86_400_000).toISOString().slice(0, 10);

/** The germ-phase VPD range while lit (sourced preset, spec §5.2). */
const DAY_VPD = [0.4, 0.8];
/** The germ-phase VPD range in the dark (sourced night preset). */
const NIGHT_VPD = [0.5, 0.9];

test('vpd bands step day/night per the Light Cycle without a bound light', async ({
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
  // VPD derives from the temperature + humidity pair, so both bind.
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
  // their own ranges (germination day 0.4–0.8, night 0.5–0.9 kPa).
  const history = await api<{ bands: BandSegment[] }>(
    request,
    'GET',
    `api/grows/${growId}/history?role=vpd&window=day`,
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
    const expected = band.light_on ? DAY_VPD : NIGHT_VPD;
    expect(band.min, 'range floor follows the light state').toBeCloseTo(expected[0]);
    expect(band.max, 'range ceiling follows the light state').toBeCloseTo(expected[1]);
  }

  // The visible half: the VPD chart draws several filled band regions
  // at different vertical offsets — the night band sits above the day
  // band (their spans match; their floors differ).
  await page.goto(url(`/grows/${growId}`));
  const svg = page.locator('svg[aria-label="vpd history"]');
  await expect(svg).toBeVisible();
  const regions = await bandRegions(svg);
  expect(
    regions.length,
    'several band regions render, not one uniform corridor',
  ).toBeGreaterThanOrEqual(2);
  const tops = new Set(regions.map((region) => Math.round(region.top)));
  expect(tops.size, 'the day and night regions sit at different heights').toBeGreaterThanOrEqual(2);
});

test('vpd band steps follow the bound light entity on/off history', async ({
  page,
  request,
}) => {
  const growId = await createGrow(request, 'E2E Stepped Fixture', {
    started_on: threeDaysAgo(),
  });
  await api(request, 'PUT', `api/grows/${growId}/bindings`, {
    bindings: {
      temperature: 'sensor.grow_temperature',
      humidity: 'sensor.grow_humidity',
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
    `api/grows/${growId}/history?role=vpd&window=day`,
  );
  const bands = history!.bands;
  expect(bands.length, 'the staircase splits the always-on schedule').toBeGreaterThanOrEqual(3);
  expect(
    bands.some((band) => band.light_on === false),
    'a dark stretch exists that the 24 h schedule cannot explain',
  ).toBe(true);
  for (const band of bands) {
    const expected = band.light_on ? DAY_VPD : NIGHT_VPD;
    expect(band.min).toBeCloseTo(expected[0]);
    expect(band.max).toBeCloseTo(expected[1]);
  }

  // Visible: the chart shows the fixture's steps as separate regions
  // with differing geometry, not the schedule's one uniform band.
  await page.goto(url(`/grows/${growId}`));
  const svg = page.locator('svg[aria-label="vpd history"]');
  await expect(svg).toBeVisible();
  const regions = await bandRegions(svg);
  expect(regions.length, 'the fixture steps are visible as regions').toBeGreaterThanOrEqual(3);
  const tops = new Set(regions.map((region) => Math.round(region.top)));
  expect(tops.size, 'the dark and lit regions sit at different heights').toBeGreaterThanOrEqual(2);
});
