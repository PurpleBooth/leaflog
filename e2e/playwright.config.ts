// Playwright config for the Leaflog frontend e2e suite.
//
// The stack under test is started by the justfile `e2e` recipe (fresh
// SQLite DB, mock Supervisor, frontend dist under a fixed simulated
// ingress prefix on a private port) — this config only points the
// browser at it. Tests must address everything through the helpers'
// `url()` so no request ever bypasses the ingress prefix.

import { defineConfig, devices } from '@playwright/test';

const port = process.env.LEAFLOG_E2E_PORT ?? '8199';

export default defineConfig({
  testDir: './src',
  // One shared backend and database: tests must not race each other.
  // Numeric file-name prefixes give a deliberate order (01 onboarding
  // runs on the fresh DB and dismisses the wizard for the rest).
  workers: 1,
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  reporter: [['list'], ['html', { open: 'never' }]],
  use: {
    baseURL: `http://127.0.0.1:${port}`,
    trace: 'on-first-retry',
    screenshot: 'only-on-failure',
    actionTimeout: 10_000,
  },
  projects: [{ name: 'chromium', use: { ...devices['Desktop Chrome'] } }],
  outputDir: 'test-results',
});
