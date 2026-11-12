// Grow-page banner truth (the v0.4.0 inversion regression): a grow
// whose every binding is available never shows the "No data" banner —
// neither while the env snapshot is still loading nor after it lands,
// and the readings strip renders as before. The dead-binding positive
// case lives at the backend integration seam
// (`unavailable_bound_entities_render_no_data_and_can_rebind`): mock HA
// cannot serve an `unavailable` entity, so the e2e suite covers only
// the healthy absence.

import { test, expect } from '@playwright/test';
import { api, createGrow, uniqueName, url } from './helpers';

/** The "No data" drift banner (role=status; errors use role=alert). */
function banner(page: import('@playwright/test').Page) {
  return page.locator('p[role="status"]', { hasText: 'No data:' });
}

test('healthy bindings show no No-data banner, at load or after load', async ({
  page,
  request,
}) => {
  const name = uniqueName('E2E Healthy Grow');
  const growId = await createGrow(request, name);

  // Bind live mock-HA sensors so "healthy" is not vacuous: every
  // binding present and available, readings on screen (the exact
  // state the inverted condition lied about).
  await api(request, 'PUT', `api/grows/${growId}/bindings`, {
    bindings: {
      temperature: 'sensor.grow_temperature',
      humidity: 'sensor.grow_humidity',
    },
  });
  // PUTting bindings resubscribes the bridge on a fresh WS connection
  // (teardown clears the live map, then the resubscribed stream seeds
  // it), so an immediate env fetch can legitimately still see both
  // bindings dead: poll until the initial states have landed.
  await expect
    .poll(
      async () => {
        const env = await api<{
          bindings: Array<{ role: string; available: boolean }>;
        }>(request, 'GET', `api/grows/${growId}/env`);
        return env!.bindings;
      },
      { timeout: 10_000 },
    )
    // The healthy case must not be vacuous: both live bindings above
    // present AND available once the poll settles.
    .toEqual(
      expect.arrayContaining([
        expect.objectContaining({ role: 'temperature', available: true }),
        expect.objectContaining({ role: 'humidity', available: true }),
      ]),
    );

  await page.goto(url(`/grows/${growId}`));
  await expect(page.getByRole('heading', { name, exact: true })).toBeVisible();

  // While the snapshot is still in flight the page must stay quiet.
  await expect(banner(page)).toHaveCount(0);

  // Snapshot landed (readings strip rendered): still no banner, and a
  // real temperature reading next to it.
  await expect(page.getByText('RH window')).toBeVisible();
  await expect(banner(page)).toHaveCount(0);
  await expect(page.getByText('°C /')).toBeVisible();
});
