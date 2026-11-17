// First-run onboarding wizard (runs first: the fresh database shows it,
// and finishing it dismisses the wizard for every later spec).
//
// The wizard shows exactly once per database — the first test that
// creates a grow takes that run — so the skip path runs before the
// happy path and resets the dismissal flag for it.

import { test, expect } from '@playwright/test';
import { api, armNoReloadGuard, expectNoReload, uniqueName, url } from './helpers';

test('wizard skips clean: every step optional, nothing picked is nothing bound', async ({
  page,
  request,
}) => {
  await page.goto(url('/'));
  const wizard = page.locator('section[aria-label="First-run setup"]');
  await expect(wizard).toBeVisible();

  // Skip the bind step, then dismiss on plants: no grow, no bindings,
  // and the dismissal is the only trace (every step gates nothing).
  await wizard.getByRole('button', { name: 'Skip this step' }).click();
  await expect(wizard.getByRole('heading', { name: '2. Plants' })).toBeVisible();
  await wizard
    .getByRole('button', { name: "Dismiss — I'll explore on my own" })
    .click();
  await expect(wizard).toHaveCount(0);

  const grows = await api<Array<{ id: string }>>(request, 'GET', 'api/grows');
  expect(grows).toHaveLength(0);
  const state = await api<{ dismissed: boolean; has_grows: boolean }>(
    request,
    'GET',
    'api/onboarding',
  );
  expect(state!.dismissed).toBe(true);
  expect(state!.has_grows).toBe(false);

  // Restart for the happy path below: the empty-state card's API twin.
  await api(request, 'PUT', 'api/settings', { onboarding_dismissed: false });
});

test('wizard happy path: sensors, grow and plants land on the dashboard', async ({
  page,
}) => {
  const grow = uniqueName('E2E Wizard Grow');

  await page.goto(url('/'));
  const wizard = page.locator('section[aria-label="First-run setup"]');
  await expect(wizard).toBeVisible();

  // Step 1: bind one sensor — the temperature role gets the mock's
  // entity; the pick must land on the created grow as its binding.
  // Address the select by role + anchored name: an accessible name
  // carries the option texts, so a plain 'Temperature' substring
  // matches every role picker once the entity list lands.
  await wizard
    .getByRole('combobox', { name: /^Temperature/ })
    .selectOption('sensor.grow_temperature');
  await wizard.getByRole('button', { name: 'Next' }).click();
  await expect(wizard.getByRole('heading', { name: '2. Plants' })).toBeVisible();

  // Feature-strip, final shape: the bind step and the plants step are
  // the whole wizard (D075 removed the strain step — the label and
  // photoperiod sit inline here). No space or strain form anywhere.
  await expect(page.getByLabel('Width (cm)')).toHaveCount(0);
  await expect(wizard.getByText('Strain', { exact: true })).toHaveCount(0);

  // Step 2: the wizard's one real form. "Start the grow" must be
  // intercepted by the app, never a native submit (D061).
  await page.getByLabel('Grow name').fill(grow);
  await page.getByLabel('Strain label').fill('Northern Lights #5');
  await page.getByLabel('Photoperiod').selectOption('auto');
  await page.getByLabel('Name prefix').fill('Wiz');
  await page.getByLabel('Plants', { exact: true }).fill('2');
  await armNoReloadGuard(page);
  await wizard.getByRole('button', { name: 'Start the grow' }).click();

  // Finishing navigates to the new grow's dashboard, inside the prefix.
  await expect(page.getByRole('heading', { name: grow, exact: true })).toBeVisible();
  expect(page.url()).toContain(url('/grows/'));
  await expect(page.getByRole('link', { name: 'Wiz 1', exact: true })).toBeVisible();
  await expect(page.getByRole('link', { name: 'Wiz 2', exact: true })).toBeVisible();
  await expectNoReload(page);

  // The step-1 pick persisted: the grow page's binding picker shows the
  // bound entity (the API is the independent source of truth).
  const growId = new URL(page.url()).pathname.split('/').pop()!;
  const bindings = await api<Record<string, string>>(
    page.context().request,
    'GET',
    `api/grows/${growId}/bindings`,
  );
  expect(bindings!.temperature).toBe('sensor.grow_temperature');

  // The inline strain facts carried into the created grow: the label
  // badges the dashboard header.
  await expect(page.getByText('Northern Lights #5')).toBeVisible();

  // Everything persisted: a reload of the same deep link shows the
  // grow and both plants again.
  await page.reload();
  await expect(page.getByRole('heading', { name: grow, exact: true })).toBeVisible();
  await expect(page.getByRole('link', { name: 'Wiz 1', exact: true })).toBeVisible();

  // The wizard is dismissed for good.
  await page.goto(url('/'));
  await expect(wizard).toHaveCount(0);
});

test('nav has no Equipment or Spaces entries and the pages are gone', async ({
  page,
}) => {
  await page.goto(url('/'));
  const nav = page.locator('nav[aria-label="Main"]');
  await expect(nav.getByRole('link', { name: 'Equipment' })).toHaveCount(0);
  await expect(nav.getByRole('link', { name: 'Spaces' })).toHaveCount(0);

  // The old deep links render the SPA shell but none of the removed
  // pages' forms (unknown routes fall back to the shell, not a 404).
  await page.goto(url('/equipment'));
  await expect(page.getByRole('button', { name: 'Add equipment' })).toHaveCount(0);
  await page.goto(url('/spaces'));
  await expect(page.getByRole('button', { name: 'New space' })).toHaveCount(0);
});
