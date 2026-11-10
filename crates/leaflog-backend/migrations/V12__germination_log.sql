-- Leaflog schema migration V12 (germination-test log, spec §4.13 /
-- D056).
--
-- One row per germination test of a seed batch: date, number sown and
-- number sprouted (paper-towel or direct). The observed rate is user
-- evidence shown beside the §5.4 preset window — it never overwrites
-- the preset (spec §4.13).
--
-- Conventions as in V1: ULID TEXT ids, RFC 3339 UTC timestamps,
-- YYYY-MM-DD dates; sprouted <= sown is validated in Rust.

CREATE TABLE germination_log (
    id             TEXT PRIMARY KEY,
    seed_batch_id  TEXT NOT NULL REFERENCES seed_batch(id) ON DELETE CASCADE,
    tested_on      TEXT NOT NULL,             -- YYYY-MM-DD
    sown           INTEGER NOT NULL,
    sprouted       INTEGER NOT NULL,
    notes          TEXT,
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL
);

CREATE INDEX idx_germination_batch ON germination_log(seed_batch_id, tested_on);
