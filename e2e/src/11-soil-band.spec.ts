// The soil-moisture band preset (ticket 07): editable on the grow
// overview, persisted under settings, reset back to the sourced
// default. Since ticket 03 the editor lives inside the Soil moisture
// chart card, so every assertion scopes to that card — the accessible
// field labels are unchanged. Since ticket 04 the card only renders
// when the role is bound, so the flow runs on a dedicated grow with
// the mock sensor bound (the landing page's pick is not deterministic
// under the parallel suite).

import { test, expect } from '@playwright/test';
import { api, createGrow, dismissOnboarding, url } from './helpers';

test.beforeAll(async ({ request }) => {
  await dismissOnboarding(request);
});

test('soil band saves, persists and resets to the default', async ({
  page,
  request,
}) => {
  const growId = await createGrow(request, 'E2E Soil Band');
  await api(request, 'PUT', `api/grows/${growId}/bindings`, {
    bindings: { soil_moisture: 'sensor.grow_soil_moisture' },
  });
  await page.goto(url(`/grows/${growId}`));

  // The editor renders inside the soil moisture card (ticket 03).
  const soilCard = page.locator('.card', {
    has: page.getByRole('heading', { name: 'Soil moisture' }),
  });
  const minInput = soilCard.getByLabel('Band min %');
  await expect(minInput).toBeVisible();
  await minInput.fill('25');
  await soilCard.getByLabel('Band max %').fill('45');
  await soilCard.getByRole('button', { name: 'Apply band' }).click();
  await expect(page.getByText('Saved.')).toBeVisible();

  // The override is stored in settings and survives a reload.
  const settings = await api<Record<string, unknown>>(
    request,
    'GET',
    'api/settings',
  );
  const band = settings!['soil_moisture_band'] as { min: number };
  expect(band.min).toBe(25);
  await page.reload();
  await expect(
    page
      .locator('.card', {
        has: page.getByRole('heading', { name: 'Soil moisture' }),
      })
      .getByLabel('Band min %'),
  ).toHaveValue('25');

  // Reset clears the override back to the sourced default.
  await page.getByRole('button', { name: 'Reset to default' }).click();
  await expect(page.getByText('Reset to default.')).toBeVisible();
  const reset = await api<Record<string, unknown>>(
    request,
    'GET',
    'api/settings',
  );
  expect(reset!['soil_moisture_band']).toBeFalsy();
});
