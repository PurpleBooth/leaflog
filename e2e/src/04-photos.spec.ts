// Photo upload round-trip: the multipart input posts under the ingress
// prefix and the gallery re-renders with the stored photo.

import { test, expect } from '@playwright/test';
import { createGrow, uniqueName, url } from './helpers';

// A 1×1 transparent PNG.
const PNG = Buffer.from(
  'iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==',
  'base64',
);

test('photo upload lands in the gallery and survives a reload', async ({
  page,
  request,
}) => {
  const growId = await createGrow(request, uniqueName('E2E Photo Grow'));
  await page.goto(url(`/grows/${growId}`));

  await page.setInputFiles('input[type=file]', {
    name: 'e2e.png',
    mimeType: 'image/png',
    buffer: PNG,
  });
  await expect(page.getByText('Photos uploaded')).toBeVisible();

  const thumb = page.locator('img[alt^="day "]');
  await expect(thumb).toBeVisible();
  // The thumbnail actually decoded: it loaded from the backend.
  expect(
    await thumb.evaluate((el) => (el as HTMLImageElement).naturalWidth),
  ).toBeGreaterThan(0);

  await page.reload();
  await expect(page.locator('img[alt^="day "]')).toBeVisible();
});
