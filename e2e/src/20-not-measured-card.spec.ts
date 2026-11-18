// The not-measured card (ticket 03): a grow with partial bindings
// renders one quiet card at the end of the Environment section,
// chipping every sensor role that has no binding — each chip is a
// link into the grow edit page's Entity bindings flow. A fully bound
// grow shows no card at all, and the unbound roles render no chart
// cards (the card-visibility rule stays put).

import { expect, test, type Page } from '@playwright/test';
import { api, armNoReloadGuard, createGrow, dismissOnboarding, expectNoReload, url } from './helpers';

test.beforeAll(async ({ request }) => {
  await dismissOnboarding(request);
});

/** Binds the mock's sensors per the given map and waits for them. */
async function bind(
  request: import('@playwright/test').APIRequestContext,
  growId: string,
  bindings: Record<string, string>,
) {
  await api(request, 'PUT', `api/grows/${growId}/bindings`, { bindings });
  await expect
    .poll(async () => {
      const env = await api<{
        bindings: Array<{ role: string; available: boolean }>;
      }>(request, 'GET', `api/grows/${growId}/env`);
      return env!.bindings.every((binding) => binding.available);
    }, { timeout: 10_000 })
    .toBe(true);
}

/** The not-measured card, located by its heading. */
function cardOf(page: Page) {
  return page.locator('.card', {
    has: page.getByRole('heading', { name: 'Not measured' }),
  });
}

test('partial bindings chip the unbound roles with a bind affordance', async ({
  page,
  request,
}) => {
  const growId = await createGrow(request, 'E2E Not Measured');
  // Partial: only temperature is bound, so humidity, CO₂ and
  // soil moisture have neither a chart card nor a binding row. Light
  // is unbound too but chips nowhere since ticket 05 retired its
  // card — the binding would feed the Air charts' night shading.
  await bind(request, growId, { temperature: 'sensor.grow_temperature' });

  await page.goto(url(`/grows/${growId}`));
  await expect(
    page.locator('svg[aria-label="temperature history"]'),
  ).toBeVisible();

  // One quiet card, exactly the three unbound charted roles in
  // card order.
  const card = cardOf(page);
  await expect(card).toBeVisible();
  await expect(card.getByRole('listitem')).toHaveText([
    'Humidity Bind',
    'CO₂ Bind',
    'Soil moisture Bind',
  ]);

  // The unbound roles render no chart cards: the chips stand in for
  // them, not "no data" ghosts (VPD needs temperature + humidity, so
  // it hides too without its own chip).
  await expect(page.locator('svg[aria-label="humidity history"]')).toHaveCount(0);
  await expect(page.locator('svg[aria-label="co2 history"]')).toHaveCount(0);
  await expect(page.locator('svg[aria-label="vpd history"]')).toHaveCount(0);

  // Each chip is the bind affordance into the existing flow on the
  // edit page, under the ingress prefix like every in-app href.
  const editHref = url(`/grows/${growId}/edit`);
  for (const chip of await card.getByRole('link').all()) {
    expect(await chip.getAttribute('href'), 'chip links to the edit page').toBe(editHref);
  }

  // Clicking a chip opens that flow as an in-app navigation — no
  // native reload of the document (the D061 guard class).
  await armNoReloadGuard(page);
  await card.getByRole('link', { name: 'Humidity Bind' }).click();
  await expect(page.getByRole('heading', { name: 'Entity bindings' })).toBeVisible();
  await expectNoReload(page);
});

test('a fully bound grow shows no not-measured card', async ({ page, request }) => {
  const growId = await createGrow(request, 'E2E Fully Bound');
  await bind(request, growId, {
    temperature: 'sensor.grow_temperature',
    humidity: 'sensor.grow_humidity',
    co2: 'sensor.grow_co2',
    light: 'light.grow_led',
    soil_moisture: 'sensor.grow_soil_moisture',
  });

  await page.goto(url(`/grows/${growId}`));
  await expect(
    page.locator('svg[aria-label="temperature history"]'),
  ).toBeVisible();

  // Every charted role is bound: no chip list to show.
  await expect(page.getByRole('heading', { name: 'Not measured' })).toHaveCount(0);
});
