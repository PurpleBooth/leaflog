# Leaflog

A self-hosted cannabis grow diary packaged as a **Home Assistant app** that
lives in your HA sidebar via ingress. Backend and frontend are both Rust.

> Status: v0.1.0 released (2026-11-08); milestones M1-M8 complete. See
> [`docs/SPEC.md`](docs/SPEC.md) for the full product spec and
> [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for how it fits together.

## Features

- Grows, plants, strains, equipment inventory and per-grow snapshots
- Action + photo log with automatic Home Assistant sensor capture
- Phase forecast (germination → cure) with earliest/likely/latest windows
- Feeding schedules stretched/shrunk to the plant's real timeline
- VPD, lamp-height advisor, substrate-aware and strain-aware coaching
- Fully local: no telemetry, no cloud calls, EXIF/GPS stripped from photos

## Development

Requirements: a one-time host setup for Nix/Devbox (see
[`docs/DEV_SETUP.md`](docs/DEV_SETUP.md)), then:

```sh
devbox shell   # enters the pinned toolchain environment
just dev       # backend + mock Home Assistant + frontend, simulated ingress
just ci        # fmt, clippy (native + wasm), tests, WASM build
```

## Installing as a Home Assistant app

**Option 1: HACS (recommended)**

1. Add this repository to HACS as a custom repository
   (type: **Integration**)
2. Install the **Leaflog** integration — this tracks the repository
   and delivers update notifications
3. Add `https://github.com/PurpleBooth/leaflog` as a local add-on
   repository in Home Assistant
   (Settings → Add-ons → ⋮ → Repositories)
4. Install the **Leaflog** app from the Add-on Store

**Option 2: Add-on Store directly**

1. Add this repository as a local repository in Home Assistant
   (Settings → Add-ons → ⋮ → Repositories)
2. Install **Leaflog** — it appears as a sidebar tab

Both `amd64` and `aarch64` images are published to
`ghcr.io/purplebooth/leaflog`.

## Development workflow

Commits follow the [Conventional Commits](https://www.conventionalcommits.org/)
specification; [cocogitto](https://github.com/cocogitto/cocogitto) enforces
them (`cog check`) and generates the changelogs on release — the root
`CHANGELOG.md` for GitHub and `leaflog/CHANGELOG.md`, which Home
Assistant serves as the in-app changelog.

```sh
devbox shell
just dev          # develop against mock Home Assistant
just ci           # full gate: fmt, clippy, tests, WASM build
```

### Releasing

1. Merge all changes to `main` (conventional commits enforced on PRs)
2. Tag a release: `cog bump --auto` (or manually `git tag vX.Y.Z`)
3. Push the tag — the Release workflow runs the full CI, generates
   the changelogs, creates a GitHub Release, and publishes the
   multi-arch image

## License

Released into the public domain via the [Unlicense](UNLICENSE)
(SPDX: `Unlicense`). Bundled strain/seed data, when present, is third-party
and carries its own source and licence per record.
