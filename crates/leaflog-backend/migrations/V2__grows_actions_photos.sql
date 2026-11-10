-- Leaflog schema migration 0002 (M3: grows, plants, action log, photos).
-- Conventions as in 0001: ULID TEXT ids, RFC 3339 UTC timestamps,
-- JSON-in-TEXT for nested values, snake_case labels of leaflog-core
-- enums validated in Rust.

CREATE TABLE grow (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    strain_id   TEXT REFERENCES strain(id) ON DELETE SET NULL,
    substrate   TEXT NOT NULL,          -- soil | coco | hydro | user label
    started_on  TEXT NOT NULL,          -- YYYY-MM-DD, seed/clone start
    status      TEXT NOT NULL,          -- active | archived
    notes       TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE INDEX idx_grow_status ON grow(status);

CREATE TABLE plant (
    id          TEXT PRIMARY KEY,
    grow_id     TEXT NOT NULL REFERENCES grow(id) ON DELETE CASCADE,
    name        TEXT NOT NULL,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE INDEX idx_plant_grow ON plant(grow_id);

CREATE TABLE action_log (
    id              TEXT PRIMARY KEY,
    grow_id         TEXT NOT NULL REFERENCES grow(id) ON DELETE CASCADE,
    plant_ids       TEXT NOT NULL DEFAULT '[]',   -- JSON array of plant ULIDs
    occurred_at     TEXT NOT NULL,                -- RFC 3339 UTC
    action_type     TEXT NOT NULL,                -- leaflog-core ActionType label
    phase           TEXT,                         -- auto-filled phase label
    grow_day        INTEGER NOT NULL,             -- days since grow start, 0-based
    details         TEXT NOT NULL DEFAULT '{}',   -- JSON ActionDetails
    notes           TEXT,                         -- free text
    appearance_tags TEXT NOT NULL DEFAULT '[]',   -- JSON array
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE INDEX idx_action_grow_time ON action_log(grow_id, occurred_at DESC);
CREATE INDEX idx_action_type ON action_log(action_type);

CREATE TABLE photo (
    id              TEXT PRIMARY KEY,
    grow_id         TEXT NOT NULL REFERENCES grow(id) ON DELETE CASCADE,
    plant_id        TEXT REFERENCES plant(id) ON DELETE SET NULL,
    action_log_id   TEXT REFERENCES action_log(id) ON DELETE SET NULL,
    captured_at     TEXT NOT NULL,                -- EXIF time or upload time
    grow_day        INTEGER NOT NULL,
    phase           TEXT,
    tags            TEXT NOT NULL DEFAULT '[]',   -- JSON array
    original_name   TEXT NOT NULL,                -- file name under photos/originals/<grow>/
    thumbnail_name  TEXT,                         -- file name under photos/thumbnails/<grow>/
    bytes_stored    INTEGER NOT NULL,             -- original size in bytes
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

CREATE INDEX idx_photo_grow ON photo(grow_id, captured_at DESC);
