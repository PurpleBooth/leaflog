# Leaflog automation. Everything CI and development runs goes through
# these recipes (no bash-only scripts, per the spec).

port := "8099"
mock_port := "8098"
frontend_dir := "crates/leaflog-frontend"
static_dir := frontend_dir / "dist"

# E2E stack (justfile `e2e`): own ports so `just dev` can keep running
# alongside, and a fixed simulated ingress prefix so test URLs stay
# deterministic. Everything else (fresh DB, photos) is throwaway.
e2e_port := "8199"
e2e_mock_port := "8198"
e2e_prefix := "/api/hassio_ingress/e2e"

# Migration tooling lives in migrate.just: `just migrate new <name>`,
# `just migrate status`, `just migrate apply`, dev-only `just migrate redo`.
mod migrate

# Fetch the daisyUI plugin bundles for the Tailwind standalone CLI
# (build-time only, never in the image; no Node anywhere). Not
# vendored (working agreement): pinned release + sha256 gate, same
# pattern as Trunk's own tool downloads. Output dir is gitignored.
daisyui_version := "5.7.50"
daisyui_dir := frontend_dir / "tailwind"
daisyui_sha_daisyui := "d69de07c7463bb1f41548e2688f88ee105cdadf9779fce5b3c47b3bfaa6be472"
daisyui_sha_theme := "e48a35d8326d650181ca600560e8520cf6b44cf9242bbd9d45825607e641f251"

# The ECharts npm project and its built bundle (npm + esbuild,
# tree-shaken to the SVG renderer; D088). Build-time only — Node
# never reaches the image.
echarts_dir := frontend_dir / "charts"

# Download and verify the daisyUI bundles into tailwind/ (idempotent).
tailwind-daisyui:
    #!/usr/bin/env bash
    set -euo pipefail
    mkdir -p {{daisyui_dir}}
    base="https://github.com/saadeghi/daisyui/releases/download/v{{daisyui_version}}"
    curl -sL -o "{{daisyui_dir}}/.daisyui.mjs.tmp" "$base/daisyui.mjs"
    curl -sL -o "{{daisyui_dir}}/.daisyui-theme.mjs.tmp" "$base/daisyui-theme.mjs"
    echo "{{daisyui_sha_daisyui}}  {{daisyui_dir}}/.daisyui.mjs.tmp" | sha256sum -c - >/dev/null
    echo "{{daisyui_sha_theme}}  {{daisyui_dir}}/.daisyui-theme.mjs.tmp" | sha256sum -c - >/dev/null
    mv "{{daisyui_dir}}/.daisyui.mjs.tmp" "{{daisyui_dir}}/daisyui.mjs"
    mv "{{daisyui_dir}}/.daisyui-theme.mjs.tmp" "{{daisyui_dir}}/daisyui-theme.mjs"
    echo "daisyUI v{{daisyui_version}} bundles ready (sha256 verified)"

# Bundle the tree-shaken ECharts build (SVG renderer, D088) and verify
# it: composition, local-only loading, index.html wiring. npm ci from
# the committed lockfile keeps the build reproducible.
build-echarts:
    #!/usr/bin/env bash
    set -euo pipefail
    command -v npm >/dev/null 2>&1 || {
        echo "npm not found; run inside devbox: devbox run -- just build-echarts" >&2
        exit 1
    }
    (cd {{echarts_dir}} && npm ci && node build.mjs && node verify.mjs)

# Run backend + mock Home Assistant + frontend with simulated ingress.
dev: tailwind-daisyui build-echarts
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

# Install the e2e suite's node dependencies and Playwright's chromium
# (test-time tooling only; Node never enters the shipped image, D069).
e2e-install:
    #!/usr/bin/env bash
    set -euo pipefail
    command -v npm >/dev/null 2>&1 || {
        echo "npm not found; run inside devbox: devbox run -- just e2e-install" >&2
        exit 1
    }
    (cd e2e && npm ci && npx playwright install chromium)

# Frontend E2E: built dist/ served by the backend against the mock
# Supervisor, under a fixed ingress prefix with a throwaway database,
# then the Playwright suite (e2e/README.md). Skips when node or the
# deps are absent, like gitleaks/deny — CI runs a dedicated e2e job.
# The whole stack is throwaway and port-parametric, so several stacks
# run concurrently (CI agents, parallel agents):
#
#   just e2e_port=8301 e2e_mock_port=8300 e2e
#
# The frontend bundle builds into the run's own temp dist/, never the
# shared {{static_dir}} — a concurrent `just dev` or second stack
# cannot race it.
e2e: tailwind-daisyui build-echarts
    #!/usr/bin/env bash
    set -euo pipefail
    if ! command -v npm >/dev/null 2>&1; then
        echo "npm not found; skipping e2e (devbox run -- just e2e)"
        exit 0
    fi
    if [ ! -d e2e/node_modules ]; then
        echo "e2e deps missing; run 'just e2e-install' first — skipping e2e"
        exit 0
    fi
    # Pre-flight (D085): both ports must be free before any build time
    # is spent. A listener here is a stale stack (just dev, an old e2e
    # run) that the run would only collide with after the builds.
    for probe_port in {{e2e_port}} {{e2e_mock_port}}; do
        if (exec 3<>"/dev/tcp/127.0.0.1/$probe_port") 2>/dev/null; then
            echo "e2e port $probe_port is already in use — stop the stale stack (just dev / an old e2e run) or pick free ports: just e2e_port=<port> e2e_mock_port=<port> e2e" >&2
            exit 1
        fi
    done
    cargo build -p mock-ha --quiet
    cargo build -p leaflog-backend --no-default-features --quiet
    work="$(mktemp -d)"
    mock_pid=""
    backend_pid=""
    cleanup() {
        kill "$mock_pid" "$backend_pid" 2>/dev/null || true
        wait "$mock_pid" "$backend_pid" 2>/dev/null || true
        rm -rf "$work"
    }
    trap cleanup EXIT INT TERM
    echo "── Leaflog e2e ────────────────────────────────────────────"
    echo "  app: http://127.0.0.1:{{e2e_port}}{{e2e_prefix}}/"
    echo "  db:  ${work} (throwaway)"
    echo "────────────────────────────────────────────────────────────"
    (cd {{frontend_dir}} && trunk build --release --dist "$work/dist")
    (cd {{frontend_dir}} && node charts/verify.mjs "$work/dist")
    # The e2e mock reports no language (MOCK_HA_LANGUAGE="" means
    # "none" in the mock's config response), so the locale chain (D112)
    # falls through to the browser locale — the step the German spec
    # forces and the default-English spec pins.
    MOCK_HA_PORT={{e2e_mock_port}} MOCK_HA_LANGUAGE="" ./target/debug/mock-ha &
    mock_pid=$!
    # The dev default supervisor URL is just dev's mock port (8098);
    # the e2e mock listens on its own port, so point the backend at it
    # explicitly or its WS loop dials a dead socket forever.
    LEAFLOG_PORT={{e2e_port}} \
    LEAFLOG_ENV=dev \
    LEAFLOG_DEV_PREFIX={{e2e_prefix}} \
    LEAFLOG_STATIC_DIR="$work/dist" \
    LEAFLOG_DB_PATH="$work/leaflog.db" \
    LEAFLOG_PHOTOS_DIR="$work/photos" \
    LEAFLOG_SUPERVISOR_URL="http://127.0.0.1:{{e2e_mock_port}}" \
    ./target/debug/leaflog-backend &
    backend_pid=$!
    # Wait for liveness before the suite: a poll on /api/health, plus
    # the bind check — the probe can pass against a stale stack
    # squatting on the port while our own backend died on bind; catch
    # that here rather than as confusing test failures on someone
    # else's database.
    LEAFLOG_E2E_HEALTH_URL="http://127.0.0.1:{{e2e_port}}{{e2e_prefix}}/api/health" \
    node -e '
        const url = process.env.LEAFLOG_E2E_HEALTH_URL;
        const deadline = Date.now() + 60000;
        (async () => {
            while (Date.now() < deadline) {
                try {
                    if ((await fetch(url)).ok) process.exit(0);
                } catch {}
                await new Promise((resolve) => setTimeout(resolve, 500));
            }
            process.exit(1);
        })();
    ' || { echo "e2e stack did not become healthy" >&2; exit 1; }
    kill -0 "$backend_pid" 2>/dev/null || {
        echo "e2e backend died — port {{e2e_port}} already in use (stale stack? stop just dev / old e2e runs)" >&2
        exit 1
    }
    cd e2e && LEAFLOG_E2E_PORT={{e2e_port}} npx playwright test

# Allow-register guard (AGENTS.md lint policy): every clippy allow in
# the sources must be logged in docs/DECISIONS.md. Group names from the
# mandated lint block (nursery, pedantic, …) are policy, not exceptions.
# Catches an unregistered exception at CI speed instead of at review.
allow-register:
    #!/usr/bin/env bash
    set -euo pipefail
    status=0
    while IFS= read -r lint; do
        if ! grep -q "$lint" docs/DECISIONS.md; then
            echo "unregistered allow: $lint" >&2
            grep -rn "$lint" crates --include='*.rs' | sed 's/^/  /' >&2
            status=1
        fi
    done < <(grep -rhoE 'clippy::[a-z_]+' crates --include='*.rs' \
        | grep -vE 'clippy::(nursery|cargo|complexity|correctness|perf|style|suspicious|pedantic)' \
        | sort -u)
    exit "$status"

# Decision-id guard (D083): every `### D<3digits>:` heading in the
# decision log must be unique. Parallel tickets numbered from
# different bases claimed the same number twice (D012 and D013 each
# headed two decisions), so the collision becomes a gate failure that
# names the offenders instead of a review catch.
decision-ids:
    #!/usr/bin/env bash
    set -euo pipefail
    status=0
    while IFS= read -r id; do
        echo "duplicate decision id: $id" >&2
        grep -n "^${id%%:*}" docs/DECISIONS.md | sed 's/^/  /' >&2
        status=1
    done < <(grep -oE '^### D[0-9]{3}:' docs/DECISIONS.md | sort | uniq -d)
    exit "$status"

# Check conventional commits (cocogitto; skip if not installed).
# Merge commits are skipped: git's default "Merge branch '…'" message
# is not conventional and carries no changelog content (D068).
cog-check:
    #!/usr/bin/env bash
    if command -v cog &>/dev/null; then
        cog check --ignore-merge-commits
    else
        echo "cocogitto not installed; skipping conventional commit check"
    fi

# Regenerate both changelogs from conventional commits (cocogitto; D042,
# D062). Both files are generated output — never edit them by hand. They
# range over the released tags only (root..latest tag), so an Unreleased
# section never lands in a file; the release workflow regenerates and
# commits the same bytes. Uses cocogitto's own header/footer constants.
#
#   CHANGELOG.md         — cocogitto output verbatim (GitHub-facing).
#   leaflog/CHANGELOG.md — the file Home Assistant serves. Supervisor
#     reads it from the app directory of its default-branch clone, and
#     HA Core slices release notes on headings that must be exactly
#     "## <version>" (the version from leaflog/config.yaml), so the
#     cocogitto "## vX.Y.Z - YYYY-MM-DD" headings become "## X.Y.Z"
#     plus a "_Released ..._" line.
changelog:
    #!/usr/bin/env bash
    set -euo pipefail
    command -v cog >/dev/null 2>&1 || {
        echo "cocogitto not installed; run inside devbox: devbox run -- just changelog" >&2
        exit 1
    }
    latest_tag="$(git describe --tags --abbrev=0)"
    root_commit="$(git rev-list --max-parents=0 HEAD | tail -1)"
    body="$(mktemp)"
    trap 'rm -f "$body"' EXIT
    cog changelog "${root_commit}..${latest_tag}" > "$body"
    {
        printf '# Changelog\nAll notable changes to this project will be documented in this file. See [conventional commits](https://www.conventionalcommits.org/) for commit guidelines.\n\n- - -\n'
        cat "$body"
        printf '\n- - -\n\nChangelog generated by [cocogitto](https://github.com/cocogitto/cocogitto).'
    } > CHANGELOG.md
    {
        printf '# Changelog\nAll notable changes to the Leaflog Home Assistant app. Version headings are exactly `## <version>` — Home Assistant slices release notes on them. Generated file; never edit by hand.\n\n- - -\n'
        sed -E 's/^## v([0-9]+\.[0-9]+\.[0-9]+) - ([0-9]{4}-[0-9]{2}-[0-9]{2})$/## \1\n\n_Released \2._\n/' "$body"
        printf '\n- - -\n\nChangelog generated by [cocogitto](https://github.com/cocogitto/cocogitto).'
    } > leaflog/CHANGELOG.md
    echo "CHANGELOG.md and leaflog/CHANGELOG.md regenerated for ${latest_tag}"

# Scan full git history for committed secrets (gitleaks; skip if not installed).
gitleaks:
    #!/usr/bin/env bash
    if command -v gitleaks &>/dev/null; then
        gitleaks git --verbose --redact .
    else
        echo "gitleaks not installed; skipping secret scan"
    fi

# Dependency hygiene: RustSec advisories + licence allowlist (cargo-deny;
# skip if not installed). Config lives in deny.toml (D046).
deny:
    #!/usr/bin/env bash
    if command -v cargo-deny &>/dev/null; then
        cargo deny --workspace check --hide-inclusion-graph
    else
        echo "cargo-deny not installed; run inside devbox: devbox run -- just deny"
    fi

# Prose gate (vale; skip if not installed): markdown and Rust
# comments must not contain LLM-sounding vocabulary or wall-of-text
# sentences (D065). The former UI-string extractor left the gate in
# D117 — locale text lives in the catalogs (D112). Changelogs and
# the licence are not prose and are not linted.
lint-prose:
    #!/usr/bin/env bash
    set -euo pipefail
    if ! command -v vale &>/dev/null; then
        echo "vale not installed; run inside devbox: devbox run -- just lint-prose"
        exit 0
    fi
    status=0
    vale README.md AGENTS.md docs leaflog/README.md leaflog/DOCS.md || status=1
    tmp="$(mktemp -d)"
    trap 'rm -rf "$tmp"' EXIT
    # Rust comments: strip //, ///, //! and join consecutive comment
    # lines into paragraphs so the sentence cap sees sentences wrapped
    # across lines. Rustdoc headings (# Panics) and list items start
    # their own paragraph, mirroring how rustdoc renders them; each
    # paragraph is closed with a period (comment fragments rarely end
    # with one) and separated by a blank line — vale's text format
    # runs sentences across paragraphs until it meets real
    # punctuation. One .txt per source file keeps vale's output
    # addressing real file names.
    while IFS= read -r f; do
        out="$tmp/comments/${f}.txt"
        mkdir -p "$(dirname "$out")"
        awk '
            function flush() {
                if (buf != "") {
                    if (buf !~ /[.!?:]$/) buf = buf "."
                    print buf; print ""; buf = ""
                }
            }
            /^[[:space:]]*\/\/[!\/]?/ {
                line = $0
                sub(/^[[:space:]]*\/\/[!\/]?[[:space:]]?/, "", line)
                if (line ~ /^[[:space:]]*(#|[[:space:]]*-)[[:space:]]/) {
                    flush()
                    print line "."; print ""   # heading/bullet: own paragraph
                    next
                }
                buf = buf (buf == "" ? "" : " ") line
                next
            }
            { flush() }
            END { flush() }
        ' "$f" > "$out"
    done < <(find crates -type f -name '*.rs' | sort)
    vale "$tmp/comments" || status=1
    exit "$status"

# Everything CI checks: fmt, clippy (native + wasm), tests, WASM
# build, commits, allow register, decision ids, secrets, deps, prose,
# e2e. Self-serializes on a host lock (D084): two full gates at once
# OOM-killed the 14 Gi dev host, so a second concurrent gate waits.
# CI's separate jobs call `just e2e` / `just lint-prose` directly and
# are unaffected; a host without flock runs unlocked.
ci:
    #!/usr/bin/env bash
    set -euo pipefail
    if command -v flock >/dev/null 2>&1; then
        mkdir -p /tmp/leaflog-fs
        exec flock /tmp/leaflog-fs/ci.lock just _ci-impl
    fi
    exec just _ci-impl

# The gate's implementation; run it through `ci` so the lock holds.
_ci-impl: cog-check allow-register decision-ids fmt clippy clippy-wasm build-frontend test test-wasm gitleaks deny lint-prose e2e

# Check formatting.
fmt:
    cargo fmt --all --check

# Clippy for native targets with the strict lint policy.
clippy:
    cargo clippy --workspace --all-targets -- -D warnings

# Clippy for the frontend crate on wasm32.
clippy-wasm:
    cargo clippy -p leaflog-frontend --target wasm32-unknown-unknown --all-targets -- -D warnings

# Run all tests.
test:
    cargo test --workspace

# Component-test tooling (D139): pinned wasm-bindgen CLI + Chrome for
# Testing with the matching ChromeDriver, downloaded on first use into
# the gitignored wasm-test-tools/ with the same sha256-gate pattern as
# the daisyUI bundles. Not vendored (working agreement): pinned
# releases, throwaway once extracted.
wasm_bindgen_version := "0.2.130"
# Upstream ships aarch64 for glibc but x86_64 only for musl (0.2.130
# has no x86_64-unknown-linux-gnu asset; the release page 404s).
wasm_bindgen_sha_aarch64 := "cc8dd8e90aa9111758168d30c63a14b7f0276997d9d60d3e160a1fb5e7d3b919"
wasm_bindgen_sha_x86_64 := "9687f3bb551b9f66f37ed32a40d7913a00f7962db653054e56857bc306c3227f"
cft_version := "158.0.8127.36"
cft_sha_chrome_aarch64 := "f811b64a50cbc73f94fda7b3f3b3bf62be5d9409b3207e6ead9b61e6706afffe"
cft_sha_chrome_x86_64 := "dba9cc838e1ce269c33ea68170881127bd15d078fc3ca5891c90802819a4d7c7"
cft_sha_driver_aarch64 := "cbb268390b87b79ad8543364bbf5cc851b1403a2ce76c2636dfc2fbe8e196b7d"
cft_sha_driver_x86_64 := "d20e70d69d68c5c983e2526452ed0779fd65dc4ade5d6741f1bb3d0d82754d3a"
wasm_tools_dir := "wasm-test-tools"

# Download and unpack the wasm-bindgen CLI + Chrome for Testing
# (idempotent). Exits non-zero on any mismatch; the tag has no `v`
# prefix (the `v`-prefixed URL 404s).
wasm-tools:
    #!/usr/bin/env bash
    set -euo pipefail
    case "$(uname -m)" in
        aarch64) arch="aarch64"; libc="gnu"; cft_arch="linux-arm64" ;;
        x86_64)  arch="x86_64";  libc="musl"; cft_arch="linux64" ;;
        *) echo "unsupported architecture: $(uname -m)" >&2; exit 1 ;;
    esac
    # Fresh checkouts have no tools dir at all; the symlinks below
    # need `bin` to exist even when every download is skipped.
    mkdir -p "{{wasm_tools_dir}}/bin"
    wb_dir="{{wasm_tools_dir}}/wasm-bindgen-{{wasm_bindgen_version}}-${arch}-unknown-linux-${libc}"
    if [ ! -x "$wb_dir/wasm-bindgen-test-runner" ]; then
        case "$arch" in
            aarch64) wb_sha="{{wasm_bindgen_sha_aarch64}}" ;;
            x86_64)  wb_sha="{{wasm_bindgen_sha_x86_64}}" ;;
        esac
        mkdir -p "{{wasm_tools_dir}}"
        tgz="{{wasm_tools_dir}}/.wb.tar.gz"
        # `-f` fails on an HTTP error instead of saving the 404 body;
        # without it a missing asset hashes "Not Found" and passes.
        curl -sfL -o "$tgz" "https://github.com/rustwasm/wasm-bindgen/releases/download/{{wasm_bindgen_version}}/wasm-bindgen-{{wasm_bindgen_version}}-${arch}-unknown-linux-${libc}.tar.gz"
        echo "$wb_sha  $tgz" | sha256sum -c -
        rm -rf "$wb_dir"
        tar -xzf "$tgz" -C "{{wasm_tools_dir}}"
        rm -f "$tgz"
    fi
    for tool in chrome chromedriver; do
        out_dir="{{wasm_tools_dir}}/dl/${tool}-${cft_arch}"
        if [ ! -x "$out_dir/$tool" ]; then
            case "$arch" in
                aarch64) case "$tool" in
                    chrome)       tool_sha="{{cft_sha_chrome_aarch64}}" ;;
                    chromedriver) tool_sha="{{cft_sha_driver_aarch64}}" ;;
                esac ;;
                x86_64) case "$tool" in
                    chrome)       tool_sha="{{cft_sha_chrome_x86_64}}" ;;
                    chromedriver) tool_sha="{{cft_sha_driver_x86_64}}" ;;
                esac ;;
            esac
            mkdir -p "{{wasm_tools_dir}}/dl/.${tool}.tmp"
            zip_name="${tool}-${cft_arch}.zip"
            zip_path="{{wasm_tools_dir}}/dl/.${tool}.tmp/${zip_name}"
            curl -sfL -o "$zip_path" "https://storage.googleapis.com/chrome-for-testing-public/{{cft_version}}/${cft_arch}/${zip_name}"
            echo "$tool_sha  $zip_path" | sha256sum -c -
            unzip -oq "$zip_path" -d "{{wasm_tools_dir}}/dl/.${tool}.tmp"
            rm -rf "$out_dir"
            mv "{{wasm_tools_dir}}/dl/.${tool}.tmp/${tool}-${cft_arch}" "$out_dir"
            rm -rf "{{wasm_tools_dir}}/dl/.${tool}.tmp"
        fi
        ln -sfn "../dl/${tool}-${cft_arch}/$tool" "{{wasm_tools_dir}}/bin/$tool"
    done
    ln -sfn "../dl/chrome-${cft_arch}/chrome" "{{wasm_tools_dir}}/bin/google-chrome"
    # chromedriver searches its own directory before PATH and before the
    # hardcoded /opt/google/chrome/chrome that GitHub's runner image
    # ships (a different version than our pin, so the session dies with
    # "only supports Chrome version N"). A link beside the driver wins
    # that search.
    ln -sfn "../chrome-${cft_arch}/chrome" "{{wasm_tools_dir}}/dl/chromedriver-${cft_arch}/chrome"

# Browser-run component tests (D139): compile the frontend for
# wasm32 and run src/component_tests in headless Chrome via the
# wasm-bindgen test runner. Skips with a notice when the pinned
# tools cannot be fetched; CI runs the recipe unconditionally so
# the gate is enforced there.
test-wasm: wasm-tools
    #!/usr/bin/env bash
    set -euo pipefail
    runner=$(find "{{wasm_tools_dir}}" -name wasm-bindgen-test-runner -type f | head -1)
    export PATH="$PWD/{{wasm_tools_dir}}/bin:$PATH"
    export CARGO_TARGET_WASM32_UNKNOWN_UNKNOWN_RUNNER="$PWD/$runner"
    cargo test -p leaflog-frontend --target wasm32-unknown-unknown

# Build the frontend bundle (relative public URL, never a baked base).
build-frontend: tailwind-daisyui build-echarts
    cd {{frontend_dir}} && trunk build --release && node charts/verify.mjs dist

# Build the HA app image locally with Podman (native arch by
# default; the Dockerfile only knows amd64 and aarch64).
host_arch := `uname -m`
arch := if host_arch == "aarch64" { "aarch64" } else if host_arch == "x86_64" { "amd64" } else { error("unsupported architecture for a local image build: " + host_arch) }
image arch=arch:
    #!/usr/bin/env bash
    set -euo pipefail
    cd {{justfile_directory()}}
    if [ ! -f "{{static_dir}}/index.html" ]; then
        just build-frontend
    fi
    podman build \
        --build-arg TARGETARCH={{arch}} \
        --build-arg BUILD_VERSION=dev \
        -f leaflog/Dockerfile \
        -t "ghcr.io/purplebooth/leaflog-{{arch}}:dev" \
        .

# Print the recipes (default target).
default:
    @just --list
