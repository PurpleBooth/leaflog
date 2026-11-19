-- Feeding schedules: product-dose grids replacing the EC templates
-- (ADR 0004). The bundled generic templates and every stored plan day
-- die here — no EC-to-dose conversion exists; grows keep living
-- without a plan until a schedule is assigned.
--
-- The grow keeps no database-level foreign key to the schedule:
-- SQLite cannot retarget the V11 FK and a full table rebuild would
-- cascade-delete live data under `foreign_keys = ON`. Integrity lives
-- in Rust: assignment validates the schedule; schedule deletion nulls
-- the grow references in the same transaction.

DROP TABLE IF EXISTS feed_plan_day;
DROP TABLE IF EXISTS feed_template_row;
DROP TABLE IF EXISTS feed_template;

ALTER TABLE grow DROP COLUMN feed_template_id;
ALTER TABLE grow ADD COLUMN feed_schedule_id TEXT;

CREATE TABLE feed_schedule (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    source TEXT,
    flower_weeks INTEGER NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE TABLE feed_schedule_product (
    id TEXT PRIMARY KEY,
    schedule_id TEXT NOT NULL REFERENCES feed_schedule(id) ON DELETE CASCADE,
    position INTEGER NOT NULL,
    name TEXT NOT NULL,
    unit TEXT NOT NULL,
    group_label TEXT NOT NULL,
    note TEXT,
    doses TEXT NOT NULL,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
);

CREATE INDEX idx_feed_schedule_product_schedule
    ON feed_schedule_product(schedule_id);

CREATE TABLE feed_plan (
    grow_id TEXT PRIMARY KEY REFERENCES grow(id) ON DELETE CASCADE,
    schedule_id TEXT NOT NULL,
    schedule_name TEXT NOT NULL,
    flower_weeks INTEGER NOT NULL,
    fitted_at TEXT NOT NULL
);

CREATE TABLE feed_plan_day (
    id TEXT PRIMARY KEY,
    grow_id TEXT NOT NULL REFERENCES grow(id) ON DELETE CASCADE,
    grow_day INTEGER NOT NULL,
    phase TEXT NOT NULL,
    week INTEGER,
    doses TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE(grow_id, grow_day)
);

CREATE INDEX idx_feed_plan_day_grow ON feed_plan_day(grow_id);
