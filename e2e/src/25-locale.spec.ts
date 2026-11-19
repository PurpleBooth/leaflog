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

  test('sweep surfaces render german', async ({ page }) => {
    // Ticket 04 sweep: the nav landmark label, Home, the ingress
    // spike and the history report join the keyed surfaces.

    // Home is the landing fallback for a grow-less instance (D073);
    // sibling specs create grows in the shared throwaway DB, so pin
    // the empty list for this page alone to reach it.
    await page.route('**/api/grows', (route) =>
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: '[]',
      }),
    );

    // Nav landmark (catalog nav.main) on the landing page.
    await page.goto(url('/'));
    await expect(
      page.getByRole('navigation', { name: 'Hauptnavigation' }),
    ).toBeVisible();

    // Home marketing copy and backend-health block (catalog page.*).
    await expect(page.getByText(/Verfolge deine Anbauten/)).toBeVisible();
    await expect(page.getByText(/Alles hängt jetzt am Anbau/)).toBeVisible();
    await expect(
      page.getByRole('heading', { name: 'Backend-Zustand' }),
    ).toBeVisible();

    // The grow-less empty banner (catalog grows.empty) — the spec's
    // "a banner" assertion, on /grows with the pinned empty list.
    await page.goto(url('/grows'));
    await expect(
      page.getByText('Noch keine Anbauten — starte deinen ersten.'),
    ).toBeVisible();

    // The ingress spike (catalog page.spike_*, grows.back_home).
    await page.goto(url('/spike'));
    await expect(
      page.getByRole('heading', { name: 'Ingress-Spike' }),
    ).toBeVisible();
    await expect(page.getByText(/Dieser Deep Link wurde von der SPA/)).toBeVisible();
    await expect(
      page.getByRole('link', { name: 'Zurück zur Startseite' }),
    ).toBeVisible();

    // The history report: predicted-vs-actual heading plus, since
    // earlier specs leave grows behind, the per-row accuracy terms
    // and the keyed aggregate headings/table headers (catalog
    // history.*). The grow-less empty line and the "nothing weighed"
    // fallbacks only render on a fresh instance and are pinned via
    // their absence in the English sweep below.
    await page.goto(url('/history'));
    await expect(
      page.getByRole('heading', { name: 'Prognose und Ist' }),
    ).toBeVisible();
    await expect(page.getByText('Plan-Genauigkeit').first()).toBeVisible();
    await expect(
      page.getByText('noch kein Flip und keine Ernte eingetragen').first(),
    ).toBeVisible();
    await expect(
      page.getByRole('heading', { name: 'Ertrag nach Sortenetikett' }),
    ).toBeVisible();
    await expect(
      page.getByRole('heading', { name: 'Ertrag nach Substrat' }),
    ).toBeVisible();
    await expect(
      page.getByRole('heading', { name: 'Ertrag nach Düngervorlage' }),
    ).toBeVisible();
    await expect(
      page.getByRole('columnheader', { name: 'Nass (g)' }).first(),
    ).toBeVisible();
    await expect(
      page.getByRole('columnheader', { name: 'Anbauten' }).first(),
    ).toBeVisible();
    await expect(
      page.getByRole('link', { name: 'Zurück zur Startseite' }),
    ).toBeVisible();

    // And none of the freshly keyed English surfaces renders.
    await expect(page.getByText(/Track your grows/)).toHaveCount(0);
    await expect(
      page.getByRole('heading', { name: 'Backend health' }),
    ).toHaveCount(0);
    await expect(
      page.getByRole('heading', { name: 'Predicted vs actual' }),
    ).toHaveCount(0);
    await expect(page.getByText('Nothing weighed yet.')).toHaveCount(0);
    await expect(
      page.getByRole('heading', { name: 'Ingress spike' }),
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

  test('sweep surfaces stay english', async ({ page }) => {
    // Ticket 04 sweep on the default (English) locale: nav label,
    // home copy, spike and history report keep their English strings.
    // Home is the landing fallback for a grow-less instance (D073);
    // pin the empty list for this page alone to reach it.
    await page.route('**/api/grows', (route) =>
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: '[]',
      }),
    );
    await page.goto(url('/'));
    await expect(page.getByRole('navigation', { name: 'Main' })).toBeVisible();
    await expect(page.getByText(/Track your grows/)).toBeVisible();
    await expect(
      page.getByRole('heading', { name: 'Backend health' }),
    ).toBeVisible();

    // The grow-less empty banner in the default locale (catalog
    // grows.empty), mirroring the German sweep's banner assertion.
    await page.goto(url('/grows'));
    await expect(
      page.getByText('No grows yet — start your first one.'),
    ).toBeVisible();

    await page.goto(url('/spike'));
    await expect(
      page.getByRole('heading', { name: 'Ingress spike' }),
    ).toBeVisible();
    await expect(
      page.getByRole('link', { name: 'Back to home' }),
    ).toBeVisible();

    await page.goto(url('/history'));
    await expect(
      page.getByRole('heading', { name: 'Predicted vs actual' }),
    ).toBeVisible();
    await expect(page.getByText('Plan accuracy').first()).toBeVisible();
    await expect(
      page.getByText('no flip or harvest logged yet').first(),
    ).toBeVisible();
    await expect(
      page.getByRole('heading', { name: 'Yield by substrate' }),
    ).toBeVisible();
    await expect(
      page.getByRole('columnheader', { name: 'Wet g' }).first(),
    ).toBeVisible();
    await expect(
      page.getByRole('columnheader', { name: 'Grows' }).first(),
    ).toBeVisible();

    // And no German leaks through — including the grow-less fallback
    // lines this shared-DB run can never render.
    await expect(
      page.getByRole('heading', { name: 'Prognose und Ist' }),
    ).toHaveCount(0);
    await expect(page.getByText('Noch nichts gewogen.')).toHaveCount(0);
    await expect(page.getByText(/Noch keine Anbauten/)).toHaveCount(0);
  });
});
