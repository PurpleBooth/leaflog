// The Light Cycle editor (ticket 05): set, persist and reset the
// per-grow lights-on time and hours-on per phase, with the cited
// preset defaults as placeholders (photoperiod 12 h flower vs an
// autoflower's long days) and clear errors for out-of-range hours.

import { test, expect } from '@playwright/test';
import {
  api,
  createGrow,
  dismissOnboarding,
  expectNoReload,
  armNoReloadGuard,
  uniqueName,
  url,
} from './helpers';

interface LightCycle {
  lights_on: string | null;
  hours_on: Record<string, number | null>;
}

test.beforeAll(async ({ request }) => {
  await dismissOnboarding(request);
});

test('photoperiod defaults show, the cycle saves, persists and resets', async ({
  page,
  request,
}) => {
  const growId = await createGrow(request, uniqueName('E2E Cycle'));
  await page.goto(url(`/grows/${growId}`));

  // No strain linked: photoperiod semantics. The placeholders carry
  // the cited defaults — 18 h through veg, 12 h flower.
  const lightsOn = page.getByLabel('Lights on time (local)');
  await expect(lightsOn).toBeVisible();
  await expect(lightsOn).toHaveAttribute('placeholder', '06:00');
  await expect(page.getByLabel('Vegetative hours on')).toHaveAttribute(
    'placeholder',
    '18',
  );
  await expect(page.getByLabel('Flowering hours on')).toHaveAttribute(
    'placeholder',
    '12',
  );

  // Reset starts hidden — nothing is overridden yet.
  await expect(
    page.getByRole('button', { name: 'Reset to default' }),
  ).toHaveCount(0);

  await lightsOn.fill('07:30');
  await page.getByLabel('Vegetative hours on').fill('20');
  await page.getByLabel('Flowering hours on').fill('11');
  armNoReloadGuard(page);
  await page.getByRole('button', { name: 'Apply cycle' }).click();
  await expectNoReload(page);
  await expect(page.getByText('Saved.')).toBeVisible();

  const stored = await api<LightCycle>(
    request,
    'GET',
    `api/grows/${growId}/light-cycle`,
  );
  expect(stored!.lights_on).toBe('07:30');
  expect(stored!.hours_on.vegetative).toBe(20);
  expect(stored!.hours_on.flower).toBe(11);

  // The values survive a reload (fresh page fetch from the store).
  await page.reload();
  await expect(page.getByLabel('Lights on time (local)')).toHaveValue(
    '07:30',
  );
  await expect(page.getByLabel('Vegetative hours on')).toHaveValue('20');
  await expect(
    page.getByRole('button', { name: 'Reset to default' }),
  ).toBeVisible();

  // Reset clears the override back to the defaults.
  await page.getByRole('button', { name: 'Reset to default' }).click();
  await expect(page.getByText('Reset to default.')).toBeVisible();
  await expect(page.getByLabel('Lights on time (local)')).toHaveValue('');
  await expect(page.getByLabel('Vegetative hours on')).toHaveValue('');
  const reset = await api<LightCycle>(
    request,
    'GET',
    `api/grows/${growId}/light-cycle`,
  );
  expect(reset!.lights_on).toBeNull();
  expect(reset!.hours_on.vegetative).toBeNull();
});

test('an autoflower grow defaults to long days through flower', async ({
  page,
  request,
}) => {
  // The grow carries its photoperiod (feature strip): an autoflower
  // grow keeps long days, so the flower field prefills 18, not 12.
  const growId = await createGrow(request, uniqueName('E2E Cycle Auto'), {
    photoperiod: 'auto',
  });
  await page.goto(url(`/grows/${growId}`));

  // Autos keep long days: the flower field prefills 18, not 12.
  await expect(page.getByLabel('Flowering hours on')).toHaveAttribute(
    'placeholder',
    '18',
  );
  await expect(page.getByLabel('Vegetative hours on')).toHaveAttribute(
    'placeholder',
    '18',
  );
});

test('out-of-range hours are rejected with a clear error', async ({
  page,
  request,
}) => {
  const growId = await createGrow(request, uniqueName('E2E Cycle Bad'));
  await page.goto(url(`/grows/${growId}`));

  await page.getByLabel('Flowering hours on').fill('30');
  armNoReloadGuard(page);
  await page.getByRole('button', { name: 'Apply cycle' }).click();
  await expectNoReload(page);
  await expect(
    page.getByText(/hours_on for flower must be 1–24/),
  ).toBeVisible();

  // Nothing was stored: the cycle stays on the defaults.
  const stored = await api<LightCycle>(
    request,
    'GET',
    `api/grows/${growId}/light-cycle`,
  );
  expect(stored!.lights_on).toBeNull();
  expect(stored!.hours_on.flower).toBeNull();
});
