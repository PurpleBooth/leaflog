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

# Run backend + mock Home Assistant + frontend with simulated ingress.
dev: tailwind-daisyui
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
# Stop `just dev` first: both write {{static_dir}}.
e2e: build-frontend
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
    cargo build -p mock-ha --quiet
    cargo build -p leaflog-backend --no-default-features --quiet
    work="$(mktemp -d)"
    echo "── Leaflog e2e ────────────────────────────────────────────"
    echo "  app: http://127.0.0.1:{{e2e_port}}{{e2e_prefix}}/"
    echo "  db:  ${work} (throwaway)"
    echo "────────────────────────────────────────────────────────────"
    MOCK_HA_PORT={{e2e_mock_port}} ./target/debug/mock-ha &
    mock_pid=$!
    # The dev default supervisor URL is just dev's mock port (8098);
    # the e2e mock listens on its own port, so point the backend at it
    # explicitly or its WS loop dials a dead socket forever.
    LEAFLOG_PORT={{e2e_port}} \
    LEAFLOG_ENV=dev \
    LEAFLOG_DEV_PREFIX={{e2e_prefix}} \
    LEAFLOG_STATIC_DIR={{static_dir}} \
    LEAFLOG_DB_PATH="$work/leaflog.db" \
    LEAFLOG_PHOTOS_DIR="$work/photos" \
    LEAFLOG_SUPERVISOR_URL="http://127.0.0.1:{{e2e_mock_port}}" \
    ./target/debug/leaflog-backend &
    backend_pid=$!
    cleanup() {
        kill "$mock_pid" "$backend_pid" 2>/dev/null || true
        wait "$mock_pid" "$backend_pid" 2>/dev/null || true
        rm -rf "$work"
    }
    trap cleanup EXIT INT TERM
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
    # The health probe can pass against a stale stack squatting on the
    # port while our own backend died on bind — catch that here rather
    # than as confusing test failures on someone else's database.
    kill -0 "$backend_pid" 2>/dev/null || {
        echo "e2e backend died — port {{e2e_port}} already in use (stale stack? stop just dev / old e2e runs)" >&2
        exit 1
    }
    cd e2e && npx playwright test

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

# Prose gate (vale; skip if not installed): markdown, Rust comments
# and UI strings must not contain LLM-sounding vocabulary or
# wall-of-text sentences (D065). Changelogs and the licence are not
# prose and are not linted.
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
    # UI strings: prose-looking string literals from the frontend.
    # Keep literals with at least three plain alphabetic words (real
    # labels and sentences); drop Leptos attribute fragments
    # (" aria-label="), format-spec/separator-only strings, {…}
    # placeholders, and Tailwind utility-class lists (their hyphen/
    # slash/colon tokens are not prose words). Each string is closed
    # with a period and separated by a blank line so the sentence cap
    # judges each label on its own.
    while IFS= read -r f; do
        out="$tmp/ui/${f}.txt"
        mkdir -p "$(dirname "$out")"
        grep -hoE '"[^"]*[[:space:]][^"]*"' "$f" \
            | sed -E 's/^"//; s/"$//; s/[{][^}]*[}]/#/g' \
            | grep -vE '^[[:space:]]*[a-z0-9-]+=$' \
            | awk '{
                words = 0
                for (i = 1; i <= NF; i++)
                    if ($i ~ /^[A-Za-z][A-Za-z]+$/ && length($i) >= 2) words++
                if (words >= 3) {
                    if ($0 !~ /[.!?:]$/) print $0 "."
                    else print
                    print ""
                }
              }' \
            > "$out" || true
    done < <(find crates/leaflog-frontend/src -type f -name '*.rs' | sort)
    vale "$tmp/ui" || status=1
    exit "$status"

# Everything CI checks: fmt, clippy (native + wasm), tests, WASM build, commits, secrets, deps, prose, e2e.
ci: cog-check fmt clippy clippy-wasm build-frontend test gitleaks deny lint-prose e2e

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
build-frontend: tailwind-daisyui
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
