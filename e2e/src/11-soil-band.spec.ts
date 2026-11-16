// The soil-moisture band preset (ticket 07): editable on the grow
// overview, persisted under settings, reset back to the sourced
// default. Since ticket 03 the editor lives inside the Soil moisture
// chart card, so every assertion scopes to that card — the accessible
// field labels are unchanged.

import { test, expect } from '@playwright/test';
import { api, dismissOnboarding, url } from './helpers';

test.beforeAll(async ({ request }) => {
  await dismissOnboarding(request);
});

test('soil band saves, persists and resets to the default', async ({
  page,
  request,
}) => {
  await page.goto(url('/'));

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
