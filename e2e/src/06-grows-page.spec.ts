// Grows page fetch resilience: the page fetches grows, strains and
// spaces separately, so one aborted request cannot blank the list or
// fake an empty state, and the shared error banner offers a retry.

import { test, expect } from '@playwright/test';
import { createGrow, uniqueName, url } from './helpers';

test('aborted strains request keeps the grows list with an error banner', async ({
  page,
  request,
}) => {
  const name = uniqueName('E2E Tri-state Grow');
  await createGrow(request, name);

  // Abort the strains request under the ingress prefix; the grows and
  // spaces requests answer normally.
  await page.route(url('api/strains'), (route) => route.abort());
  await page.goto(url('/grows'));

  // The list renders from its own fetch…
  const card = page.locator('li.card', { hasText: name });
  await expect(card).toBeVisible();
  // …the shared error banner shows for the aborted request…
  await expect(page.locator('p[role="alert"]')).toBeVisible();
  // …and the empty state never claims the list is gone.
  await expect(page.getByText('No grows yet — start your first one.')).toHaveCount(
    0,
  );

  // Retry recovers without navigation: lift the abort and retry.
  await page.unroute(url('api/strains'));
  await page.getByRole('button', { name: 'Retry' }).click();
  await expect(page.locator('p[role="alert"]')).toHaveCount(0);
  await expect(card).toBeVisible();
});
