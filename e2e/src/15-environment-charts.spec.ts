// Environment charts (ticket 03): humidity, VPD, the light staircase
// and soil moisture render through the ECharts option-builder seam —
// a value axis named with the role's unit, time tick labels, and a
// hover tooltip with value, unit, and UTC time. CO₂ has no mock
// sensor, so it asserts the ticket 04 card-visibility rule: an
// unbound role renders no card at all. The pH/EC pair scatters are
// ticket 04's own flow (12-ph-ec.spec.ts).

import { expect, test, type Locator, type Page } from '@playwright/test';
import { api, createGrow, dismissOnboarding, url } from './helpers';

test.beforeAll(async ({ request }) => {
  await dismissOnboarding(request);
});

/** Hovers the middle of one chart's SVG so the axis tooltip snaps. */
async function hoverCenter(page: Page, svg: Locator) {
  await svg.scrollIntoViewIfNeeded();
  const box = await svg.boundingBox();
  expect(box, 'chart laid out for hover').not.toBeNull();
  await page.mouse.move(box!.x + box!.width * 0.5, box!.y + box!.height * 0.5, {
    steps: 5,
  });
}

test('humidity, vpd, light and soil charts draw axes and tooltips', async ({
  page,
  request,
}) => {
  const growId = await createGrow(request, 'E2E Env Charts');

  // Bind every role the mock can feed: three numeric sensors and the
  // light entity. VPD derives from the temperature + humidity pairs.
  await api(request, 'PUT', `api/grows/${growId}/bindings`, {
    bindings: {
      temperature: 'sensor.grow_temperature',
      humidity: 'sensor.grow_humidity',
      soil_moisture: 'sensor.grow_soil_moisture',
      light: 'light.grow_led',
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

  // Three log entries auto-capture every bound role (spec §4.2), so
  // each chart gets in-window points.
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

  const cardOf = (label: string) =>
    page.locator('.card', { has: page.locator(`svg[aria-label="${label}"]`) });

  // Humidity: the % axis name plus tick labels, then a tooltip with
  // the percent value and the UTC time.
  const humiditySvg = page.locator('svg[aria-label="humidity history"]');
  await expect(humiditySvg).toBeVisible();
  expect(
    await humiditySvg
      .locator('text')
      .filter({ hasText: '%' })
      .count(),
    'percent axis name plus tick labels',
  ).toBeGreaterThanOrEqual(2);
  await hoverCenter(page, humiditySvg);
  await expect(cardOf('humidity history').getByText(/%.*UTC/)).toBeVisible();

  // VPD: the kPa axis and the same tooltip contract.
  const vpdSvg = page.locator('svg[aria-label="vpd history"]');
  await expect(vpdSvg).toBeVisible();
  await expect(
    vpdSvg.locator('text').filter({ hasText: 'kPa' }).first(),
  ).toBeAttached();
  await hoverCenter(page, vpdSvg);
  await expect(cardOf('vpd history').getByText(/kPa.*UTC/)).toBeVisible();

  // Light: the staircase draws as a step series; its axis reads
  // on / off and the tooltip speaks the state, not a bare number.
  const lightSvg = page.locator('svg[aria-label="light history"]');
  await expect(lightSvg).toBeVisible();
  await expect(
    lightSvg.locator('text').filter({ hasText: 'on / off' }).first(),
  ).toBeAttached();
  await hoverCenter(page, lightSvg);
  await expect(cardOf('light history').getByText(/(on|off).*UTC/)).toBeVisible();

  // Soil moisture: the % axis and tooltip, with the band editor in
  // the same card (its flow is 11-soil-band.spec.ts).
  const soilSvg = page.locator('svg[aria-label="soil_moisture history"]');
  await expect(soilSvg).toBeVisible();
  await expect(
    soilSvg.locator('text').filter({ hasText: '%' }).first(),
  ).toBeAttached();
  await hoverCenter(page, soilSvg);
  await expect(cardOf('soil_moisture history').getByText(/%.*UTC/)).toBeVisible();
  await expect(
    cardOf('soil_moisture history').getByLabel('Band min %'),
  ).toBeVisible();

  // CO₂ has no mock sensor: the card-visibility rule (ticket 04)
  // renders no card at all for the unbound role — no heading, no
  // svg, no placeholder text anywhere on the page.
  await expect(
    page.locator('.card', {
      has: page.getByRole('heading', { name: 'CO₂' }),
    }),
  ).toHaveCount(0);
  await expect(
    page.getByText('No sensor bound for this role.'),
  ).toHaveCount(0);
});
