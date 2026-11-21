// Feeding-section layout containment (D143). The feeding section owns
// the two widest things on the grow page — the this-week dose list and
// the fitted plan table — and both used to size the page: the section
// grid sized its single track to the table's max-content, so the whole
// page panned sideways on a phone, and the dose list rendered with no
// gap between a dose and the next product's name, so "0,25 ml/L" read
// as Root-Juice's dose. The fix: an explicit capped grid track, shrink-
// able cards, an in-card scroll wrapper with the product column pinned,
// and the prototype's dose-list metrics (1.5rem column gap, dashed
// rule per product row).

import { expect, test, type Page } from '@playwright/test';
import { api, dismissOnboarding, uniqueName, url } from './helpers';

test.beforeAll(async ({ request }) => {
  await dismissOnboarding(request);
});

/** A UTC date `days` before now, `YYYY-MM-DD`. */
function utcDateDaysAgo(days: number): string {
  return new Date(Date.now() - days * 86_400_000).toISOString().slice(0, 10);
}

/** Asserts nothing spills past the viewport sideways.
 *
 * The feeding section's scroll containment is the point of this spec,
 * so this checks the document itself, not any inner wrapper: inner
 * scrollers are expected, page panning is the defect. */
async function expectNoHorizontalOverflow(page: Page) {
  await expect
    .poll(
      async () =>
        page.evaluate(
          () =>
            document.documentElement.scrollWidth -
            document.documentElement.clientWidth,
        ),
      { message: 'no horizontal page overflow' },
    )
    .toBeLessThanOrEqual(0);
}

/** Creates an active photoperiod grow on a fitted schedule.
 *
 * The grow carries `planned_veg_weeks`, so its forecast completes
 * without any phase events and create-time fitting (ADR 0004) stores
 * the plan from the grow's first day. It starts in the propagation
 * segment (seedling window is 14–21 days), where two products carry a
 * propagation dose — the multi-row this-week card where a dose can
 * read as the next product's name. */
async function createFittedGrow(
  request: Parameters<typeof api>[0],
): Promise<string> {
  const schedule = await api<{ id: string }>(request, 'POST', 'api/schedules', {
    name: uniqueName('E2E Feed Schedule'),
    flower_weeks: 8,
    products: [
      {
        name: 'Root Juice',
        unit: 'ml_per_l',
        group: 'Rooting',
        doses: { propagation: [4000, 4000] },
      },
      {
        name: 'CalMag',
        unit: 'ml_per_l',
        group: 'Feeds',
        doses: {
          propagation: [250, 250],
          veg: [250, 250],
          flower: Array.from({ length: 8 }, () => [250, 250]),
        },
      },
      {
        name: 'Bio Bloom',
        unit: 'ml_per_l',
        group: 'Feeds',
        doses: {
          flower: [[1000, 4000], null, null, null, null, null, null, null],
        },
      },
    ],
  });
  const grow = await api<{ id: string }>(request, 'POST', 'api/grows', {
    name: uniqueName('E2E Feed Grow'),
    feed_schedule_id: schedule!.id,
    substrate: 'soil',
    started_on: utcDateDaysAgo(5),
    status: 'active',
    notes: null,
    photoperiod: 'photoperiod',
    planned_veg_weeks: 3,
  });
  // The plan must exist from creation; no explicit refit call — if the
  // assign-time fit regresses, this spec must fail, not paper over it.
  const plan = await api<{ days: Array<unknown> }>(
    request,
    'GET',
    `api/grows/${grow!.id}/feed/plan`,
  );
  expect(
    plan!.days.length,
    'create-time fit stored the plan (ADR 0004)',
  ).toBeGreaterThan(0);
  return grow!.id;
}

/** The feeding section's scroll wrapper around the fitted table. */
function scrollWrapper(page: Page) {
  return page.locator('div.min-w-0.overflow-x-auto');
}

test('feeding section scrolls in place with a fitted plan on a phone', async ({
  page,
  request,
}) => {
  const growId = await createFittedGrow(request);
  await page.setViewportSize({ width: 390, height: 844 });
  await page.goto(url(`/grows/${growId}`));

  const week = page.locator('.card', { hasText: 'This week' }).first();
  await expect(week).toBeVisible();
  await expectNoHorizontalOverflow(page);

  // The fitted table lives in its own in-card scroller: wider than
  // the card, but the card and the page never widen.
  const wrapper = scrollWrapper(page);
  await expect(wrapper).toBeVisible();
  const boxes = await wrapper.evaluate((el) => ({
    client: el.clientWidth,
    scroll: el.scrollWidth,
  }));
  expect(
    boxes.scroll,
    'fitted table is wider than its phone-width card',
  ).toBeGreaterThan(boxes.client);

  // Scrolling happens inside the wrapper, not the page.
  await wrapper.evaluate((el) => {
    el.scrollLeft = el.scrollWidth;
  });
  await expectNoHorizontalOverflow(page);

  // The product column stays pinned: its header is still at the
  // card's left edge after the widest scroll.
  const product = wrapper.locator('th', { hasText: 'Product' }).first();
  const productBox = await product.boundingBox();
  const wrapperBox = await wrapper.boundingBox();
  expect(productBox).not.toBeNull();
  expect(wrapperBox).not.toBeNull();
  expect(
    productBox!.x,
    'sticky product column stays at the left edge',
  ).toBeGreaterThanOrEqual(wrapperBox!.x - 1);
});

test('this-week list keeps dose and name inside one cell', async ({
  page,
  request,
}) => {
  const growId = await createFittedGrow(request);
  await page.goto(url(`/grows/${growId}`));

  const week = page.locator('.card', { hasText: 'This week' }).first();
  await expect(week).toBeVisible();

  // The prototype metrics: 1.5rem column gap and a dashed rule per
  // product row (computed styles, not class names — the pin is on the
  // rendered association, not the spelling).
  const metrics = await week.locator('ul li').first().evaluate((li) => {
    const style = getComputedStyle(li);
    const list = getComputedStyle(li.parentElement!);
    return {
      rule: style.borderBottomStyle,
      fontSize: parseFloat(style.fontSize),
      columnGap: parseFloat(list.columnGap),
      rowGap: parseFloat(list.rowGap),
    };
  });
  expect(metrics.rule, 'dashed rule per dose row').toBe('dashed');
  expect(metrics.fontSize, 'dose rows render at the prototype size').toBe(14);
  expect(metrics.columnGap, 'prototype 1.5rem column gap').toBe(24);
  expect(metrics.rowGap, 'prototype 0.375rem row gap').toBe(6);

  // Each dose is kept on one line with its unit.
  const dose = week.locator('li span.ml-auto').first();
  await expect(dose).toHaveCSS('white-space', 'nowrap');
});

test('this-week metrics hold at tablet width', async ({ page, request }) => {
  const growId = await createFittedGrow(request);
  await page.setViewportSize({ width: 1122, height: 900 });
  await page.goto(url(`/grows/${growId}`));

  const week = page.locator('.card', { hasText: 'This week' }).first();
  await expect(week).toBeVisible();

  // Multi-column layout: the horizontal space between the first
  // column's dose cell and the second column's product name must be
  // the prototype's column gap, not a cramped few pixels.
  const gap = await week.locator('ul').evaluate((ul) => {
    const columns = [...ul.querySelectorAll('li')].reduce(
      (acc: Array<{ x: number; right: number }>, li) => {
        const box = li.getBoundingClientRect();
        const last = acc[acc.length - 1];
        // One entry per column: a column key is a fresh x.
        if (last === undefined || box.x - last.x > 8) {
          acc.push({ x: box.x, right: box.right });
        }
        return acc;
      },
      [],
    );
    return columns.length >= 2 ? columns[1]!.x - columns[0]!.right : 0;
  });
  expect(
    gap,
    'column gap separates the dose from the next product',
  ).toBeGreaterThanOrEqual(20);
  await expectNoHorizontalOverflow(page);
});
