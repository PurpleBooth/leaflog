// Undoable timeline entry deletes (D126): the × on a timeline row
// deletes with no confirmation and raises an undo banner; Undo replays
// the entry under a new id (the payload is what is captured, the
// backend mints the id), and letting the 8 s window lapse keeps the
// entry deleted.

import { test, expect, type APIRequestContext, type Page } from '@playwright/test';
import {
  api,
  createGrow,
  dismissOnboarding,
  uniqueName,
  url,
} from './helpers';

interface LoggedEntry {
  id: string;
  details: { volume_ml?: number | null };
}

test.beforeAll(async ({ request }) => {
  await dismissOnboarding(request);
});

async function seedWaterings(
  request: APIRequestContext,
  growId: string,
): Promise<void> {
  for (const volume of [250, 180]) {
    await api<unknown>(request, 'POST', `api/grows/${growId}/actions`, {
      plant_ids: [],
      occurred_at: new Date().toISOString(),
      action_type: 'water',
      details: { volume_ml: volume },
      notes: null,
      appearance_tags: [],
    });
  }
}

function waterRow(page: Page, volume: number) {
  return page
    .locator('ul.list-none li')
    .filter({ has: page.locator('strong:text-is("Water")') })
    .filter({ hasText: String(volume) });
}

test('deleting a timeline entry raises the banner and Undo brings it back', async ({
  page,
  request,
}) => {
  const growId = await createGrow(request, uniqueName('E2E Undo'));
  await seedWaterings(request, growId);
  const idsBefore = (
    await api<LoggedEntry[]>(request, 'GET', `api/grows/${growId}/actions`)
  )?.map((entry) => entry.id);
  expect(idsBefore).toHaveLength(2);

  await page.goto(url(`/grows/${growId}`));

  // No confirmation dialog: the first click deletes and raises the
  // undo banner.
  await waterRow(page, 250).getByRole('button', { name: 'Delete entry' }).click();
  await expect(page.getByText('Entry deleted.')).toBeVisible();
  await expect(waterRow(page, 250)).toHaveCount(0);

  // Undo replays the captured payload under a new id.
  await page.getByRole('button', { name: 'Undo' }).click();
  await expect(page.getByText('Entry deleted.')).toBeHidden();
  await expect(waterRow(page, 250)).toBeVisible();

  const after = await api<LoggedEntry[]>(request, 'GET', `api/grows/${growId}/actions`);
  expect(after?.map((entry) => entry.id)).toHaveLength(2);
  // Same volume, new id — ids are internal, the payload is the point.
  expect(
    after?.some(
      (entry) => entry.details.volume_ml === 250 && !idsBefore?.includes(entry.id),
    ),
  ).toBe(true);
});

test('letting the undo window lapse keeps the entry deleted', async ({
  page,
  request,
}) => {
  const growId = await createGrow(request, uniqueName('E2E Undo Lapse'));
  await seedWaterings(request, growId);
  await page.goto(url(`/grows/${growId}`));

  await waterRow(page, 180).getByRole('button', { name: 'Delete entry' }).click();
  await expect(page.getByText('Entry deleted.')).toBeVisible();
  await expect(waterRow(page, 180)).toHaveCount(0);

  // Nobody undoes: the banner's 8 s window closes on its own and the
  // row stays gone (12 s budget = window plus refresh slack).
  await expect(page.getByText('Entry deleted.')).toBeHidden({ timeout: 12_000 });
  await expect(waterRow(page, 180)).toHaveCount(0);

  const after = await api<LoggedEntry[]>(request, 'GET', `api/grows/${growId}/actions`);
  expect(after?.map((entry) => entry.details.volume_ml)).toEqual([250]);
});
