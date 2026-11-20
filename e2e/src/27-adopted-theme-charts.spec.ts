// Adopted-theme charts (D135): an adopted Home Assistant theme must
// not blank the charts. The app used to adopt the parent palette as
// oklch() custom properties — which the CSS engine wears happily but
// the chart renderer's own colour parser cannot read, so every
// accent-stroked mark silently vanished under an adopted theme (the
// built-in hex themes never reproduced it, which is why the chart
// specs stayed green while the product was broken). The fix
// serializes the adoption as rgb()/rgba() — parseable by the CSS
// engine, Tailwind's opacity modifiers and the renderer alike — and
// the chart probes reject unparseable formats before they reach the
// option builder. This spec frames the app in a hand-rolled parent
// page wearing an HA-style hex theme (as HA themes ship: #rrggbb
// custom properties on the root) and asserts the data line is
// stroked in exactly the adopted colour, the tooltip speaks plainly
// without the renderer's synthetic series names, and the connect
// crosshair still syncs.

import { expect, test, type Locator, type Page } from '@playwright/test';
import { api, createGrow, dismissOnboarding, url } from './helpers';

test.beforeAll(async ({ request }) => {
  await dismissOnboarding(request);
});

/** Hovers the middle of one chart's SVG so the axis tooltip snaps. */
async function hoverCenter(page: Page, svg: Locator) {
  await svg.scrollIntoViewIfNeeded();
  const box = await svg.boundingBox();
  expect(box, 'chart laid out for hover').not.toBeNull();
  await page.mouse.move(box!.x + box!.width * 0.5, box!.y + box!.height * 0.5, {
    steps: 5,
  });
}

/** A fake HA parent page: the hex variables HA themes actually ship,
 *  purple on dark, and the app framed full-viewport beneath them. */
function parentPage(appUrl: string): string {
  return `<!doctype html>
<html style="--primary-color:#7c4dff;--accent-color:#00e5ff;--primary-background-color:#111318;--card-background-color:#1c1f24;--divider-color:rgba(255,255,255,.12);--primary-text-color:#e2e4e9;--text-primary-color:#ffffff;--success-color:#43a047;--warning-color:#ffa726;--error-color:#e53935;--info-color:#039be5">
<body style="margin:0;height:100vh"><iframe src="${appUrl}" style="width:100vw;height:100vh;border:0"></iframe></body></html>`;
}

test('an adopted hex theme strokes the chart lines', async ({
  page,
  request,
}) => {
  const growId = await createGrow(request, 'E2E Adopted Theme Charts');

  // Bind the mock-fed air roles so the section draws real lines.
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

  // Three log entries auto-capture every bound role, so the charts
  // get in-window points (the same seeding 15-environment-charts
  // uses).
  for (const note of ['first', 'second', 'third']) {
    await api(request, 'POST', `api/grows/${growId}/actions`, {
      plant_ids: [],
      occurred_at: new Date().toISOString(),
      action_type: 'note',
      details: {},
      notes: note,
      appearance_tags: [],
    });
    await page.waitForTimeout(1_200);
  }

  // Establish the app origin first, then swap the document for the
  // framed parent page (setContent keeps the origin, so the app's
  // parent-theme reader can read our hex variables).
  await page.goto(url(`/grows/${growId}`));
  await page.setContent(parentPage(url(`/grows/${growId}`)));

  const frame = page.frameLocator('iframe');
  const appRoot = frame.locator('html');

  // The adoption wears sRGB — exactly the converted parent colours,
  // never the oklch() serialization that blanked the charts (D100
  // amendment).
  await expect
    .poll(() => appRoot.getAttribute('style'))
    .toContain('--color-primary: rgb(124, 77, 255)');
  expect(await appRoot.getAttribute('style')).not.toContain('oklch(');

  const tempSvg = frame.locator('svg[aria-label="temperature history"]');
  await expect(tempSvg).toBeVisible();

  // The regression itself: the data line is stroked in exactly the
  // adopted colour — proof the renderer parsed it, not merely that a
  // path exists.
  const line = tempSvg.locator('path[stroke-width="2"]:not([d=""])').first();
  await expect(line).toBeVisible();
  await expect(line).toHaveAttribute('stroke', 'rgb(124, 77, 255)');

  // The Target Band rides the same accent (it used to vanish too).
  const band = tempSvg
    .locator('path[fill="rgb(124, 77, 255)"]')
    .first();
  await expect(band).toBeAttached();

  // The tooltip speaks plainly — value — time, no synthetic "series0"
  // prefix — and the crosshair still syncs: one hover, every air
  // card's tooltip at the same timestamp.
  await hoverCenter(page, tempSvg);
  const tempCard = frame.locator('.card', { has: tempSvg });
  await expect(tempCard.getByText(/°C.*UTC/).first()).toBeVisible();
  const humiditySvg = frame.locator('svg[aria-label="humidity history"]');
  const humidityCard = frame.locator('.card', { has: humiditySvg });
  await expect(humidityCard.getByText(/%.*UTC/).first()).toBeVisible();
  await expect(frame.getByText(/series\d/)).toHaveCount(0);
});
