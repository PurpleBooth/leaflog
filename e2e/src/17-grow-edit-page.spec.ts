// The grow edit page (grow-page rework ticket 01): the single home
// for grow configuration. The header's Edit control navigates here;
// the page hosts the grow form (with its Advanced disclosure), the
// Entity bindings for all seven roles, the Light Cycle editor and the
// delete danger zone. Saving or deleting returns to the overview, and
// the overview itself renders none of the old configuration surfaces.
// Deep links work under the ingress prefix like every other route.

import { test, expect } from '@playwright/test';
import {
  api,
  armNoReloadGuard,
  createGrow,
  dismissOnboarding,
  expectNoReload,
  uniqueName,
  url,
} from './helpers';

test.beforeAll(async ({ request }) => {
  await dismissOnboarding(request);
});

/** Opens the edit page for a fresh grow and waits for it to render. */
async function openEdit(page: import('@playwright/test').Page, growId: string) {
  await page.goto(url(`/grows/${growId}/edit`));
  await expect(page.getByRole('heading', { name: 'Edit grow' })).toBeVisible();
}

/** One role picker in the Entity bindings grid, by its accessible
 * name (the wrapping fieldset label resists getByLabel). */
function roleSelect(page: import('@playwright/test').Page, role: string) {
  return page.getByRole('combobox', { name: role, exact: true });
}

/** The seven binding roles, in picker order. */
const ROLES = [
  'Temperature',
  'Humidity',
  'CO2',
  'Soil moisture',
  'Light',
  'Reservoir pH',
  'Reservoir EC',
];

test('edit page: deep-linkable under the ingress prefix', async ({
  page,
  request,
}) => {
  const name = uniqueName('E2E Deep Link Grow');
  const growId = await createGrow(request, name);
  await openEdit(page, growId);

  // The served SPA route answers under the ingress prefix, prefilled
  // with the grow's stored values.
  await expect(page).toHaveURL(url(`/grows/${growId}/edit`));
  await expect(page.getByLabel('Name *', { exact: true })).toHaveValue(name);

  // A reload of the deep link lands on the same page, not a 404 or a
  // bounce to the list.
  await page.reload();
  await expect(page.getByRole('heading', { name: 'Edit grow' })).toBeVisible();
  await expect(page.getByLabel('Name *', { exact: true })).toHaveValue(name);
});

test('edit page: the header Edit control links here', async ({
  page,
  request,
}) => {
  const name = uniqueName('E2E Link Grow');
  const growId = await createGrow(request, name);
  await page.goto(url(`/grows/${growId}`));
  await expect(page.getByRole('heading', { name, exact: true })).toBeVisible();

  // The control is a real link (deep-linkable, opens in a new tab)
  // with an ingress-safe relative target, not a native /static or
  // absolute path, styled like the quiet button it replaces.
  const editLink = page.getByRole('link', { name: 'Edit grow' });
  await expect(editLink).toBeVisible();
  await expect(editLink).toHaveClass(/btn/);
  const href = await editLink.first().getAttribute('href');
  expect(href).not.toBeNull();
  expect(href!.endsWith(`/grows/${growId}/edit`)).toBe(true);
  expect(href!.startsWith('/')).toBe(true);

  await editLink.click();
  await expect(page).toHaveURL(url(`/grows/${growId}/edit`));
  await expect(page.getByLabel('Name *', { exact: true })).toHaveValue(name);
});

test('edit page: grow form, all seven binding roles and the light cycle', async ({
  page,
  request,
}) => {
  const name = uniqueName('E2E Host Grow');
  const growId = await createGrow(request, name);
  await openEdit(page, growId);

  // The grow form is here, Advanced disclosure collapsed by default.
  await expect(page.getByLabel('Name *', { exact: true })).toHaveValue(name);
  await expect(page.getByLabel('Heat tolerance')).toBeHidden();
  await page.getByText('Advanced settings', { exact: true }).click();
  await expect(page.getByLabel('Heat tolerance')).toBeVisible();

  // All seven binding roles have their picker on this page.
  for (const role of ROLES) {
    await expect(roleSelect(page, role)).toBeVisible();
  }

  // The Light Cycle editor lives here too.
  await expect(page.getByLabel('Lights on time (local)')).toBeVisible();
});

test('edit page: saving returns to the overview with updated values', async ({
  page,
  request,
}) => {
  const name = uniqueName('E2E Save Grow');
  const growId = await createGrow(request, name);
  await openEdit(page, growId);

  const renamed = uniqueName('E2E Save Grow v2');
  await page.getByLabel('Name *', { exact: true }).fill(renamed);
  await armNoReloadGuard(page);
  await page.getByRole('button', { name: 'Save' }).click();

  // Back on the overview, without a native reload (D061).
  await expect(page).toHaveURL(url(`/grows/${growId}`));
  await expect(page.getByRole('heading', { name: renamed, exact: true })).toBeVisible();
  await expectNoReload(page);

  // The API holds the persisted truth, independent of the page.
  const stored = await api<{ name: string }>(request, 'GET', `api/grows/${growId}`);
  expect(stored!.name).toBe(renamed);
});

test('edit page: binding picks land on the unchanged endpoint', async ({
  page,
  request,
}) => {
  const name = uniqueName('E2E Bind Grow');
  const growId = await createGrow(request, name);
  await openEdit(page, growId);

  // Selecting an entity saves the whole map immediately, like the old
  // live-panel pickers did. The options arrive with the entities
  // fetch, so wait for the entity to be offered before picking it.
  const temperature = roleSelect(page, 'Temperature');
  await expect(
    temperature.locator('option[value="sensor.grow_temperature"]'),
  ).toBeAttached();
  await temperature.selectOption('sensor.grow_temperature');
  // The GET returns the bare map; the PUT wraps it in { bindings }.
  await expect
    .poll(() =>
      api<Record<string, string>>(
        request,
        'GET',
        `api/grows/${growId}/bindings`,
      ),
    )
    .toEqual({ temperature: 'sensor.grow_temperature' });

  // The pick survives a reload of the page.
  await page.reload();
  await expect(page.getByRole('heading', { name: 'Edit grow' })).toBeVisible();
  await expect(roleSelect(page, 'Temperature')).toHaveValue(
    'sensor.grow_temperature',
  );

  // Choosing the unbound slot removes the role from the map.
  await roleSelect(page, 'Temperature').selectOption('');
  await expect
    .poll(() =>
      api<Record<string, string>>(
        request,
        'GET',
        `api/grows/${growId}/bindings`,
      ),
    )
    .toEqual({});
});

test('edit page: delete lives in the danger zone and returns to the overview', async ({
  page,
  request,
}) => {
  const name = uniqueName('E2E Delete Grow');
  const growId = await createGrow(request, name);
  await openEdit(page, growId);

  // The danger zone hosts the delete control with the same
  // confirmation dialog as before.
  await expect(page.getByRole('heading', { name: 'Danger zone' })).toBeVisible();
  page.once('dialog', (dialog) => dialog.accept());
  await page.getByRole('button', { name: 'Delete grow' }).click();

  // Deletion redirects to the grows list (the grow's own page would
  // 404) and the grow is gone from the API.
  await expect(page).toHaveURL(url('/grows'));
  await expect(page.getByRole('heading', { name: 'Grows', exact: true })).toBeVisible();
  await expect(page.getByText(name, { exact: true })).toHaveCount(0);
  const gone = await request.fetch(url(`api/grows/${growId}`));
  expect(gone.ok()).toBe(false);
});

test('edit page: dismissing the delete confirm keeps the page', async ({
  page,
  request,
}) => {
  const name = uniqueName('E2E Keep Grow');
  const growId = await createGrow(request, name);
  await openEdit(page, growId);

  page.once('dialog', (dialog) => dialog.dismiss());
  await page.getByRole('button', { name: 'Delete grow' }).click();

  await expect(page).toHaveURL(url(`/grows/${growId}/edit`));
  const stillThere = await request.fetch(url(`api/grows/${growId}`));
  expect(stillThere.ok()).toBe(true);
});

test('edit page: a clean form leaves without the unsaved-changes guard', async ({
  page,
  request,
}) => {
  const name = uniqueName('E2E Clean Exit Grow');
  const growId = await createGrow(request, name);
  await openEdit(page, growId);

  // Nothing edited yet: the nav anchor navigates directly, no confirm.
  await page.getByRole('link', { name: 'Grows', exact: true }).click();
  await expect(page).toHaveURL(url('/grows'));
});

test('edit page: abandoning unsaved edits asks first and can stay', async ({
  page,
  request,
}) => {
  const name = uniqueName('E2E Guard Stay Grow');
  const growId = await createGrow(request, name);
  await openEdit(page, growId);

  // Edit the name, then click a plain nav anchor. Playwright
  // auto-dismisses dialogs by default, so the handler is wired
  // explicitly: capture the message, then dismiss.
  const renamed = uniqueName('E2E Guard Stay v2');
  const nameField = page.getByLabel('Name *', { exact: true });
  await nameField.fill(renamed);
  let asked: string | null = null;
  page.once('dialog', (dialog) => {
    asked = dialog.message();
    return dialog.dismiss();
  });
  await page.getByRole('link', { name: 'Grows', exact: true }).click();

  // Dismissed: still on the edit page, edits intact, nothing saved.
  expect(asked).toBe('You have unsaved changes. Leave anyway?');
  await expect(page).toHaveURL(url(`/grows/${growId}/edit`));
  await expect(nameField).toHaveValue(renamed);
  const stored = await api<{ name: string }>(request, 'GET', `api/grows/${growId}`);
  expect(stored!.name).toBe(name);
});

test('edit page: abandoning unsaved edits can be confirmed', async ({
  page,
  request,
}) => {
  const name = uniqueName('E2E Guard Leave Grow');
  const growId = await createGrow(request, name);
  await openEdit(page, growId);

  const renamed = uniqueName('E2E Guard Leave v2');
  await page.getByLabel('Name *', { exact: true }).fill(renamed);
  let asked: string | null = null;
  page.once('dialog', (dialog) => {
    asked = dialog.message();
    return dialog.accept();
  });
  await page.getByRole('link', { name: 'Grows', exact: true }).click();

  // Accepted: the guard asked, the SPA navigated, the edit was
  // never saved.
  expect(asked).toBe('You have unsaved changes. Leave anyway?');
  await expect(page).toHaveURL(url('/grows'));
  const stored = await api<{ name: string }>(request, 'GET', `api/grows/${growId}`);
  expect(stored!.name).toBe(name);
});

test('overview: no configuration surfaces left on the grow page', async ({
  page,
  request,
}) => {
  const name = uniqueName('E2E Lean Grow');
  const growId = await createGrow(request, name);
  await page.goto(url(`/grows/${growId}`));
  await expect(page.getByRole('heading', { name, exact: true })).toBeVisible();

  // The old configuration surfaces moved to the edit page: no inline
  // edit card, no binding pickers, no Light Cycle editor.
  await expect(page.getByRole('button', { name: 'Edit grow' })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Delete grow' })).toHaveCount(0);
  await expect(page.getByRole('heading', { name: 'Entity bindings' })).toHaveCount(0);
  await expect(page.getByLabel('Lights on time (local)')).toHaveCount(0);

  // The header's Edit control remains, as a link to the edit page.
  await expect(page.getByRole('link', { name: 'Edit grow' })).toBeVisible();
});
