-- Leaflog schema migration V18 (per-grow Light Cycle, spec §4.9).
--
-- The grow's daily lights-on local time plus hours-on per lit phase
-- as one JSON document (leaflog-core::light_cycle::LightCycle).
-- Nullable: `NULL` — and any `None` field inside — means the cited
-- preset defaults apply; reset-to-default writes `NULL`.

ALTER TABLE grow ADD COLUMN light_cycle TEXT;
