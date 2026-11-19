// The Light Cycle editor (ticket 05): set, persist and reset the
// per-grow lights-on time and hours-on per phase, with the cited
// preset defaults as placeholders (photoperiod 12 h flower vs an
// autoflower's long days) and clear errors for out-of-range hours.
// The editor lives on the grow edit page since the grow-page rework
// (ticket 01). When a light entity is bound, a caption reports the
// pattern learned from its history (grow-page rework, ticket 03) —
// display only, the fields stay editable defaults.

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
  await page.goto(url(`/grows/${growId}/edit`));

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

// Undoing a reset re-PUTs the values the form showed before the
// reset, through the same save endpoint (D126).
test('a reset raises the undo banner and Undo restores the values', async ({
  page,
  request,
}) => {
  const growId = await createGrow(request, uniqueName('E2E Cycle Undo'));
  await page.goto(url(`/grows/${growId}/edit`));

  const lightsOn = page.getByLabel('Lights on time (local)');
  await lightsOn.fill('07:30');
  await page.getByLabel('Vegetative hours on').fill('20');
  await page.getByLabel('Flowering hours on').fill('11');
  await page.getByRole('button', { name: 'Apply cycle' }).click();
  await expect(page.getByText('Saved.')).toBeVisible();

  await page.getByRole('button', { name: 'Reset to default' }).click();
  // The reset itself raises the undo banner instead of a bare status.
  await expect(page.getByText('Reset to default.')).toBeVisible();
  await expect(lightsOn).toHaveValue('');

  await page.getByRole('button', { name: 'Undo' }).click();
  await expect(page.getByText('Saved.')).toBeVisible();
  await expect(lightsOn).toHaveValue('07:30');
  await expect(page.getByLabel('Vegetative hours on')).toHaveValue('20');
  await expect(page.getByLabel('Flowering hours on')).toHaveValue('11');

  const restored = await api<LightCycle>(
    request,
    'GET',
    `api/grows/${growId}/light-cycle`,
  );
  expect(restored!.lights_on).toBe('07:30');
  expect(restored!.hours_on.vegetative).toBe(20);
  expect(restored!.hours_on.flower).toBe(11);
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
  await page.goto(url(`/grows/${growId}/edit`));

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

test('the bound light captions the editor with its learned reality', async ({
  page,
  request,
}) => {
  const growId = await createGrow(request, uniqueName('E2E Cycle Reality'));
  // The mock HA's light.grow_led history is a staircase with one lit
  // stretch: on 20 h ago, off 18 h ago — two learned hours on.
  await api(request, 'PUT', `api/grows/${growId}/bindings`, {
    bindings: { light: 'light.grow_led' },
  });
  await page.goto(url(`/grows/${growId}/edit`));

  // The learned time follows the mock's wall clock, the hours do not.
  await expect(page.getByTestId('light-reality')).toHaveText(
    /Reality from: light\.grow_led · learned \d{2}:\d{2} · 2 h/,
  );

  // The learned values stay display-only: blanks still mean the
  // defaults, never a write-back from reality (ADR 0002).
  await expect(page.getByLabel('Lights on time (local)')).toHaveValue('');
  await expect(page.getByLabel('Vegetative hours on')).toHaveValue('');
});

test('binding a light through the picker captions without a reload', async ({
  page,
  request,
}) => {
  const growId = await createGrow(request, uniqueName('E2E Cycle Live'));
  await page.goto(url(`/grows/${growId}/edit`));

  // Unbound: only the editable defaults, no caption.
  await expect(page.getByTestId('light-reality')).toHaveCount(0);

  // The form's "Light tolerance" select shares the word: scope to
  // the bindings row whose label text is exactly "Light".
  const lightPicker = page
    .locator('label.fieldset', {
      has: page.getByText('Light', { exact: true }),
    })
    .getByRole('combobox');
  armNoReloadGuard(page);
  await lightPicker.selectOption('light.grow_led');
  await expectNoReload(page);
  await expect(page.getByTestId('light-reality')).toHaveText(
    /Reality from: light\.grow_led · learned \d{2}:\d{2} · 2 h/,
  );

  // Unbinding drops it again, still inside the SPA.
  await lightPicker.selectOption('');
  await expectNoReload(page);
  await expect(page.getByTestId('light-reality')).toHaveCount(0);
});

test('out-of-range hours are rejected with a clear error', async ({
  page,
  request,
}) => {
  const growId = await createGrow(request, uniqueName('E2E Cycle Bad'));
  await page.goto(url(`/grows/${growId}/edit`));

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
