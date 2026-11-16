// Env strip layout (grow-overview ticket 09): the four reading tiles
// (Temp, RH, VPD, RH window) share one row that spans the full
// container width with equal tiles on desktop, wrap to two columns on
// phones and a single column on very narrow screens — and never spill
// into horizontal overflow. Live mock-HA sensors feed the tiles so the
// assertions run on real values (same setup as 07-grow-page).

import { test, expect, type Page } from '@playwright/test';
import { api, createGrow, uniqueName, url } from './helpers';

/** The four tile labels, in strip order. */
const LABELS = ['Temp', 'RH', 'VPD', 'RH window'];

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

/** Asserts nothing spills past the viewport sideways. */
async function expectNoHorizontalOverflow(page: Page) {
  const overflow = await page.evaluate(
    () =>
      document.documentElement.scrollWidth -
      document.documentElement.clientWidth,
  );
  expect(overflow, 'no horizontal overflow').toBeLessThanOrEqual(0);
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

  // Desktop: all four tiles in one row, equally wide, spanning the
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
  const lastRight = boxes[3].x + boxes[3].width;
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
