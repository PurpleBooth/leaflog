// Live values on the env strip (grow-page rework ticket 02): the
// overview renders no Live sensors section at all — no heading, no
// "Loading…" status line — and the six environment tiles (Temp, RH,
// VPD, RH window, CO₂, soil moisture) are the single live surface:
// one EventSource subscription on the existing SSE endpoint updates
// them as the mock pushes state, with browser-native reconnect and
// quiet degradation (last values or "—", never an alarm banner).
// The old 5-second poll is gone: one open connection per page.

import { test, expect, type Page } from '@playwright/test';
import { api, createGrow, dismissOnboarding, uniqueName, url } from './helpers';

/** The six tile labels, in strip order. */
const LABELS = ['Temp', 'RH', 'VPD', 'RH window', 'CO₂', 'Soil moisture'];

test.beforeAll(async ({ request }) => {
  await dismissOnboarding(request);
});

/** One tile of the env strip, by its exact caption. */
function tile(page: Page, label: string) {
  return page.locator('#env-strip').getByText(label, { exact: true });
}

/** The value line of one tile (the caption's sibling span). */
function tileValue(page: Page, label: string) {
  return tile(page, label).locator('xpath=following-sibling::span[1]');
}

test('six env tiles update live over one SSE connection', async ({
  page,
  request,
}) => {
  // The value-movement poll below may legitimately wait out a slow
  // rounding step on the derived RH-window tile (its display rounds
  // to whole percent, the stream moves 0.3 %/tick).
  test.setTimeout(75_000);
  const name = uniqueName('E2E Live Strip Grow');
  const growId = await createGrow(request, name);

  // Bind all four numeric sensors so every tile has a live source
  // (CO₂ and soil moisture are live-only readings).
  await api(request, 'PUT', `api/grows/${growId}/bindings`, {
    bindings: {
      temperature: 'sensor.grow_temperature',
      humidity: 'sensor.grow_humidity',
      co2: 'sensor.grow_co2',
      soil_moisture: 'sensor.grow_soil_moisture',
    },
  });
  await expect
    .poll(
      async () => {
        const env = await api<{
          bindings: Array<{ role: string; available: boolean }>;
        }>(request, 'GET', `api/grows/${growId}/env`);
        return env!.bindings.every((binding) => binding.available);
      },
      { timeout: 10_000 },
    )
    .toBe(true);

  // Count every request to the SSE endpoint from navigation on: the
  // EventSource connection is request one and must stay the only one
  // (the retired 5-second poll would add more within this test).
  let liveRequests = 0;
  page.on('request', (req) => {
    if (req.url().includes('/api/live')) liveRequests += 1;
  });

  await page.goto(url(`/grows/${growId}`));
  await expect(
    page.getByRole('heading', { name, exact: true }),
  ).toBeVisible();

  // The Live sensors section is gone entirely: no heading, and no
  // permanent "Loading…" line anywhere on the settled page.
  await expect(page.getByText('Live sensors')).toHaveCount(0);
  await expect(page.getByText('Loading…', { exact: true })).toHaveCount(0);

  // All six tiles render real readings from the bound entities.
  for (const label of LABELS) {
    await expect(tile(page, label)).toBeVisible();
  }
  await expect(tileValue(page, 'Temp')).toContainText('°C /');
  await expect(tileValue(page, 'RH')).toContainText('%');
  await expect(tileValue(page, 'VPD')).toContainText('kPa');
  await expect(tileValue(page, 'CO₂')).toContainText('ppm');
  await expect(tileValue(page, 'Soil moisture')).toContainText('%');

  // Live updates: the mock pushes every 2 s, and each tile's value
  // text must move off its first reading (derived tiles recompute —
  // VPD and the RH window follow the live temperature, not a stale
  // snapshot). The slowest digit to roll is the RH window's, so the
  // poll allows a generous window.
  const initial = await Promise.all(
    LABELS.map(async (label) => tileValue(page, label).innerText()),
  );
  await expect
    .poll(
      async () => {
        const now = await Promise.all(
          LABELS.map(async (label) => tileValue(page, label).innerText()),
        );
        return LABELS.every(
          (_, index) => now[index] !== initial[index],
        );
      },
      { message: 'every tile value moves off its first reading', timeout: 40_000 },
    )
    .toBe(true);

  // One connection per page: the stream stayed open through all the
  // pushes above (longer than the retired poll's 5-second interval).
  expect(liveRequests, 'exactly one request to the SSE endpoint').toBe(1);
  await page.waitForTimeout(1_500);
  expect(
    liveRequests,
    'still one request after another poll interval would have fired',
  ).toBe(1);
});

test('a dead SSE stream degrades quietly, without an alarm banner', async ({
  page,
  request,
}) => {
  const name = uniqueName('E2E Quiet Grow');
  const growId = await createGrow(request, name);
  await api(request, 'PUT', `api/grows/${growId}/bindings`, {
    bindings: {
      temperature: 'sensor.grow_temperature',
      humidity: 'sensor.grow_humidity',
    },
  });
  await expect
    .poll(
      async () => {
        const env = await api<{ temperature_c: number | null }>(
          request,
          'GET',
          `api/grows/${growId}/env`,
        );
        return env!.temperature_c !== null;
      },
      { timeout: 10_000 },
    )
    .toBe(true);

  await page.goto(url(`/grows/${growId}`));
  await expect(tile(page, 'RH window')).toBeVisible();
  await expect(tileValue(page, 'Temp')).toContainText('°C /');

  // The page stays open across an outage of the live stream: from
  // here on every request to the SSE endpoint is cut. The tiles keep
  // showing their values (the backend's, via the env snapshot and the
  // last received events) and no alarm banner appears — the browser's
  // native reconnect works silently in the background.
  await page.route('**/api/live*', (route) => route.abort());
  await page.waitForTimeout(4_000);

  await expect(tileValue(page, 'Temp')).toContainText('°C /');
  await expect(
    page.getByRole('alert'),
    'a cut stream never raises an alarm',
  ).toHaveCount(0);
  await expect(page.getByText('unreachable')).toHaveCount(0);
});

test('the dead-binding banner points at the grow edit page', async ({
  page,
  request,
}) => {
  const name = uniqueName('E2E Dead Binding Grow');
  const growId = await createGrow(request, name);

  // Bind temperature to an entity the mock never serves: a dead
  // binding, the only thing the overview may call "no data".
  await api(request, 'PUT', `api/grows/${growId}/bindings`, {
    bindings: {
      temperature: 'sensor.ghost',
      humidity: 'sensor.grow_humidity',
    },
  });
  await expect
    .poll(
      async () => {
        const env = await api<{
          bindings: Array<{ role: string; available: boolean }>;
        }>(request, 'GET', `api/grows/${growId}/env`);
        return env!.bindings;
      },
      { timeout: 10_000 },
    )
    .toEqual(
      expect.arrayContaining([
        expect.objectContaining({ role: 'temperature', available: false }),
        expect.objectContaining({ role: 'humidity', available: true }),
      ]),
    );

  await page.goto(url(`/grows/${growId}`));
  const banner = page.locator('p[role="status"]', { hasText: 'No data:' });
  await expect(banner).toBeVisible();
  await expect(banner).toContainText(
    'rebind it on the grow edit page',
  );

  // The dead tile reads "—", not a stale or invented number.
  await expect(tileValue(page, 'Temp')).toHaveText('—');

  // The named fix-it place is a working link: it navigates to the
  // grow edit page, where the Entity bindings now live (ticket 01).
  const editLink = banner.getByRole('link', { name: 'grow edit page' });
  await expect(editLink).toHaveAttribute(
    'href',
    new RegExp(`/grows/${growId}/edit$`),
  );
  await editLink.click();
  await expect(
    page.getByRole('heading', { name: 'Edit grow' }),
  ).toBeVisible();
});
