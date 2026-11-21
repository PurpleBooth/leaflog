// Photo upload round-trip (D144): the Foto quick-add chip stages the
// files, Log saves the timeline entry first and uploads the files
// second (linked via action_log_id), and the gallery re-renders with
// the stored photo. The grow page itself has no upload fieldset.

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

  // The upload input lives in the quick-add form only.
  const input = page.locator('#quick-add-form input[type=file]');
  await input.setInputFiles(PNG_FILE);
  await page.getByRole('button', { name: 'Log it' }).click();

  await expect(page.getByText('Photos uploaded')).toBeVisible();
  // The timeline entry saved too — upload and entry share one flow.
  await expect(
    page
      .locator('ul.list-none li')
      .filter({ has: page.locator('strong:text-is("Photo")') }),
  ).toBeVisible();

  const thumb = page.locator('img[alt^="day "]');
  await expect(thumb).toBeVisible();
  // The thumbnail actually decoded: it loaded from the backend.
  expect(
    await thumb.evaluate((el) => (el as HTMLImageElement).naturalWidth),
  ).toBeGreaterThan(0);

  await page.reload();
  await expect(page.locator('img[alt^="day "]')).toBeVisible();
});

test('the grow page has no standalone upload fieldset', async ({
  page,
  request,
}) => {
  const growId = await createGrow(request, uniqueName('E2E Photo Grow'));
  await page.goto(url(`/grows/${growId}`));

  await expect(page.locator('input[type=file]')).toHaveCount(0);
  // The gallery itself stays: already-uploaded photos still render.
  await expect(page.getByRole('heading', { name: 'Photos' })).toBeVisible();
});
