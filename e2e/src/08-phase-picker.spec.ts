// The phase picker (D073, D148): picking a phase records the right
// transition event — a flip on a photoperiod flowering — shows on the
// timeline, and survives a reload. The picker itself lives in the
// quick-add dialog's Milestones group ("Set phase"): picking it opens
// the inline phase card, the same pick/fill/log loop the actions use.

import { test, expect } from '@playwright/test';
import {
  armNoReloadGuard,
  createGrow,
  dismissOnboarding,
  expectNoReload,
  uniqueName,
  url,
} from './helpers';

test.beforeAll(async ({ request }) => {
  await dismissOnboarding(request);
});

/** Opens the quick-add dialog and picks "Set phase" (D148). */
async function openPhaseCard(page: import('@playwright/test').Page) {
  const trigger = page.getByRole('button', { name: 'Quick add', exact: true });
  const dialog = page.getByRole('dialog', { name: 'Add an entry' });
  await trigger.click();
  await expect(dialog).toBeVisible();
  await dialog.getByRole('button', { name: 'Set phase', exact: true }).click();
  await expect(dialog).toBeHidden();
  await expect(page.locator('#phase-picker')).toBeFocused();
}

test('phase picker: flowering records a flip and shows on the timeline', async ({
  page,
  request,
}) => {
  // No photoperiod set: the grow defaults to photoperiod semantics,
  // so Flowering maps to the flip.
  const name = uniqueName('E2E Pick');
  const growId = await createGrow(request, name);
  await page.goto(url(`/grows/${growId}`));

  await openPhaseCard(page);
  await page.locator('#phase-picker select').selectOption({ label: 'Flowering' });
  await armNoReloadGuard(page);
  await page.locator('#phase-picker button', { hasText: 'Record phase' }).click();
  await expectNoReload(page);

  // The pick lands on the timeline as the flip it maps to.
  await expect(
    page.locator('[data-testid="story-day"] li', { hasText: 'Flip 12/12' }).first(),
  ).toBeVisible();

  // And it persists.
  await page.reload();
  await expect(
    page.locator('[data-testid="story-day"] li', { hasText: 'Flip 12/12' }).first(),
  ).toBeVisible();
});

test('phase picker: an autoflower flowering records a phase change', async ({
  page,
  request,
}) => {
  // The auto path comes from the grow's photoperiod toggle, not a
  // strain record.
  const name = uniqueName('E2E Pick Auto');
  const growId = await createGrow(request, name, { photoperiod: 'auto' });
  await page.goto(url(`/grows/${growId}`));

  await openPhaseCard(page);
  await page.locator('#phase-picker select').selectOption({ label: 'Flowering' });
  await armNoReloadGuard(page);
  await page.locator('#phase-picker button', { hasText: 'Record phase' }).click();
  await expectNoReload(page);

  await expect(
    page.locator('[data-testid="story-day"] li', { hasText: 'Phase change' }).first(),
  ).toBeVisible();
});

test('phase picker: drying records a harvest', async ({ page, request }) => {
  const name = uniqueName('E2E Pick Dry');
  const growId = await createGrow(request, name);
  await page.goto(url(`/grows/${growId}`));

  await openPhaseCard(page);
  await page.locator('#phase-picker select').selectOption({ label: 'Drying' });
  await armNoReloadGuard(page);
  await page.locator('#phase-picker button', { hasText: 'Record phase' }).click();
  await expectNoReload(page);

  await expect(
    page.locator('[data-testid="story-day"] li', { hasText: 'Harvest' }).first(),
  ).toBeVisible();
});
