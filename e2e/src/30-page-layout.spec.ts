// Every route shares the same page inset, and list page actions keep
// their size and the same breathing room above the first card.

import { test, expect, type Page } from '@playwright/test';
import { createGrow, createPlant, createSchedule, uniqueName, url } from './helpers';

async function expectPageInset(page: Page): Promise<void> {
  const gap = await page.evaluate(() => {
    const nav = document.querySelector('header nav')!;
    const main = document.querySelector('main')!;
    const style = getComputedStyle(main);
    return {
      top: main.getBoundingClientRect().top - nav.getBoundingClientRect().bottom,
      padding: style.paddingTop,
      bottom: style.paddingBottom,
    };
  });
  expect(gap).toEqual({ top: 0, padding: '24px', bottom: '48px' });
}

test('page template keeps route spacing and list actions consistent', async ({
  page,
  request,
}) => {
  const growName = uniqueName('Layout grow');
  const growId = await createGrow(request, growName);
  const plantName = uniqueName('Layout plant');
  const plantId = await createPlant(request, growId, plantName);
  const scheduleName = uniqueName('Layout schedule');
  const scheduleId = await createSchedule(request, scheduleName);

  for (const width of [1280, 375]) {
    await page.setViewportSize({ width, height: 900 });
    const routes = [
      { path: '/', heading: growName },
      { path: '/grows', heading: 'Grows' },
      { path: `/grows/${growId}`, heading: growName },
      { path: `/grows/${growId}/edit`, heading: 'Edit grow' },
      { path: '/schedules', heading: 'Feeding schedules' },
      { path: '/schedules/new', heading: 'New feeding schedule' },
      { path: `/schedules/${scheduleId}/edit`, heading: 'Edit feeding schedule' },
      { path: '/history', heading: 'History' },
    ];
    for (const route of routes) {
      await page.goto(url(route.path));
      await expect(page.getByRole('heading', { name: route.heading, exact: true })).toBeVisible();
      await expectPageInset(page);
      const headingTop = await page.getByRole('heading', { name: route.heading, exact: true })
        .evaluate((heading) => heading.getBoundingClientRect().top);
      const navBottom = await page.locator('header nav')
        .evaluate((nav) => nav.getBoundingClientRect().bottom);
      expect(headingTop - navBottom).toBeGreaterThanOrEqual(24);
      expect(headingTop - navBottom).toBeLessThanOrEqual(28);
    }
    await page.goto(url(`/plants/${plantId}`));
    await expect(page.getByText(plantName, { exact: true })).toBeVisible();
    await expectPageInset(page);

    await page.goto(url('/grows'));
    const growAction = page.getByRole('button', { name: 'Start a grow' });
    const growCard = page.getByRole('link', { name: growName, exact: true }).locator('xpath=ancestor::li');
    await expect(growAction).toBeVisible();
    const growButton = await growAction.boundingBox();
    const growCardBox = await growCard.boundingBox();
    expect(growCardBox!.y - (growButton!.y + growButton!.height)).toBeGreaterThanOrEqual(16);

    await page.goto(url(`/grows/${growId}`));
    const quickAdd = page.getByRole('button', { name: 'Quick add', exact: true });
    await expect(quickAdd).toBeVisible();
    const quickAddButton = await quickAdd.boundingBox();
    expect(quickAddButton!.height).toBe(growButton!.height);

    await page.goto(url('/schedules'));
    const scheduleAction = page.getByRole('link', { name: 'New schedule', exact: true });
    await expect(scheduleAction).toBeVisible();
    const scheduleButton = await scheduleAction.boundingBox();
    expect(scheduleButton!.height).toBe(growButton!.height);
  }
});

test('phase coach advice uses regular body weight', async ({ page, request }) => {
  const growId = await createGrow(request, uniqueName('Coach layout'));
  await page.route('**/api/grows/*/coach', (route) => route.fulfill({
    json: {
      phase: 'vegetative',
      light_on: true,
      equipment_suggestions: [],
      tips: [
        { code: 'vpd_band', title: '', why: '' },
        { code: 'soil_wet_dry', title: '', why: '' },
        { code: 'coco_fertigate', title: '', why: '' },
      ],
    },
  }));
  await page.goto(url(`/grows/${growId}`));
  const heading = page.getByRole('heading', { name: /Phase coach/ });
  await expect(heading).toBeVisible();
  const tips = heading.locator('..').locator('li');
  await expect(tips).toHaveCount(3);
  await expect(tips.nth(0)).toContainText('Keep VPD');
  await expect(tips.nth(1)).toContainText('Water when the top');
  await expect(tips.nth(2).locator('p')).toContainText('In coco');
  for (const tip of await tips.all()) {
    await expect(tip.locator('span')).toHaveCSS('font-weight', '400');
  }
});

test('empty schedule library uses the same page inset and action size', async ({ page }) => {
  await page.route('**/api/schedules', (route) => route.fulfill({ json: [] }));
  await page.goto(url('/schedules'));
  const action = page.getByRole('link', { name: 'New schedule', exact: true });
  await expect(action).toBeVisible();
  await expectPageInset(page);
  await expect(action).toHaveClass(/\bbtn-sm\b/);
  const card = page.locator('main .card').first();
  const offset = await card.evaluate((element) => {
    const main = document.querySelector('main')!;
    return element.getBoundingClientRect().top - main.getBoundingClientRect().top;
  });
  expect(offset).toBe(24);
});
