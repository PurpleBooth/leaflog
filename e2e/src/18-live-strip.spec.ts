// Live values on the env strip (grow-page rework ticket 02): the
// overview renders no Live sensors section at all — no heading, no
// "Loading…" status line — and the five environment tiles (Temp, RH,
// VPD, CO₂, soil moisture; the RH window rides under RH as a band
// footnote, D136) are the single live surface: one EventSource
// subscription on the existing SSE endpoint updates them as the mock
// pushes state, with browser-native reconnect and quiet degradation
// (last values or "—", never an alarm banner). The old 5-second poll
// is gone: one open connection per page.

import { test, expect, type Page } from '@playwright/test';
import { api, createGrow, dismissOnboarding, uniqueName, url } from './helpers';

/** The five tile labels, in strip order. */
const LABELS = ['Temp', 'RH', 'VPD', 'CO₂', 'Soil moisture'];

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

/** The tile's frame div (the caption span's parent) — its class is the colour. */
function tileFrame(page: Page, label: string) {
  return tile(page, label).locator('xpath=..');
}

test('five env tiles update live over one SSE connection', async ({
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

  // All five tiles render real readings from the bound entities.
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
  // VPD follows the live temperature and humidity, not a stale
  // snapshot).
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

test('tile colours follow a live push across the threshold', async ({
  page,
  request,
}) => {
  const name = uniqueName('E2E Live Colour Grow');
  const growId = await createGrow(request, name);
  await api(request, 'PUT', `api/grows/${growId}/bindings`, {
    bindings: {
      temperature: 'sensor.grow_temperature',
      humidity: 'sensor.grow_humidity',
    },
  });

  // Read the grow's thresholds to derive deterministic readings
  // (rounded to the display's one decimal); the mock's own drift is
  // irrelevant — the crafted events below control exactly what the
  // tile sees.
  const env = await api<{
    thresholds: { floor_c: number; ceiling_c: number };
  }>(request, 'GET', `api/grows/${growId}/env`);
  const round1 = (celsius: number) => Math.round(celsius * 10) / 10;

  // Fulfil the SSE endpoint with crafted events instead of the
  // mock's drift. The browser reconnects after each fulfilled
  // one-event stream and picks up the current reading, so flipping
  // `reading` moves the tile twice: an in-band value first (success
  // tint), then one past the ceiling (warning tint).
  let reading = round1(env!.thresholds.ceiling_c - 5);
  await page.route('**/api/live*', (route) =>
    route.fulfill({
      status: 200,
      headers: {
        'content-type': 'text/event-stream',
        'cache-control': 'no-cache',
      },
      body: `retry: 1000\ndata: ${JSON.stringify({
        connected: true,
        entities: {
          'sensor.grow_temperature': {
            entity_id: 'sensor.grow_temperature',
            state: String(reading),
            attributes: {},
          },
          'sensor.grow_humidity': {
            entity_id: 'sensor.grow_humidity',
            state: '60',
            attributes: {},
          },
        },
      })}\n\n`,
    }),
  );

  await page.goto(url(`/grows/${growId}`));
  await expect(tile(page, 'Temp')).toBeVisible();

  // The in-band live reading paints the tile with the success tint —
  // the colour tracks the merged live values, not the page-load
  // snapshot's flags.
  await expect(tileValue(page, 'Temp')).toContainText(String(reading));
  await expect(tileFrame(page, 'Temp')).toHaveClass(/border-success/);

  reading = round1(env!.thresholds.ceiling_c + 10);
  await expect(tileValue(page, 'Temp')).toContainText(String(reading));
  await expect(tileFrame(page, 'Temp')).toHaveClass(/border-warning/);
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
  await expect(page.locator('#env-strip').getByText(/band: \d+–\d+ %/)).toBeVisible();
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

test('a dead binding marks its tile and points at the grow edit page', async ({
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

  // The dead tile reads "—", not a stale or invented number.
  await expect(tileValue(page, 'Temp')).toHaveText('—');

  // One quiet warning mark rides on the dead tile's caption — the
  // healthy RH tile stays unmarked (one mark per dead binding, not a
  // page-wide alarm).
  const mark = tile(page, 'Temp').locator('xpath=following-sibling::a[1]');
  await expect(mark).toHaveText('⚠');
  await expect(tile(page, 'RH').locator('xpath=following-sibling::a[1]')).toHaveCount(0);

  // The mark's accessible name carries the "no data" truth (spec
  // §4.1) and names the fix-it place: a working link to the grow
  // edit page, where the Entity bindings now live (ticket 01).
  await expect(mark).toHaveAttribute(
    'aria-label',
    'No data: sensor.ghost — rebind it on the grow edit page',
  );
  await expect(mark).toHaveAttribute(
    'href',
    new RegExp(`/grows/${growId}/edit$`),
  );
  await mark.click();
  await expect(
    page.getByRole('heading', { name: 'Edit grow' }),
  ).toBeVisible();
});
