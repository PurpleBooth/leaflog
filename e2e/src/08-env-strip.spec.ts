// Env strip layout (grow-overview ticket 09, live values ticket 02):
// the six reading tiles (Temp, RH, VPD, RH window, CO₂, soil
// moisture) share one row that spans the full container width with
// equal tiles on desktop, wrap to two columns on phones and a single
// column on very narrow screens — and never spill into horizontal
// overflow. Live mock-HA sensors feed the tiles so the assertions run
// on real values (same setup as 07-grow-page); the CO₂ and soil tiles
// stay present even when their roles are unbound (they read "—").

import { test, expect, type Page } from '@playwright/test';
import { api, createGrow, uniqueName, url } from './helpers';

/** The six tile labels, in strip order. */
const LABELS = ['Temp', 'RH', 'VPD', 'RH window', 'CO₂', 'Soil moisture'];

/** The bounding box of one tile, found by its exact label. Scoped to
 * the strip — role charts carry the same labels ("VPD", "RH"). */
async function tileBox(page: Page, label: string) {
  const box = await page
    .locator('#env-strip')
    .getByText(label, { exact: true })
    .locator('xpath=..')
    .boundingBox();
  expect(box, `tile ${label} rendered`).not.toBeNull();
  return box!;
}

/** The bounding box of the strip (the tiles' shared grid parent). */
async function stripBox(page: Page) {
  const box = await page
    .locator('#env-strip')
    .getByText(LABELS[0], { exact: true })
    .locator('xpath=../..')
    .boundingBox();
  expect(box, 'strip container rendered').not.toBeNull();
  return box!;
}

/** Asserts nothing spills past the viewport sideways.
 *
 * Settle-tolerant on purpose: after a viewport change the chart's
 * resize handler re-renders asynchronously, so a freshly resized ECharts
 * SVG can still carry its pre-resize width for a frame. The poll only
 * waits for that re-render — a chart that never shrinks still fails
 * (the stale-width bug class this assertion guards). */
async function expectNoHorizontalOverflow(page: Page) {
  await expect
    .poll(
      async () =>
        page.evaluate(
          () =>
            document.documentElement.scrollWidth -
            document.documentElement.clientWidth,
        ),
      { message: 'no horizontal overflow' },
    )
    .toBeLessThanOrEqual(0);
}

test('env strip spans the container and wraps without overflow', async ({
  page,
  request,
}) => {
  const name = uniqueName('E2E Env Strip Grow');
  const growId = await createGrow(request, name);

  // Bind live sensors and poll until the snapshot carries real
  // readings, so the tiles render values instead of dashes.
  await api(request, 'PUT', `api/grows/${growId}/bindings`, {
    bindings: {
      temperature: 'sensor.grow_temperature',
      humidity: 'sensor.grow_humidity',
    },
  });
  await expect
    .poll(
      async () => {
        const env = await api<{
          bindings: Array<{ role: string; available: boolean }>;
          temperature_c: number | null;
          humidity_pct: number | null;
        }>(request, 'GET', `api/grows/${growId}/env`);
        return (
          env!.bindings.every((binding) => binding.available) &&
          env!.temperature_c !== null &&
          env!.humidity_pct !== null
        );
      },
      { timeout: 10_000 },
    )
    .toBe(true);

  await page.goto(url(`/grows/${growId}`));
  await expect(page.getByText('RH window')).toBeVisible();

  // The bound roles' charts render on the overview; unbound roles
  // render no card at all (ticket 04), so no placeholder text either.
  await expect(
    page.locator('svg[aria-label="temperature history"]'),
  ).toBeVisible();
  await expect(page.locator('svg[aria-label="humidity history"]')).toBeVisible();
  await expect(page.locator('svg[aria-label="vpd history"]')).toBeVisible();
  await expect(
    page.getByText('No sensor bound for this role.'),
  ).toHaveCount(0);

  // Chart window toggles read 24h / 7d / 30d (ticket 02): rolling
  // windows ending now, never a calendar day. Only the bound roles
  // carry cards (temperature, humidity, and VPD derived from both —
  // ticket 04's visibility rule), so three toggle groups.
  const chartCards = page.locator('.card', {
    has: page.getByRole('group', { name: 'Chart window' }),
  });
  await expect(chartCards).toHaveCount(3);
  await expect(
    page.getByRole('button', { name: /^(Day|Week|Month)$/ }),
  ).toHaveCount(0);
  await expect(
    page.getByRole('button', { name: '24h', exact: true }),
  ).toHaveCount(3);
  await expect(
    page.getByRole('button', { name: '7d', exact: true }),
  ).toHaveCount(3);
  await expect(
    page.getByRole('button', { name: '30d', exact: true }),
  ).toHaveCount(3);

  // Switching a Chart window refetches and re-renders (the
  // temperature card now goes through ECharts; ticket 02).
  const tempCard = page.locator('.card', {
    has: page.locator('svg[aria-label="temperature history"]'),
  });
  await tempCard.getByRole('button', { name: '7d', exact: true }).click();
  await expect(
    tempCard.getByRole('button', { name: '7d', exact: true }),
  ).toHaveClass(/btn-primary/);
  await expect(
    page.locator('svg[aria-label="temperature history"]'),
  ).toBeVisible();

  // Desktop: all six tiles in one row, equally wide, spanning the
  // strip edge to edge.
  const strip = await stripBox(page);
  const boxes = await Promise.all(
    LABELS.map(async (label) => tileBox(page, label)),
  );
  const tops = boxes.map((box) => box.y);
  expect(
    Math.max(...tops) - Math.min(...tops),
    'desktop tiles share one row',
  ).toBeLessThanOrEqual(2);
  const widths = boxes.map((box) => box.width);
  expect(
    Math.max(...widths) - Math.min(...widths),
    'desktop tiles stretch equally',
  ).toBeLessThanOrEqual(1);
  expect(
    Math.abs(strip.x - boxes[0].x),
    'strip starts at its first tile',
  ).toBeLessThanOrEqual(2);
  const lastRight = boxes[5].x + boxes[5].width;
  expect(
    Math.abs(strip.x + strip.width - lastRight),
    'strip ends at its last tile (full container width)',
  ).toBeLessThanOrEqual(2);
  await expectNoHorizontalOverflow(page);

  // Phone width: the strip wraps to two columns, still without
  // horizontal overflow.
  await page.setViewportSize({ width: 390, height: 844 });
  await expect(page.getByText('RH window')).toBeVisible();
  await expectNoHorizontalOverflow(page);
  const phoneStrip = await stripBox(page);
  const phoneBoxes = await Promise.all(
    LABELS.map(async (label) => tileBox(page, label)),
  );
  for (const box of phoneBoxes) {
    expect(
      box.width,
      'phone tiles sit two per row',
    ).toBeLessThanOrEqual(phoneStrip.width * 0.6);
  }

  // Very narrow: one column, still without horizontal overflow.
  await page.setViewportSize({ width: 320, height: 480 });
  await expect(page.getByText('RH window')).toBeVisible();
  await expectNoHorizontalOverflow(page);
  const thinStrip = await stripBox(page);
  const thinBoxes = await Promise.all(
    LABELS.map(async (label) => tileBox(page, label)),
  );
  for (const box of thinBoxes) {
    expect(
      Math.abs(box.width - thinStrip.width),
      'very narrow tiles take the full row',
    ).toBeLessThanOrEqual(2);
  }
});

test('an aborted history fetch keeps the bound card and names the failure', async ({
  page,
  request,
}) => {
  const growId = await createGrow(
    request,
    uniqueName('E2E Fetch Fail Grow'),
  );
  await api(request, 'PUT', `api/grows/${growId}/bindings`, {
    bindings: {
      temperature: 'sensor.grow_temperature',
      humidity: 'sensor.grow_humidity',
    },
  });
  await page.goto(url(`/grows/${growId}`));
  const tempCard = page.locator('.card', {
    has: page.locator('svg[aria-label="temperature history"]'),
  });
  await expect(tempCard).toBeVisible();

  // Abort the temperature role's history fetches, then switch the
  // window: the fetch fails, and the bound card stays — with the
  // degraded banner naming the failure. Hiding is the unbound state;
  // a bound card never vanishes on a failed fetch.
  const historyRoute = url(`api/grows/${growId}/history*role=temperature*`);
  await page.route(historyRoute, (route) => route.abort());
  await tempCard.getByRole('button', { name: '7d', exact: true }).click();
  await expect(tempCard.getByRole('alert')).toContainText(
    'History unavailable',
  );
  await expect(
    page.locator('svg[aria-label="temperature history"]'),
  ).toBeVisible();

  // Lifting the abort recovers without a reload: the next window
  // switch retries, the banner clears, the chart redraws.
  await page.unroute(historyRoute);
  await tempCard.getByRole('button', { name: '24h', exact: true }).click();
  await expect(tempCard.getByRole('alert')).toHaveCount(0);
  await expect(
    page.locator('svg[aria-label="temperature history"]'),
  ).toBeVisible();
});
