# Leaflog

A self-hosted cannabis grow diary packaged as a **Home Assistant app** that
lives in your HA sidebar via ingress. Backend and frontend are both Rust.

> Status: M1 skeleton (workspace, HA packaging, ingress spike). See
> [`docs/SPEC.md`](docs/SPEC.md) for the full product spec and
> [`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) for how it fits together.

## Features (planned)

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

Add this repository as a local app repository in Home Assistant
(Settings → Add-ons → Local repository), then install **Leaflog**.
It appears as a sidebar tab. Both `amd64` and `aarch64` images are published
to `ghcr.io/purplebooth/leaflog`.

## License

Released into the public domain via the [Unlicense](UNLICENSE)
(SPDX: `Unlicense`). Bundled strain/seed data, when present, is third-party
and carries its own source and licence per record.
