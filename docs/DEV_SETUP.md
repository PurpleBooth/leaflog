# Development Setup

## One-time host setup (Fedora Silverblue)

The toolchain is managed by [Devbox](https://www.jetify.com/devbox), which
needs Nix. Silverblue's immutable root does not provide `/nix`, so pick
**one** of the options below once, manually — this is deliberately not
automated.

| Option | How | Trade-offs |
|---|---|---|
| Transient root + Nix installer (ostree mode) | `sudo rpm-ostree install ...` is *not* used; instead enter a transient root shell (`sudo /bin/bash` in a `toolbox` or via `podman run --rm -ti --privileged ...`) and run the [Determinate Systems Nix installer](https://github.com/DeterminateSystems/nix-installer) in `ostree` mode | Cleanest integration; survives rebases; slightly unusual install path |
| Fedora `nix` package | `sudo rpm-ostree install nix` (if available in your Fedora release) then enable the daemon | Managed by the OS; may lag Nix upstream; needs a reboot |
| Single-user Nix in a toolbox | `toolbox enter` and run the standard Nix installer there | No host changes at all; Devbox must then also run inside the toolbox |

> This development machine already has `/nix` installed via the
> Determinate Systems installer — nothing to do here.

Keep **Podman on the host** (outside Devbox) for image builds; Devbox
does not manage it.

## Enter the environment

```sh
cd Leaflog
devbox shell          # installs/pins the toolchain on first run
```

`devbox.json` pins: `rustup` (toolchain via `rust-toolchain.toml`),
`just`, `trunk`, `sqlite`, `pkg-config`, `libiconv`, `cocogitto`. The
init hook adds the `wasm32-unknown-unknown` target and clippy/rustfmt.

## Daily commands

```sh
just dev    # backend + mock Home Assistant + frontend, simulated ingress
just ci     # everything CI checks: fmt, clippy (native+wasm), tests, wasm build
just image  # build the HA app image locally with Podman (amd64 by default)
```

## Testing against a real Home Assistant

`just dev` targets the bundled `mock-ha` by default. To point the backend
at a real Supervisor:

1. Run the app image locally or on your HA host.
2. Inside HA, the Supervisor injects `SUPERVISOR_TOKEN`; there is nothing
   to configure.
3. For a local backend against a real HA instance, set
   `LEAFLOG_ENV=dev`, `SUPERVISOR_TOKEN` and `LEAFLOG_SUPERVISOR_URL`
   in the environment (never commit values — `.env` is git-ignored).
