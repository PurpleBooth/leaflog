// Root-zone lazy rendering and empty-window honesty (ticket 07):
// the Root zone charts mount only when the group scrolls into view
// (IntersectionObserver, immediate render when the API is missing),
// and one mount means one history fetch per role — never a loop on
// repeated intersections. A bound role whose window holds no
// readings says so with the inline hint instead of drawing silence
// (the Target Bands alone never count as readings). The group label
// itself is structural and always renders: only the chart grid is
// lazy, so the section's outline never flickers.

import {
  expect,
  test,
  type APIRequestContext,
  type Page,
} from "@playwright/test";
import {
  api,
  createGrow,
  dismissOnboarding,
  revealRootZone,
  uniqueName,
  url,
} from "./helpers";

test.beforeAll(async ({ request }) => {
  await dismissOnboarding(request);
});

/** One chart's svg by role. */
function chartSvg(page: Page, role: string): Locator {
  return page.locator(`svg[aria-label="${role} history"]`);
}

/** The Root zone group's label — structural, always rendered, and
 *  the honest stand-in for the user's scroll into the group. */
function rootZoneLabel(page: Page): Locator {
  return page.getByRole("heading", { name: "Root zone", exact: true });
}

/** Binds the mock-fed roles and waits for all of them to be live. */
async function bindAndAwaitLive(
  request: APIRequestContext,
  growId: string,
  bindings: Record<string, string>,
) {
  await api(request, "PUT", `api/grows/${growId}/bindings`, { bindings });
  await expect
    .poll(
      async () => {
        const env = await api<{
          bindings: Array<{ role: string; available: boolean }>;
        }>(request, "GET", `api/grows/${growId}/env`);
        return env!.bindings.every((binding) => binding.available);
      },
      { timeout: 10_000 },
    )
    .toBe(true);
}

/** Counts every history response one role fetches (failures
 *  included — a retry loop must show up here). */
function historyCounter(
  page: Page,
  growId: string,
  role: string,
): () => number {
  let count = 0;
  page.on("response", (response) => {
    const path = `api/grows/${growId}/history`;
    if (
      response.url().includes(path) &&
      response.url().includes(`role=${role}&`)
    ) {
      count += 1;
    }
  });
  return () => count;
}

test("root zone charts wait below the fold, then fetch exactly once", async ({
  page,
  request,
}) => {
  // A phone-class viewport pins the whole Root zone below the fold
  // while the Air charts (bound, four cards tall here) fill it.
  await page.setViewportSize({ width: 390, height: 700 });
  const growId = await createGrow(request, uniqueName("E2E Root Lazy"));
  await bindAndAwaitLive(request, growId, {
    temperature: "sensor.grow_temperature",
    humidity: "sensor.grow_humidity",
    soil_moisture: "sensor.grow_soil_moisture",
    light: "light.grow_led",
  });
  // One feed entry gives pH and EC their scatter points, so all
  // three Root zone cards ride the same scroll (it auto-captures a
  // soil snapshot too).
  await api(request, "POST", `api/grows/${growId}/actions`, {
    plant_ids: [],
    occurred_at: new Date().toISOString(),
    action_type: "feed",
    details: {
      input_ph: 6.2,
      runoff_ph: 6.8,
      input_ec_us: 1200,
      runoff_ec_us: 1500,
    },
    notes: null,
    appearance_tags: [],
  });

  const fetches = {
    soil: historyCounter(page, growId, "soil_moisture"),
    ph: historyCounter(page, growId, "ph"),
    ec: historyCounter(page, growId, "ec"),
  };
  await page.goto(url(`/grows/${growId}`));

  // The section's outline is structural: the Air charts render now,
  // the Root zone label greets the scroll — but nothing below it
  // has fetched or mounted yet.
  await expect(chartSvg(page, "temperature")).toBeVisible();
  await expect(rootZoneLabel(page)).toBeVisible();
  for (const role of ["soil_moisture", "ph", "ec"]) {
    await expect(chartSvg(page, role)).toHaveCount(0);
  }
  expect(fetches.soil(), "soil fetches before the scroll").toBe(0);
  expect(fetches.ph(), "ph fetches before the scroll").toBe(0);
  expect(fetches.ec(), "ec fetches before the scroll").toBe(0);

  // First scroll into view: every Root zone chart mounts and fetches
  // exactly once.
  await rootZoneLabel(page).scrollIntoViewIfNeeded();
  for (const role of ["soil_moisture", "ph", "ec"]) {
    await expect(chartSvg(page, role)).toBeVisible();
  }
  expect(fetches.soil(), "one soil fetch on first view").toBe(1);
  expect(fetches.ph(), "one ph fetch on first view").toBe(1);
  expect(fetches.ec(), "one ec fetch on first view").toBe(1);

  // Scrolling away and back never re-runs the work: the mount is
  // once-only, and a settle beat proves no timer-driven loop.
  await page
    .getByRole("heading", { name: "Air", exact: true })
    .scrollIntoViewIfNeeded();
  await page.waitForTimeout(400);
  await rootZoneLabel(page).scrollIntoViewIfNeeded();
  await page.waitForTimeout(400);
  expect(fetches.soil(), "no re-fetch on re-entry").toBe(1);
  expect(fetches.ph(), "no re-fetch on re-entry").toBe(1);
  expect(fetches.ec(), "no re-fetch on re-entry").toBe(1);

  // A window switch is one deliberate re-fetch per role — never a
  // burst.
  await page
    .getByRole("group", { name: "History window" })
    .getByRole("button", { name: "24h", exact: true })
    .click();
  await expect.poll(fetches.soil, "the switch re-fetches soil").toBe(2);
  await expect.poll(fetches.ph, "ph follows the switch once").toBe(2);
  await expect.poll(fetches.ec, "ec follows the switch once").toBe(2);
  await page.waitForTimeout(400);
  expect(fetches.soil(), "no loop after the switch").toBe(2);
  expect(fetches.ph(), "no loop after the switch").toBe(2);
  expect(fetches.ec(), "no loop after the switch").toBe(2);
});

test("a bound role with an empty window says so with the inline hint", async ({
  page,
  request,
}) => {
  // The feed roles can honestly show an empty window against the
  // mock: their points come from feed actions, so one entry just
  // outside the selected window leaves the card bound (it has data)
  // yet empty (nothing in view). A bound *sensor* role cannot reach
  // that state here — the mock serves fresh points for every bound
  // sensor — so the sensor-variant hint ("No readings in this
  // window.", tolerance band drawn but no dots) stays covered by
  // the chart_option unit tests instead.
  const threeDaysAgo = new Date(Date.now() - 3 * 24 * 60 * 60 * 1000)
    .toISOString()
    .slice(0, 10);
  const growId = await createGrow(request, uniqueName("E2E Empty Window"), {
    started_on: threeDaysAgo,
  });
  await api(request, "POST", `api/grows/${growId}/actions`, {
    plant_ids: [],
    occurred_at: new Date(Date.now() - 25 * 60 * 60 * 1000).toISOString(),
    action_type: "feed",
    details: {
      input_ph: 6.0,
      runoff_ph: 6.4,
      input_ec_us: 900,
      runoff_ec_us: 1100,
    },
    notes: null,
    appearance_tags: [],
  });
  await page.goto(url(`/grows/${growId}`));
  await revealRootZone(page);
  const phCard = page.locator(".card", { has: chartSvg(page, "ph") });
  const ecCard = page.locator(".card", { has: chartSvg(page, "ec") });
  await expect(phCard).toBeVisible();
  await expect(ecCard).toBeVisible();
  // The entry sits 25 hours back: inside the default 7-day window
  // (dots draw), outside the 24-hour one (the hint appears).
  await page.getByRole("button", { name: "24h" }).click();
  await expect(
    phCard.getByText("No feed entries in this window."),
  ).toBeVisible();
  await expect(
    ecCard.getByText("No feed entries in this window."),
  ).toBeVisible();

  // Unbound stays unbound: no card and no hint for a role without
  // data — that story belongs to the not-measured card.
  await expect(
    page.locator('svg[aria-label="temperature history"]'),
  ).toHaveCount(0);
  await expect(page.getByText("No readings in this window.")).toHaveCount(0);
});
