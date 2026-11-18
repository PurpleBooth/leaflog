// The verification sweep (ticket 07): the shared window, the marker
// ticks, the night shading and the Target Bands have all landed —
// this pass proves they stay quiet together. Marker ticks stay
// single-glyph across window switches (no doubled glyphs at a shared
// timestamp), a window switch re-uses the chart's svg root instead
// of tearing it down (no flicker), and the whole section is captured
// as screenshots across viewport × colour scheme × markers × window
// for the visual quietness review (no new hues, nothing shouting).

import {
  expect,
  test,
  type APIRequestContext,
  type Locator,
  type Page,
} from "@playwright/test";
import {
  api,
  createGrow,
  dismissOnboarding,
  markerTicks,
  revealRootZone,
  url,
} from "./helpers";

test.beforeAll(async ({ request }) => {
  await dismissOnboarding(request);
});

/** One chart's svg by role. */
function chartSvg(page: Page, role: string): Locator {
  return page.locator(`svg[aria-label="${role} history"]`);
}

/** A log action at `hours` before now. */
async function logAction(
  request: APIRequestContext,
  growId: string,
  actionType: string,
  hoursAgo: number,
  details: Record<string, unknown> = {},
) {
  await api(request, "POST", `api/grows/${growId}/actions`, {
    plant_ids: [],
    occurred_at: new Date(Date.now() - hoursAgo * 3_600_000).toISOString(),
    action_type: actionType,
    details,
    notes: null,
    appearance_tags: [],
  });
}

/** Every distinct x a set of ticks sits at. */
function uniqueTickXs(ticks: { x: number }[]): number[] {
  return [...new Set(ticks.map((tick) => tick.x))];
}

/** Seed one grow that exercises every layer of the section. */
async function seedSweepGrow(request: APIRequestContext): Promise<string> {
  const growId = await createGrow(request, "E2E Sweep");
  await api(request, "PUT", `api/grows/${growId}/bindings`, {
    bindings: {
      temperature: "sensor.grow_temperature",
      humidity: "sensor.grow_humidity",
      soil_moisture: "sensor.grow_soil_moisture",
      light: "light.grow_led",
    },
  });
  // Root-zone readings plus their markers, air markers, and a night
  // staircase from the mock — every quiet layer at once.
  await api(request, "POST", `api/grows/${growId}/actions`, {
    plant_ids: [],
    occurred_at: new Date(Date.now() - 2 * 3_600_000).toISOString(),
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
  await logAction(request, growId, "water", 5);
  await logAction(request, growId, "lights", 8);
  return growId;
}

/** The pending history responses for every bound chart of the grow. */
function pendingHistoryResponses(page: Page, growId: string) {
  return ["temperature", "humidity", "soil_moisture", "ph", "ec"].map((role) =>
    page.waitForResponse(
      (response) =>
        response.url().includes(`api/grows/${growId}/history`) &&
        response.url().includes(`role=${role}&`),
      { timeout: 10_000 },
    ),
  );
}

test("marker ticks stay single-glyph and the svg root survives window switches", async ({
  request,
  page,
}) => {
  const growId = await seedSweepGrow(request);
  await page.goto(url(`/grows/${growId}`));
  await expect(chartSvg(page, "temperature")).toBeVisible();
  await revealRootZone(page);
  await expect(chartSvg(page, "soil_moisture")).toBeVisible();

  const control = page.getByRole("group", { name: "History window" });
  const svgRootOf = (role: string) => chartSvg(page, role);

  // Tag the mounted svg roots; a switch that re-uses them keeps the
  // tag, a re-mount loses it.
  await svgRootOf("temperature").evaluate((el) => {
    el.dataset.sweepTag = "original";
  });
  await svgRootOf("soil_moisture").evaluate((el) => {
    el.dataset.sweepTag = "original";
  });

  for (const [label, expectedTicks] of [
    ["7d", 2],
    ["24h", 2],
    ["30d", 2],
  ] as const) {
    const settled = Promise.all(pendingHistoryResponses(page, growId));
    await control.getByRole("button", { name: label, exact: true }).click();
    await settled;
    // The svg roots are the same elements ECharts setOption'd into.
    await expect
      .poll(() =>
        svgRootOf("temperature").evaluate((el) => el.dataset.sweepTag),
      )
      .toBe("original");
    await expect
      .poll(() =>
        svgRootOf("soil_moisture").evaluate((el) => el.dataset.sweepTag),
      )
      .toBe("original");
    // One glyph per event, never a stack of ticks at one x.
    const ticks = await markerTicks(svgRootOf("soil_moisture"));
    expect(ticks.length, `ticks in the ${label} window`).toBe(expectedTicks);
    expect(uniqueTickXs(ticks).length, `distinct tick xs in ${label}`).toBe(
      ticks.length,
    );
  }

  // The Markers switch clears every tick and brings them back clean.
  await page.getByLabel("Markers").uncheck();
  await expect
    .poll(async () => markerTicks(svgRootOf("soil_moisture")))
    .toEqual([]);
  await page.getByLabel("Markers").check();
  const backTicks = await markerTicks(svgRootOf("soil_moisture"));
  expect(uniqueTickXs(backTicks).length).toBe(backTicks.length);
});

test("the section stays visually quiet across the screenshot sweep", async ({
  browser,
  request,
}) => {
  const growId = await seedSweepGrow(request);

  // Every pass of the sweep: viewport × colour scheme × markers ×
  // window. The pictures are the artefact — each lands next to this
  // test's results for the quietness review.
  for (const viewport of [
    { name: "desktop", width: 1280, height: 720 },
    { name: "mobile", width: 390, height: 812 },
  ]) {
    for (const scheme of ["light", "dark"] as const) {
      const context = await browser.newContext({
        viewport: { width: viewport.width, height: viewport.height },
        colorScheme: scheme,
      });
      const page = await context.newPage();
      await page.goto(url(`/grows/${growId}`));
      await expect(chartSvg(page, "temperature")).toBeVisible();
      // The Root zone group is below the fold at both sizes — bring
      // the whole section into the story before the pictures.
      await revealRootZone(page);
      await expect(chartSvg(page, "soil_moisture")).toBeVisible();

      const section = page.locator("#env-section");
      for (const markers of ["on", "off"] as const) {
        if (markers === "off") {
          await page.getByLabel("Markers").uncheck();
        }
        const control = page.getByRole("group", { name: "History window" });
        for (const label of ["24h", "7d", "30d"]) {
          const settled = Promise.all(pendingHistoryResponses(page, growId));
          await control
            .getByRole("button", { name: label, exact: true })
            .click();
          await settled;
          await expect(section).toBeVisible();
          // One picture per window value: the artefact of the pass.
          const shot = `env-${viewport.name}-${scheme}-markers-${markers}-${label}.png`;
          test
            .info()
            .annotations.push({ type: "screenshot", description: shot });
          await section.screenshot({ path: test.info().outputPath(shot) });
        }
      }
      await context.close();
    }
  }
});
