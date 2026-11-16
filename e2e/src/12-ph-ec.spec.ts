// pH/EC event scatter charts (ticket 08): feed-entry measurements
// plot as dots — input series plus the runoff compare series.

import { test, expect } from '@playwright/test';
import { api, createGrow, dismissOnboarding, url } from './helpers';

test.beforeAll(async ({ request }) => {
  await dismissOnboarding(request);
});

test('feed ph and ec measurements plot as scatter points', async ({
  page,
  request,
}) => {
  const growId = await createGrow(request, 'E2E PhEc');
  await api<{ id: string }>(request, 'POST', `api/grows/${growId}/actions`, {
    plant_ids: [],
    occurred_at: new Date().toISOString(),
    action_type: 'feed',
    details: {
      input_ph: 6.2,
      runoff_ph: 6.8,
      input_ec_us: 1200,
      runoff_ec_us: 1500,
    },
    notes: null,
    appearance_tags: [],
  });

  await page.goto(url(`/grows/${growId}`));

  // Both charts render dots: the main series and the runoff overlay.
  const phChart = page.locator('svg[aria-label="ph history"]');
  await expect(phChart).toBeVisible();
  await expect(phChart.locator('circle.fill-primary')).toHaveCount(1);
  await expect(phChart.locator('circle.fill-secondary')).toHaveCount(1);

  const ecChart = page.locator('svg[aria-label="ec history"]');
  await expect(ecChart).toBeVisible();
  await expect(ecChart.locator('circle.fill-primary')).toHaveCount(1);
  await expect(ecChart.locator('circle.fill-secondary')).toHaveCount(1);
});
