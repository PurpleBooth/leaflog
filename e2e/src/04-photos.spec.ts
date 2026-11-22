// Photo upload round-trips (D144, D152): quick-add saves the
// photograph in the grow story; a day card can upload an independent
// photograph with its own capture date and plant associations.

import { test, expect } from '@playwright/test';
import { createGrow, createPlant, uniqueName, url } from './helpers';

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
  const now = Date.now();
  const dateDaysAgo = (days: number) => new Date(now - days * 86_400_000).toISOString().slice(0, 10);
  const noteDate = dateDaysAgo(2);
  // Put the photo on the previous seven-day page, before the grow started.
  const captureDate = dateDaysAgo(9);
  const growId = await createGrow(request, uniqueName('E2E Story Photo'), { started_on: noteDate });
  const plantName = uniqueName('E2E Photo Plant');
  const plantId = await createPlant(request, growId, plantName);
  const seeded = await request.post(url(`/api/grows/${growId}/actions`), {
    data: {
      plant_ids: [], occurred_at: `${noteDate}T12:00:00Z`, action_type: 'note',
      details: {}, notes: 'Seed note for photo upload', appearance_tags: [],
    },
  });
  expect(seeded.ok()).toBeTruthy();
  await page.goto(url(`/grows/${growId}`));

  const days = page.getByTestId('story-day');
  const day = days.filter({ hasText: 'Seed note for photo upload' });
  await day.locator('summary').click();
  await day.locator('input[type=file]').setInputFiles(PNG_FILE);
  await day.locator('input[type=datetime-local]').fill(`${captureDate}T03:04`);
  await day.getByRole('checkbox', { name: plantName }).check();
  const uploaded = page.waitForResponse((response) =>
    new URL(response.url()).pathname === url(`/api/grows/${growId}/photos`) &&
    response.request().method() === 'POST',
  );
  await day.getByRole('button', { name: 'Upload photo', exact: true }).click();
  expect((await uploaded).ok()).toBeTruthy();
  await expect(day.locator('input[type=file]')).toBeHidden();
  await expect(days.locator('img[alt]')).toHaveCount(0);

  await page.getByRole('button', { name: 'Load older days' }).click();
  const photo = days.filter({ has: page.locator('img[alt]') });
  await expect(photo.getByRole('img', { name: plantName, exact: false })).toBeVisible();

  const older = await request.get(url(`/api/grows/${growId}/story?before=${noteDate}`));
  expect(older.ok()).toBeTruthy();
  expect((await older.json()).days).toEqual([
    expect.objectContaining({
      date: captureDate,
      actions: [],
      photos: [expect.objectContaining({ plant_ids: [plantId], action_log_id: null })],
    }),
  ]);
});
