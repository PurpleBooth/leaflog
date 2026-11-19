# ADR 0003: Compile-time localisation catalogs; the chain is HA language → browser locale → English

Status: accepted
Date: 2026-11-18
Decided in: EN+DE localisation spec (git-bug 11052b1)
Related: D112 (mechanism record), D113 (backend catalog), D115 (ICU compiled data), D117 (prose gate narrowing), D118 (build-only advisory ignore), ARCHITECTURE.md §Localisation

## Context

The app ships in English and German. Strings appear on three surfaces the
spec separates: the frontend UI, backend alert templates and error
wrappers, and nothing else — seed data, evidence tags, breeder/strain
names and deep scientific detail text stay English by design. The app is
local-first: any mechanism that fetches strings, checks a licence server
or phones home at runtime is disqualified. The frontend is Leptos CSR
built with Trunk; the backend is Axum with no template engine. Language
is not a per-user account setting inside Leaflog — Home Assistant already
knows the user's language, and the app has no login screen of its own.

## Decision

Localization is compile-time on both sides, keyed, with no runtime
fetches:

- **Frontend** — `leptos_i18n` 0.6.3 with JSON catalogs under
  `crates/leaflog-frontend/locales/` (`en.json`, `de.json`). `en` is the
  default locale and the key source of truth; the parity of keys between
  the two catalogs is a build error (enforced in `build.rs`). Plural keys
  run on ICU message format with the `icu_compiled_data` feature so no
  runtime data provider is needed. Locale resolution: Home Assistant
  user language (via `GET /api/locale`) first, then the browser's
  `navigator.languages` (leptos_i18n's implicit CSR matching at first
  paint, cookie disabled — the HA step is applied explicitly by
  `LocaleChain`), then English. No language picker; a reload after
  changing the HA language is acceptable until ticket a3d56d9 makes the
  chain re-fetch.
- **Backend** — a small hand-rolled catalog (`catalog.rs`, D113):
  const template maps plus `format!`, an exhaustive `(locale, message)`
  match whose non-exhaustiveness is a compile error, so adding a message
  kind without a German rendering fails the build. The published HA
  language is refreshed on every Supervisor reconnect.
- **Rust keeps keys, not literals**: UI prose lives in the catalogs,
  which the prose gate never scans (D117). Wire codes (D114) carry
  localisation from `leaflog-core` engines to the views without core
  depending on either language.
- **German is du-form** (HA register), agent-drafted, with the owner's
  proofread as the release gate (ticket 4e037a5).

## Consequences

Catalogs grow with every keyed surface (307 keys per locale as of the
keying effort); parity and plural coverage are machine-checked at build
time. The mechanism's transitive build-time dependency on `serde_yaml`
made CI's `cargo deny` gate fail on an unmaintained-crate advisory
(D118) — ignored with a re-check trigger at the `leptos_i18n` upgrade.
English remains a valid, complete experience per spec; German coverage
is total for UI prose, partial for deep detail text by decision.
