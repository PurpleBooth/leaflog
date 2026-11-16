// Temperature history chart (ticket 02): the card renders through the
// ECharts option-builder seam — a labelled dual-unit value axis, time
// tick labels, gridlines, and a hover tooltip with value, time, and
// unit. The option JSON is unit-tested in Rust; these cases assert the
// rendered DOM (the wasm serialization boundary is where a broken
// option can only show up as a blank or bare chart).

import { test, expect } from '@playwright/test';
import { api, createGrow, dismissOnboarding, url } from './helpers';

test.beforeAll(async ({ request }) => {
  await dismissOnboarding(request);
});

test('temperature chart renders axes and a hover tooltip', async ({
  page,
  request,
}) => {
  const growId = await createGrow(request, 'E2E Temp Chart');

  // Bind the temperature role and wait for a live mock reading, so the
  // action snapshots below carry real values.
  await api(request, 'PUT', `api/grows/${growId}/bindings`, {
    bindings: { temperature: 'sensor.grow_temperature' },
  });
  await expect
    .poll(async () => {
      const env = await api<{ temperature_c: number | null }>(
        request,
        'GET',
        `api/grows/${growId}/env`,
      );
      return env!.temperature_c !== null;
    })
    .toBe(true);

  // Log entries auto-capture bound sensors (spec §4.2); three give the
  // chart a line to draw.
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
  const tempCard = page.locator('.card', {
    has: page.locator('svg[aria-label="temperature history"]'),
  });
  const tempSvg = tempCard.locator('svg[aria-label="temperature history"]');
  await expect(tempSvg).toBeVisible();

  // The option reached the renderer: the value axis carries its dual
  // unit name, and tick labels exist for the value and time axes.
  await expect(
    tempSvg.locator('text').filter({ hasText: '°C / °F' }),
  ).toHaveCount(1);
  expect(
    await tempSvg.locator('text').count(),
    'axis name plus tick labels rendered',
  ).toBeGreaterThanOrEqual(4);

  // The series draws (not a bare frame): the line is a path.
  await expect(tempSvg.locator('path').first()).toBeAttached();

  // Hovering the grid reveals the tooltip: value with unit plus a UTC
  // timestamp (the wasm formatter's exact shape).
  await tempSvg.scrollIntoViewIfNeeded();
  const box = await tempSvg.boundingBox();
  expect(box, 'chart laid out for hover').not.toBeNull();
  await page.mouse.move(box!.x + box!.width * 0.5, box!.y + box!.height * 0.5, {
    steps: 5,
  });
  await expect(tempCard.getByText(/°C.*UTC/)).toBeVisible();
});
