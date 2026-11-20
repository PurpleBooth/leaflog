// Night shading behind the Air charts (ticket 05): the light story
// rides the Air-group charts as a shaded back layer — the bound
// light's observed history when a light entity is bound, the Light
// Cycle schedule otherwise — while the Root-zone charts stay clean
// and the standalone Light card is gone. Light Cycle edits reach the
// shading on the next page load, and the Air group's label carries
// the legend hint naming the tint.

import { expect, test, type Locator, type Page } from '@playwright/test';
import { api, createGrow, dismissOnboarding, nightShades, revealRootZone, url } from './helpers';

test.beforeAll(async ({ request }) => {
  await dismissOnboarding(request);
});

/** One chart's svg by role. */
function chartSvg(page: Page, role: string): Locator {
  return page.locator(`svg[aria-label="${role} history"]`);
}

/** Puts one explicit Light Cycle override, every phase at the same
 *  hours — the grow's phase then cannot leak a preset default into
 *  the assertion. */
async function putCycle(
  request: import('@playwright/test').APIRequestContext,
  growId: string,
  lightsOn: string,
  hoursOn: number,
): Promise<void> {
  await api(request, 'PUT', `api/grows/${growId}/light-cycle`, {
    lights_on: lightsOn,
    hours_on: {
      germination: hoursOn,
      seedling: hoursOn,
      vegetative: hoursOn,
      flower: hoursOn,
    },
  });
}

test('a bound light shades the observed nights behind the Air charts only', async ({
  page,
  request,
}) => {
  const growId = await createGrow(request, 'E2E Night Bound');
  await api(request, 'PUT', `api/grows/${growId}/bindings`, {
    bindings: {
      temperature: 'sensor.grow_temperature',
      humidity: 'sensor.grow_humidity',
      soil_moisture: 'sensor.grow_soil_moisture',
      light: 'light.grow_led',
    },
  });
  await page.goto(url(`/grows/${growId}`));
  await revealRootZone(page);

  // The mock's light staircase runs off → on → off anchored at
  // now−23 h / −20 h / −18 h, so the rolling week window holds at
  // least the two observed dark stretches behind the Air charts.
  const temperature = chartSvg(page, 'temperature');
  await expect(temperature).toBeVisible();
  expect(
    (await nightShades(temperature)).length,
    'observed dark stretches shade the Air chart',
  ).toBeGreaterThanOrEqual(2);

  // The Root zone stays clean: soil moisture renders through the
  // same section but carries no night layer.
  const soil = chartSvg(page, 'soil_moisture');
  await expect(soil).toBeVisible();
  expect(await nightShades(soil), 'no night layer on the Root zone').toEqual([]);

  // The standalone Light card is gone: its story rides the shading.
  await expect(page.locator('svg[aria-label="light history"]')).toHaveCount(0);
});

test('an unbound light shades the Light Cycle, and edits reach the next load', async ({
  page,
  request,
}) => {
  // Started eight days ago, so the whole week window sits inside
  // the grow's lifetime: every local day then carries the cycle's
  // own light/dark rhythm, and no pre-start stretch (which the
  // schedule shades as dark whatever the cycle) can dilute the
  // counts.
  const started = new Date(Date.now() - 8 * 24 * 3600 * 1000)
    .toISOString()
    .slice(0, 10);
  const growId = await createGrow(request, 'E2E Night Cycle', {
    started_on: started,
  });
  await api(request, 'PUT', `api/grows/${growId}/bindings`, {
    bindings: { temperature: 'sensor.grow_temperature' },
  });

  // A 12 h cycle leaves a dark half-day: the week window holds a
  // night per local day behind the Air chart.
  await putCycle(request, growId, '06:00', 12);
  await page.goto(url(`/grows/${growId}`));
  const temperature = chartSvg(page, 'temperature');
  await expect(temperature).toBeVisible();
  expect(
    (await nightShades(temperature)).length,
    'the cycle schedule shades a night per day',
  ).toBeGreaterThanOrEqual(5);

  // Always-on: no dark stretch anywhere in the grow's lifetime, so
  // the shading disappears on the next load — the cycle edit
  // reached the chart.
  await putCycle(request, growId, '06:00', 24);
  await page.reload();
  await expect(temperature).toBeVisible();
  expect(await nightShades(temperature), '24 h on leaves no night').toEqual([]);

  // And a shorter cycle brings the nights back the same way.
  await putCycle(request, growId, '06:00', 12);
  await page.reload();
  await expect(temperature).toBeVisible();
  expect(
    (await nightShades(temperature)).length,
    'the shading returns with the cycle',
  ).toBeGreaterThanOrEqual(5);
});
