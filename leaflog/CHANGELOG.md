# Changelog
All notable changes to the Leaflog Home Assistant app. Version headings are exactly `## <version>` — Home Assistant slices release notes on them. Generated file; never edit by hand.

- - -
## 0.8.0

_Released 2026-11-17._

#### Bug Fixes
- **(frontend)** a bound chart card survives a failed history fetch - (e0f9f15) - billie.codes
- **(mock-ha)** push a flipped light state on the next tick (bug c62200c) - (7b99597) - billie.codes
- a live-connection resubscribe skips the failure backoff - (4eab684) - billie.codes
#### Documentation
- register the LightCycleEditor lint exception - (69d5b9c) - billie.codes
- spec 4.9 precedence, glossary terms and d085-d086 close the spec (bug c62200c) - (ab7bdd5) - billie.codes
#### Features
- **(backend)** the coach resolves the real light state at call time (bug c62200c) - (10cc334) - billie.codes
- **(core)** the night VPD tip follows the research and the schedule answers now (bug c62200c) - (4727047) - billie.codes
#### Miscellaneous
- **(version)** 0.8.0 - (506a76f) - billie.codes
#### Refactoring
- **(core)** drop the unused window end and name the point-query window - (2ea29e6) - billie.codes
- **(core)** one lit rule for every light reader - (058de92) - billie.codes
#### Tests
- **(backend)** the temperature corridor steps with the default schedule - (10366e6) - billie.codes
- **(backend)** any on/off entity bound to the Light role drives the staircase - (51fcc7c) - billie.codes

- - -

## 0.7.0

_Released 2026-11-17._

#### Bug Fixes
- **(frontend)** give ECharts the option as plain objects (bug 1b2b540) - (c27c7af) - billie.codes
- **(frontend)** editor buttons submit the right thing - (1cede2d) - billie.codes
- tri-state grow-page test survives the removed side fetches - (c60b0ae) - billie.codes
#### Build
- **(frontend)** bundle tree-shaken echarts via npm + esbuild (D075) - (d136c35) - billie.codes
#### CI
- unique decision ids, a serialized gate, and e2e port pre-flight (D083-D085) - (f9449b0) - billie.codes
- build the echarts bundle in the image and ci pipelines - (956c56b) - billie.codes
#### Documentation
- **(release)** the v0.7.0 upgrade note - (56e8598) - billie.codes
- AGENTS.md status points at the changelog - (2b48c6e) - billie.codes
- the decision log matches the code (D081-D085) - (161a87c) - billie.codes
- the lamp fallback cites D044, not D021 - (bb1cd83) - billie.codes
- d082-d084 record the stepped-band choices and precedence (bug 745472e) - (7e509eb) - billie.codes
- the spec and app docs match the stripped app (71d8a42) - (339d8a0) - billie.codes
- d081 records the pair scatter migration and the card visibility rule (bug 89198dc) - (c4ee560) - billie.codes
- D080 records the role-mapped option builder and the band editor's new home - (1240b08) - billie.codes
- e2e follows the strain removal; THREAT-MODEL R3 resolved - (7ad9012) - billie.codes
- D079 records the option's plain-object wasm boundary - (76fef5e) - billie.codes
- d077 grow-sourced engine defaults - (d68c145) - billie.codes
- chart window glossary entry and D079 for the ECharts seam - (7c3e3ec) - billie.codes
- d075 feature-strip decision and glossary reframe (grow carries label/photoperiod/tolerances/watts) - (9453a79) - billie.codes
- renumber the light cycle decision to d078 - (68ade4c) - billie.codes
- d075 light cycle storage and defaults, glossary term - (e999d4d) - billie.codes
- echarts rendering decision (adr 0001, D075, AGENTS.md) - (82f825a) - billie.codes
- decisions for the night leaf offset and the VPD zero clamp - (ad8bbc2) - billie.codes
#### Features
- **(backend)** split history bands day/night from light state (bug 745472e) - (b78c2f5) - billie.codes
- **(backend)** drop the strain catalogue, the grow label carries the history - (62637ef) - billie.codes
- **(backend)** per-grow light cycle endpoint with validation and reset - (2b967ab) - billie.codes
- **(core)** expand light-state timelines and stepped band corridors (bug 745472e) - (ea5b9a7) - billie.codes
- **(core)** lights-off presets with sources and a pinned VPD zero clamp - (3fb7fa1) - billie.codes
- **(core)** strain-aware light cycle defaults with validation - (cf00ba0) - billie.codes
- **(frontend)** render stepped band segments as per-segment regions (bug 745472e) - (40348b5) - billie.codes
- **(frontend)** draw the ph/ec pairs as echarts scatters (bug 89198dc) - (45f8fbd) - billie.codes
- **(frontend)** render the environment charts through ECharts (bug 3cadef8) - (cb6f2dd) - billie.codes
- **(frontend)** the grow carries its strain — page, nav, picker and wizard step removed - (8ce6608) - billie.codes
- **(frontend)** phase picker follows the grow's photoperiod toggle - (633e234) - billie.codes
- **(frontend)** render the temperature chart through ECharts (bug 1b2b540) - (5cb3bae) - billie.codes
- **(frontend)** grow form strain label, photoperiod and advanced settings - (ce0be3c) - billie.codes
- **(frontend)** light cycle editor on the grow overview - (7bdcd84) - billie.codes
- drop spaces and the equipment catalogue end to end (9d39219) - (bb74cb9) - billie.codes
- recommendations read the grow, not the catalogue (5cf01b8) - (49b8beb) - billie.codes
- grow carries strain label, photoperiod and lamp facts (V14) - (aba9a79) - billie.codes
- drop the seed bank end to end (fb99a14) - (5f72c3f) - billie.codes
#### Miscellaneous
- **(release)** bump version to 0.6.0 - (0fec11d) - github-actions[bot]
- **(version)** 0.7.0 - (0cc6d8f) - billie.codes
#### Refactoring
- the forecast's flower override loses the strain name (71d8a42) - (20eac09) - billie.codes
#### Style
- cargo fmt after the fold - (7c65ba1) - billie.codes
#### Tests
- **(e2e)** the entity-ids payload drops strain_id; helper docs repaired - (daf58ed) - billie.codes
- **(e2e)** prove the stepped day/night band regions in the browser (bug 745472e) - (6ec3900) - billie.codes
- **(e2e)** pin the pair scatters and the hidden unbound cards (bug 89198dc) - (328ebd9) - billie.codes
- **(e2e)** pin the removed spaces/equipment surface and the wizard's skip path - (4a2748e) - billie.codes
- **(frontend)** judge bundle composition on the esbuild output graph - (c567473) - billie.codes
- **(frontend)** bundle checks for the echarts pipeline (D075) - (dd8158e) - billie.codes

- - -

## 0.6.0

_Released 2026-11-17._

#### Bug Fixes
- **(frontend)** space and seed-batch selects keep their pick on refetch - (16b7b3e) - billie.codes
#### CI
- concurrent e2e stacks, allow-register gate, weekly drift CI - (4776985) - billie.codes
#### Documentation
- prose lint fixes in the grow overview notes - (7d765ff) - billie.codes
- d074 float_cmp allowance and planned veg weeks note - (e54df5b) - billie.codes
- d073 phases-are-events, grow-overview glossary, tracker cli notes - (9af0358) - billie.codes
- move issue tracker to git-bug (D072) - (0f5b866) - billie.codes
- release runbook and AGENTS.md pointer - (84a112b) - billie.codes
#### Features
- **(backend)** grow history chart endpoint - (fefc527) - billie.codes
- **(core)** curing pin and collapsed history for skipped phases (d073) - (e124289) - billie.codes
- **(core)** flower-start pin for photoperiod and auto (d073) - (c4e1993) - billie.codes
- **(core)** forecast boundary pins for germ, seedling, veg (d073) - (dee0a56) - billie.codes
- **(core)** phase-change action type (d073) - (f67e8ec) - billie.codes
- **(core)** chart series merge and phase band corridor - (c0b0170) - billie.codes
- **(core)** open-ended photoperiod veg - (5e699aa) - billie.codes
- **(frontend)** land on the newest active grow overview - (0f3505d) - billie.codes
- **(frontend)** long ha entity ids wrap inside their cards - (ea1bd00) - billie.codes
- **(frontend)** environment strip spans the container - (2391745) - billie.codes
- **(frontend)** history chart with the shaded temperature corridor - (da56745) - billie.codes
- **(frontend)** phase picker on the grow overview (d073) - (b30e75f) - billie.codes
- ph and ec event scatter charts - (f2f1a5e) - billie.codes
- soil-moisture band preset with editor and chart - (c68b6ff) - billie.codes
- role charts for humidity, vpd, co2 and light - (9c767b3) - billie.codes
- appearance vocabulary with labels, meanings and groups - (2628245) - billie.codes
- phase events and planned vegetative weeks through the api - (44f0807) - billie.codes
#### Miscellaneous
- **(release)** bump version to 0.5.1 - (7fa8226) - billie.codes
- **(version)** 0.6.0 - (888ac46) - billie.codes
#### Tests
- **(e2e)** phase picker records the right transition event - (bda241f) - billie.codes

- - -

## 0.5.1

_Released 2026-11-17._

#### Bug Fixes
- **(dashboard)** show the No-data banner only for real dead bindings - (d788fe3) - billie.codes
- **(e2e)** point the stack at its mock; honest env poll, strain locators - (f9ce5aa) - billie.codes
- **(frontend)** cover timeout retry, gate retry button, tidy specs - (53a71ae) - billie.codes
- **(frontend)** strain select survives options mount and refetch - (6b3fe34) - billie.codes
- **(frontend)** position today marker by day count within gantt track - (9902108) - billie.codes
- **(grows)** tri-state list with per-resource fetch, timeout and retry - (a15f8e9) - billie.codes
- **(grows)** quick-add chips logged every entry as a note - (5bff00a) - billie.codes
#### Documentation
- **(agents)** D070 local issue tracker, triage labels, domain docs - (9b1d86f) - billie.codes
- **(decisions)** register fetch-list allows, tighten D071 prose - (64a89de) - billie.codes
- **(decisions)** D071 empty_enums allowance on Leptos-generated code - (d3666a4) - billie.codes
#### Features
- **(grows)** quiet header cluster with medium glyph and strain - (2645acc) - billie.codes
#### Miscellaneous
- **(release)** bump version to 0.5.0 - (670112e) - github-actions[bot]
- **(version)** 0.5.1 - (5ed19f2) - billie.codes
#### Tests
- **(e2e)** Playwright suite for every form under the ingress prefix - (ad193db) - billie.codes

- - -

## 0.5.0

_Released 2026-11-17._

#### Bug Fixes
- **(ci)** UI prose extraction drops Tailwind class lists (D065) - (063b515) - billie.codes
- **(image)** fetch the daisyUI bundles inside the container build - (ca3df1f) - billie.codes
- **(image)** Debian frontend stage so the Tailwind CLI execs (D064) - (8c8f841) - billie.codes
#### CI
- **(changelog)** serve an HA-sliceable app changelog (D062) - (34abd6b) - billie.codes
- add vale prose gate — LLM-tell vocabulary, 50-word cap (D065) - (b4bd02f) - billie.codes
#### Documentation
- discourage vendoring in AGENTS.md - (6a4b501) - billie.codes
#### Features
- **(backend)** space sensor items seed grow env bindings (D067) - (1ded3e6) - billie.codes
- **(frontend)** sensor-default guidance in picker and live panel (D067) - (efa2a6d) - billie.codes
- **(frontend)** grow and plant editing with delete (D066) - (2265e50) - billie.codes
- **(frontend)** Tailwind v4 + daisyUI 5 with green themes (D063, D064) - (dfcc429) - billie.codes
#### Miscellaneous
- **(ci)** cog check ignores merge commits (D068) - (6a970c4) - billie.codes
- **(release)** bump version to 0.4.1 - (12d10ca) - github-actions[bot]
- **(version)** 0.5.0 - (4bb86ae) - billie.codes

- - -

## 0.4.1

_Released 2026-11-17._

#### Bug Fixes
- **(frontend)** submit handlers must prevent_default themselves (D061) - (f949965) - billie.codes
#### Miscellaneous
- **(release)** bump version to 0.4.0 - (f31b5c9) - github-actions[bot]
- **(version)** 0.4.1 - (6c5e86f) - billie.codes

- - -

## 0.4.0

_Released 2026-11-17._

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

## 0.3.0

_Released 2026-11-17._

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

## 0.2.0

_Released 2026-11-17._

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

## 0.1.0

_Released 2026-11-17._

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