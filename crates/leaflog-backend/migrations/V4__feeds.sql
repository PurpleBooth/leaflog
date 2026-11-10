-- Leaflog schema migration 0004 (M6: feed templates, plan days,
-- substrate profiles).

CREATE TABLE feed_template (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    substrate   TEXT NOT NULL,     -- soil | coco | hydro | user label
    source      TEXT,              -- provenance note
    is_builtin  INTEGER NOT NULL DEFAULT 0,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

CREATE TABLE feed_template_row (
    id           TEXT PRIMARY KEY,
    template_id  TEXT NOT NULL REFERENCES feed_template(id) ON DELETE CASCADE,
    phase        TEXT NOT NULL,    -- FeedPhase label
    day_offset   INTEGER NOT NULL,
    ec_target_us INTEGER NOT NULL,
    ph_min       REAL NOT NULL,
    ph_max       REAL NOT NULL
);

CREATE INDEX idx_template_row_template ON feed_template_row(template_id);

CREATE TABLE feed_plan_day (
    id           TEXT PRIMARY KEY,
    grow_id      TEXT NOT NULL REFERENCES grow(id) ON DELETE CASCADE,
    grow_day     INTEGER NOT NULL,
    phase        TEXT NOT NULL,
    ec_target_us INTEGER NOT NULL,
    ph_min       REAL NOT NULL,
    ph_max       REAL NOT NULL,
    protected    INTEGER NOT NULL DEFAULT 0,
    created_at   TEXT NOT NULL,
    UNIQUE (grow_id, grow_day)
);

CREATE INDEX idx_plan_day_grow ON feed_plan_day(grow_id, grow_day);

CREATE TABLE substrate_profile (
    id              TEXT PRIMARY KEY,
    label           TEXT NOT NULL UNIQUE,
    ph_min          REAL NOT NULL,
    ph_max          REAL NOT NULL,
    watering_style  TEXT NOT NULL,
    flush_days_min  INTEGER NOT NULL,
    flush_days_max  INTEGER NOT NULL,
    runoff_check    TEXT NOT NULL,
    ec_bands        TEXT NOT NULL,   -- JSON EcBands
    source          TEXT NOT NULL,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);
