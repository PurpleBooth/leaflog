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

test('strains: the page and its nav entry are gone', async ({ page }) => {
  // Feature-strip (D075): the strain catalogue left the product — the
  // grow carries its strain label itself. No nav link, and the old
  // deep link renders nothing: no strain form anywhere.
  await page.goto(url('/'));
  await expect(page.getByRole('link', { name: 'Strains' })).toHaveCount(0);
  await page.goto(url('/strains'));
  await expect(page.getByRole('button', { name: 'Add strain' })).toHaveCount(0);
});

test('seedbank: the page and its nav entry are gone', async ({ page }) => {
  // Feature-strip: the seed bank left the product. No nav link, and
  // the old deep link renders nothing — no batch form anywhere.
  await page.goto(url('/'));
  await expect(page.getByRole('link', { name: 'Seedbank' })).toHaveCount(0);
  await page.goto(url('/seedbank'));
  await expect(page.getByRole('button', { name: 'Add batch' })).toHaveCount(0);
  await expect(page.getByLabel('Seeds *', { exact: true })).toHaveCount(0);
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

test('grow header cluster: the strain label shows, left of the buttons', async ({
  page,
  request,
}) => {
  const name = uniqueName('E2E Cluster Grow');
  const label = uniqueName('E2E Cluster Label');
  const id = await createGrow(request, name, { strain_label: label });
  await page.goto(url(`/grows/${id}`));
  await expect(page.getByRole('heading', { name, exact: true })).toBeVisible();

  // The label joins the quiet header cluster beside the medium glyph.
  const labelBadge = page.getByText(label, { exact: true });
  await expect(labelBadge).toBeVisible();
  await expect(page.locator('[aria-hidden="true"]', { hasText: '🪴' })).toBeVisible();

  // The action buttons sit right of the cluster — loose geometry, like
  // the today-marker case: x position only, never pixel-exact.
  const editButton = page.getByRole('button', { name: 'Edit grow' });
  await expect(editButton).toBeVisible();
  expect((await editButton.boundingBox())!.x).toBeGreaterThan(
    (await labelBadge.boundingBox())!.x,
  );

  // The label persists across a reload.
  await page.reload();
  await expect(page.getByText(label, { exact: true })).toBeVisible();
});

test('grow header cluster: an unlabelled grow shows no placeholder', async ({
  page,
  request,
}) => {
  // A grow without a strain label renders nothing in the label slot:
  // there is no catalogue left that could leak in as a badge (D075).
  const name = uniqueName('E2E Bare Grow');
  const id = await createGrow(request, name);
  await page.goto(url(`/grows/${id}`));
  await expect(page.getByRole('heading', { name, exact: true })).toBeVisible();
  await expect(page.locator('span.badge', { hasText: '🪴' })).toHaveCount(1);
  // Exactly one badge (the medium): no strain badge beside it.
  const badges = page
    .locator('div.flex.flex-wrap.items-center.gap-2')
    .first()
    .locator('span.badge');
  await expect(badges).toHaveCount(1);
});

test('quick add menu: trigger, sixteen iconified actions, keyboard access', async ({
  page,
  request,
}) => {
  const name = uniqueName('E2E Menu Grow');
  const id = await createGrow(request, name);
  await page.goto(url(`/grows/${id}`));

  const trigger = page.getByRole('button', { name: 'Quick add', exact: true });
  const menu = page.getByRole('menu', { name: 'Quick add action' });

  // The trigger carries a plus and a chevron SVG and starts closed.
  await expect(trigger).toHaveAttribute('aria-haspopup', 'menu');
  await expect(trigger).toHaveAttribute('aria-expanded', 'false');
  await expect(trigger.locator('svg')).toHaveCount(2);
  await expect(menu).toBeHidden();

  // Click opens: sixteen menuitems in the order the chip row had,
  // each with exactly one SVG icon — no emoji regression (ticket 06).
  await trigger.click();
  await expect(trigger).toHaveAttribute('aria-expanded', 'true');
  await expect(menu).toBeVisible();
  const items = menu.getByRole('menuitem');
  await expect(items).toHaveCount(16);
  await expect(items).toHaveText([
    'Lights',
    'Water',
    'Feed',
    'Trim',
    'Train',
    'Transplant',
    'Flip 12/12',
    'Pest/IPM',
    'Env change',
    'Light change',
    'Measure',
    'Photo',
    'Note',
    'Trichomes',
    'Harvest',
    'Dry/cure',
  ]);
  for (let i = 0; i < 16; i += 1) {
    await expect(items.nth(i).locator('svg')).toHaveCount(1);
  }

  // Escape closes and hands focus back to the trigger (APG
  // menu-button pattern).
  await page.keyboard.press('Escape');
  await expect(menu).toBeHidden();
  await expect(trigger).toHaveAttribute('aria-expanded', 'false');
  await expect(trigger).toBeFocused();

  // Keyboard path: ArrowDown from the trigger opens the menu and
  // focuses the first item; End jumps to the last.
  await trigger.focus();
  await page.keyboard.press('ArrowDown');
  await expect(menu).toBeVisible();
  await expect(items.first()).toBeFocused();
  await page.keyboard.press('End');
  await expect(items.last()).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(menu).toBeHidden();
  await expect(trigger).toBeFocused();
});

test('quick add: log a watering from the grow page', async ({ page, request }) => {
  const name = uniqueName('E2E Log Grow');
  const id = await createGrow(request, name);
  await page.goto(url(`/grows/${id}`));

  // The sixteen-button row is one dropdown menu now (ticket 06);
  // picking Water opens the same inline form card the chip did.
  await page.getByRole('button', { name: 'Quick add', exact: true }).click();
  const menu = page.getByRole('menu', { name: 'Quick add action' });
  await menu.getByRole('menuitem', { name: 'Water' }).click();
  await expect(menu).toBeHidden();
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
