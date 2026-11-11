// Ingress correctness: the app must live entirely under the simulated
// `/api/hassio_ingress/e2e` prefix — injected base, prefixed nav
// hrefs, deep links that load by full navigation.

import { test, expect } from '@playwright/test';
import { INGRESS_PREFIX, createSpace, uniqueName, url } from './helpers';

test('served shell carries the runtime ingress base', async ({ page }) => {
  await page.goto(url('/'));
  await expect(page.locator('base')).toHaveAttribute('href', `${INGRESS_PREFIX}/`);
  expect(await page.evaluate('window.__LEAFLOG_BASE__')).toBe(INGRESS_PREFIX);
  // The SPA actually booted (wasm fetched and mounted under the prefix).
  await expect(page.getByRole('heading', { name: 'Leaflog' })).toBeVisible();
});

test('nav links stay inside the ingress prefix', async ({ page }) => {
  await page.goto(url('/'));
  const nav = page.locator('nav[aria-label="Main"]');
  const grows = nav.getByRole('link', { name: 'Grows' });
  await expect(grows).toHaveAttribute('href', url('/grows'));

  await grows.click();
  await expect(page.getByRole('heading', { name: 'Grows', exact: true })).toBeVisible();
  // The address bar stayed inside the prefix (page.url() is absolute).
  expect(new URL(page.url()).pathname).toContain(url('/'));
});

test('deep links load the page under the prefix', async ({ page, request }) => {
  const name = uniqueName('E2E Deep Space');
  const id = await createSpace(request, name);

  // A full browser navigation (not an SPA route change) straight to
  // the detail page; the backend must serve the SPA shell there.
  await page.goto(url(`/spaces/${id}`));
  await expect(page.getByRole('heading', { name, exact: true })).toBeVisible();
});
