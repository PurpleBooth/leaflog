# Leaflog automation. Everything CI and development runs goes through
# these recipes (no bash-only scripts, per the spec).

port := "8099"
mock_port := "8098"
frontend_dir := "crates/leaflog-frontend"
static_dir := frontend_dir / "dist"

# Run backend + mock Home Assistant + frontend with simulated ingress.
dev:
    #!/usr/bin/env bash
    set -euo pipefail
    prefix="/api/hassio_ingress/dev-${RANDOM}${RANDOM}"
    echo "── Leaflog dev ────────────────────────────────────────────"
    echo "  Ingress URL (looks like real HA ingress):"
    echo "      http://127.0.0.1:{{port}}${prefix}/"
    echo "  Mock Supervisor states:"
    echo "      http://127.0.0.1:{{mock_port}}/core/api/states"
    echo "  Ctrl-C stops everything."
    echo "────────────────────────────────────────────────────────────"
    (cd {{frontend_dir}} && trunk watch) &
    trunk_pid=$!
    cleanup() { kill "$trunk_pid" 2>/dev/null || true; }
    trap cleanup EXIT INT TERM
    MOCK_HA_PORT={{mock_port}} cargo run -p mock-ha --quiet &
    mock_pid=$!
    cleanup() { kill "$trunk_pid" 2>/dev/null || true; kill "$mock_pid" 2>/dev/null || true; }
    trap cleanup EXIT INT TERM
    LEAFLOG_PORT={{port}} \
    LEAFLOG_ENV=dev \
    LEAFLOG_DEV_PREFIX="$prefix" \
    LEAFLOG_STATIC_DIR={{static_dir}} \
    cargo run -p leaflog-backend --quiet --no-default-features

# Everything CI checks: fmt, clippy (native + wasm), tests, WASM build.
ci: fmt clippy clippy-wasm build-frontend test

# Check formatting.
fmt:
    cargo fmt --all --check

# Clippy for native targets with the strict lint policy.
clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# Clippy for the frontend crate on wasm32.
clippy-wasm:
    cargo clippy -p leaflog-frontend --target wasm32-unknown-unknown -- -D warnings

# Run all tests.
test:
    cargo test --workspace

# Build the frontend bundle (relative public URL, never a baked base).
build-frontend:
    cd {{frontend_dir}} && trunk build --release

# Build the HA app image locally with Podman (amd64 by default).
arch := "amd64"
image arch=arch:
    #!/usr/bin/env bash
    set -euo pipefail
    cd {{justfile_directory()}}
    if [ ! -f "{{static_dir}}/index.html" ]; then
        just build-frontend
    fi
    podman build \
        --build-arg TARGETARCH={{arch}} \
        -f leaflog/Dockerfile \
        -t "ghcr.io/purplebooth/leaflog-{{arch}}:dev" \
        .

# Print the recipes (default target).
default:
    @just --list
