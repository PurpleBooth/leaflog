// Ticket 05 (grow page rework): the overview badge shows the current
// phase with its Phase day, anchored at the transition event. The
// timeline keeps showing the Day count, so the two stay distinct.

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

/** A UTC date `days` before now, `YYYY-MM-DD`. */
function utcDateDaysAgo(days: number): string {
  return new Date(Date.now() - days * 86_400_000).toISOString().slice(0, 10);
}

/** The display names the UI spells phase labels with. */
const DISPLAY: Record<string, string> = {
  germination: 'Germination',
  seedling: 'Seedling',
  vegetative: 'Vegetative',
  flower: 'Flowering',
  drying: 'Drying',
  curing: 'Curing',
};

test('phase day badge: equals the day count before any phase event', async ({
  page,
  request,
}) => {
  const name = uniqueName('E2E Phase Day');
  const growId = await createGrow(request, name, {
    started_on: utcDateDaysAgo(5),
  });

  await page.goto(url(`/grows/${growId}`));

  // No phase event exists yet, so the badge must show the Day count.
  // The expected numbers come from the forecast API; the mock HA
  // timezone is Europe/Berlin, so its today_day is the truth.
  const forecast = await api<{ today_day: number; phase: string }>(
    request,
    'GET',
    `api/grows/${growId}/forecast`,
  );
  await expect(page.locator('#phase-day')).toHaveText(
    `${DISPLAY[forecast.phase]} · day ${forecast.today_day}`,
  );
});

test('phase day badge: a backdated pick re-anchors it without a reload', async ({
  page,
  request,
}) => {
  // Grow started ten days ago. The Flowering pick is dated three
  // days ago, so its flip lands on grow day 7.
  const name = uniqueName('E2E Phase Day Flip');
  const growId = await createGrow(request, name, {
    started_on: utcDateDaysAgo(10),
  });

  await page.goto(url(`/grows/${growId}`));
  await expect(page.locator('#phase-day')).toBeVisible();

  // Set phase lives in the quick-add dialog now (D148); the card
  // opens with today's date preseeded, which the backdate overwrites.
  const trigger = page.getByRole('button', { name: 'Quick add', exact: true });
  const dialog = page.getByRole('dialog', { name: 'Add an entry' });
  await trigger.click();
  await expect(dialog).toBeVisible();
  await dialog.getByRole('button', { name: 'Set phase', exact: true }).click();
  await expect(dialog).toBeHidden();
  await expect(page.locator('#phase-picker')).toBeFocused();

  await page.locator('#phase-picker input[type="date"]').fill(utcDateDaysAgo(3));
  await page.locator('#phase-picker select').selectOption({ label: 'Flowering' });
  await armNoReloadGuard(page);
  await page.locator('#phase-picker button', { hasText: 'Record phase' }).click();
  await expectNoReload(page);

  // The badge updates in place: Flowering counts from the flip on
  // grow day 7, which was three days ago.
  const forecast = await api<{ today_day: number }>(
    request,
    'GET',
    `api/grows/${growId}/forecast`,
  );
  await expect(page.locator('#phase-day')).toHaveText(
    `Flowering · day ${forecast.today_day - 7}`,
  );

  // The timeline entry keeps the Day count: the flip shows grow
  // day 7, distinct from the badge's phase day.
  await expect(
    page.locator('[data-testid="story-day"] li', { hasText: 'Flip 12/12' }).first(),
  ).toContainText('day 7');
});
