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
# Trunk is a static musl release binary; pick the one for this arch.
# ---------------------------------------------------------------------------
FROM rust:1.99.1-alpine AS frontend-builder
ARG TARGETARCH
# curl (not busybox wget): GitHub release downloads redirect in ways
# busybox wget mishandles.
RUN apk add --no-cache curl \
    && case "$TARGETARCH" in \
      amd64)   TRIPLET=x86_64-unknown-linux-musl  ;; \
      aarch64) TRIPLET=aarch64-unknown-linux-musl ;; \
      *) echo "unsupported TARGETARCH: $TARGETARCH" >&2; exit 1 ;; \
    esac \
    && curl -fsSL "https://github.com/trunk-rs/trunk/releases/download/v0.21.14/trunk-${TRIPLET}.tar.gz" \
      | tar -xz -C /usr/local/bin && trunk --version
RUN rustup target add wasm32-unknown-unknown
WORKDIR /build
COPY Cargo.toml Cargo.lock* rust-toolchain.toml ./
COPY crates crates
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
