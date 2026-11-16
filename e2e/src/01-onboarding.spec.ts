// First-run onboarding wizard (runs first: the fresh database shows it,
// and finishing it dismisses the wizard for every later spec).

import { test, expect } from '@playwright/test';
import { api, armNoReloadGuard, expectNoReload, uniqueName, url } from './helpers';

test('wizard happy path: sensors, grow and plants land on the dashboard', async ({
  page,
}) => {
  const grow = uniqueName('E2E Wizard Grow');

  await page.goto(url('/'));
  const wizard = page.locator('section[aria-label="First-run setup"]');
  await expect(wizard).toBeVisible();

  // Step 1: bind one sensor — the temperature role gets the mock's
  // entity; the pick must land on the created grow as its binding.
  await wizard.getByLabel('Temperature').selectOption('sensor.grow_temperature');
  await wizard.getByRole('button', { name: 'Next' }).click();
  await expect(wizard.getByRole('heading', { name: '2. Strain' })).toBeVisible();

  // Step 2: a strain is optional; skip the pick.
  await wizard.getByRole('button', { name: 'Next' }).click();
  await expect(wizard.getByRole('heading', { name: '3. Plants' })).toBeVisible();

  // Feature-strip: equipment and space steps left the wizard, and the
  // step numbering stays coherent (bind sensors, strain, plants).
  await expect(page.getByLabel('Width (cm)')).toHaveCount(0);

  // Step 3: the wizard's one real form. "Start the grow" must be
  // intercepted by the app, never a native submit (D061).
  await page.getByLabel('Grow name').fill(grow);
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
