// Grow page fetch resilience: a page with several requests keeps
// rendering from whichever requests answer when one aborts — the
// failure lands in the shared error banner, never a blank page, and
// the empty state cannot fake "no data" for the whole grow.

import { test, expect } from '@playwright/test';
import { createGrow, createPlant, uniqueName, url } from './helpers';

test('aborted photos request keeps the grow page with an error banner', async ({
  page,
  request,
}) => {
  const name = uniqueName('E2E Tri-state Grow');
  const plantName = uniqueName('E2E Tri-state Plant');
  const growId = await createGrow(request, name);
  await createPlant(request, growId, plantName);

  // Abort the photos request under the ingress prefix; the grow,
  // plants and actions requests answer normally.
  await page.route(url(`api/grows/${growId}/photos`), (route) => route.abort());
  await page.goto(url(`/grows/${growId}`));

  // The page renders from its own fetches: the grow's heading and the
  // plant are there…
  await expect(page.getByRole('heading', { name, exact: true })).toBeVisible();
  await expect(
    page.getByRole('link', { name: plantName, exact: true }),
  ).toBeVisible();
  // …and the shared error banner shows for the aborted request.
  await expect(page.locator('p[role="alert"]')).toBeVisible();

  // Lifting the abort and reloading heals the page: no stale banner.
  await page.unroute(url(`api/grows/${growId}/photos`));
  await page.reload();
  await expect(page.locator('p[role="alert"]')).toHaveCount(0);
  await expect(page.getByRole('heading', { name, exact: true })).toBeVisible();
});
