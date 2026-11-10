-- Leaflog schema migration V11 (harvest & yield, spec §4.14 / D055).
--
-- The Harvest action gains structured per-plant weights: wet, dry and
-- trimmed, grams internally (SI; the UI displays per the HA unit
-- system). Nullable columns on plant per the data model (spec §8):
-- unset weights mean "not weighed yet", not zero.
--
-- Grows also record which feed template was fitted (nullable; fits
-- predating this column stay unattributed), so the §4.15 analytics can
-- group yield by template and "start next run" can copy the template
-- by reference.

ALTER TABLE plant ADD COLUMN wet_g     REAL;
ALTER TABLE plant ADD COLUMN dry_g     REAL;
ALTER TABLE plant ADD COLUMN trimmed_g REAL;

ALTER TABLE grow ADD COLUMN feed_template_id TEXT REFERENCES feed_template(id) ON DELETE SET NULL;
