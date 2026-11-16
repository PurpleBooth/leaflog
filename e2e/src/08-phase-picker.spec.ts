// The phase picker (D073): picking a phase records the right
// transition event — a flip on a photoperiod flowering — shows on the
// timeline, and survives a reload.

import { test, expect } from '@playwright/test';
import {
  api,
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

test('phase picker: flowering records a flip and shows on the timeline', async ({
  page,
  request,
}) => {
  const name = uniqueName('E2E Pick');
  const growId = await createGrow(request, name);
  await page.goto(url(`/grows/${growId}`));

  await page.locator('#phase-picker select').selectOption({ label: 'Flowering' });
  await armNoReloadGuard(page);
  await page.locator('#phase-picker button', { hasText: 'Record phase' }).click();
  await expectNoReload(page);

  // The pick lands on the timeline as the flip it maps to.
  await expect(page.locator('li.card', { hasText: 'Flip 12/12' }).first()).toBeVisible();

  // And it persists.
  await page.reload();
  await expect(page.locator('li.card', { hasText: 'Flip 12/12' }).first()).toBeVisible();
});

test('phase picker: an autoflower flowering records a phase change', async ({
  page,
  request,
}) => {
  const strain = await api<{ id: string }>(request, 'POST', 'api/strains', {
    name: uniqueName('E2E Auto'),
    photoperiod: 'auto',
    sex: 'feminized',
    landrace_tags: [],
    heat_tolerance: 'average',
    cold_tolerance: 'average',
    light_tolerance: 'average',
  });
  const name = uniqueName('E2E Pick Auto');
  const growId = await createGrow(request, name, { strain_id: strain!.id });
  await page.goto(url(`/grows/${growId}`));

  await page.locator('#phase-picker select').selectOption({ label: 'Flowering' });
  await armNoReloadGuard(page);
  await page.locator('#phase-picker button', { hasText: 'Record phase' }).click();
  await expectNoReload(page);

  await expect(
    page.locator('li.card', { hasText: 'Phase change' }).first(),
  ).toBeVisible();
});

test('phase picker: drying records a harvest', async ({ page, request }) => {
  const name = uniqueName('E2E Pick Dry');
  const growId = await createGrow(request, name);
  await page.goto(url(`/grows/${growId}`));

  await page.locator('#phase-picker select').selectOption({ label: 'Drying' });
  await armNoReloadGuard(page);
  await page.locator('#phase-picker button', { hasText: 'Record phase' }).click();
  await expectNoReload(page);

  await expect(page.locator('li.card', { hasText: 'Harvest' }).first()).toBeVisible();
});
