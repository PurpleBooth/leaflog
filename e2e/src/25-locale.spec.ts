// Locale chain (D112, ticket 01): a German-forced browser renders the
// seeded catalog strings in German, and the default (English) locale
// keeps English. The e2e mock Supervisor reports no language
// (`MOCK_HA_LANGUAGE=""` in the justfile), so the browser-fallback
// step of the chain — HA language → browser locale → English — is
// exactly what runs here. Backend-driven German (HA language `de`) is
// covered by the backend integration tests; unseeded strings stay
// English at this stage.

import { test, expect } from '@playwright/test';
import { createGrow, url, uniqueName, createSchedule, api } from './helpers';

test.describe('german-forced browser', () => {
  test.use({ locale: 'de-DE' });

  test('seeded strings render german', async ({ page, request }) => {
    const name = uniqueName('E2E Deutsche Anbau');
    await createGrow(request, name);

    await page.goto(url('/grows'));

    // Navigation labels (catalog nav.*): the bench bar carries the
    // three sections — no Home link since the nav rework (the brand
    // wordmark is the home link).
    const nav = page.getByRole('navigation', { name: 'Hauptnavigation' });
    await expect(
      nav.getByRole('link', { name: 'Anbauten', exact: true }),
    ).toBeVisible();
    await expect(
      nav.getByRole('link', { name: 'Düngepläne', exact: true }),
    ).toBeVisible();
    await expect(
      nav.getByRole('link', { name: 'Verlauf', exact: true }),
    ).toBeVisible();
    await expect(
      nav.getByRole('link', { name: 'Startseite', exact: true }),
    ).toHaveCount(0);

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

    // The enriched card renders the localized substrate and
    // photoperiod wording (catalog form.substrate_soil and
    // form.photoperiodic) — never the raw wire values.
    await expect(card.getByText('Erde', { exact: true })).toBeVisible();
    await expect(card.getByText('Fotoperiodisch', { exact: true })).toBeVisible();
    await expect(card.getByText('soil', { exact: true })).toHaveCount(0);
    await expect(card.getByText('photoperiod', { exact: true })).toHaveCount(0);

    // Nothing re-rendered English: the German page heading is still
    // there after the locale chain settled (the mock reports no HA
    // language, so the browser locale stands).
    await expect(page.getByRole('heading', { name: 'Grows', exact: true })).toHaveCount(0);
  });

  test('grow detail renders german surfaces', async ({ page, request }) => {
    const name = uniqueName('E2E Deutsche Details');
    const scheduleId = await createSchedule(request, 'Deutscher Düngeplan');
    const growId = await createGrow(request, name, {
      feed_schedule_id: scheduleId,
    });
    // One manual entry so the timeline has a day group to render.
    await api(request, 'POST', `api/grows/${growId}/actions`, {
      plant_ids: [],
      occurred_at: new Date().toISOString(),
      action_type: 'water',
      details: { volume_ml: 250 },
      notes: null,
      appearance_tags: [],
    });

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

    // Feeding schedule section heading (catalog grow.feed_schedule).
    await expect(
      page.getByRole('heading', { name: 'Düngeplan', exact: true }),
    ).toBeVisible();

    // The timeline groups manual entries under German day headings
    // (catalog grow.day); the quiet auto tier (grow.auto_heading)
    // is pinned when mock HA light transitions exist.
    await expect(
      page.locator('[data-testid="timeline-day"]').first(),
    ).toHaveText(/Tag \d+/);

    // And nothing drifted back to English.
    await expect(
      page.getByRole('heading', { name: 'Environment', exact: true }),
    ).toHaveCount(0);
  });

  test('sweep surfaces render german', async ({ page }) => {
    // Ticket 04 sweep: the nav landmark label and the history report
    // join the keyed surfaces. The ingress spike is a debug-only page
    // since D124, so it has no release surface to sweep.

    // The Grows page is the landing fallback for a grow-less instance
    // (nav rework 2026-11-22); sibling specs create grows in the
    // shared throwaway DB, so pin the empty list for this page alone
    // to reach it.
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

    // The Grows page's empty banner (catalog grows.empty) and the
    // footer (catalog page.*): privacy line, version stamp and the
    // live backend status line.
    await page.goto(url('/grows'));
    await expect(
      page.getByText('Noch keine Anbauten — starte deinen ersten.'),
    ).toBeVisible();
    const footer = page.getByRole('contentinfo');
    await expect(
      footer.getByText(/Läuft lokal in deiner Home Assistant/),
    ).toBeVisible();
    await expect(footer.getByText(/\bv[0-9]/)).toBeVisible();
    await expect(footer.getByText('Funktioniert')).toBeVisible();

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
      page.getByRole('heading', { name: 'Ertrag nach Düngeplan' }),
    ).toBeVisible();
    await expect(
      page.getByRole('columnheader', { name: 'Nass (g)' }).first(),
    ).toBeVisible();
    await expect(
      page.getByRole('columnheader', { name: 'Anbauten' }).first(),
    ).toBeVisible();

    // The dashboard band above the aggregates (history.stat_*):
    // scoped to the band, since the nav link and a table header
    // reuse the word "Anbauten".
    const band = page.locator('[data-testid="history-summary"]');
    await expect(band.getByText('Anbauten', { exact: true })).toBeVisible();
    await expect(band.getByText('Ø Abweichung', { exact: true })).toBeVisible();
    await expect(band.getByText('Ertrag (trocken)', { exact: true })).toBeVisible();

    // The old 'Zurück zur Startseite' link is gone by design: the
    // Verlauf rework replaced the trailing link with the dashboard
    // band, and the header nav owns navigation now.

    // And none of the freshly keyed English surfaces renders.
    await expect(
      page.getByText(/Runs locally on your Home Assistant/),
    ).toHaveCount(0);
    await expect(
      page.getByText(/no cloud, no telemetry/),
    ).toHaveCount(0);
    await expect(
      page.getByRole('heading', { name: 'Predicted vs actual' }),
    ).toHaveCount(0);
    await expect(page.getByText('Nothing weighed yet.')).toHaveCount(0);
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

    // The bench-bar nav (nav rework 2026-11-22) has no Home link;
    // assert the three section links, one localised to de by the
    // forced-german spec above.
    await expect(
      page.getByRole('link', { name: 'History', exact: true }),
    ).toBeVisible();
    await expect(
      page.getByRole('link', { name: 'Grows', exact: true }),
    ).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Grows', exact: true })).toBeVisible();
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
    // grows empty copy and history report keep their English strings.
    // The ingress spike is a debug-only page since D124.
    // The Grows page is the landing fallback for a grow-less instance
    // (nav rework 2026-11-22); pin the empty list for this page alone
    // to reach it.
    await page.route('**/api/grows', (route) =>
      route.fulfill({
        status: 200,
        contentType: 'application/json',
        body: '[]',
      }),
    );
    await page.goto(url('/'));
    await expect(page.getByRole('navigation', { name: 'Main' })).toBeVisible();

    // The Grows page's empty banner and the footer (catalog page.*):
    // privacy line, version stamp and the live backend status line.
    await page.goto(url('/grows'));
    await expect(
      page.getByText('No grows yet — start your first one.'),
    ).toBeVisible();
    const footer = page.getByRole('contentinfo');
    await expect(
      footer.getByText(/Runs locally on your Home Assistant/),
    ).toBeVisible();
    await expect(footer.getByText(/\bv[0-9]/)).toBeVisible();
    await expect(footer.getByText('Healthy')).toBeVisible();

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
