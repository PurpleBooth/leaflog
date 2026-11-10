-- Leaflog schema migration V7 (read-only bundled strains, spec §4.4 /
-- §8 / D050).
--
-- The strain table gains the explicit `is_builtin` flag the data model
-- reserves for it, matching equipment (V5). Existing rows are backfilled
-- from provenance: only the seeder ever wrote source_url, so every row
-- carrying one is bundled. Bundled strains become read-only at the API
-- (edit/delete rejected, clone-to-modify), like bundled equipment.

ALTER TABLE strain ADD COLUMN is_builtin INTEGER NOT NULL DEFAULT 0;

UPDATE strain SET is_builtin = 1 WHERE source_url IS NOT NULL;
