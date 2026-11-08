-- Leaflog schema migration 0001 (M2: equipment, strains, settings).
-- ULIDs as TEXT primary keys, UTC timestamps as RFC 3339 TEXT, SI units
-- for physical quantities (watts are SI already). Enum-ish columns hold
-- the snake_case labels of leaflog-core types; validation happens in
-- Rust (single source of truth), not in CHECK constraints.

CREATE TABLE strain (
    id                   TEXT PRIMARY KEY,
    name                 TEXT NOT NULL,
    breeder              TEXT,
    photoperiod          TEXT NOT NULL,          -- photoperiod | auto
    sex                  TEXT NOT NULL,          -- feminized | regular
    indica_pct           INTEGER,                -- 0..100, NULL = unknown
    landrace_tags        TEXT NOT NULL DEFAULT '[]', -- JSON array of strings
    flower_days_min      INTEGER,
    flower_days_max      INTEGER,
    stretch_factor       REAL,                   -- e.g. 1.5 = +50% height
    veg_weeks_min        INTEGER,
    veg_weeks_max        INTEGER,
    height_class         TEXT,                   -- short | medium | tall
    auto_total_days_min  INTEGER,
    auto_total_days_max  INTEGER,
    yield_notes          TEXT,
    terpene_notes        TEXT,
    effect_notes         TEXT,
    heat_tolerance       TEXT NOT NULL,          -- sensitive | average | tolerant
    cold_tolerance       TEXT NOT NULL,
    light_tolerance      TEXT NOT NULL,
    user_rating          INTEGER,                -- 0..5
    notes                TEXT,
    evidence             TEXT NOT NULL DEFAULT '{}', -- JSON map field -> evidence tag
    source_url           TEXT,
    source_licence       TEXT,
    is_phenotype_of      TEXT,                   -- parent strain ULID
    created_at           TEXT NOT NULL,
    updated_at           TEXT NOT NULL
);

CREATE INDEX idx_strain_name ON strain(name);

CREATE TABLE equipment_item (
    id             TEXT PRIMARY KEY,
    name           TEXT NOT NULL,
    category       TEXT NOT NULL,   -- leaflog-core EquipmentCategory label
    brand_model    TEXT,
    real_watts     REAL,            -- measured wall watts, not marketing
    dimmable       INTEGER NOT NULL DEFAULT 0,
    purchase_date  TEXT,            -- YYYY-MM-DD
    purchase_price REAL,
    currency       TEXT,
    notes          TEXT,
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL
);

CREATE INDEX idx_equipment_category ON equipment_item(category);

CREATE TABLE settings (
    key        TEXT PRIMARY KEY,
    value      TEXT NOT NULL,       -- JSON-encoded value
    updated_at TEXT NOT NULL
);
