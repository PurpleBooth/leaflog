# Changelog
All notable changes to the Leaflog Home Assistant app. Version headings are exactly `## <version>` — Home Assistant slices release notes on them. Generated file; never edit by hand.

- - -
## 0.22.0

_Released 2026-11-27._

#### Features
- **(http)** compress responses, validate caches with ETags, pin h2c - (a9bcf0c) - Billie Thompson
- **(story)** sum each device's unavailable hours instead of flagging partial days - (f6bf18d) - Billie Thompson
#### Miscellaneous
- **(release)** bump version to 0.21.1 - (789a014) - github-actions[bot]
- **(version)** 0.22.0 - (6e26fe8) - Billie Thompson

- - -

## 0.21.1

_Released 2026-11-27._

#### Bug Fixes
- **(ci)** scan the OCI platform name and keep playwright inside e2e/ - (5db9144) - Billie Thompson
- embed the time zone database so the charts keep their bands - (cadbfab) - Billie Thompson
#### CI
- hash-pin tool downloads, audit npm, scan and sign the image - (e6a5466) - Billie Thompson
#### Miscellaneous
- **(release)** bump version to 0.21.0 - (e8d015c) - github-actions[bot]
- **(version)** 0.21.1 - (1d9ffe2) - Billie Thompson

- - -

## 0.21.0

_Released 2026-11-27._

#### Bug Fixes
- **(build)** build the local image for the host architecture - (7390a55) - Billie Thompson
- **(story)** one quiet caveat line per day, auto-light legend once - (50ba35e) - Billie Thompson
#### Build
- **(app)** swap the s6 base image for a scratch container - (33862dc) - Billie Thompson
#### Miscellaneous
- **(build)** split release profiles — wasm size, server speed - (3427c74) - Billie Thompson
- **(build)** optimize the release profile for size - (416041f) - Billie Thompson
- **(i18n)** drop the dead grow.auto_heading key - (4e8eea5) - Billie Thompson
- **(release)** bump version to 0.20.0 - (cc13d8d) - github-actions[bot]
- **(version)** 0.21.0 - (716b3c7) - Billie Thompson

- - -

## 0.20.0

_Released 2026-11-27._

#### Features
- **(bindings)** read dead plug lights as off, per grow - (6b95e90) - Billie Thompson
- **(logging)** trace every HTTP request (TraceLayer) - (dfe634d) - Billie Thompson
- **(logging)** tracing on both sides of the wasm bridge - (7229cef) - Billie Thompson
#### Miscellaneous
- **(release)** bump version to 0.19.0 - (fa59a64) - github-actions[bot]
- **(version)** 0.20.0 - (e552299) - Billie Thompson

- - -

## 0.19.0

_Released 2026-11-27._

#### Bug Fixes
- **(api)** accept the band editor's min/max object for soil_moisture_band - (bfeb334) - billie.codes
- **(api)** validate setting value shapes and write settings in one transaction - (927576f) - billie.codes
- **(backend)** allow data: images in the ingress CSP - (e745509) - Billie Thompson
- **(backend)** show diary entries dated later on their day - (d08ecc5) - billie.codes
- **(env)** show a dash when no humidity band exists - (4eb28a1) - billie.codes
- **(env)** advise lamp height from the bound light's real dim level - (2f92c7a) - billie.codes
- **(feeds)** label the stored dose unit in one place - (d232f1f) - billie.codes
- **(frontend)** bound gantt caption spill to the chart - (949fd50) - Billie Thompson
- **(frontend)** keep the plant name as the story filter's link - (a5f8d2d) - Billie Thompson
- **(frontend)** carry log confirmation to the entry it created - (25a2fa8) - Billie Thompson
- **(frontend)** collapse the trait wall and add an all-plants toggle - (f51a67b) - Billie Thompson
- **(frontend)** show the undo window and acknowledge photo deletes - (b4a31f8) - Billie Thompson
- **(frontend)** day-scope the story buttons and name the photo delete - (f087bca) - Billie Thompson
- **(frontend)** make the story plant filter row clickable as a whole - (811d343) - Billie Thompson
- **(frontend)** visible focus ring and unclipped gantt captions - (abf70e8) - Billie Thompson
- **(frontend)** resolve entry stamps and two labels in the app locale - (1dcdcaf) - Billie Thompson
- **(ha)** time out the Supervisor handshakes and keep the dim level while the light is off - (8bfd089) - billie.codes
- **(ha)** bound every Supervisor REST call with connect and total timeouts - (4bf7da5) - billie.codes
- **(ha)** drop a Supervisor socket that stays silent past 2.5 heartbeats - (969a846) - billie.codes
- **(photos)** decode uploads and read files off the DB lock - (a5834c3) - billie.codes
- **(photos)** keep the thumbnail aspect ratio and honour the EXIF orientation - (74f8098) - billie.codes
- **(story)** name each photo link after its photo - (12a43a2) - billie.codes
- **(ui)** contrast floors, touch targets, reduced motion and page headings - (458894b) - billie.codes
#### CI
- pin workflow actions to commit SHAs and record the open tool downloads - (e408227) - billie.codes
- lint the wasm test targets too - (64ab700) - billie.codes
#### Documentation
- reconcile the allow register with the code - (e0a62fc) - billie.codes
#### Features
- **(settings)** give the seven backend settings a screen - (36dab41) - billie.codes
- **(story)** jump straight to the photographs - (8aa8d1b) - Billie Thompson
#### Miscellaneous
- **(deps)** bump cookie to 0.18.3 for RUSTSEC-2026-0594 - (80a98ad) - billie.codes
- **(impeccable)** drop the stale feed-schedules surface file - (22b4f3a) - billie.codes
- **(release)** bump version to 0.18.0 - (266e523) - github-actions[bot]
- **(version)** 0.19.0 - (3dce8b3) - Billie Thompson
- pre-prefix the devbox command - (ea6b106) - Billie Thompson
- Strip down some noise in the AGENTS.md - (f6a4695) - Billie Thompson
#### Performance
- **(chart)** fetch the roles' HA history concurrently - (7a87755) - billie.codes
- **(story)** window the action and photo rows in SQL before mapping - (a0b2e07) - billie.codes
- **(story)** read only the rows the page window needs - (412d07d) - billie.codes
#### Refactoring
- **(frontend)** bundle the story signals for the fetch helpers - (b854a6e) - Billie Thompson
- **(tests)** share started_today via tests/common - (760de2a) - billie.codes
- drop comments the allows and docs already carry - (0b2375d) - billie.codes
- satisfy flagged lints at the source - (91fccd4) - billie.codes
- delete lint allows the compiler proves dead - (537dd1e) - billie.codes
- tighten comments and drop duplication from the cleanup review - (ff6e439) - billie.codes
- delete dead options, dead code and stale text - (6ea2547) - billie.codes
#### Style
- address clippy nits from the critique pass - (36aa1cc) - Billie Thompson
- cargo fmt - (97910a2) - Billie Thompson
#### Tests
- **(frontend)** add photo_dates to the story component-test stub - (b4ecab4) - Billie Thompson
- **(grows)** drop the equipment-catalogue remark from the watt test - (5be1c31) - billie.codes

- - -

## 0.18.0

_Released 2026-11-27._

#### Bug Fixes
- **(frontend)** tighten feeding schedule card headers - (fc39cdb) - billie.codes
#### Features
- follow Home Assistant measurement units - (97e0343) - billie.codes
#### Miscellaneous
- **(release)** bump version to 0.17.0 - (276dcb9) - github-actions[bot]
- **(version)** 0.18.0 - (c0b30c9) - billie.codes
#### Tests
- **(e2e)** bound backdated photo pagination - (3c7798f) - billie.codes

- - -

## 0.17.0

_Released 2026-11-27._

#### Features
- **(backend)** serve paginated grow stories - (7ac333b) - billie.codes
- **(core)** model local-day grow stories - (41aa107) - billie.codes
- **(frontend)** unite the grow diary and photos - (32618b8) - billie.codes
#### Miscellaneous
- **(release)** bump version to 0.16.1 - (e24bfc7) - github-actions[bot]
- **(version)** 0.17.0 - (e92923d) - billie.codes

- - -

## 0.16.1

_Released 2026-11-27._

#### Bug Fixes
- **(frontend)** unify page spacing and coach text - (71a9b8b) - billie.codes
#### Miscellaneous
- **(release)** bump version to 0.16.0 - (503fa4c) - github-actions[bot]
- **(version)** 0.16.1 - (c764fb3) - billie.codes

- - -

## 0.16.0

_Released 2026-11-27._

#### Bug Fixes
- **(backend)** cache Trunk's bundles when the hash is shorter than 16 digits - (6553b8e) - billie.codes
- **(frontend)** add space above the grows heading - (a019c9b) - billie.codes
- **(frontend)** move the dead-binding mark onto the caption line (D149) - (b89b6a0) - billie.codes
- **(test)** compare the humidity band's end with its own window edge - (4073d84) - billie.codes
#### Features
- **(frontend)** move the phase picker into the quick-add menu (D148) - (f7e2618) - billie.codes
#### Miscellaneous
- **(design)** refresh the design tool configuration - (041dc9e) - billie.codes
- **(release)** bump version to 0.15.4 - (1039683) - github-actions[bot]
- **(version)** 0.16.0 - (cb10e45) - billie.codes
#### Refactoring
- **(frontend)** drop the schedules intro and delete-note captions - (bc2004e) - billie.codes

- - -

## 0.15.4

_Released 2026-11-27._

#### Features
- **(frontend)** bench-bar navigation, Grows-first landing and quiet footer (D147) - (92a0907) - billie.codes
- **(frontend)** fold the drift banner into per-tile warning marks (D146) - (ba4e23c) - billie.codes
#### Miscellaneous
- **(release)** bump version to 0.15.3 - (efc538d) - github-actions[bot]
- **(version)** 0.15.4 - (5766064) - billie.codes

- - -

## 0.15.3

_Released 2026-11-27._

#### Bug Fixes
- **(frontend)** short attribution line and Foto uploads through quick add (D144) - (341c22d) - billie.codes
- close the audit findings (D145) - (27a5a47) - billie.codes
#### Miscellaneous
- **(release)** bump version to 0.15.2 - (6ac3768) - github-actions[bot]
- **(version)** 0.15.3 - (dd97ee4) - billie.codes

- - -

## 0.15.2

_Released 2026-11-27._

#### Bug Fixes
- contain the feeding section and restore the prototype dose-list metrics (D143) - (e4f19c3) - billie.codes
#### Miscellaneous
- **(release)** bump version to 0.15.1 - (b5146a1) - github-actions[bot]
- **(version)** 0.15.2 - (c7d0bc7) - billie.codes

- - -

## 0.15.1

_Released 2026-11-27._

#### Bug Fixes
- **(test)** compare the Air shading with slack instead of byte equality - (5fd9360) - billie.codes
- revalidate the app shell so a new release actually reaches the browser (D142) - (8fc3a22) - billie.codes
#### Miscellaneous
- **(release)** bump version to 0.15.0 - (66cf658) - github-actions[bot]
- **(version)** 0.15.1 - (1b3a0f9) - billie.codes

- - -

## 0.15.0

_Released 2026-11-27._

#### Bug Fixes
- **(ci)** pin chromedriver to our Chrome build, not the runner image's - (4c54463) - billie.codes
- **(ci)** create the wasm tools bin directory before linking into it - (f45a179) - billie.codes
- **(ci)** download the x86_64 wasm-bindgen asset that exists and fail on HTTP errors - (5b20e6e) - billie.codes
- align the history bands and extract the chip, fact and card-title recipes (D141) - (8ee1fec) - billie.codes
- repaint rejected number entries and keep form labels locale-reactive (D140) - (2209d75) - billie.codes
- give the overview's section headings and group labels breathing room (D138) - (cdc967d) - billie.codes
- serialize the adopted theme as chart-safe sRGB and name the main line - (4414163) - billie.codes
#### Features
- fold the RH window under RH and drop the overview explainer copy (D136-D137) - (1c528e0) - billie.codes
#### Miscellaneous
- **(release)** bump version to 0.14.0 - (0bc9616) - github-actions[bot]
- **(version)** 0.15.0 - (a44b171) - billie.codes
#### Refactoring
- migrate the remaining fieldset wrappers onto ui::Field (D140) - (89eb15e) - billie.codes
#### Tests
- add browser-run component tests and extract the shared field/button components (D139-D140) - (28c8407) - billie.codes

- - -

## 0.14.0

_Released 2026-11-27._

#### Bug Fixes
- **(frontend)** reserve root-zone space when the feed log has pH/EC - (0740721) - billie.codes
- **(frontend)** align dimmable row and card the edit-page sections - (eb691aa) - billie.codes
- **(frontend)** fit the schedule editor grid inside the container - (10bbd7a) - billie.codes
- **(frontend)** schedule library unit labels and German intro copy - (553bce0) - billie.codes
- **(frontend)** localize substrate and photoperiod option labels - (7b7d9aa) - billie.codes
- **(frontend)** scope link rule to non-button anchors - (c3ebbc9) - billie.codes
#### Documentation
- record the overview and dashboard cleanup decisions (D131-D134) - (eeb9733) - billie.codes
#### Features
- **(frontend)** reserve lazy root-zone space only for bound grows - (ad13278) - billie.codes
- **(frontend)** group the timeline by day with a quiet auto tier - (aaafa9c) - billie.codes
- **(frontend)** enrich grow list cards - (d2ea98e) - billie.codes
- **(frontend)** rework Verlauf as a dashboard band - (583a6c3) - billie.codes
#### Miscellaneous
- **(release)** bump version to 0.13.0 - (1c800a9) - github-actions[bot]
- **(version)** 0.14.0 - (e61ef4a) - billie.codes
#### Tests
- **(e2e)** pin german band, card wording and timeline day headings - (c9d7938) - billie.codes
- **(e2e)** pin the localised substrate label on the grow card - (9dbe13a) - billie.codes

- - -

## 0.13.0

_Released 2026-11-27._

#### Documentation
- **(release)** the v0.13.0 upgrade note - (a745746) - billie.codes
#### Features
- feeding schedules are product-dose grids, fit per grow - (a61f76f) - billie.codes
#### Miscellaneous
- **(release)** bump version to 0.12.0 - (2fa43a1) - github-actions[bot]
- **(version)** 0.13.0 - (93d6994) - billie.codes

- - -

## 0.12.0

_Released 2026-11-27._

#### Bug Fixes
- **(backend)** land the subscribe_entities seed so steady entities report available - (1bd23b7) - billie.codes
- **(frontend)** key the light-cycle reset rollback to its captured values - (74b0551) - billie.codes
- **(frontend)** gate the spike route to debug builds and summarize the health card - (32750cf) - billie.codes
- **(frontend)** place the humidity window tile beside the humidity reading - (9d54643) - billie.codes
- **(frontend)** route the remaining decimal sites through the locale formatter - (4a8c8be) - billie.codes
- **(frontend)** locale-aware number formatting and german copy pass - (1fd7b04) - billie.codes
- **(frontend)** classify api failures and add dismissable, retryable error banners - (795f345) - billie.codes
#### Features
- **(frontend)** remember recent quick-add actions - (e6dfc11) - billie.codes
- **(frontend)** add undoable entry deletion and light-cycle reset - (88bd4a3) - billie.codes
- **(frontend)** warn before abandoning unsaved grow edits - (89f078c) - billie.codes
- **(frontend)** regroup quick-add actions into a dialog picker - (b1c1ca7) - billie.codes
#### Miscellaneous
- **(release)** bump version to 0.11.0 - (3ba944a) - github-actions[bot]
- **(version)** 0.12.0 - (4e861e0) - billie.codes
#### Refactoring
- **(frontend)** extract ui primitives and re-anchor spacing to the design spec - (f9bf746) - billie.codes

- - -

## 0.11.0

_Released 2026-11-27._

#### Bug Fixes
- **(backend)** window-bound the marker scan before its limit - (3b3c646) - billie.codes
- **(frontend)** bake icu plural data so plural keys render - (ddde21c) - billie.codes
- **(frontend)** drift-banner badge shows the entity id again - (0887bc6) - billie.codes
- **(frontend)** hold the lazy mount until the section layout settles - (820f1a7) - billie.codes
- **(frontend)** fixed grid insets align every connect-group plot - (1846aa5) - billie.codes
#### CI
- **(prose)** narrow the prose gate to markdown and Rust comments - (c3cc3b0) - billie.codes
#### Documentation
- **(adr)** record the localisation mechanism as ADR 0003 - (e9f889f) - billie.codes
- **(decisions)** split the D110 opener under the prose sentence cap - (85fc491) - billie.codes
- **(i18n)** D112 decision entry and the keying pattern - (c50ea3e) - billie.codes
- correct the locale-chain and parity comments - (5d1c30b) - billie.codes
- record the sweep's residual-English inventory as D116 - (47de281) - billie.codes
- correct D105's chip list — light retired by D110 - (8ccc601) - billie.codes
- endorse the palette on record; log D111 (lazy watch, hint keying, sweep) - (870fe6c) - billie.codes
- night-shading decision D110; fix the duplicate D106 id and a stray merge fragment - (e8f36d4) - billie.codes
- log the marker zone split and the hide/replace render (D107-D108) - (7e2c4bc) - billie.codes
#### Features
- **(backend)** hand-rolled message catalog for alerts and error wrappers - (4a7aec8) - billie.codes
- **(backend)** capture the HA user language and serve it on /api/locale - (65a595e) - billie.codes
- **(backend)** night stretches on the Air-group history payload - (a67fb09) - billie.codes
- **(backend)** markers on the grow history response - (c3af46f) - billie.codes
- **(core)** event-marker selection for chart roles - (fe01bc1) - billie.codes
- **(env)** temperature and VPD Target Bands - (7a8ed78) - billie.codes
- **(frontend)** key home, spike, nav label and the history report - (98a0fa7) - billie.codes
- **(frontend)** localise the api failure fallback for error banners - (cfc48d5) - billie.codes
- **(frontend)** add keys for the remaining surfaces to the locale catalogs - (6008606) - billie.codes
- **(frontend)** key the plant view page and the default plant name - (976007f) - billie.codes
- **(frontend)** key the grow edit page and the light cycle editor - (a9af62d) - billie.codes
- **(frontend)** key the bindings grid and the onboarding wizard - (8c25042) - billie.codes
- **(frontend)** add form-surface keys to the locale catalogs - (c82f5a6) - billie.codes
- **(frontend)** key the grow-form name placeholder; e2e and docs - (dafca2a) - billie.codes
- **(frontend)** key environment, gantt and feed surfaces for locale - (7b05a02) - billie.codes
- **(frontend)** i18n catalogs, label helpers and coach/lamp wiring - (bad1f73) - billie.codes
- **(frontend)** leptos_i18n catalogs with the locale chain wired - (a0d02ba) - billie.codes
- **(frontend)** lazy root-zone charts and honest empty-window hint - (9e2bb8e) - billie.codes
- **(frontend)** fold the Light card into the Air charts' night shading - (0f95828) - billie.codes
- **(frontend)** night shading as a silent back layer in the chart option - (927560c) - billie.codes
- **(frontend)** shared marker toggle in the section header - (255a717) - billie.codes
- **(frontend)** log-event ticks on history charts - (e03535d) - billie.codes
- **(frontend)** Air and Root zone chart groups with crosshair sync - (53e6c9a) - billie.codes
- **(frontend)** shared y-label gutter and line crosshair for the connect group - (c420855) - billie.codes
- **(frontend)** a not-measured card chips the unbound sensor roles - (9b81de6) - billie.codes
- **(frontend)** the Environment section owns its shared history window - (323f33f) - billie.codes
#### Miscellaneous
- **(deps)** ignore the build-only unsafe-libyaml advisory - (b210dd0) - billie.codes
- **(release)** bump version to 0.10.0 - (f299ff3) - github-actions[bot]
- **(version)** 0.11.0 - (960b847) - billie.codes
#### Refactoring
- **(core)** stable codes for coach tips and lamp advice, citations dropped - (3054a29) - billie.codes
- **(core)** make chart_group the single role-to-group table - (363a7dd) - billie.codes
- **(e2e)** one filled-path walker under the chart predicates - (650c2cc) - billie.codes
- **(frontend)** drop the dead joined_crosshair opt-out - (a78cc6c) - billie.codes
#### Tests
- **(backend)** the marker history rides real chart data - (25394db) - billie.codes
- **(e2e)** assert the grow-less banner in both locale sweeps - (cd01a95) - billie.codes
- **(e2e)** sweep the newly keyed surfaces in both locales - (6f04841) - billie.codes
- **(e2e)** assert the german-forced grow edit form labels - (74fbed5) - billie.codes
- **(e2e)** pin the German-forced browser and the English default - (8a7a47b) - billie.codes
- **(e2e)** verification sweep — quiet ticks, stable svg roots, screenshot matrix - (195240e) - billie.codes
- **(e2e)** night shading coverage; retire the light-card assertions - (44118ec) - billie.codes
- **(e2e)** chart-group and crosshair-sync specs, mock history spread - (f1f4819) - billie.codes
- **(e2e)** log-event marker selection, ticks, toggle, tooltip - (0e29fd3) - billie.codes

- - -

## 0.10.0

_Released 2026-11-27._

#### Features
- **(frontend)** the logging loop opens in the grow header - (841c0fc) - billie.codes
#### Miscellaneous
- **(design)** track the impeccable detector config - (a5fa39c) - billie.codes
- **(release)** bump version to 0.9.0 - (8cc9e92) - github-actions[bot]
- **(version)** 0.10.0 - (267f7e9) - billie.codes

- - -

## 0.9.0

_Released 2026-11-27._

#### Bug Fixes
- **(frontend)** tile_class is const - (a474b03) - billie.codes
- **(frontend)** env tile colours track the merged live readings - (bd5e463) - billie.codes
- **(frontend)** apply the built-in light theme on :root at last (07) - (06475d9) - billie.codes
- **(frontend)** read the cross-realm parent root and observe with an attribute array (07) - (a82a791) - billie.codes
#### CI
- the release gate runs the shared cog-check recipe - (ab73f7f) - billie.codes
- cog check ignores merge commits - (619e33b) - billie.codes
#### Documentation
- **(frontend)** one accurate doc line for empty_action - (3874c15) - billie.codes
- **(threat-model)** TM4.1 credits the delete confirmation it has - (8834469) - billie.codes
- capture product truth and design system for impeccable - (60f31bf) - billie.codes
- drop a stray diff3 base marker in DECISIONS.md - (31d5b38) - billie.codes
- ADR 0002 drops a 'There are' opening for the prose gate - (4112ff2) - billie.codes
- D099 quick-add icons via leptos_icons + Lucide - (b47544e) - billie.codes
- reword ADR 0002 sentence that tripped the prose lint - (8a8c040) - billie.codes
- fix the ADR 0002 sentence the prose lint rejects - (d434486) - billie.codes
- reword ADR 0002's 'There are' opener for the prose gate - (eda34d3) - billie.codes
- D104 logs the raw-event anchor for Phase day - (a50e38c) - billie.codes
- D098 — recompute, not migrate, for the Germination pick - (55441f4) - billie.codes
- ADR 0002 (light cycle observes, never actuates) and grow-rework glossary terms - (969290e) - billie.codes
- multi-session habits, the wasm boundary rule, and the devbox flag quirk - (b4d986b) - billie.codes
#### Features
- **(backend)** dev-only theme harness page framing the app (07) - (93b7a8c) - billie.codes
- **(backend)** the Germination pick sets the start date - (93b69c6) - billie.codes
- **(core)** HA theme adoption colour math and daisyUI mapping (07) - (8152201) - billie.codes
- **(core)** local_date — the local calendar date of an instant - (bc6a2ec) - billie.codes
- **(frontend)** env strip renders live values over one SSE connection - (63657f3) - billie.codes
- **(frontend)** quick add collapses into one dropdown menu - (6323721) - billie.codes
- **(frontend)** adopt the parent HA theme at startup and live (07) - (7775b1a) - billie.codes
- **(frontend)** grow edit page hosts all grow configuration - (b932f43) - billie.codes
- **(frontend)** the grow overview shows a Phase day badge - (e47dca2) - billie.codes
- **(light-cycle)** the editor captions the light entity's learned pattern - (f47ef0b) - billie.codes
- the forecast anchors Phase day at the transition event - (785e1e6) - billie.codes
#### Miscellaneous
- **(release)** bump version to 0.8.0 - (b3157b8) - github-actions[bot]
- **(test)** drop a weasel word the prose lint flags - (f8dfcae) - billie.codes
- **(version)** 0.9.0 - (d317848) - billie.codes
- merge in new main commits - (71353fe) - billie.codes
#### Refactoring
- **(backend)** one RFC 3339 parse step for the UTC date fallbacks - (7e16c49) - billie.codes
- **(backend)** the observed-reality cache owns its lookups - (ef4df71) - billie.codes
- **(core)** ObservedReality embeds ObservedCycle - (2138dda) - billie.codes
- **(core)** one sRGB transfer helper for luminance and Oklab - (dac2964) - billie.codes
- **(frontend)** the live merge reads the canonical role constants - (8ff8be3) - billie.codes
#### Tests
- **(e2e)** theme harness adoption, live re-adoption and built-in fallback (07) - (2ae2abe) - billie.codes
- **(e2e)** quick-add logging goes through the menu - (42908e4) - billie.codes
- **(e2e)** the header cluster counts the phase-day badge - (cf9244b) - billie.codes
- **(e2e)** the Phase day badge equals the Day count until a pick lands - (1474d0e) - billie.codes

- - -

## 0.8.0

_Released 2026-11-27._

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
- **(release)** bump version to 0.7.0 - (71ea905) - github-actions[bot]
- **(version)** 0.8.0 - (506a76f) - billie.codes
#### Refactoring
- **(core)** drop the unused window end and name the point-query window - (2ea29e6) - billie.codes
- **(core)** one lit rule for every light reader - (058de92) - billie.codes
#### Tests
- **(backend)** the temperature corridor steps with the default schedule - (10366e6) - billie.codes
- **(backend)** any on/off entity bound to the Light role drives the staircase - (51fcc7c) - billie.codes

- - -

## 0.7.0

_Released 2026-11-27._

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

_Released 2026-11-27._

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

_Released 2026-11-27._

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

_Released 2026-11-27._

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

_Released 2026-11-27._

#### Bug Fixes
- **(frontend)** submit handlers must prevent_default themselves (D061) - (f949965) - billie.codes
#### Miscellaneous
- **(release)** bump version to 0.4.0 - (f31b5c9) - github-actions[bot]
- **(version)** 0.4.1 - (6c5e86f) - billie.codes

- - -

## 0.4.0

_Released 2026-11-27._

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

_Released 2026-11-27._

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

_Released 2026-11-27._

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

_Released 2026-11-27._

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