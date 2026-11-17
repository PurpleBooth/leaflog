// HA entity ids (ticket 10): one long lowercase token must wrap
// inside its card instead of overflowing the layout, with the full
// id on hover. The id now surfaces as text in the grow page's drift
// banner for a dead binding (the equipment page that used to show it
// is gone — feature-strip), so that badge carries the treatment.

import { test, expect } from '@playwright/test';
import { api, dismissOnboarding, uniqueName, url } from './helpers';

test.beforeAll(async ({ request }) => {
  await dismissOnboarding(request);
});

test('a long ha entity id wraps inside its card', async ({ page, request }) => {
  const name = uniqueName('E2E Long Entity');
  const longId =
    'sensor.growhub_e42a_inside_humidity_temperature_sensor_of_doom';
  const growId = await api<{ id: string }>(request, 'POST', 'api/grows', {
    name,
    feed_template_id: null,
    substrate: 'soil',
    started_on: new Date().toISOString().slice(0, 10),
    status: 'active',
    notes: null,
  }).then((grow) => grow!.id);
  // The entity is unknown to mock HA, so the binding reads as dead and
  // the drift banner renders it — with the wrap-anywhere treatment.
  await api(request, 'PUT', `api/grows/${growId}/bindings`, {
    bindings: { temperature: longId },
  });

  await page.goto(url(`/grows/${growId}`));
  // The full unbroken id is on hover.
  const badge = page.locator('span.entity-id[title="' + longId + '"]');
  await expect(badge).toBeVisible();
  await expect(badge).toContainText(longId);

  // And the wrapped text keeps the page free of horizontal overflow.
  const overflow = await page.evaluate(
    () =>
      document.documentElement.scrollWidth -
      document.documentElement.clientWidth,
  );
  expect(overflow, 'no horizontal overflow').toBeLessThanOrEqual(0);
});
