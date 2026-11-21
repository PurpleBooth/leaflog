// Grow-page dead-tile truth (the v0.4.0 inversion regression): a grow
// whose every binding is available never shows a dead-binding warning
// mark in the readings strip — neither while the env snapshot is still
// loading nor after it lands. The dead-binding positive case lives at
// the backend integration seam
// (`unavailable_bound_entities_render_no_data_and_can_rebind`) and in
// spec 18 (mock HA cannot serve an `unavailable` entity, so this spec
// covers only the healthy absence).

import { test, expect } from '@playwright/test';
import { api, createGrow, uniqueName, url } from './helpers';

/** A dead-binding warning mark inside the env strip. */
function deadMark(page: import('@playwright/test').Page) {
  return page.locator('#env-strip').locator('a.link-warning');
}

test('healthy bindings show no dead-tile marks, at load or after load', async ({
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
  await expect(deadMark(page)).toHaveCount(0);

  // Snapshot landed (readings strip rendered): still no marks, and a
  // real temperature reading next to them. Scoped to the strip — the
  // history chart's axis labels carry the same "°C /" text (ticket 05).
  // The RH window footnote proves the strip's full shape (D136).
  await expect(page.locator('#env-strip').getByText(/band: \d+–\d+ %/)).toBeVisible();
  await expect(deadMark(page)).toHaveCount(0);
  await expect(page.locator('#env-strip').getByText('°C /')).toBeVisible();
});
