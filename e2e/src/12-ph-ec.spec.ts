// pH/EC event scatter charts (ticket 04): feed-entry measurements
// plot as ECharts scatter dots — the input series plus the runoff
// compare series in a distinct colour, on a value axis named with the
// role's unit, with a hover tooltip naming both readings. The aria
// labels the hand-rolled SVG carried survive the migration. A grow
// with no entries renders no pH/EC card at all (the card-visibility
// rule): hidden cards are the unbound state, never a dead binding.

import { expect, test, type Locator, type Page } from '@playwright/test';
import { api, createGrow, dismissOnboarding, url } from './helpers';

test.beforeAll(async ({ request }) => {
  await dismissOnboarding(request);
});

/** The dot fills on one chart, de-duplicated (input vs runoff). The
 * SVG renderer draws scatter symbols as filled paths; the chart's
 * other paths (grid, axes) carry fill="none", so the non-none fills
 * are the dots. The log-event tick glyphs (ticket 04) are narrow 2px
 * rect paths in the muted colour — this spec's feed entry is a
 * root-zone event, so its ticks draw too; shapes at most 4px wide
 * are ticks, not dots, and drop out. */
async function dotFills(svg: Locator): Promise<string[]> {
  return svg.locator('circle, path').evaluateAll((shapes) => [
    ...new Set(
      shapes
        .filter((shape) => shape.getBoundingClientRect().width > 4)
        .map((shape) => shape.getAttribute('fill') ?? getComputedStyle(shape).fill)
        .filter((fill) => fill && fill !== 'none'),
    ),
  ]);
}

/** Hovers one chart's grid so the axis tooltip snaps. */
async function hoverCenter(page: Page, svg: Locator) {
  await svg.scrollIntoViewIfNeeded();
  const box = await svg.boundingBox();
  expect(box, 'chart laid out for hover').not.toBeNull();
  await page.mouse.move(box!.x + box!.width * 0.5, box!.y + box!.height * 0.5, {
    steps: 5,
  });
}

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

  // Both charts render through ECharts under the old aria labels.
  const phChart = page.locator('svg[aria-label="ph history"]');
  await expect(phChart).toBeVisible();
  const ecChart = page.locator('svg[aria-label="ec history"]');
  await expect(ecChart).toBeVisible();

  // Each pair draws input + runoff dots in two distinct colours.
  expect(await dotFills(phChart), 'ph: two distinct dot colours').toHaveLength(
    2,
  );
  expect(await dotFills(ecChart), 'ec: two distinct dot colours').toHaveLength(
    2,
  );

  // The value axis names the role's unit: pH dimensionless, EC in
  // canonical µS/cm.
  await expect(
    phChart.locator('text').filter({ hasText: 'pH' }).first(),
  ).toBeAttached();
  await expect(
    ecChart.locator('text').filter({ hasText: 'µS/cm' }).first(),
  ).toBeAttached();

  // Hovering reveals both readings with value, unit, and time — named
  // input and runoff, so the two dot colours are tellable apart.
  const phCard = page.locator('.card', { has: phChart });
  await hoverCenter(page, phChart);
  await expect(phCard.getByText(/6\.2.*UTC/)).toBeVisible();
  await expect(phCard.getByText(/Runoff.*6\.8/)).toBeVisible();

  const ecCard = page.locator('.card', { has: ecChart });
  await hoverCenter(page, ecChart);
  await expect(ecCard.getByText(/1500.*µS\/cm.*UTC/)).toBeVisible();
});

test('a grow with no entries renders no ph or ec card', async ({
  page,
  request,
}) => {
  const growId = await createGrow(request, 'E2E PhEc Empty');
  await page.goto(url(`/grows/${growId}`));

  await expect(
    page.locator('.card', { has: page.getByRole('heading', { name: 'pH' }) }),
  ).toHaveCount(0);
  await expect(
    page.locator('.card', { has: page.getByRole('heading', { name: 'EC' }) }),
  ).toHaveCount(0);
  await expect(page.locator('svg[aria-label="ph history"]')).toHaveCount(0);
  await expect(page.locator('svg[aria-label="ec history"]')).toHaveCount(0);
});
