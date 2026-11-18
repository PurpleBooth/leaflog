// Log-event markers on the environment charts (ticket 04): the
// history response carries the role-relevant action-log events inside
// the window as `markers`, each chart draws them as quiet vertical
// ticks near the axis floor, the tooltip names the event instead of a
// reading, and one Markers switch in the section header hides and
// shows every chart's ticks at once. The tick geometry (muted colour,
// 2x12 rects) is unit-tested in Rust against both themes; these cases
// assert the API selection and the rendered DOM.

import { expect, test, type Locator, type Page } from '@playwright/test';
import { api, createGrow, dismissOnboarding, markerTicks, revealRootZone, url } from './helpers';

test.beforeAll(async ({ request }) => {
  await dismissOnboarding(request);
});

/** A log action at `hours` before now. */
async function logAction(
  request: import('@playwright/test').APIRequestContext,
  growId: string,
  actionType: string,
  hoursAgo: number,
  details: Record<string, unknown> = {},
) {
  await api(request, 'POST', `api/grows/${growId}/actions`, {
    plant_ids: [],
    occurred_at: new Date(Date.now() - hoursAgo * 3_600_000).toISOString(),
    action_type: actionType,
    details,
    notes: null,
    appearance_tags: [],
  });
}

/** One role's history markers for a window. */
async function fetchMarkers(
  request: import('@playwright/test').APIRequestContext,
  growId: string,
  role: string,
  window: string,
) {
  const history = await api<{ markers: { at: string; kind: string }[] }>(
    request,
    'GET',
    `api/grows/${growId}/history?role=${role}&window=${window}`,
  );
  return history!.markers.map((marker) => marker.kind);
}

test('the history markers carry each role its in-window events', async ({
  request,
}) => {
  const growId = await createGrow(request, 'E2E Markers API');

  // Root-zone events for soil_moisture, air events for temperature,
  // one water outside the day window (but inside the week), and a
  // note — never a marker for any role.
  await logAction(request, growId, 'water', 2);
  await logAction(request, growId, 'feed', 3);
  await logAction(request, growId, 'transplant', 5);
  await logAction(request, growId, 'lights', 4);
  await logAction(request, growId, 'phase_change', 6, {
    target_phase: 'vegetative',
  });
  await logAction(request, growId, 'water', 30);
  await logAction(request, growId, 'note', 1);

  // Root zone sees the watering trio, oldest first; the note and the
  // out-of-window water stay out.
  expect(await fetchMarkers(request, growId, 'soil_moisture', 'day')).toEqual(
    ['transplant', 'feed', 'water'],
  );
  // The air role sees the light transition and the phase pick only —
  // the watering events are not its story.
  expect(await fetchMarkers(request, growId, 'temperature', 'day')).toEqual([
    'phase_change',
    'lights',
  ]);
  // The week window reaches back to the 30-hour-old watering.
  expect(await fetchMarkers(request, growId, 'soil_moisture', 'week')).toEqual(
    ['water', 'transplant', 'feed', 'water'],
  );
  // Roles outside both zones (light carries no markers by design).
  expect(await fetchMarkers(request, growId, 'light', 'day')).toEqual([]);
});

test('marker ticks draw, hide behind the toggle, and name the event', async ({
  page,
  request,
}) => {
  const growId = await createGrow(request, 'E2E Markers UI');

  // Bind the mock-fed temperature role (a live reading makes the
  // action log auto-capture a point the line can draw) and the
  // soil moisture role for the root-zone half.
  await api(request, 'PUT', `api/grows/${growId}/bindings`, {
    bindings: {
      temperature: 'sensor.grow_temperature',
      soil_moisture: 'sensor.grow_soil_moisture',
    },
  });
  await expect
    .poll(async () => {
      const env = await api<{ temperature_c: number | null }>(
        request,
        'GET',
        `api/grows/${growId}/env`,
      );
      return env!.temperature_c !== null;
    })
    .toBe(true);

  // A snapshot point plus both zones' events, two hours back — well
  // inside the 24h window and clear of the window's right edge.
  await logAction(request, growId, 'note', 0);
  await logAction(request, growId, 'lights', 2);
  await logAction(request, growId, 'water', 2);

  await page.goto(url(`/grows/${growId}`));
  await revealRootZone(page);
  const tempCard = page.locator('.card', {
    has: page.locator('svg[aria-label="temperature history"]'),
  });
  const tempSvg = tempCard.locator('svg[aria-label="temperature history"]');
  const soilSvg = page.locator('svg[aria-label="soil_moisture history"]');
  await expect(tempSvg).toBeVisible();
  await expect(soilSvg).toBeVisible();

  // The 24h window puts the two-hour-old events at nine tenths of
  // the width, away from the live snapshot cluster at the edge.
  await page.getByRole('button', { name: '24h' }).first().click();

  // Both zones render their own event: one tick each (plus more from
  // nothing else — the note logs no marker).
  await expect
    .poll(async () => (await markerTicks(tempSvg)).length, 'temperature tick')
    .toBeGreaterThanOrEqual(1);
  await expect
    .poll(async () => (await markerTicks(soilSvg)).length, 'soil tick')
    .toBeGreaterThanOrEqual(1);

  // The section-header switch hides every chart's ticks at once.
  const toggle = page.getByLabel('Markers');
  await toggle.uncheck();
  await expect
    .poll(async () => (await markerTicks(tempSvg)).length, 'ticks hidden')
    .toBe(0);
  expect(await markerTicks(soilSvg), 'soil ticks hidden too').toHaveLength(0);

  // Back on, and the hover tooltip names the event: the tick's y is
  // placement geometry, so the line says what happened, not a value.
  await toggle.check();
  await expect
    .poll(async () => (await markerTicks(tempSvg)).length, 'ticks back')
    .toBeGreaterThanOrEqual(1);
  await hoverAt(page, tempSvg, 0.9, 0.5);
  await expect(tempCard.getByText(/Lights — .*UTC/)).toBeVisible();
});

/** Hovers one chart at a relative position inside its box. */
async function hoverAt(
  page: Page,
  svg: Locator,
  xRatio: number,
  yRatio: number,
) {
  await svg.scrollIntoViewIfNeeded();
  const box = await svg.boundingBox();
  expect(box, 'chart laid out for hover').not.toBeNull();
  await page.mouse.move(
    box!.x + box!.width * xRatio,
    box!.y + box!.height * yRatio,
    { steps: 5 },
  );
}
