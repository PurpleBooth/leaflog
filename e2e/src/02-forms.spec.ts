// The form regression suite. Every `<form>` in the frontend gets a
// case here: fill it, submit, see the result without a page reload
// (the D061 guard), then reload and see it persisted. New form in the
// app? Add a case (docs/DECISIONS.md D069).

import { test, expect } from '@playwright/test';
import {
  api,
  armNoReloadGuard,
  createGrow,
  createPlant,
  createSpace,
  dismissOnboarding,
  expectNoReload,
  uniqueName,
  url,
} from './helpers';

test.beforeAll(async ({ request }) => {
  // Onboarding spec finished the wizard already; dismiss again so this
  // file is safe to run alone against a used database.
  await dismissOnboarding(request);
});

test('equipment: create, then edit', async ({ page }) => {
  const name = uniqueName('E2E Lamp');
  await page.goto(url('/equipment'));
  await page.getByRole('button', { name: 'Add equipment' }).click();
  await page.getByLabel('Name *', { exact: true }).fill(name);
  await page.getByLabel('Brand / model').fill('E2E brand');
  await armNoReloadGuard(page);
  await page.getByRole('button', { name: 'Save' }).click();

  const card = page.locator('li.card', { hasText: name });
  await expect(card).toBeVisible();
  await expect(card).toContainText('E2E brand');
  await expectNoReload(page);

  // Edit the created item: rename, no reload, persists after reload.
  const renamed = uniqueName('E2E Lamp v2');
  await card.getByRole('button', { name: 'Edit', exact: true }).click();
  await page.getByLabel('Name *', { exact: true }).fill(renamed);
  await armNoReloadGuard(page);
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.locator('li.card', { hasText: renamed })).toBeVisible();
  await expectNoReload(page);
  await page.reload();
  await expect(page.locator('li.card', { hasText: renamed })).toBeVisible();
  await expect(page.locator('li.card', { hasText: name })).toHaveCount(0);
});

test('strain: create, then edit', async ({ page }) => {
  const name = uniqueName('E2E Strain');
  await page.goto(url('/strains'));
  await page.getByRole('button', { name: 'Add strain' }).click();
  await page.getByLabel('Name *', { exact: true }).fill(name);
  await page.getByLabel('Breeder').fill('E2E breeder');
  await armNoReloadGuard(page);
  await page.getByRole('button', { name: 'Save' }).click();

  const card = page.locator('li.card', { hasText: name });
  await expect(card).toBeVisible();
  await expect(card).toContainText('E2E breeder');
  await expectNoReload(page);

  const renamed = uniqueName('E2E Strain v2');
  await card.getByRole('button', { name: 'Edit', exact: true }).click();
  await page.getByLabel('Name *', { exact: true }).fill(renamed);
  await armNoReloadGuard(page);
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.locator('li.card', { hasText: renamed })).toBeVisible();
  await expectNoReload(page);
  await page.reload();
  await expect(page.locator('li.card', { hasText: renamed })).toBeVisible();
});

test('grow space: create from the overview page', async ({ page }) => {
  const name = uniqueName('E2E Space');
  await page.goto(url('/spaces'));
  await page.getByRole('button', { name: 'New space' }).click();
  await page.getByLabel('Name *', { exact: true }).fill(name);
  await page.getByLabel('Width (cm)').fill('60');
  await page.getByLabel('Depth (cm)').fill('60');
  await page.getByLabel('Height (cm)').fill('180');
  await armNoReloadGuard(page);
  await page.getByRole('button', { name: 'Save' }).click();

  const card = page.locator('li.card', { hasText: name });
  await expect(card).toBeVisible();
  await expect(card).toContainText('W 60');
  await expectNoReload(page);
  await page.reload();
  await expect(page.locator('li.card', { hasText: name })).toBeVisible();
});

test('grow space: edit from the detail page', async ({ page, request }) => {
  const name = uniqueName('E2E Space');
  const id = await createSpace(request, name);
  await page.goto(url(`/spaces/${id}`));
  await expect(page.getByRole('heading', { name, exact: true })).toBeVisible();

  await page.getByRole('button', { name: 'Edit space' }).click();
  const renamed = uniqueName('E2E Space v2');
  await page.getByLabel('Name *', { exact: true }).fill(renamed);
  await armNoReloadGuard(page);
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByRole('heading', { name: renamed, exact: true })).toBeVisible();
  await expectNoReload(page);
  await page.reload();
  await expect(page.getByRole('heading', { name: renamed, exact: true })).toBeVisible();
});

test('seed batch: create, then edit', async ({ page }) => {
  await page.goto(url('/seedbank'));
  await page.getByRole('button', { name: 'Add batch' }).click();
  await page.getByLabel('Seeds *', { exact: true }).fill('5');
  await page.getByLabel('Acquired (YYYY-MM-DD) *').fill('2026-01-15');
  await page.getByLabel('Acquired from').fill('E2E bank');
  await armNoReloadGuard(page);
  await page.getByRole('button', { name: 'Save' }).click();

  const card = page.locator('li.card', { hasText: 'E2E bank' });
  await expect(card).toBeVisible();
  await expect(card).toContainText('5 ×');
  await expectNoReload(page);

  await card.getByRole('button', { name: 'Edit', exact: true }).click();
  await page.getByLabel('Seeds *', { exact: true }).fill('7');
  await armNoReloadGuard(page);
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.locator('li.card', { hasText: 'E2E bank' })).toContainText('7 ×');
  await expectNoReload(page);
  await page.reload();
  await expect(page.locator('li.card', { hasText: 'E2E bank' })).toContainText('7 ×');
});

test('grow: create from the grows page', async ({ page }) => {
  const name = uniqueName('E2E Grow');
  await page.goto(url('/grows'));
  await page.getByRole('button', { name: 'Start a grow' }).click();
  await page.getByLabel('Name *', { exact: true }).fill(name);
  await page.getByLabel('Started (seed/clone) *').fill('2026-02-01');
  await armNoReloadGuard(page);
  await page.getByRole('button', { name: 'Save' }).click();

  const card = page.locator('li.card', { hasText: name });
  await expect(card).toBeVisible();
  await expect(card).toContainText('soil');
  await expectNoReload(page);
  await page.reload();
  await expect(page.locator('li.card', { hasText: name })).toBeVisible();
});

test('grow: edit from the detail page', async ({ page, request }) => {
  const name = uniqueName('E2E Grow');
  const id = await createGrow(request, name);
  await page.goto(url(`/grows/${id}`));
  await expect(page.getByRole('heading', { name, exact: true })).toBeVisible();

  await page.getByRole('button', { name: 'Edit grow' }).click();
  const renamed = uniqueName('E2E Grow v2');
  await page.getByLabel('Name *', { exact: true }).fill(renamed);
  await armNoReloadGuard(page);
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByRole('heading', { name: renamed, exact: true })).toBeVisible();
  await expectNoReload(page);
  await page.reload();
  await expect(page.getByRole('heading', { name: renamed, exact: true })).toBeVisible();
});

test('grow header cluster: strain shows after picking, left of the buttons', async ({
  page,
  request,
}) => {
  const name = uniqueName('E2E Cluster Grow');
  const id = await createGrow(request, name);
  await page.goto(url(`/grows/${id}`));
  await expect(page.getByRole('heading', { name, exact: true })).toBeVisible();

  // A grow without a strain renders nothing in the strain slot: the
  // seeded catalogue must not leak in as a placeholder.
  await expect(page.getByText('Northern Lights')).toHaveCount(0);

  // Pick a bundled strain in the edit form and save.
  await page.getByRole('button', { name: 'Edit grow' }).click();
  await page
    .getByLabel('Strain', { exact: true })
    .selectOption({ label: 'Northern Lights' });
  await armNoReloadGuard(page);
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByRole('heading', { name, exact: true })).toBeVisible();
  await expectNoReload(page);

  // The strain joins the quiet header cluster beside the medium glyph.
  const strainBadge = page.getByText('Northern Lights');
  await expect(strainBadge).toBeVisible();
  await expect(page.locator('[aria-hidden="true"]', { hasText: '🪴' })).toBeVisible();

  // The action buttons sit right of the cluster — loose geometry, like
  // the today-marker case: x position only, never pixel-exact.
  const editButton = page.getByRole('button', { name: 'Edit grow' });
  await expect(editButton).toBeVisible();
  expect((await editButton.boundingBox())!.x).toBeGreaterThan(
    (await strainBadge.boundingBox())!.x,
  );

  // The pick persisted.
  await page.reload();
  await expect(page.getByText('Northern Lights')).toBeVisible();
});

test('quick add: log a watering from the grow page', async ({ page, request }) => {
  const name = uniqueName('E2E Log Grow');
  const id = await createGrow(request, name);
  await page.goto(url(`/grows/${id}`));

  await page.getByRole('button', { name: 'Water', exact: true }).click();
  await page.getByLabel('Volume (ml)').fill('250');
  await armNoReloadGuard(page);
  await page.getByRole('button', { name: 'Log it' }).click();

  // The timeline (not the quick-add chips, not the coaching panel)
  // shows the entry: timeline items carry the action type as a strong.
  const entry = page
    .locator('ul.list-none li')
    .filter({ has: page.locator('strong:text-is("Water")') });
  await expect(entry).toBeVisible();
  await expect(entry).toContainText('250');
  await expectNoReload(page);
  await page.reload();
  await expect(
    page.locator('ul.list-none li').filter({ has: page.locator('strong:text-is("Water")') }),
  ).toContainText('250');
});

test('plant: edit weights from the plant page', async ({ page, request }) => {
  const plantName = uniqueName('E2E Plant');
  const growId = await createGrow(request, uniqueName('E2E Plant Grow'));
  const plantId = await createPlant(request, growId, plantName);
  await page.goto(url(`/plants/${plantId}`));
  await expect(page.getByText(plantName, { exact: true })).toBeVisible();

  await page.getByRole('button', { name: 'Edit plant' }).click();
  await page.getByLabel('Wet weight (g)').fill('120');
  await armNoReloadGuard(page);
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByText('120 g')).toBeVisible();
  await expectNoReload(page);
  await page.reload();
  await expect(page.getByText('120 g')).toBeVisible();
});

test('strains seeded from the bundle are visible', async ({ page }) => {
  await page.goto(url('/strains'));
  await expect(page.getByText('Northern Lights').first()).toBeVisible();
  // Sanity for the API helper this suite leans on: the list endpoint
  // answers under the ingress prefix.
  const strains = await api<Array<{ id: string }>>(
    page.context().request,
    'GET',
    'api/strains',
  );
  expect(strains!.length).toBeGreaterThan(0);
});
