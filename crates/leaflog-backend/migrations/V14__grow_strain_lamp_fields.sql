-- Leaflog schema migration V14 (grow strain label, photoperiod and
-- lamp facts; D075 feature strip, step 01).
--
-- Purely additive: every column is nullable (dimmable defaulted), so
-- existing rows and suites are untouched and no backfill happens yet —
-- deriving label and photoperiod from the former strain links is the
-- later drop migration's job. Text columns store the core enums'
-- stable snake_case labels (photoperiod/auto, sensitive/average/
-- tolerant); NULL reads back as the field's default.

ALTER TABLE grow ADD COLUMN strain_label TEXT;
ALTER TABLE grow ADD COLUMN photoperiod TEXT;
ALTER TABLE grow ADD COLUMN heat_tolerance TEXT;
ALTER TABLE grow ADD COLUMN cold_tolerance TEXT;
ALTER TABLE grow ADD COLUMN light_tolerance TEXT;
ALTER TABLE grow ADD COLUMN real_watts REAL;
ALTER TABLE grow ADD COLUMN dimmable INTEGER NOT NULL DEFAULT 1;
