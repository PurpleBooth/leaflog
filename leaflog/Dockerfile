# syntax=docker/dockerfile:1
# Leaflog app image. Each architecture builds natively (per-arch CI
# runners, or plain Podman locally); no cross toolchain involved. The
# musl build happens inside the container, so the Devbox environment
# needs no musl targets (spec §7 / docs/DECISIONS.md D007).

ARG TARGETARCH

# ---------------------------------------------------------------------------
# Stage 1: build the frontend (Trunk, relative public URL).
# Debian (glibc), not Alpine: Trunk's [tools] fetch of the Tailwind
# standalone CLI brings a glibc binary that cannot exec on musl
# (relocation errors even under gcompat), and this stage only produces
# dist/ files — no static-musl requirement (D064). Tool triplets differ
# per upstream: trunk ships gnu builds, wasm-bindgen linux builds are
# musl-only — and being static, they run on glibc too. --retry: the
# release downloads truncate now and then behind this network. Every
# archive is checked against a sha256 recorded here before extraction
# (R4 in docs/THREAT-MODEL.md; the upstream checksum files are the
# source, so a version bump means a new hash).
# ---------------------------------------------------------------------------
FROM rust:1.99.1-slim-bookworm AS frontend-builder
ARG TARGETARCH
# Tool versions and sha256 of the per-arch release archives.
ARG TRUNK_VERSION=0.21.14
ARG TRUNK_SHA256_AMD64=f2b4680cd239693a646a2795e4633c625328d7b2a044fbe749fa3a2fe9e7036b
ARG TRUNK_SHA256_AARCH64=b1d8e60e454f7fc182d9a4d95d1506ffbae947d8ba90f8f6f02da93b60f980f9
ARG WASM_BINDGEN_VERSION=0.2.130
ARG WASM_BINDGEN_SHA256_AMD64=9687f3bb551b9f66f37ed32a40d7913a00f7962db653054e56857bc306c3227f
ARG WASM_BINDGEN_SHA256_AARCH64=99ffc93ce80081b91632e2b539b782821a8f3439ab0fddc645287462b2f55d3d
ARG JUST_VERSION=1.40.0
ARG JUST_SHA256_AMD64=181b91d0ceebe8a57723fb648ed2ce1a44d849438ce2e658339df4f8db5f1263
ARG JUST_SHA256_AARCH64=d065d0df1a1f99529869fba8a5b3e0a25c1795b9007099b00dfabe29c7c1f7b6
# nodejs + npm run the ECharts bundle step (D088): build-time only,
# this stage's outputs are dist/ files and the app stage below never
# sees Node. Debian bookworm ships node 18 — esbuild needs >=18, and
# the npm project pins exact echarts/esbuild versions via the
# committed lockfile.
RUN apt-get update && apt-get install -y --no-install-recommends curl ca-certificates nodejs npm \
    && rm -rf /var/lib/apt/lists/* \
    && case "$TARGETARCH" in \
      amd64)   GNU=x86_64-unknown-linux-gnu;  MUSL=x86_64-unknown-linux-musl;  SUFFIX=AMD64 ;; \
      aarch64) GNU=aarch64-unknown-linux-gnu; MUSL=aarch64-unknown-linux-musl; SUFFIX=AARCH64 ;; \
      *) echo "unsupported TARGETARCH: $TARGETARCH" >&2; exit 1 ;; \
    esac \
    && fetch() { \
         tool="$1"; version="$2"; url="$3"; dir="$4"; shift 4; \
         curl -fsSL --retry 3 --retry-all-errors "$url" -o "/tmp/$tool.tar.gz" \
         && echo "$(eval "echo \${${tool}_SHA256_${SUFFIX}}")  /tmp/$tool.tar.gz" | sha256sum -c - \
         && tar -xzf "/tmp/$tool.tar.gz" -C "$dir" "$@" && rm "/tmp/$tool.tar.gz"; \
       } \
    && fetch TRUNK "$TRUNK_VERSION" "https://github.com/trunk-rs/trunk/releases/download/v${TRUNK_VERSION}/trunk-${GNU}.tar.gz" /usr/local/bin \
      && trunk --version \
    && fetch WASM_BINDGEN "$WASM_BINDGEN_VERSION" "https://github.com/rustwasm/wasm-bindgen/releases/download/${WASM_BINDGEN_VERSION}/wasm-bindgen-${WASM_BINDGEN_VERSION}-${MUSL}.tar.gz" /usr/local/bin --strip-components=1 \
      && wasm-bindgen --version \
    && fetch JUST "$JUST_VERSION" "https://github.com/casey/just/releases/download/${JUST_VERSION}/just-${JUST_VERSION}-${MUSL}.tar.gz" /usr/local/bin just \
      && just --version
RUN rustup target add wasm32-unknown-unknown
WORKDIR /build
COPY Cargo.toml Cargo.lock* rust-toolchain.toml justfile migrate.just ./
COPY crates crates
# The daisyUI bundles are gitignored (fetched, never vendored); the
# same pinned+verified recipe that `just dev`/`just ci` run fetches
# them here too. The ECharts bundle builds the same way: npm ci from
# the committed lockfile, then the verified esbuild bundle (D088).
RUN just tailwind-daisyui
RUN just build-echarts
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
    cargo build --profile server -p leaflog-backend \
    && strip target/server/leaflog-backend \
    && cp target/server/leaflog-backend /leaflog-backend

# ---------------------------------------------------------------------------
# Stage 3: the app image — `FROM scratch` (D161).
# The backend is a static musl binary with the frontend embedded
# (embed-assets, D003). It needs none of what a base image ships: no
# shell, no tzdata (the backend works in UTC by design), and no CA
# bundle (reqwest has no TLS feature — the Supervisor API is plain
# HTTP on the internal network). No s6 overlay either: `config.yaml`
# keeps the default `init: true`, so Docker's init process reaps and
# forwards SIGTERM. We build outside the HA builder actions, so the
# io.hass labels are ours to set
# (contributors.home-assistant.io/apps/guides/dockerfile/#labels).
# ---------------------------------------------------------------------------
FROM scratch AS app
ARG TARGETARCH
ARG BUILD_VERSION
LABEL \
  io.hass.version="${BUILD_VERSION}" \
  io.hass.type="app" \
  io.hass.arch="${TARGETARCH}"
COPY --from=backend-builder /leaflog-backend /leaflog-backend
ENTRYPOINT ["/leaflog-backend"]
