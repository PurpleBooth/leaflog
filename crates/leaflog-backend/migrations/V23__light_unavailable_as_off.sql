-- Leaflog schema migration V23 (dead-light stance, spec §4.1 / D159).
--
-- One boolean per grow: when set, the bound Light entity's dead HA
-- states (`unavailable` / `unknown`) read as lights-off at every
-- interpretation site (observed cycle, charts, corridors, coach) —
-- the plug hardware that drops its radio while off stays honest in
-- the bindings view regardless. Default 0 keeps today's read: a dead
-- state is simply no reading.
ALTER TABLE grow ADD COLUMN light_unavailable_as_off INTEGER NOT NULL DEFAULT 0;
