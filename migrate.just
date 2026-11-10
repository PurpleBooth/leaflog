# Migration tooling (spec §3, D048). Files in
# crates/leaflog-backend/migrations are discovered at compile time by
# refinery — creating the next numbered file is the whole workflow.
# Recipes run from the repository root (the module file's directory).

dir := "crates/leaflog-backend/migrations"

# Create the next migration file: `just migrate new add_seedbank`.
new name:
    #!/usr/bin/env bash
    set -euo pipefail
    last="$(ls {{dir}} | sed -n 's/^V\([0-9]\+\)__.*/\1/p' | sort -n | tail -1)"
    [ -n "$last" ] || { echo "no existing migrations found in {{dir}}" >&2; exit 1; }
    next=$(( last + 1 ))
    slug="$(echo "{{name}}" | tr '[:upper:]' '[:lower:]' | sed -E 's/[^a-z0-9]+/_/g; s/^_+//; s/_+$//')"
    [ -n "$slug" ] || { echo "migration name must contain letters or digits" >&2; exit 1; }
    file="{{dir}}/V${next}__${slug}.sql"
    [ ! -e "$file" ] || { echo "${file} already exists" >&2; exit 1; }
    cat > "$file" <<EOF
    -- Leaflog schema migration V${next} (${slug}).
    -- Conventions: ULID TEXT ids, RFC 3339 UTC timestamps, snake_case
    -- enum labels validated in Rust (see V1__init.sql). Each file applies
    -- in its own transaction, and applied files are checksum-verified at
    -- startup: never edit a shipped file, add a new one instead.
    EOF
    # refinery embeds the directory at compile time; poke the crate root
    # so the next build picks the new file up even if cargo's change
    # detection misses it.
    touch crates/leaflog-backend/src/db.rs
    echo "created ${file}; run 'just migrate status' to see it pending"

# Show applied and pending schema migrations.
status:
    cargo run -p leaflog-backend --quiet --no-default-features -- migrate status

# Apply pending migrations (boot applies them on every start anyway;
# this is for standalone runs).
apply:
    cargo run -p leaflog-backend --quiet --no-default-features -- migrate apply

# Dev-only: wipe the development database and rebuild it from every
# migration file. Destroys data/leaflog.db by design.
redo:
    #!/usr/bin/env bash
    set -euo pipefail
    rm -f data/leaflog.db data/leaflog.db-wal data/leaflog.db-shm
    just migrate apply
