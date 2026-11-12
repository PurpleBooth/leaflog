// Forecast chart geometry: the today marker must sit in the track
// area — past the phase-label gutter — at the day-count percentage of
// the track width, aligned with the bars (grow-page correctness 05).

import { test, expect } from '@playwright/test';
import { api, armNoReloadGuard, expectNoReload, uniqueName, url } from './helpers';

test('today marker sits in the track area, past the label gutter', async ({
  page,
  request,
}) => {
  // A grow started ~10 days ago: early in a roughly 150-day forecast,
  // so the buggy geometry (percentage of the full chart width, label
  // gutter included) pins the marker inside the gutter; the fixed
  // geometry puts it at the same fraction of the track width.
  const name = uniqueName('E2E Marker Grow');
  const started = new Date(Date.now() - 10 * 24 * 60 * 60 * 1000)
    .toISOString()
    .slice(0, 10);
  const grow = await api<{ id: string }>(request, 'POST', 'api/grows', {
    name,
    strain_id: null,
    grow_space_id: null,
    seed_batch_id: null,
    feed_template_id: null,
    substrate: 'soil',
    started_on: started,
    status: 'active',
    notes: null,
  });

  await page.goto(url(`/grows/${grow!.id}`));
  await armNoReloadGuard(page);

  const chart = page.locator('div.gantt-chart');
  await expect(chart).toBeVisible();
  const marker = page.locator('div.gantt-today');
  await expect(marker).toBeVisible();

  // Loose positional assertion, not pixel-exact geometry: the marker's
  // left edge must sit beyond the label gutter (the gutter ends where
  // the phase labels start) and inside the track area (where the bars
  // begin).
  const markerBox = await marker.boundingBox();
  const labelBox = await page.locator('span.gantt-phase-label').first().boundingBox();
  const trackBox = await page.locator('div.gantt-track').first().boundingBox();
  expect(markerBox).not.toBeNull();
  expect(labelBox).not.toBeNull();
  expect(trackBox).not.toBeNull();
  expect(
    markerBox!.x,
    'the today marker must sit past the label gutter, not inside it',
  ).toBeGreaterThan(labelBox!.x);
  expect(
    markerBox!.x,
    'the today marker must sit inside the track area with the bars',
  ).toBeGreaterThanOrEqual(trackBox!.x);

  // The marker stayed on one document (no navigation re-render).
  await expectNoReload(page);
});
