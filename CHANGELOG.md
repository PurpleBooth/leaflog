# Changelog
All notable changes to this project will be documented in this file. See [conventional commits](https://www.conventionalcommits.org/) for commit guidelines.

- - -
## v0.4.0 - 2026-11-10
#### CI
- **(changelog)** generate CHANGELOG.md with cocogitto in CI - (fed0dea) - billie.codes
- **(deny)** cargo-deny gate for advisories and licences (D047) - (780f227) - billie.codes
#### Documentation
- spec roadmap M11/M12 - harvest, yield, cross-grow history, EN+DE (D046) - (99589e6) - billie.codes
- equipment catalogue and grow spaces (D043, D044) - (3392731) - billie.codes
#### Features
- **(api)** bundled equipment and strains are read-only (D050) - (7908e41) - billie.codes
- **(backend)** stable bundled ids with insert-only upgrades (D051) - (19d5307) - billie.codes
- **(backend)** refinery migrations with checksum-verified history (D048) - (f9fc855) - billie.codes
- **(backend)** grow spaces with equipment references and space-aware lamp advice - (25e5251) - billie.codes
- **(core)** trim equipment catalogue to four categories (D049) - (3042669) - billie.codes
- **(core)** grow-space domain types and equipment catalogue flags - (b015b74) - billie.codes
- **(frontend)** spaces tab, space picker and bundled badges - (e417e1e) - billie.codes
- environment alerts route to HA notifications (D060) - (e7b02af) - billie.codes
- canonical timezone with local calendar grow days (D059) - (eb0108f) - billie.codes
- snapshot retention and entity-drift status (D058) - (bf335b2) - billie.codes
- cross-grow history, per-plant views and start-next-run (D057) - (3299aea) - billie.codes
- germination-test log with observed rate (D056) - (1c0912f) - billie.codes
- harvest weights, yield summary and strain write-back (D055) - (07f2068) - billie.codes
- first-run onboarding wizard with derived state (D054) - (049c3f3) - billie.codes
- seedbank with viability windows and batch-linked grows (D053) - (003caed) - billie.codes
- sensor items are picked Home Assistant entities (D052) - (a312fa8) - billie.codes
#### Miscellaneous
- **(version)** 0.4.0 - (741a2cb) - billie.codes

- - -

## v0.3.0 - 2026-11-10
#### Bug Fixes
- **(backend)** add nosniff and a hash-pinned CSP to served responses - (bae8103) - billie.codes
- **(backend)** reject traversal and interior separators in photo path builders - (e139c7e) - billie.codes
- **(backend)** wrap multi-statement store mutations in transactions - (e7b9434) - billie.codes
- **(backend)** treat a failed subscribe_entities result as disconnected - (01b3d3e) - billie.codes
- **(backend)** percent-encode entity_id in the Supervisor history URL - (34962e3) - billie.codes
- **(backend)** store every uploaded photo part, all-or-nothing - (e980bf1) - billie.codes
- **(backend)** re-derive phase from the incoming action on update - (f922bc5) - billie.codes
- **(backend)** accept datetime-local action timestamps as UTC - (621dbca) - billie.codes
- **(ci)** harden release version-sync and drop unused dispatch input - (47f7943) - billie.codes
#### Features
- **(backend)** seed predefined equipment and extra strains on empty DB - (d8cd4f4) - billie.codes
#### Miscellaneous
- **(version)** 0.3.0 - (e6d88a6) - billie.codes
- add gitleaks to dev env - (bb87315) - billie.codes

- - -

## v0.2.0 - 2026-11-10
#### Bug Fixes
- **(backend)** normalize photos to rgb8 so non-rgb inputs cannot panic - (ebcf88e) - billie.codes
- **(ci)** cocogitto tarball extracts to x86_64-unknown-linux-musl/cog - (726b341) - billie.codes
- **(ci)** cocogitto tarball extracts to a dir; find and copy the binary - (9f0163a) - billie.codes
- **(core)** keep fitted feed days unique and avoid flush-tail underflow - (af6c2ab) - billie.codes
- **(frontend)** keep in-app links inside the ingress prefix - (30bf156) - billie.codes
#### Build
- cocogitto for conventional-commit enforcement - (27b4518) - billie.codes
#### CI
- add gitleaks full-history secret scan - (97e11b9) - billie.codes
- add workflow_call trigger for reusable workflow calls - (2c43802) - billie.codes
- release pipeline and PR conventional-commit validation - (dc1636a) - billie.codes
#### Documentation
- update status sections from M1 skeleton to v0.1.0 - (cf1cc66) - billie.codes
- fix stale references, dialect slips, and a duplicated M6 block - (c4801a4) - billie.codes
- threat model with risk register and open items - (7a13afa) - billie.codes
- HACS installation and release workflow instructions - (c1553aa) - billie.codes
#### Features
- **(hacs)** HACS compatibility via companion integration - (d6397e4) - billie.codes
#### Miscellaneous
- **(release)** bump version to 0.1.0 - (9aad472) - github-actions[bot]
- **(version)** 0.2.0 - (70d3af8) - billie.codes

- - -

## v0.1.0 - 2026-11-10
#### Bug Fixes
- **(ci)** reference arch-tagged images when merging the multi-arch manifest - (23a7e5a) - billie.codes
- **(ha)** pin wasm-bindgen-cli in the image builder (trunk auto-fetch failed on aarch64 CI) - (d458f19) - billie.codes
- **(ha)** copy s6 rootfs from the repo-root build context - (7854a67) - billie.codes
- **(ha)** pin base image with the real GHCR tag scheme (3.24-2026.11.0) - (0686b70) - billie.codes
- **(ha)** fetch trunk release with curl (busybox wget chokes on redirects) - (b38be3b) - billie.codes
#### Build
- **(backend)** migration 0004 and feed/substrate stores - (bdbbc62) - billie.codes
- **(backend)** register sensors store module - (6518f3d) - billie.codes
- **(backend)** migration 0003 (sensor bindings and snapshots) - (48f2352) - billie.codes
- **(backend)** migration 0002 and stores for grows, actions, photos - (424c5f2) - billie.codes
- **(backend)** SQLite storage with versioned migrations - (ca59246) - billie.codes
- **(ha)** app packaging with pinned base and multi-arch Dockerfile - (29fcf0d) - billie.codes
- justfile recipes for dev, ci, and image - (6c2fdc8) - billie.codes
- cargo workspace with the mandatory lint policy - (7160045) - billie.codes
#### CI
- GitHub Actions CI gate and per-arch image publishing - (2655c94) - billie.codes
#### Documentation
- screenshots (desktop + mobile) and accessibility audit - (c005b2b) - billie.codes
- M8 decision log (backup test, unit display, screenshot plan) - (c964fb5) - billie.codes
- M7 decision log (VPD, lamp advisor, thresholds, coach) - (d8a1677) - billie.codes
- M6 decision log (feed fit, substrate profiles, TDS) - (483da01) - billie.codes
- M5 decision log (forecast chain, reality compression, Gantt) - (5f6664f) - billie.codes
- M4 decision log (bridge, token hygiene, auto-capture, mock protocol) - (ca700fd) - billie.codes
- M3 decision log (photo pipeline, EXIF timezone, phase derivation) - (a4b6e32) - billie.codes
- M2 decision log (storage, migrations, seed sourcing, lint register) - (e2d14bd) - billie.codes
- record base image tag scheme and s6 deprecation notice - (b0b9e9f) - billie.codes
- architecture, decision log, and dev setup - (df070e8) - billie.codes
#### Features
- **(backend)** env, lamp and coach API endpoints - (4a02b57) - billie.codes
- **(backend)** feed API, substrate profiles, template seeding - (78bddf9) - billie.codes
- **(backend)** GET /api/grows/{id}/forecast - (d39a5ca) - billie.codes
- **(backend)** HA API routes - entities, history, SSE live, bindings - (d48966e) - billie.codes
- **(backend)** Supervisor bridge - REST, WebSocket, backoff reconnect - (da5e3f1) - billie.codes
- **(backend)** grows, action log and photo pipeline API - (aff65fe) - billie.codes
- **(backend)** equipment, strain and settings API with seed catalogue - (5ae42a2) - billie.codes
- **(backend)** ingress-correct SPA serving with runtime base injection - (ca1121e) - billie.codes
- **(core)** unit-aware display helpers - (d1d4d20) - billie.codes
- **(core)** VPD engine, lamp advisor, phase coach - (c6a6e63) - billie.codes
- **(core)** feed-fit engine, substrate profiles, TDS scales - (6571cd0) - billie.codes
- **(core)** phase forecast engine - (d262311) - billie.codes
- **(core)** action log and grow domain types - (b0a132e) - billie.codes
- **(core)** equipment and strain domain types - (c0deb69) - billie.codes
- **(frontend)** dual-unit display in dashboard and timeline - (1542eac) - billie.codes
- **(frontend)** env dashboard with VPD gauge and coach - (ede5f13) - billie.codes
- **(frontend)** feed schedule panel with before/after diff - (7769a01) - billie.codes
- **(frontend)** Gantt timeline with today marker - (f430a4c) - billie.codes
- **(frontend)** grows overview and grow detail page - (c80d7af) - billie.codes
- **(frontend)** equipment and strains pages - (d759969) - billie.codes
- **(frontend)** Leptos CSR spike with runtime router base - (26630df) - billie.codes
- **(mock-ha)** real WS protocol, history, lib/bin split (D023) - (312bd44) - billie.codes
- **(mock-ha)** fake Supervisor REST stub for development - (2d8d48f) - billie.codes
#### Miscellaneous
- update Cargo.lock for M5 - (da59dc0) - billie.codes
- bootstrap Leaflog repository (license, spec, toolchain) - (eda059a) - billie.codes
#### Tests
- **(backend)** backup→restore keeps DB and photos (spec 13) - (65391d2) - billie.codes
- **(backend)** M3 integration suites - (0c06e0e) - billie.codes



- - -

Changelog generated by [cocogitto](https://github.com/cocogitto/cocogitto).