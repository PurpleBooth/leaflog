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

test('grow: strain label, photoperiod and advanced settings persist', async ({
  page,
  request,
}) => {
  const name = uniqueName('E2E Advanced Grow');
  const id = await createGrow(request, name);
  await page.goto(url(`/grows/${id}`));
  await page.getByRole('button', { name: 'Edit grow' }).click();

  // The strain facts sit with the main grow fields.
  await page.getByLabel('Strain label', { exact: true }).fill('Moby Dick');
  await page.getByLabel('Photoperiod').selectOption('auto');

  // The tuning knobs live under Advanced settings, collapsed by
  // default so the daily flow stays simple.
  await expect(page.getByLabel('Heat tolerance')).toBeHidden();
  await page.getByText('Advanced settings', { exact: true }).click();

  await page.getByLabel('Heat tolerance').selectOption('sensitive');
  await page.getByLabel('Cold tolerance').selectOption('tolerant');
  await page.getByLabel('Light tolerance').selectOption('tolerant');
  await page.getByLabel('Real wall watts').fill('113.5');
  await page.getByLabel('Dimmable').uncheck();

  await armNoReloadGuard(page);
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByRole('heading', { name, exact: true })).toBeVisible();
  await expectNoReload(page);

  // The API holds the persisted truth, independent of the form.
  const stored = await api<{
    strain_label: string;
    photoperiod: string;
    heat_tolerance: string;
    cold_tolerance: string;
    light_tolerance: string;
    real_watts: number;
    dimmable: boolean;
  }>(request, 'GET', `api/grows/${id}`);
  expect(stored!.strain_label).toBe('Moby Dick');
  expect(stored!.photoperiod).toBe('auto');
  expect(stored!.heat_tolerance).toBe('sensitive');
  expect(stored!.cold_tolerance).toBe('tolerant');
  expect(stored!.light_tolerance).toBe('tolerant');
  expect(stored!.real_watts).toBeCloseTo(113.5);
  expect(stored!.dimmable).toBe(false);

  // Reopening the edit form shows the stored values, not defaults.
  await page.reload();
  await page.getByRole('button', { name: 'Edit grow' }).click();
  await expect(page.getByLabel('Strain label', { exact: true })).toHaveValue(
    'Moby Dick',
  );
  await expect(page.getByLabel('Photoperiod')).toHaveValue('auto');
  await page.getByText('Advanced settings', { exact: true }).click();
  await expect(page.getByLabel('Heat tolerance')).toHaveValue('sensitive');
  await expect(page.getByLabel('Cold tolerance')).toHaveValue('tolerant');
  await expect(page.getByLabel('Light tolerance')).toHaveValue('tolerant');
  await expect(page.getByLabel('Real wall watts')).toHaveValue('113.5');
  await expect(page.getByLabel('Dimmable')).not.toBeChecked();
});

test('grow: picked strain survives late options, save and reopen', async ({
  page,
  request,
}) => {
  const name = uniqueName('E2E Strain Grow');
  // The strain id comes from the API, not the UI: the select's value is
  // asserted against an independent source of truth.
  const strains = await api<Array<{ id: string; name: string }>>(
    request,
    'GET',
    'api/strains',
  );
  const northern = strains!.find((strain) => strain.name === 'Northern Lights');
  const grow = await api<{ id: string }>(request, 'POST', 'api/grows', {
    name,
    strain_id: northern!.id,
    grow_space_id: null,
    seed_batch_id: null,
    feed_template_id: null,
    substrate: 'soil',
    started_on: new Date().toISOString().slice(0, 10),
    status: 'active',
    notes: null,
  });

  // Hold the strains list back so the <option> nodes mount after the
  // select: the bound strain must display as selected once they land,
  // never the placeholder while the draft keeps the pick.
  await page.route('**/api/strains', async (route) => {
    await new Promise((resolve) => {
      setTimeout(resolve, 300);
    });
    await route.continue();
  });

  await page.goto(url(`/grows/${grow!.id}`));
  await expect(page.getByRole('heading', { name, exact: true })).toBeVisible();

  await page.getByRole('button', { name: 'Edit grow' }).click();
  // The catalogue strain select, addressed by role + name so the free
  // text "Strain label" input beside it never collides. No `exact:
  // true` on select names: the accessible name includes the <option>
  // texts, so an exact match never resolves (true for every select in
  // this form, not a strain quirk).
  const strainSelect = page.getByRole('combobox', { name: 'Strain' });
  await expect(strainSelect).toHaveValue(northern!.id);

  // Pick explicitly and save (the D061 guard: no native submit, no reload).
  await strainSelect.selectOption(northern!.id);
  await armNoReloadGuard(page);
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByRole('heading', { name, exact: true })).toBeVisible();
  await expectNoReload(page);

  // Saving refetches the strains list; reopening the form must show the
  // pick, never reset to the placeholder.
  await page.getByRole('button', { name: 'Edit grow' }).click();
  await expect(strainSelect).toHaveValue(northern!.id);
});

test('grow: bound space survives late options, save and reopen', async ({
  page,
  request,
}) => {
  const name = uniqueName('E2E Space Grow');
  const spaceId = await createSpace(request, uniqueName('E2E Space'));
  const grow = await api<{ id: string }>(request, 'POST', 'api/grows', {
    name,
    strain_id: null,
    grow_space_id: spaceId,
    seed_batch_id: null,
    feed_template_id: null,
    substrate: 'soil',
    started_on: new Date().toISOString().slice(0, 10),
    status: 'active',
    notes: null,
  });

  // Hold the spaces list back so the <option> nodes mount after the
  // select: the bound space must display as selected once they land,
  // never the placeholder while the draft keeps the binding.
  await page.route('**/api/spaces', async (route) => {
    await new Promise((resolve) => {
      setTimeout(resolve, 300);
    });
    await route.continue();
  });

  await page.goto(url(`/grows/${grow!.id}`));
  await expect(page.getByRole('heading', { name, exact: true })).toBeVisible();

  await page.getByRole('button', { name: 'Edit grow' }).click();
  await expect(page.getByLabel('Space')).toHaveValue(spaceId);

  // Pick explicitly and save (the D061 guard: no native submit, no reload).
  await page.getByLabel('Space').selectOption(spaceId);
  await armNoReloadGuard(page);
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByRole('heading', { name, exact: true })).toBeVisible();
  await expectNoReload(page);

  // Saving refetches the spaces list; reopening the form must show the
  // binding, never reset to the placeholder.
  await page.getByRole('button', { name: 'Edit grow' }).click();
  await expect(page.getByLabel('Space')).toHaveValue(spaceId);
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

  // Pick a bundled strain in the edit form and save (role + name, same
  // reason as the strain-pick test: the accessible name carries the
  // option texts, and the free-text "Strain label" input sits beside it).
  await page.getByRole('button', { name: 'Edit grow' }).click();
  await page
    .getByRole('combobox', { name: 'Strain' })
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
