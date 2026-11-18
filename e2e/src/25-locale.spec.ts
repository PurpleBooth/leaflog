// Locale chain (D112, ticket 01): a German-forced browser renders the
// seeded catalog strings in German, and the default (English) locale
// keeps English. The e2e mock Supervisor reports no language
// (`MOCK_HA_LANGUAGE=""` in the justfile), so the browser-fallback
// step of the chain — HA language → browser locale → English — is
// exactly what runs here. Backend-driven German (HA language `de`) is
// covered by the backend integration tests; unseeded strings stay
// English at this stage.

import { test, expect } from '@playwright/test';
import { createGrow, url, uniqueName } from './helpers';

test.describe('german-forced browser', () => {
  test.use({ locale: 'de-DE' });

  test('seeded strings render german', async ({ page, request }) => {
    const name = uniqueName('E2E Deutsche Anbau');
    await createGrow(request, name);

    await page.goto(url('/grows'));

    // Navigation labels (catalog nav.*).
    await expect(
      page.getByRole('link', { name: 'Startseite', exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole('link', { name: 'Verlauf', exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole('link', { name: 'Anbauten', exact: true }),
    ).toBeVisible();

    // Page heading (catalog page.grows_heading).
    await expect(page.getByRole('heading', { name: 'Anbauten' })).toBeVisible();

    // The page action (catalog grows.start_grow)…
    await expect(
      page.getByRole('button', { name: 'Anbau starten' }),
    ).toBeVisible();

    // …and the created grow's card carries the interpolated du-form
    // "started" phrasing (catalog grows.started). Scoped to the
    // grow's own card: earlier specs leave grows in the shared DB.
    const card = page.getByRole('listitem').filter({ hasText: name });
    await expect(card.getByRole('link', { name, exact: true })).toBeVisible();
    await expect(card.getByText(/Gestartet am \d{4}-\d{2}-\d{2}/)).toBeVisible();

    // Nothing re-rendered English: the German page heading is still
    // there after the locale chain settled (the mock reports no HA
    // language, so the browser locale stands).
    await expect(page.getByRole('heading', { name: 'Grows', exact: true })).toHaveCount(0);
  });

  test('grow detail renders german surfaces', async ({ page, request }) => {
    const name = uniqueName('E2E Deutsche Details');
    const growId = await createGrow(request, name);

    await page.goto(url(`/grows/${growId}`));

    // Heading area (the grow's own h2 stays data) with the keyed
    // phase-day badge beside it: phase name (catalog phases.*) plus
    // the German day count (catalog grow.day, singular "Tag").
    await expect(page.getByRole('heading', { name, exact: true })).toBeVisible();
    await expect(page.locator('#phase-day')).toHaveText(/· Tag \d+$/);

    // Coach card heading (catalog env.coach_heading).
    await expect(
      page.getByRole('heading', { name: /Phasen-Coach/ }),
    ).toBeVisible();

    // Environment section heading and a group micro-label
    // (catalog env.heading, env.group_air).
    await expect(
      page.getByRole('heading', { name: 'Umwelt', exact: true }),
    ).toBeVisible();
    await expect(page.getByText('Luft', { exact: true })).toBeVisible();

    // Feed schedule section heading (catalog grow.feed_schedule).
    await expect(
      page.getByRole('heading', { name: 'Düngeplan', exact: true }),
    ).toBeVisible();

    // And nothing drifted back to English.
    await expect(
      page.getByRole('heading', { name: 'Environment', exact: true }),
    ).toHaveCount(0);
  });

  test('grow edit form renders german labels', async ({ page, request }) => {
    const name = uniqueName('E2E Deutsche Bearbeitung');
    const growId = await createGrow(request, name);

    await page.goto(url(`/grows/${growId}/edit`));

    // The form surface (ticket 03): page heading plus keyed field
    // labels and buttons (catalog form.*, grows.save).
    await expect(
      page.getByRole('heading', { name: 'Anbau bearbeiten' }),
    ).toBeVisible();
    await expect(page.getByLabel('Sortenetikett')).toBeVisible();
    await expect(
      page.getByRole('button', { name: 'Speichern' }),
    ).toBeVisible();

    // Bindings grid (catalog binding.heading) and its German role
    // labels (catalog binding.role_temperature).
    await expect(
      page.getByRole('heading', { name: 'Entitäts-Bindungen' }),
    ).toBeVisible();
    await expect(page.getByText('Temperatur', { exact: true })).toBeVisible();

    // Light cycle editor (catalog light_cycle.heading/lights_on).
    await expect(page.getByRole('heading', { name: 'Lichtzyklus' })).toBeVisible();
    await expect(page.getByLabel('Licht-An-Zeit (lokal)')).toBeVisible();

    // Danger zone (catalog grow_edit.*).
    await expect(
      page.getByRole('heading', { name: 'Gefahrenzone' }),
    ).toBeVisible();
    await expect(
      page.getByRole('button', { name: 'Anbau löschen' }),
    ).toBeVisible();

    // And nothing drifted back to English.
    await expect(page.getByLabel('Strain label')).toHaveCount(0);
  });
});

test.describe('default locale', () => {
  test('seeded strings stay english', async ({ page, request }) => {
    const name = uniqueName('E2E English Grow');
    await createGrow(request, name);

    await page.goto(url('/grows'));

    await expect(
      page.getByRole('link', { name: 'Home', exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole('link', { name: 'History', exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole('heading', { name: 'Grows', exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole('button', { name: 'Start a grow' }),
    ).toBeVisible();
    const card = page.getByRole('listitem').filter({ hasText: name });
    await expect(card.getByText(/Started \d{4}-\d{2}-\d{2}/)).toBeVisible();
    await expect(
      page.getByRole('heading', { name: 'Anbauten' }),
    ).toHaveCount(0);
  });
});
