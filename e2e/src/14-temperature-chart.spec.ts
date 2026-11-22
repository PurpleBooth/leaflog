// Temperature history chart (ticket 02): the card renders through the
// ECharts option-builder seam — a labelled dual-unit value axis, time
// tick labels, gridlines, and a hover tooltip with value, time, and
// unit. The option JSON is unit-tested in Rust; these cases assert the
// rendered DOM (the wasm serialization boundary is where a broken
// option can only show up as a blank or bare chart). Ticket 06 adds
// the tolerance-derived Target Band: the same thresholds the section's
// tiles colour by, shaded flat behind the line (no phase or light
// stepping — VPD carries that contract).

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

  // The axis names the selected unit; tick labels also exist.
  await expect(tempSvg.getByText('°C', { exact: true })).toHaveCount(1);
  await expect(tempSvg.getByText('°F', { exact: false })).toHaveCount(0);
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

test('temperature chart draws the tolerance Target Band', async ({
  page,
  request,
}) => {
  // Default tolerances (average/average): the band edges are the
  // 60–80 °F alert thresholds, in °C — flat, whatever the phase.
  const growId = await createGrow(request, 'E2E Temp Band');
  await api(request, 'PUT', `api/grows/${growId}/bindings`, {
    bindings: { temperature: 'sensor.grow_temperature' },
  });
  await api(request, 'POST', `api/grows/${growId}/actions`, {
    plant_ids: [],
    occurred_at: new Date().toISOString(),
    action_type: 'note',
    details: {},
    notes: 'reading',
    appearance_tags: [],
  });
  await page.waitForTimeout(1_200);

  const history = await api<{ bands: BandSegment[] }>(
    request,
    'GET',
    `api/grows/${growId}/history?role=temperature&window=day`,
  );
  expect(history!.bands.length, 'one flat band spans the window').toBeGreaterThanOrEqual(1);
  for (const band of history!.bands) {
    expect(band.light_on, 'the band carries no light state').toBeNull();
    expect(band.min).toBeCloseTo(15.56, 1);
    expect(band.max).toBeCloseTo(26.67, 1);
  }

  // Visible: the shading renders behind the line as filled regions.
  await page.goto(url(`/grows/${growId}`));
  const tempSvg = page.locator('svg[aria-label="temperature history"]');
  await expect(tempSvg).toBeVisible();
  expect(await bandRegions(tempSvg)).toHaveLength(1);

  // A heat-sensitive grow draws a tighter ceiling (−3 °F ≈ −1.67 °C):
  // the band follows the grow's tolerances, not a fixed preset.
  const sensitiveId = await createGrow(request, 'E2E Temp Band Sensitive', {
    heat_tolerance: 'sensitive',
  });
  await api(request, 'PUT', `api/grows/${sensitiveId}/bindings`, {
    bindings: { temperature: 'sensor.grow_temperature' },
  });
  const sensitive = await api<{ bands: BandSegment[] }>(
    request,
    'GET',
    `api/grows/${sensitiveId}/history?role=temperature&window=day`,
  );
  for (const band of sensitive!.bands) {
    expect(band.max, 'sensitive heat pulls the ceiling down').toBeCloseTo(25.0, 1);
    expect(band.min, 'an average cold tolerance keeps the floor').toBeCloseTo(15.56, 1);
  }
});
