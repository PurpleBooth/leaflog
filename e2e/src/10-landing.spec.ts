// The landing page (ticket 11): '/' renders the most recently updated
// active grow's overview, and /grows/:id deep links keep working.

import { test, expect } from '@playwright/test';
import { createGrow, dismissOnboarding, uniqueName, url } from './helpers';

test.beforeAll(async ({ request }) => {
  await dismissOnboarding(request);
});

test('landing shows the newest active grow, not the home page', async ({
  page,
  request,
}) => {
  const nameA = uniqueName('E2E Land A');
  await createGrow(request, nameA);
  // Distinct updated_at seconds, so "newest" is unambiguous.
  await page.waitForTimeout(1100);
  const nameB = uniqueName('E2E Land B');
  await createGrow(request, nameB);

  await page.goto(url('/'));
  // The grow overview rendered, not the marketing home.
  await expect(page.locator('#phase-picker')).toBeVisible();
  await expect(page.getByText(nameB)).toBeVisible();
  await expect(page.getByText(nameA)).toHaveCount(0);
});

test('grows/:id deep links still render the overview', async ({
  page,
  request,
}) => {
  const nameA = uniqueName('E2E Land Deep');
  const growId = await createGrow(request, nameA);
  await page.goto(url(`/grows/${growId}`));
  await expect(page.locator('#phase-picker')).toBeVisible();
  await expect(page.getByText(nameA)).toBeVisible();
});
