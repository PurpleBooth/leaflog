// Environment section skeleton (ticket 01): the live tiles and the
// history charts sit under one Environment heading with no separate
// History heading, the section opens above the Forecast, and one
// segmented 24h / 7d / 30d control in the section header drives every
// chart card. The default window is 7 days on a first visit, and the
// choice persists across page loads in localStorage. Per-card toggles
// stay until the grouping ticket retires them.

import { test, expect, type Locator, type Page } from '@playwright/test';
import { api, createGrow, dismissOnboarding, url } from './helpers';

test.beforeAll(async ({ request }) => {
  await dismissOnboarding(request);
});

/** Binds the two mock-fed roles and waits for both to be available. */
async function bindAndAwaitLive(
  request: import('@playwright/test').APIRequestContext,
  growId: string,
) {
  await api(request, 'PUT', `api/grows/${growId}/bindings`, {
    bindings: {
      temperature: 'sensor.grow_temperature',
      humidity: 'sensor.grow_humidity',
    },
  });
  await expect
    .poll(async () => {
      const env = await api<{
        bindings: Array<{ role: string; available: boolean }>;
      }>(request, 'GET', `api/grows/${growId}/env`);
      return env!.bindings.every((binding) => binding.available);
    }, { timeout: 10_000 })
    .toBe(true);
}

/** One log entry auto-captures the bound roles, so the charts get
 *  in-window snapshot points. */
async function seedSnapshots(request: import('@playwright/test').APIRequestContext, growId: string) {
  for (const note of ['first', 'second', 'third']) {
    await api(request, 'POST', `api/grows/${growId}/actions`, {
      plant_ids: [],
      occurred_at: new Date().toISOString(),
      action_type: 'note',
      details: {},
      notes: note,
      appearance_tags: [],
    });
  }
}

/** Opens the grow page and waits for the temperature card to mount. */
async function openGrowPage(page: Page, growId: string) {
  await page.goto(url(`/grows/${growId}`));
  await expect(
    page.locator('svg[aria-label="temperature history"]'),
  ).toBeVisible();
}

/** The section header's segmented window control. */
function sectionControl(page: Page): Locator {
  return page.getByRole('group', { name: 'History window' });
}

/** A chart card by its svg's aria-label. */
function cardOf(page: Page, label: string): Locator {
  return page.locator('.card', {
    has: page.locator(`svg[aria-label="${label}"]`),
  });
}

test('one Environment section holds tiles and charts, above the Forecast', async ({
  page,
  request,
}) => {
  const growId = await createGrow(request, 'E2E Env Section');
  await bindAndAwaitLive(request, growId);
  await seedSnapshots(request, growId);
  await openGrowPage(page, growId);

  // One Environment heading; the separate History heading is gone.
  await expect(page.getByRole('heading', { name: 'Environment' })).toHaveCount(1);
  await expect(page.getByRole('heading', { name: 'History' })).toHaveCount(0);

  // The live tiles are the section's top row: the env strip renders
  // inside the page, above the first chart card.
  await expect(page.locator('#env-strip')).toBeVisible();

  // Section order: Environment, Forecast, Feed schedule, Timeline,
  // Plants, Photos.
  const headings = await page
    .getByRole('heading')
    .evaluateAll((els) => els.map((el) => el.textContent?.trim() ?? ''));
  const at = (name: string) => headings.indexOf(name);
  expect(at('Environment'), `headings: ${headings.join(' | ')}`).toBeGreaterThanOrEqual(0);
  expect(at('Environment')).toBeLessThan(at('Forecast'));
  expect(at('Forecast')).toBeLessThan(at('Feed schedule'));
  expect(at('Feed schedule')).toBeLessThan(at('Timeline'));
  expect(at('Timeline')).toBeLessThan(at('Plants'));
  expect(at('Plants')).toBeLessThan(at('Photos'));
});

test('the shared control defaults to 7d and drives every chart card', async ({
  page,
  request,
}) => {
  const growId = await createGrow(request, 'E2E Shared Window');
  await bindAndAwaitLive(request, growId);
  await seedSnapshots(request, growId);
  await openGrowPage(page, growId);

  // First visit: the control shows the 7-day default as active, with
  // exactly three positions — no disabled teaser for a fourth.
  const control = sectionControl(page);
  await expect(control.getByRole('button')).toHaveCount(3);
  await expect(control.getByRole('button', { name: '7d' })).toHaveClass(/btn-primary/);
  await expect(control.getByRole('button', { name: '24h' })).not.toHaveClass(/btn-primary/);
  await expect(control.getByRole('button', { name: '30d' })).not.toHaveClass(/btn-primary/);

  // Every card follows the shared window: both cards' own toggles
  // (still present until the grouping ticket) read 7d active.
  for (const role of ['temperature history', 'humidity history']) {
    const card = cardOf(page, role);
    await expect(card.getByRole('group', { name: 'Chart window' })).toHaveCount(1);
    await expect(card.getByRole('button', { name: '7d' })).toHaveClass(/btn-primary/);
  }

  // Selecting 24h refetches every chart for the new window.
  const dayFetches = ['temperature', 'humidity'].map(
    (role) =>
      new Promise<void>((resolve) => {
        page.waitForResponse(
          (response) =>
            response.url().includes(`history?role=${role}&window=day`) &&
            response.status() === 200,
        ).then(() => resolve());
      }),
  );
  await control.getByRole('button', { name: '24h' }).click();
  await Promise.all(dayFetches);

  // The section control and every card's toggle now read 24h.
  await expect(control.getByRole('button', { name: '24h' })).toHaveClass(/btn-primary/);
  for (const role of ['temperature history', 'humidity history']) {
    await expect(cardOf(page, role).getByRole('button', { name: '24h' })).toHaveClass(/btn-primary/);
  }
});

test('the chosen window persists across page loads in localStorage', async ({
  page,
  request,
}) => {
  const growId = await createGrow(request, 'E2E Window Memory');
  await bindAndAwaitLive(request, growId);
  await seedSnapshots(request, growId);
  await openGrowPage(page, growId);

  const monthFetch = page.waitForResponse(
    (response) =>
      response.url().includes('history?role=temperature&window=month') &&
      response.status() === 200,
  );
  await sectionControl(page).getByRole('button', { name: '30d' }).click();
  await monthFetch;

  // The choice landed in localStorage under the app's own key.
  expect(
    await page.evaluate(() => localStorage.getItem('leaflog.history-window')),
  ).toBe('month');

  // A fresh page load reads the stored window, not the default.
  await page.reload();
  await expect(
    page.locator('svg[aria-label="temperature history"]'),
  ).toBeVisible();
  await expect(
    sectionControl(page).getByRole('button', { name: '30d' }),
  ).toHaveClass(/btn-primary/);
  await expect(
    sectionControl(page).getByRole('button', { name: '7d' }),
  ).not.toHaveClass(/btn-primary/);
  await expect(
    cardOf(page, 'temperature history').getByRole('button', { name: '30d' }),
  ).toHaveClass(/btn-primary/);
});
