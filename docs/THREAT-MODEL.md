# Leaflog Threat Model

Status: complete for v0.1.0 · Date: 2026-11-09 · Review triggers in
[§14](#14-revisit-triggers).

Companion documents: [`ARCHITECTURE.md`](ARCHITECTURE.md) (structure),
[`DECISIONS.md`](DECISIONS.md) (one-off choices, including the
encryption-at-rest decision D036), [`SPEC.md`](SPEC.md) (product).

## 1. Scope and method

Modelled: the Leaflog app as deployed in Home Assistant (backend, WASM
frontend, packaging, build pipeline) and every flow that touches its
data. Home Assistant itself, the host OS, and the LAN are treated as
environment: their threats are recorded wherever they reach Leaflog,
but mitigating them is out of the app's control and marked as such.

Method: assets and actors first, then trust boundaries, then seven
threat categories. Five are project-specific (the disclosure of
cultivation, abuse of the platform, manipulation of data to harm
plants, destruction of data, and access by minors and household
members), one covers encryption at rest, and one is the standard
STRIDE plus web and supply-chain sweep. Each scenario names its
mitigations as they exist in the code today, with file references,
and ends in one of:

- **Mitigated** — control in place and tested.
- **Open** — accepted gap with a recommendation in [§13](#13-open-items).
- **Accepted** — no control, and none planned; reasoning given.

Ratings are likelihood (rare / possible / likely) times impact
(low / medium / high / critical) on the scale Low, Medium, High,
Critical. Impact is judged against the owner as a person, not only
against the software: the data proves cultivation of cannabis, which
carries social stigma, employment and custody consequences, and in
some jurisdictions attracts targeted burglary.

## 2. The system on one page

```
                 internet (only if the owner exposes HA)
   ┌──────────────────────────────────────────────────────────┐
   │  TB1  Browser  ←─ HA session cookie, TLS at HA ──→  HA   │
   │                    frontend (HA origin)                  │
   │                      │ /api/hassio_ingress/<token>/      │
   │  TB2              HA ingress proxy (172.30.32.2)         │
   │                      │ X-Ingress-Path, stripped prefix   │
   │                 Leaflog backend (Axum, :8099, no host    │
   │                 port published)                           │
   │                   │                    │                  │
   │  TB3              │ SUPERVISOR_TOKEN    │ TB4 /data       │
   │                Supervisor REST+WS    leaflog.db + photos  │
   │                (admin-level token)    (per-app volume)    │
   └──────────────────────────────────────────────────────────┘
        TB5  build pipeline → published image → user's HA
        TB6  /data → HA backup archive → NAS / cloud / drawer
```

Trust boundaries:

| ID | Boundary | Crossing control |
|---|---|---|
| TB1 | Browser to HA | HA's authentication and TLS (HA's responsibility) |
| TB2 | Ingress proxy to backend | Production gate: only `172.30.32.2` may connect (`app.rs:273`); no host port is published (`leaflog/config.yaml` has no `ports:`) |
| TB3 | Backend to Supervisor | Bearer `SUPERVISOR_TOKEN`, read-only endpoints only |
| TB4 | Backend to `/data` | Plain filesystem, per-app volume mapped only into this container |
| TB5 | Build to image | Pinned base image, committed lockfile, CI credentials |
| TB6 | `/data` to backups | HA `backup: hot`; thumbnails excluded (`backup_exclude`) |

Two structural facts drive most of the analysis:

1. **The app lives on HA's origin.** The SPA is served under
   `/api/hassio_ingress/<token>/` on the same origin as the HA
   frontend, so script execution inside Leaflog (an XSS) can call HA
   APIs with the victim's session. Leaflog XSS is therefore HA XSS in
   impact.
2. **The Supervisor token is administrator-level.** Compromise of the
   Leaflog process yields full control of the HA instance (cameras,
   locks, alarms, presence). The codebase contains no `call_service`
   (verified by search): the HA link is read-only
   (`GET /core/api/states`, `GET /core/api/history/period/...`,
   WS `subscribe_entities`, `crates/leaflog-backend/src/ha.rs`).

## 3. Assets

| Asset | Where | Disclosure impact | Loss impact | Tamper impact |
|---|---|---|---|---|
| Grow diary (grows, plants, actions, feeds, sensor snapshots, settings) | `/data/leaflog.db` | High: proves cultivation | High: months of records | Medium: wrong advice |
| Photos (originals and thumbnails) | `/data/photos/` | High: depict plants, grow space, possibly faces and identifying detail | Medium | Low |
| Photo EXIF/GPS | Upload stream only | Critical if stored: location | — | — |
| `SUPERVISOR_TOKEN` | Process env, `ha.rs` | Critical: admin over HA | — | — |
| Sensor bindings and presets | DB | Low | Medium | High: engines mis-advise |
| Availability of `/data` | Host disk | — | High: diary continuity | — |
| People named or shown in notes/photos | DB, photos | High: affects third parties too | — | — |

## 4. Actors

| Actor | Capability | Intent |
|---|---|---|
| Owner (HA admin) | Everything | Accidental: fat-finger deletes, misconfiguration |
| Minors and household members | Whatever HA session they can reach | Curious or mischievous; may share what they find |
| LAN guest | Sniff, scan; reach HA's port | Passive or opportunistic |
| Internet attacker | Only if HA is exposed (port forward, weak credentials) | Full HA takeover |
| Compromised HA (phished remote access, malicious or broken add-on) | Everything Leaflog trusts | Stealthy, patient |
| Malicious co-tenant add-on | Own container on the HA network; host-path access if the owner granted it | Lateral movement |
| Host root / physical thief | Whole disk | Theft, ransomware, disposal of disks |
| Supply-chain attacker | crates.io, GitHub release artifacts, GH Actions, base image, HACS | Code execution at build or update time |
| Beneficiaries of disclosure (employer, landlord, litigant, burglar) | None technical | They consume the output of a leak; they set the impact ratings |

## 5. TM1: Disclosure that the owner cultivates cannabis

Cultivation data is the product's core secret. Legality varies (the
owner is in Germany, where up to three plants per adult are permitted
but must be secured from third parties and minors, `SPEC.md` §9), and
the realistic harms are stigma, employment and custody consequences,
and targeted burglary, not only embarrassment. Every path below is
rated for disclosure impact **high** or above unless noted.

| # | Path | Status | Detail |
|---|---|---|---|
| 1.1 | Photo EXIF/GPS leaks with stored images | Mitigated | Uploads are decoded to pixels and re-encoded as clean JPEG; metadata cannot survive the re-encode (`photos.rs:101`, D017). Tested: stored originals carry no EXIF APP1 and no GPS block. The capture timestamp is deliberately kept (`D018`) |
| 1.2 | Photo content itself (room, faces, post on the table, serial numbers) | Accepted | Not fixable in software beyond metadata; the owner curates what they photograph |
| 1.3 | HA backups and off-box copies | Open | The biggest realistic vector. `backup: hot` includes the DB and photo originals in every HA backup; wherever those archives go (NAS, cloud sync, a drawer), the disclosure follows. Thumbnails are excluded (`backup_exclude`) purely to save space. Recommendation R5: document, and steer users to HA's passphrase-encrypted backups (see TM6) |
| 1.4 | HA support bundles and diagnostics | Accepted | App logs are sparse (error lines, seed counts); a support bundle still reveals that a cannabis diary is installed (app list, panel title). Inherent to asking for support |
| 1.5 | LAN eavesdropping | Accepted (out of app control) | HA terminates TLS. On installs reached over plain HTTP on the LAN, photos and JSON are sniffable. Nothing Leaflog can do beyond not adding its own plaintext surface (there is none: no host port, no direct UI) |
| 1.6 | Browser artifacts on shared machines | Open | No `localStorage`/`sessionStorage` use anywhere in the frontend (verified), and history entries show only the opaque ingress URL. But photo responses carry no `Cache-Control` (`api_grows.rs:498`), so originals and thumbnails land in the browser disk cache of any machine the panel was used on. Recommendation R2: `private, no-store` on originals |
| 1.7 | The HA sidebar itself | Accepted | `panel_admin: true` limits the panel (icon `mdi:cannabis`, title "Leaflog") to administrator accounts (`leaflog/config.yaml:16`). Anyone who can see an admin's HA UI sees the panel. Per-user sidebar hiding exists in HA if the owner wants it |
| 1.8 | Telemetry or cloud calls | Mitigated | Local-first by construction: the only outbound connection the app makes is to the Supervisor (`lib.rs:171`). No analytics, no fonts, no CDN, no update pings (verified: frontend fetches only relative API paths, external links are user-authored and `rel="noreferrer"`) |
| 1.9 | Dev-mode exposure | Mitigated by configuration | Dev mode accepts any peer (`app.rs:273`). It exists for `just dev` only; production is auto-selected when `SUPERVISOR_TOKEN` is present (D005), and the container publishes no ports. Never set `LEAFLOG_ENV=dev` in a real install |

## 6. TM2: Leaflog used as a platform to attack others

The app must never become the weak door into the household, and must
not be usable to attack third parties.

| # | Path | Status | Detail |
|---|---|---|---|
| 2.1 | Token theft via compromise of the Leaflog process (image-decode bug, dependency exploit) | Mitigated in depth, blast radius accepted | With the process compromised, the admin token yields all of HA. Controls: the token is sent only to the Supervisor, lives behind a redacted-`Debug` newtype, and never appears in a response or log line, asserted by an integration test that exercises every HA endpoint (D021, `tests/ha.rs`). The Supervisor surface is strictly read-only, so the token is used for nothing an attacker could not do faster from HA itself |
| 2.2 | XSS in the SPA pivoting to HA (same origin) | Mitigated | Leptos contextual escaping everywhere; zero uses of `inner_html` or equivalent (verified). The one string the backend injects into HTML, the ingress base, is character-allowlisted, HTML-escaped and JS-escaped (`ingress.rs`, tested against markup and traversal payloads). One gap: user-authored strain `source_url` renders as an `href` with no scheme check (`strains.rs:177` in the frontend) — a `javascript:` URL is self-inflicted today, but becomes an attack vector the moment strain import (CSV/JSON, deferred) ships. Recommendation R3 |
| 2.3 | CSRF against Leaflog endpoints | Mitigated | Three independent blockers: HA authenticates the session before proxying anything into ingress; the per-install ingress path token is unguessable, so a hostile site cannot even address the routes; and every mutating handler requires a JSON or multipart body that a cross-site form cannot forge usefully. The app sets no cookies of its own and emits no CORS headers |
| 2.4 | SSRF via the Supervisor link | Mitigated | The backend calls only the Supervisor base URL from the environment (`lib.rs:171`). The history passthrough interpolates a user-supplied `entity_id` into the Supervisor query (`ha.rs:181`), but the caller is already an HA administrator querying their own instance: no privilege crosses a boundary |
| 2.5 | Container as a beachhead on the add-on network | Mitigated by packaging | No host network, no privileged mode, no Docker socket, `map: data:rw` only, no published ports (`leaflog/config.yaml`). A compromised container still has typical outbound network access; egress filtering is host-level and out of scope. Co-tenant add-ons cannot pass the ingress IP gate, and IP spoofing from another container is not possible without control of the host network namespace |
| 2.6 | Resource abuse aimed at HA (reconnect storms, history hammering) | Mitigated | WS reconnect backs off 1→60 s with reset after auth (`ha.rs:229`, unit-tested); history hours are clamped 1..720 (`api_ha.rs:67`); SSE streams are `watch`-channel receivers, effectively free |
| 2.7 | Reputation abuse (shipping malware via the image or HACS) | Partially open | The shipped `custom_components/leaflog` is a no-op marker for HACS update tracking (verified read: registers nothing). Build-time downloads are version-pinned but not hash-pinned: Trunk and wasm-bindgen in `leaflog/Dockerfile`, cocogitto in `.github/workflows/release.yml`. Recommendation R4. Images are not signed |

## 7. TM3: Manipulation of data to harm plants

Leaflog advises; it has no actuators. There is no `call_service`
anywhere in the codebase (verified by search), and AGENTS.md gates any
future actuation behind an explicit opt-in setting. That is the single
most important structural mitigation in this category: the worst a
data attack can do today is bad advice, executed by the owner's own
hand.

| # | Path | Status | Detail |
|---|---|---|---|
| 3.1 | Poisoned sensor data (spoofed MQTT/ESPHome device, compromised HA) shifting VPD windows, lamp advice, auto-captured snapshots | Accepted | Advisory-only engines. Partial controls: the dashboard shows the raw readings so gross anomalies are visible; feed fitting clamps EC steps (200 µS/cm default, D026); advice is presented as ranges. A patient attacker shifting values plausibly defeats it, but an adversary who controls HA already controls locks and cameras; the plants are not the crown jewels in that scenario |
| 3.2 | Diary tampering via the API by an authorised user | Accepted | Any HA administrator can rewrite or delete anything; there is no second layer of authorisation below HA admin, by design (single-user, `panel_admin: true`). Non-admin HA users cannot reach the app at all |
| 3.3 | Preset or bundled-data tampering | Mitigated (bundled), open (future import) | Bundled strain records are embedded in the signed-by-publishing binary and re-seed only into an empty table (D014). User presets are user data. The deferred CSV/JSON import is the risk: imported strains with extreme ranges would steer forecast and feed fit. Range validation is a hard requirement on that feature (tracked in R8's context, §14) |
| 3.4 | Clock or timestamp manipulation skewing day counts and phases | Accepted | Day derivation trusts grow start, logged flips/harvests and the clock. EXIF timestamps are stored naive-UTC (D018) and can shift a photo's day number by hours-to-a-day. Cosmetic-to-minor |
| 3.5 | Feed-schedule sabotage (nutrient burn or deficiency) | Partially mitigated | Templates are editable by any app user, but the fitter's EC step clamp bounds how fast a sabotaged schedule can change concentrations (D026) |
| 3.6 | Auto-log spam from a flapping `light.*` entity distorting the diary | Mitigated | Opt-out setting `auto_log_ha_events` (default on, D022); entries are labelled `auto:` in notes so they are distinguishable and prunable |

## 8. TM4: Destruction or loss of data

Months of diary and photos are the product; they live in exactly two
places, `/data` and HA backups.

| # | Path | Status | Detail |
|---|---|---|---|
| 4.1 | Accidental or hostile deletion via the API | Open | `DELETE /api/grows/{id}` cascades to plants, actions, photos and the photo directory tree (`photos.rs:198` uses `remove_dir_all`). There is no soft delete, no undo window, and no confirmation dialog in the frontend (verified: no `confirm()` anywhere). One misclick or one hostile tab can erase a grow. Recommendation R8 |
| 4.2 | App uninstall or data wipe | Mitigated by HA flow | Removing the app deletes `/data`; HA prompts for a backup first. Restore is integration-tested end to end, including lazy thumbnail regeneration (D032, `tests/backup.rs`) |
| 4.3 | Ransomware or host compromise encrypting `/data` and the backups | Open (user side) | Nothing app-level resists root. The mitigations are encrypted, geographically separate backups (TM6, R5) |
| 4.4 | Disk exhaustion | Open | Each upload is capped at 25 MiB (body limit 32 MiB, `api_grows.rs:22`) but total photo growth is unbounded; the 2 GiB `photo_warn_bytes` threshold is advisory only. A full disk risks failed writes and WAL corruption. Recommendation R6 |
| 4.5 | Upgrade or migration bug | Mitigated | Migrations are embedded, applied in order, each in its own transaction (D012). No rollback exists; the safety net is the pre-upgrade backup HA takes automatically on app updates |
| 4.6 | Decompression bomb crashing the process | Open | The 25 MiB byte cap does not bound decoded size: a small PNG claiming extreme dimensions allocates width × height × 3 bytes in RAM (`photos.rs:108`), which can abort the process (s6 restarts it; no persistent damage, no row written). Authenticated-only, hence Medium overall, but cheap to fix. Recommendation R1 |
| 4.7 | Physical media failure (SD-card-class hosts are the HA norm) | Open (user side) | WAL mode improves crash safety, not durability. Backups are the answer (R5) |

## 9. TM5: Access by minors and other household members

Two harms run in opposite directions: a child finding the diary is a
disclosure event (TM1), and a child (or anyone) with reach into the
panel can also destroy (TM4). German law additionally requires
cultivation to be secured against access by third parties, especially
minors (`SPEC.md` §9); the app's plant-count and securing reminders
are informational only and default off outside DE, per the spec.

| # | Path | Status | Detail |
|---|---|---|---|
| 5.1 | A non-admin HA account opening the panel | Mitigated | `panel_admin: true` hides the sidebar entry and ingress access from non-administrator users (`leaflog/config.yaml:16`) |
| 5.2 | A shared or kiosk device holding an admin session (family tablet, borrowed phone with the companion app, an unlocked laptop) | Accepted (app side), Open (guidance) | To the app, that session *is* the owner. No in-app PIN, passcode or lock exists. Mitigation is HA hygiene: per-person accounts, MFA on admin accounts, signing out of shared devices. Recommendation R5 covers documenting this |
| 5.3 | A minor learning or guessing the admin password (autofill, written-down credentials) | Accepted for v1 | A second factor inside the app (optional PIN gate) was considered and deferred: it protects against a subset of 5.2/5.3 at the cost of daily friction in a single-user app. Revisit if multi-user ever lands (§14) |
| 5.4 | Photos of the grow left in a shared computer's browser cache | Open | Same finding as 1.6; `Cache-Control: private, no-store` on photo responses (R2) closes it |
| 5.5 | HA exposed to the internet with weak credentials | Out of app control | If HA falls, Leaflog falls with it; nothing in the app adds resistance below HA auth. Guidance-level (R5) |

## 10. TM6: Encryption at rest

Current posture, stated plainly:

| Data | At rest | In backups |
|---|---|---|
| `leaflog.db` (whole diary) | Plaintext SQLite (WAL) | Plaintext inside the backup archive, unless the owner enables HA's passphrase-encrypted backups |
| `photos/originals`, `photos/thumbnails` | Plaintext JPEG/WebP, metadata-stripped | Originals included, thumbnails excluded (`backup_exclude`) |
| `SUPERVISOR_TOKEN` | Plaintext container env, set by the Supervisor | Never in backups (Supervisor-managed) |

Threats this exposes: physical theft or disposal of the host disk,
NAS or backup media; unencrypted cloud-synced backup folders; any
co-tenant add-on the owner has granted host-path access (SSH add-ons,
file managers) or host root, all of which read `/data` in plaintext.

App-level encryption (SQLCipher for the DB, encrypted photo files) was
evaluated and **deferred** (D036): a key stored in add-on options or
environment sits on the same disk as the ciphertext, and the
Supervisor writes options to the host filesystem, so encryption would
not resist the realistic adversaries (host root, SSH add-on, physical
theft of the whole machine). It would add key-loss data-destruction
risk (TM4) in exchange for protection against a narrow partial-disk
theft scenario. That is a poor trade for a single-user diary.

What actually moves the needle, all user-side:

1. **Encrypted HA backups** (passphrase in HA's backup settings): the
   highest-value control, because backups travel further than the
   host (NAS, cloud, drawers) and are the likeliest thing to be lost
   or stolen. Thumbnails being excluded is irrelevant to secrecy;
   originals are the secret.
2. **Full-disk encryption of the HA host** where practical: protects
   the theft and disposal cases. Honest caveat: most HA hosts are
   headless Raspberry Pis, where LUKS needs remote unlock and is
   rarely worth it; on x86 NVMe hosts it is cheap and worth doing.
3. **Restraint with host-path add-ons**: every add-on granted access
   to the host filesystem can read the diary in plaintext; that is a
   smaller attack surface than it sounds (owner-granted), but belongs
   in the hardening notes.

These belong in a user-facing hardening section (R5), not in the app.

## 11. TM7: The usual suspects (STRIDE, web, supply chain)

The systematic sweep across every boundary, so nothing rests only on
the categories above.

| Category | Finding | Status | Reference |
|---|---|---|---|
| Spoofing | End-user identity is HA's session; the app never sees or stores credentials | Delegated by design | §2 TB1 |
| Spoofing | Backend gate trusts the proxy IP `172.30.32.2` only; this authenticates the proxy, not the user, which is correct: user auth happened at TB1 | Mitigated | `app.rs:273`, tested |
| Tampering | SQL injection: every store query is parameterised (`params!` throughout `store/`; no string-formatted SQL) | Mitigated | verified by search |
| Tampering | `X-Ingress-Path` header injection into HTML/JS: character allowlist, `..` rejection, HTML and JS escaping, build-time `<base>` stripping | Mitigated | `ingress.rs`, tested |
| Tampering | Path traversal: asset serving allowlists characters and rejects `..`/absolute (`assets.rs:61`); photo paths go through `safe_join` and file names are server-generated ids, never client strings (`photos.rs:214`) | Mitigated | tested |
| Repudiation | No audit trail of who deleted what (single-user, minimal logs) | Accepted | revisit with multi-user, §14 |
| Disclosure | Token never in logs or responses | Mitigated | D021, `tests/ha.rs` |
| Disclosure | Error bodies: API errors are JSON (structure-safe); Supervisor error text is truncated to 300 chars and echoed only to the already-authenticated caller | Accepted | `ha.rs:205` |
| DoS | Body limit 32 MiB; photo cap 25 MiB; history clamp; cheap SSE; capped reconnect backoff | Mitigated | §6.6, `app.rs:24` |
| DoS | Image decompression bomb (unbounded decode) | Open | R1, see 4.6 |
| DoS | Disk exhaustion | Open | R6, see 4.4 |
| Elevation of privilege | No app accounts, no cookies, no secrets in the browser; the WASM bundle holds nothing but UI | Mitigated (nothing to elevate) | frontend verified |
| XSS | Framework escaping, zero `inner_html`, escaped base injection; URL scheme gap on `source_url` | Mostly mitigated | R3, see 2.2 |
| CSRF | Unguessable path token + JSON/multipart bodies + no app cookies | Mitigated | see 2.3 |
| SSRF | Fixed Supervisor URL from env only | Mitigated | see 2.4 |
| Clickjacking / framing | Not applicable: HA ingress panels render in HA's own iframe by design | n/a | §2 |
| Open redirects | None: the app issues no redirects | Mitigated | verified |
| Supply chain: crates | `Cargo.lock` committed; no `cargo audit`/`cargo deny` in CI yet | Open | R7 |
| Supply chain: base image | Pinned `ghcr.io/home-assistant/base:3.24-2026.11.0`, never `latest` (D007) | Mitigated | `leaflog/Dockerfile` |
| Supply chain: build tools | Trunk, wasm-bindgen (Dockerfile) and cocogitto (release workflow) downloaded by version over HTTPS without hash verification | Open | R4 |
| Supply chain: CI | Only the built-in `GITHUB_TOKEN` (no custom secrets in workflows); PR title validation; no `pull_request_target` flows | Mitigated | `.github/workflows/` |
| Supply chain: published image | Not cosign-signed; users pin trust to GHCR/PurpleBooth | Open (low) | R9 |
| Ingress conformance | Relative URLs only (build `public_url = "./"`), runtime base injection, no login redirects, explicit `DefaultBodyLimit` | Mitigated | `ARCHITECTURE.md`, `tests/ingress.rs` |

## 12. Risk register

Ratings for the open and accepted items above; everything marked
mitigated is omitted from the table.

| ID | Risk | L | I | Rating | Status |
|---|---|---|---|---|---|
| TM1.3 | Diary and photos disclosed via unencrypted backups | likely | high | **High** | Open (R5) |
| TM1.6 / TM5.4 | Photos persist in shared machines' browser cache | possible | medium | Medium | Open (R2) |
| TM2.2b | `javascript:` URL in strain `source_url` (vector via future import) | rare today | high | Low today, High with import | Open (R3) |
| TM2.7 | Un-hash-pinned build-tool downloads | rare | critical | Medium | Open (R4) |
| TM3.1 | Sensor poisoning steers advice | possible | medium | Medium | Accepted |
| TM4.1 | One-click destructive delete, no undo | possible | high | **High** | Open (R8) |
| TM4.4 | Disk exhaustion from unbounded photo growth | possible | medium | Medium | Open (R6) |
| TM4.6 | Decompression bomb crashes the app | rare | medium | Low-Medium | Open (R1) |
| TM5.2 | Shared admin session exposes the diary to household members | possible | medium | Medium | Accepted (app), Open (guidance, R5) |
| TM6 | No encryption at rest on host or (by default) in backups | possible | high | **High** | Deferred (D036), guidance (R5) |
| TM7 | No dependency-audit gate in CI | rare | medium | Low-Medium | Open (R7) |
| TM7 | Unsigned images | rare | high | Medium | Open (R9) |

## 13. Open items

Prioritised; none blocks v0.1.0.

| ID | Item | Addresses |
|---|---|---|
| R1 | Cap decoded image dimensions (reject above ~40 MP) before decode, or use the `image` crate's `Limits` | TM4.6, TM7 DoS |
| R2 | `Cache-Control: private, no-store` on photo originals (and `private, max-age=…` for thumbnails, which are small and low-detail) | TM1.6, TM5.4 |
| R3 | Validate `source_url` scheme (`http`/`https` only) on input and render. **Hard prerequisite for the strain-import feature** | TM2.2b |
| R4 | SHA256-pin the Trunk, wasm-bindgen and cocogitto downloads in Dockerfile and release workflow | TM2.7 |
| R5 | User-facing hardening notes (a `leaflog/DOCS.md` section or `docs/HARDENING.md`): encrypted HA backups, per-person HA accounts with MFA on admins, sign out of shared devices, FDE on x86 hosts, restraint with host-path add-ons, beware browser caches | TM1.3, TM4.3, TM4.7, TM5.2, TM6 |
| R6 | Optional photo storage quota setting (default off) with a clear UI state when hit | TM4.4 |
| R7 | Add `cargo audit` (or `cargo deny`) to `just ci` and the CI workflow | TM7 |
| R8 | Product decision: confirmation dialog for cascade deletes, and/or a soft-delete "recently deleted" window with purge on backup | TM4.1 |
| R9 | Cosign-sign published images (nice-to-have) | TM7 |

## 14. Revisit triggers

Re-open this document when any of the following happens:

- **Any actuation feature** (`call_service`, however gated). The
  advisory-only stance is the backbone of TM3; actuation converts bad
  advice into dead plants and re-rates the whole category. The
  AGENTS.md opt-in gate is mandatory, and this model must be updated
  in the same change.
- **The data-import feature** (CSV/JSON strains): activates TM2.2b and
  TM3.3's validation requirements.
- **Multi-user or non-admin panel access**: re-opens TM5 and the
  repudiation acceptance in TM7.
- **Any change to** the photo pipeline, `backup_exclude`, token
  handling, or the ingress gate.
- **Each milestone's HA docs re-check** (per AGENTS.md), since
  ingress semantics are upstream-controlled.
