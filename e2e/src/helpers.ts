// Shared helpers for the Leaflog end-to-end suite.
//
// Everything runs under the simulated HA ingress prefix, exactly like
// `just dev` and production: `url('/equipment')` is
// `/api/hassio_ingress/e2e/equipment`. No test may talk to the bare
// origin — ingress-relative URLs are a non-negotiable, and the whole
// suite exercises them on every navigation and API call.

import { expect, type APIRequestContext, type Page } from '@playwright/test';

/** The simulated ingress prefix the e2e stack serves under. */
export const INGRESS_PREFIX =
  process.env.LEAFLOG_E2E_PREFIX ?? '/api/hassio_ingress/e2e';

/** A route or API path under the ingress prefix. */
export function url(path: string): string {
  const suffix = path.startsWith('/') ? path : `/${path}`;
  return `${INGRESS_PREFIX}${suffix}`;
}

let counter = 0;

/**
 * A unique per-run name, so the bundled seed records (15 strains,
 * 5 equipment items) and repeated runs can never collide.
 */
export function uniqueName(prefix: string): string {
  counter += 1;
  return `${prefix} ${Date.now().toString(36)}-${counter}`;
}

// The D061 regression guard. A native form submit (the bug: a listener
// name the DOM never dispatches, so the browser submits and reloads)
// wipes this marker with the document. Arm it right before clicking a
// submit button; assert it right after the expected UI change and
// before any explicit reload.
const MARKER = '__leaflogE2eAlive';

/** Arms the no-native-submit marker on the current document. */
export async function armNoReloadGuard(page: Page): Promise<void> {
  await page.evaluate(`window.${MARKER} = true`);
}

/** Asserts the marker survived: no native form submission happened. */
export async function expectNoReload(page: Page): Promise<void> {
  expect(await page.evaluate(`window.${MARKER} === true`), [
    'the page reloaded — the form did a native submit (the D061 bug class)',
  ]).toBe(true);
}

/** One JSON REST call against the backend, under the ingress prefix. */
export async function api<T = unknown>(
  request: APIRequestContext,
  method: 'GET' | 'POST' | 'PUT' | 'DELETE',
  path: string,
  body?: unknown,
): Promise<T | undefined> {
  const response = await request.fetch(url(path), {
    method,
    data: body === undefined ? undefined : JSON.stringify(body),
    headers:
      body === undefined ? undefined : { 'content-type': 'application/json' },
  });
  const text = await response.text();
  expect(
    response.ok(),
    `${method} ${path} -> ${response.status()}: ${text}`,
  ).toBeTruthy();
  return text === '' ? undefined : (JSON.parse(text) as T);
}

/** Creates a grow via the API (day one today) and returns its id. */
export async function createGrow(
  request: APIRequestContext,
  name: string,
): Promise<string> {
  const today = new Date().toISOString().slice(0, 10);
  const grow = await api<{ id: string }>(request, 'POST', 'api/grows', {
    name,
    strain_id: null,
    grow_space_id: null,
    seed_batch_id: null,
    feed_template_id: null,
    substrate: 'soil',
    started_on: today,
    status: 'active',
    notes: null,
  });
  return grow!.id;
}

/** Creates a plant in a grow via the API and returns its id. */
export async function createPlant(
  request: APIRequestContext,
  growId: string,
  name: string,
): Promise<string> {
  const plant = await api<{ id: string }>(
    request,
    'POST',
    `api/grows/${growId}/plants`,
    { name },
  );
  return plant!.id;
}

/** Creates a grow space via the API and returns its id. */
export async function createSpace(
  request: APIRequestContext,
  name: string,
): Promise<string> {
  const space = await api<{ id: string }>(request, 'POST', 'api/spaces', {
    name,
    width_cm: null,
    depth_cm: null,
    height_cm: null,
    notes: null,
  });
  return space!.id;
}

/** Dismisses the onboarding wizard for this database. */
export async function dismissOnboarding(
  request: APIRequestContext,
): Promise<void> {
  await api(request, 'PUT', 'api/settings', { onboarding_dismissed: true });
}
