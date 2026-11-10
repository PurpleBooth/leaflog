-- Leaflog schema migration V10 (seedbank, spec §4.13 / D053).
--
-- A log of owned seed batches: strain link, quantity, provenance,
-- acquisition date and storage method (which drives the §5.4 viability
-- window, computed in Rust from sourced preset ranges). Starting plants
-- from a linked batch decrements the count; manual edits always win.
--
-- Conventions as in V1: ULID TEXT ids, RFC 3339 UTC timestamps,
-- snake_case labels of leaflog-core enums validated in Rust.

CREATE TABLE seed_batch (
    id             TEXT PRIMARY KEY,
    strain_id      TEXT REFERENCES strain(id) ON DELETE SET NULL,
    quantity       INTEGER NOT NULL,          -- seeds remaining
    acquired_from  TEXT,                      -- bank / breeder / own harvest
    acquisition_on TEXT NOT NULL,             -- YYYY-MM-DD
    storage_method TEXT NOT NULL,             -- leaflog-core StorageMethod label
    notes          TEXT,
    created_at     TEXT NOT NULL,
    updated_at     TEXT NOT NULL
);

CREATE INDEX idx_seed_batch_strain ON seed_batch(strain_id, acquisition_on);

-- Grows may name the batch their plants came from (onboarding step 4 or
-- normal grow creation); deleting the batch keeps the grow (the
-- strain_id ON DELETE SET NULL pattern).
ALTER TABLE grow ADD COLUMN seed_batch_id TEXT REFERENCES seed_batch(id) ON DELETE SET NULL;
