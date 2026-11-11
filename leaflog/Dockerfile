# syntax=docker/dockerfile:1
# Leaflog app image. Each architecture builds natively (per-arch CI
# runners, or plain Podman locally); no cross toolchain involved. The
# musl build happens inside the container, so the Devbox environment
# needs no musl targets (spec §7 / docs/DECISIONS.md D007).

ARG TARGETARCH
# Pinned Home Assistant Alpine base — never `latest` (spec §7). Tag
# scheme is <alpine>-<yyyy.mm.n>; resolved from ghcr.io on 2026-11-08.
ARG HA_BASE=ghcr.io/home-assistant/base:3.24-2026.11.0

# ---------------------------------------------------------------------------
# Stage 1: build the frontend (Trunk, relative public URL).
# Debian (glibc), not Alpine: Trunk's [tools] fetch of the Tailwind
# standalone CLI brings a glibc binary that cannot exec on musl
# (relocation errors even under gcompat), and this stage only produces
# dist/ files — no static-musl requirement (D064). Tool triplets differ
# per upstream: trunk ships gnu builds, wasm-bindgen linux builds are
# musl-only — and being static, they run on glibc too. --retry: the
# release downloads truncate now and then behind this network.
# ---------------------------------------------------------------------------
FROM rust:1.99.1-slim-bookworm AS frontend-builder
ARG TARGETARCH
RUN apt-get update && apt-get install -y --no-install-recommends curl ca-certificates \
    && rm -rf /var/lib/apt/lists/* \
    && case "$TARGETARCH" in \
      amd64)   GNU=x86_64-unknown-linux-gnu;  MUSL=x86_64-unknown-linux-musl  ;; \
      aarch64) GNU=aarch64-unknown-linux-gnu; MUSL=aarch64-unknown-linux-musl ;; \
      *) echo "unsupported TARGETARCH: $TARGETARCH" >&2; exit 1 ;; \
    esac \
    && curl -fsSL --retry 3 "https://github.com/trunk-rs/trunk/releases/download/v0.21.14/trunk-${GNU}.tar.gz" \
      | tar -xz -C /usr/local/bin && trunk --version \
    && curl -fsSL --retry 3 "https://github.com/rustwasm/wasm-bindgen/releases/download/0.2.130/wasm-bindgen-0.2.130-${MUSL}.tar.gz" \
      | tar -xz --strip-components=1 -C /usr/local/bin && wasm-bindgen --version \
    && curl -fsSL --retry 3 "https://github.com/casey/just/releases/download/1.40.0/just-1.40.0-${MUSL}.tar.gz" \
      | tar -xz -C /usr/local/bin just && just --version
RUN rustup target add wasm32-unknown-unknown
WORKDIR /build
COPY Cargo.toml Cargo.lock* rust-toolchain.toml justfile migrate.just ./
COPY crates crates
# The daisyUI bundles are gitignored (fetched, never vendored); the
# same pinned+verified recipe that `just dev`/`just ci` run fetches
# them here too.
RUN just tailwind-daisyui
RUN cd crates/leaflog-frontend \
    && trunk build --release \
    && mkdir -p /out && cp -r dist /out/dist

# ---------------------------------------------------------------------------
# Stage 2: build the backend with the embedded frontend bundle.
# ---------------------------------------------------------------------------
FROM rust:1.99.1-alpine AS backend-builder
RUN apk add --no-cache musl-dev
WORKDIR /build
COPY Cargo.toml Cargo.lock* rust-toolchain.toml ./
COPY crates crates
COPY --from=frontend-builder /out/dist crates/leaflog-frontend/dist
RUN --mount=type=cache,target=/build/target \
    --mount=type=cache,target=/root/.cargo/git \
    --mount=type=cache,target=/root/.cargo/registry \
    cargo build --release -p leaflog-backend \
    && strip target/release/leaflog-backend \
    && cp target/release/leaflog-backend /leaflog-backend

# ---------------------------------------------------------------------------
# Stage 3: the app image.
# ---------------------------------------------------------------------------
FROM ${HA_BASE} AS app
# The build context is the repository root; the s6-overlay rootfs
# (longrun service) is copied from the app folder.
COPY leaflog/rootfs/ /
COPY --from=backend-builder /leaflog-backend /usr/local/bin/leaflog-backend
