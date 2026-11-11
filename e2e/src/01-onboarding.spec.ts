// First-run onboarding wizard (runs first: the fresh database shows it,
// and finishing it dismisses the wizard for every later spec).

import { test, expect } from '@playwright/test';
import { armNoReloadGuard, expectNoReload, uniqueName, url } from './helpers';

test('wizard happy path: space, grow and plants land on the dashboard', async ({
  page,
}) => {
  const tent = uniqueName('E2E Wizard Tent');
  const grow = uniqueName('E2E Wizard Grow');

  await page.goto(url('/'));
  const wizard = page.locator('section[aria-label="First-run setup"]');
  await expect(wizard).toBeVisible();

  // Step 1: pick any bundled equipment preset and continue.
  await wizard.locator('ul li button').first().click();
  await wizard.getByRole('button', { name: 'Next' }).click();
  await expect(wizard.getByRole('heading', { name: '2. Grow space' })).toBeVisible();

  // Step 2: name the tent.
  await page.getByLabel('Name', { exact: true }).fill(tent);
  await wizard.getByRole('button', { name: 'Next' }).click();
  await expect(wizard.getByRole('heading', { name: '3. Strain' })).toBeVisible();

  // Step 3: a strain is optional; skip the pick.
  await wizard.getByRole('button', { name: 'Next' }).click();
  await expect(wizard.getByRole('heading', { name: '4. Plants' })).toBeVisible();

  // Step 4: the wizard's one real form. "Start the grow" must be
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

  // Everything persisted: a reload of the same deep link shows the
  // grow and both plants again.
  await page.reload();
  await expect(page.getByRole('heading', { name: grow, exact: true })).toBeVisible();
  await expect(page.getByRole('link', { name: 'Wiz 1', exact: true })).toBeVisible();

  // The wizard is dismissed for good.
  await page.goto(url('/'));
  await expect(wizard).toHaveCount(0);
});
