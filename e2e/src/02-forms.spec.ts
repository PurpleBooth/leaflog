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
  // The card localises the substrate label (the raw wire value
  // never renders; D-decision in the overview-fixes batch).
  await expect(card).toContainText('Soil');
  await expectNoReload(page);
  await page.reload();
  await expect(page.locator('li.card', { hasText: name })).toBeVisible();
});

test('grow: edit from the detail page', async ({ page, request }) => {
  const name = uniqueName('E2E Grow');
  const id = await createGrow(request, name);
  await page.goto(url(`/grows/${id}`));
  await expect(page.getByRole('heading', { name, exact: true })).toBeVisible();

  // The header Edit control navigates to the edit page (grow-page
  // rework ticket 01); saving returns to the overview.
  await page.getByRole('link', { name: 'Edit grow' }).click();
  await expect(page).toHaveURL(url(`/grows/${id}/edit`));
  const renamed = uniqueName('E2E Grow v2');
  await page.getByLabel('Name *', { exact: true }).fill(renamed);
  await armNoReloadGuard(page);
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page).toHaveURL(url(`/grows/${id}`));
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
  await page.getByRole('link', { name: 'Edit grow' }).click();
  await expect(page).toHaveURL(url(`/grows/${id}/edit`));

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
  await expect(page).toHaveURL(url(`/grows/${id}`));
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
  await page.getByRole('link', { name: 'Edit grow' }).click();
  await expect(page).toHaveURL(url(`/grows/${id}/edit`));
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

  // The header's Edit control (a link since the grow-page rework,
  // ticket 01) sits right of the cluster — loose geometry, like the
  // today-marker case: x position only, never pixel-exact.
  const editLink = page.getByRole('link', { name: 'Edit grow' });
  await expect(editLink).toBeVisible();
  expect((await editLink.boundingBox())!.x).toBeGreaterThan(
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
  // Two badges (medium and phase day): no strain badge beside them.
  // The phase-day badge arrives with the forecast fetch.
  const badges = page
    .locator('div.flex.flex-wrap.items-center.gap-2')
    .first()
    .locator('span.badge');
  await expect(badges).toHaveCount(2);
});

test('quick add dialog: trigger, grouped actions, keyboard access', async ({
  page,
  request,
}) => {
  const name = uniqueName('E2E Menu Grow');
  const id = await createGrow(request, name);
  await page.goto(url(`/grows/${id}`));

  const trigger = page.getByRole('button', { name: 'Quick add', exact: true });
  const dialog = page.getByRole('dialog', { name: 'Add an entry' });
  const actions = dialog.locator('.modal-box').getByRole('button');

  // The trigger keeps its accessible name, carries one plus SVG and
  // announces dialog semantics; the dialog starts closed.
  await expect(trigger).toHaveAttribute('aria-haspopup', 'dialog');
  await expect(trigger).toHaveAttribute('aria-expanded', 'false');
  await expect(trigger.locator('svg')).toHaveCount(1);
  await expect(dialog).toBeHidden();

  // Click opens: five labelled groups and seventeen action buttons
  // in the grouped order, each with exactly one SVG icon (D122,
  // D148 added Set phase to Milestones).
  await trigger.click();
  await expect(trigger).toHaveAttribute('aria-expanded', 'true');
  await expect(dialog).toBeVisible();
  for (const group of ['Watering & feeding', 'Plant care', 'Environment', 'Observations', 'Milestones']) {
    await expect(dialog.getByRole('heading', { name: group })).toBeVisible();
  }
  await expect(actions).toHaveCount(17);
  await expect(actions).toHaveText([
    'Water',
    'Feed',
    'Trim',
    'Train',
    'Transplant',
    'Pest/IPM',
    'Lights',
    'Env change',
    'Light change',
    'Measure',
    'Photo',
    'Note',
    'Trichomes',
    'Flip 12/12',
    'Harvest',
    'Dry/cure',
    'Set phase',
  ]);
  for (let i = 0; i < 17; i += 1) {
    await expect(actions.nth(i).locator('svg')).toHaveCount(1);
  }
  // Tent-first persona: every action is a 44px-min touch target.
  // expect.poll: daisyUI's modal-pop entry animation briefly scales
  // the box below its settled size, so wait it out before measuring.
  await expect
    .poll(async () => (await actions.first().boundingBox())?.height ?? 0)
    .toBeGreaterThanOrEqual(44);

  // showModal focuses the first action; the browser traps Tab inside
  // the top-layer dialog and Escape returns focus to the trigger.
  await expect(actions.first()).toBeFocused();
  await page.keyboard.press('Tab');
  await expect(actions.nth(1)).toBeFocused();
  await page.keyboard.press('Shift+Tab');
  await expect(actions.first()).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(dialog).toBeHidden();
  await expect(trigger).toHaveAttribute('aria-expanded', 'false');
  await expect(trigger).toBeFocused();

  // Keyboard path: Enter on the trigger opens the dialog too.
  await trigger.focus();
  await page.keyboard.press('Enter');
  await expect(dialog).toBeVisible();
  await expect(actions.first()).toBeFocused();
  await page.keyboard.press('Escape');
  await expect(dialog).toBeHidden();
  await expect(trigger).toBeFocused();
});

test('quick add: log a watering from the grow page', async ({ page, request }) => {
  const name = uniqueName('E2E Log Grow');
  const id = await createGrow(request, name);
  await page.goto(url(`/grows/${id}`));

  // The sixteen-button row is a grouped dialog picker now (ticket 06,
  // D122); picking Water closes it, focuses the form card and opens
  // the same inline form the chip did.
  const trigger = page.getByRole('button', { name: 'Quick add', exact: true });
  const dialog = page.getByRole('dialog', { name: 'Add an entry' });
  await trigger.click();
  await dialog.getByRole('button', { name: 'Water', exact: true }).click();
  await expect(dialog).toBeHidden();
  await expect(page.locator('#quick-add-form')).toBeFocused();
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

test('quick add recents: last picks resurface at the top', async ({ page, request }) => {
  const name = uniqueName('E2E Recents Grow');
  const id = await createGrow(request, name);
  await page.goto(url(`/grows/${id}`));
  // Fresh profile (D127): nothing remembered yet, so the recents
  // group is skipped entirely.
  await page.evaluate(() => window.localStorage.clear());

  const trigger = page.getByRole('button', { name: 'Quick add', exact: true });
  const dialog = page.getByRole('dialog', { name: 'Add an entry' });
  const recentsGroup = dialog.locator('.modal-box section').first();
  await trigger.click();
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole('heading', { name: 'Recent' })).toHaveCount(0);

  // Picking Water remembers it; the dialog closes with the form
  // card focused, exactly like the taxonomy path (D069).
  await dialog.getByRole('button', { name: 'Water', exact: true }).click();
  await expect(dialog).toBeHidden();
  await expect(page.locator('#quick-add-form')).toBeFocused();

  // Reopen: the recents group leads, above the taxonomy groups.
  await trigger.click();
  await expect(recentsGroup.getByRole('heading')).toHaveText('Recent');
  await expect(recentsGroup.getByRole('button')).toHaveText(['Water']);

  // A second pick lands in front: most recent first.
  await dialog.getByRole('button', { name: 'Feed', exact: true }).click();
  await expect(dialog).toBeHidden();
  await trigger.click();
  await expect(recentsGroup.getByRole('button')).toHaveText(['Feed', 'Water']);

  // Clicking a recent behaves exactly like its taxonomy chip:
  // same draft, same close-and-focus, same form. Log it to prove it.
  await recentsGroup.getByRole('button', { name: 'Feed', exact: true }).click();
  await expect(dialog).toBeHidden();
  await expect(page.locator('#quick-add-form')).toBeFocused();
  await page.getByLabel('Volume (ml)').fill('250');
  await armNoReloadGuard(page);
  await page.getByRole('button', { name: 'Log it' }).click();
  const entry = page
    .locator('ul.list-none li')
    .filter({ has: page.locator('strong:text-is("Feed")') });
  await expect(entry).toBeVisible();
  await expect(entry).toContainText('250');
  await expectNoReload(page);

  // Recents live in localStorage: they survive a reload unchanged.
  await page.reload();
  await trigger.click();
  await expect(recentsGroup.getByRole('heading')).toHaveText('Recent');
  await expect(recentsGroup.getByRole('button')).toHaveText(['Feed', 'Water']);
});

test('HA customary units display and save harvest weights in grams', async ({ page, request }) => {
  const growId = await createGrow(request, uniqueName('E2E US Grow'));
  const plantId = await createPlant(request, growId, uniqueName('E2E US Plant'));
  await page.route('**/api/locale', (route) => route.fulfill({
    json: { language: 'en', unit_system: 'imperial' },
  }));
  await page.goto(url(`/plants/${plantId}`));
  await page.getByRole('button', { name: 'Edit plant' }).click();
  const wet = page.getByLabel('Wet weight (oz)');
  await wet.pressSequentially('2.5');
  await expect(wet).toHaveValue('2.5');
  await page.getByRole('button', { name: 'Save' }).click();
  await expect(page.getByText('2.50 oz')).toBeVisible();
  const response = await request.get(url(`/api/plants/${plantId}/view`));
  expect(response.ok()).toBeTruthy();
  expect((await response.json()).plant.wet_g).toBeCloseTo(70.8738, 3);
  await page.unroute('**/api/locale');
  await page.reload();
  await expect(page.getByText('71 g')).toBeVisible();
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
