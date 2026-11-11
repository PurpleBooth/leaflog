# Leaflog e2e

Playwright end-to-end tests for the Leptos frontend, run against the
same stack shape as `just dev`: backend + mock Supervisor + the built
frontend `dist/`, under a fixed simulated ingress prefix
(`/api/hassio_ingress/e2e`) with a throwaway SQLite database.

## Run

```sh
just e2e-install   # once per machine: npm deps + Playwright chromium
just e2e           # builds the frontend, starts the stack, runs the suite
```

Both need Node (`devbox run -- just …` provides it). Stop `just dev`
first: both write `crates/leaflog-frontend/dist/`.

Single test or filtered re-run, against a stack already started by
`just e2e` in another terminal:

```sh
cd e2e && npx playwright test --grep "seed batch"
npx playwright show-report
```

## Layout and rules

- One shared backend and database per run: `workers: 1`, and the
  numeric file prefixes order the specs (`01-onboarding` runs on the
  fresh DB and dismisses the wizard for the rest). A failed run needs a
  fresh `just e2e` for the onboarding spec; CI retries everything.
- Every navigation and API call goes through `url()` in `src/helpers.ts`
  so nothing ever bypasses the ingress prefix.
- Every form case arms `armNoReloadGuard` before submit and calls
  `expectNoReload` after: a native form submit reloads the page and
  loses the marker — exactly the D061 bug class. It then reloads and
  asserts the data persisted.
- **New form in the app ⇒ new case in `src/02-forms.spec.ts`.**
- Tooling only: Node lives in this directory and in CI, never in the
  shipped image (docs/DECISIONS.md D069).
