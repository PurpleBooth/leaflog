// HA entity ids (ticket 10): one long lowercase token must wrap
// inside its card instead of overflowing the layout, with the full
// id on hover.

import { test, expect } from '@playwright/test';
import { api, dismissOnboarding, uniqueName, url } from './helpers';

test.beforeAll(async ({ request }) => {
  await dismissOnboarding(request);
});

test('a long ha entity id wraps inside its card', async ({ page, request }) => {
  const name = uniqueName('E2E Long Entity');
  const longId =
    'sensor.growhub_e42a_inside_humidity_temperature_sensor_of_doom';
  const created = await api<{ id: string }>(request, 'POST', 'api/equipment', {
    name,
    category: 'sensor',
    brand_model: null,
    real_watts: null,
    dimmable: false,
    purchase_date: null,
    purchase_price: null,
    currency: null,
    notes: null,
    ha_entity_id: longId,
    ha_device_class: 'temperature',
  });
  expect(created!.id).toBeTruthy();

  await page.goto(url('/equipment'));
  const card = page.locator('li.card', { hasText: name });
  await expect(card).toBeVisible();

  // The full unbroken id is on hover.
  const dd = card.locator('dd.entity-id');
  await expect(dd).toHaveAttribute('title', longId);

  // And the wrapped text keeps the page free of horizontal overflow.
  const overflow = await page.evaluate(
    () =>
      document.documentElement.scrollWidth -
      document.documentElement.clientWidth,
  );
  expect(overflow, 'no horizontal overflow').toBeLessThanOrEqual(0);
});
