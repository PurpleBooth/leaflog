-- Leaflog schema migration V17 (strain-catalogue removal, feature-strip
-- D075, step 05).
--
-- The catalogue leaves the product end to end, and the history it used
-- to anchor moves onto the grow: each grow formerly linked to a strain
-- adopts the strain's name as its free-text `strain_label` and the
-- strain's photoperiod behaviour, then the strain table and the grow's
-- `strain_id` column are dropped. Values the grower already set after
-- V14 (a hand-entered label or toggle) are kept — the backfill only
-- fills what is still NULL, so an upgrade never rewrites the user's
-- own fields.
--
-- Each grow links at most one strain (its `strain_id` is a foreign key
-- to the single-row primary key), so the correlated subqueries resolve
-- to exactly one name and one photoperiod label per linked grow;
-- unlinked grows are left unlabelled and keep the photoperiod default.
--
-- Order mirrors V15: the parent table drops first (its ON DELETE SET
-- NULL fires harmlessly on the already-copied column on the way out),
-- then the child column — SQLite refuses a column drop only when the
-- column is the *target* of a foreign key, so the child-side reference
-- drops fine (V15's note). `idx_strain_name` dies with its table.
--
-- Conventions as in V1: snake_case labels of leaflog-core enums
-- (photoperiod/auto) validated in Rust; no default backfill — NULL
-- columns keep reading as the fields' defaults.

UPDATE grow SET
    strain_label = COALESCE(
        strain_label,
        (SELECT strain.name FROM strain WHERE strain.id = grow.strain_id)
    ),
    photoperiod = COALESCE(
        photoperiod,
        (SELECT strain.photoperiod FROM strain WHERE strain.id = grow.strain_id)
    )
WHERE strain_id IS NOT NULL;

DROP TABLE strain;
ALTER TABLE grow DROP COLUMN strain_id;
