// Photo upload round-trips (D144, D152): quick-add saves the
// photograph in the grow story; a day card can upload an independent
// photograph with its own capture date and plant associations.

import { test, expect } from '@playwright/test';
import { createGrow, uniqueName, url } from './helpers';

// A 1×1 transparent PNG.
const PNG = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==',
  'base64',
);

const PNG_FILE = {
  name: 'e2e.png',
  mimeType: 'image/png' as const,
  buffer: PNG,
};

test('photo upload via the Foto chip lands in the gallery and survives a reload', async ({
  page,
  request,
}) => {
  const growId = await createGrow(request, uniqueName('E2E Photo Grow'));
  await page.goto(url(`/grows/${growId}`));

  const trigger = page.getByRole('button', { name: 'Quick add', exact: true });
  const dialog = page.getByRole('dialog', { name: 'Add an entry' });
  await trigger.click();
  await dialog.getByRole('button', { name: 'Photo', exact: true }).click();
  await expect(dialog).toBeHidden();
  await expect(page.locator('#quick-add-form')).toBeFocused();

  // The quick-add input is visible while the per-day upload remains
  // inside a closed details element.
  const input = page.locator('#quick-add-form input[type=file]');
  await input.setInputFiles(PNG_FILE);
  await page.getByRole('button', { name: 'Log it' }).click();

  await expect(page.getByText('Logged to the timeline below.')).toBeVisible();

  const thumb = page.locator('[data-testid="story-day"] img[alt]').first();
  await expect(thumb).toBeVisible();
  // The thumbnail actually decoded: it loaded from the backend.
  expect(
    await thumb.evaluate((el) => (el as HTMLImageElement).naturalWidth),
  ).toBeGreaterThan(0);

  await page.reload();
  await expect(page.locator('[data-testid="story-day"] img[alt]').first()).toBeVisible();
});

test('a day upload records its capture date and plant without an action entry', async ({
  page,
  request,
}) => {
  const growId = await createGrow(request, uniqueName('E2E Story Photo'));
  const seeded = await request.post(`api/grows/${growId}/actions`, {
    data: {
      plant_ids: [], occurred_at: '2026-11-20T10:00:00Z', action_type: 'note',
      details: {}, notes: 'Today in the story', appearance_tags: [],
    },
  });
  expect(seeded.ok()).toBeTruthy();
  const seededStory = await request.get(`api/grows/${growId}/story`);
  expect((await seededStory.json()).days).toHaveLength(1);
  await page.goto(url(`/grows/${growId}`));

  const day = page.locator('[data-testid="story-day"]', { hasText: 'Nov 20, 2026' });
  await day.locator('summary').click();
  await day.locator('input[type=file]').setInputFiles(PNG_FILE);
  await day.locator('input[type=datetime-local]').fill('2020-01-02T03:04');
  await day.getByRole('button', { name: 'Upload photo', exact: true }).click();
  // A capture date outside the current seven-day page is reachable by
  // paginating, not by inserting a huge empty run of calendar days.
  for (let pageNumber = 0; pageNumber < 450; pageNumber += 1) {
    if (await page.locator('[data-testid="story-day"]', { hasText: 'Jan 2, 2020' }).count()) {
      break;
    }
    await page.getByRole('button', { name: 'Load older days' }).click();
    await expect(page.locator('[role="status"]', { hasText: 'Loading…' })).toHaveCount(0);
  }
  const photo = page.locator('[data-testid="story-day"]', { hasText: 'Jan 2, 2020' });
  await expect(photo.locator('img[alt]')).toBeVisible();
  await expect(photo.getByText(/Jan 2, 2020.*Whole grow/)).toBeVisible();
  await expect(photo.locator('strong', { hasText: 'Photo' })).toHaveCount(0);
  await expect(page.locator('input[type=file]:visible')).toHaveCount(0);
});
