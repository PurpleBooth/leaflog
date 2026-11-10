-- Leaflog schema migration V9 (HA sensor items, spec §4.1 / D052).
--
-- A sensor item is created by picking a Home Assistant entity in the
-- entity picker (temperature and humidity device classes in v1): the
-- entity is the item's identity. The same column doubles as the
-- optional HA binding for other categories (a light.*, a power sensor,
-- a dimmer number.*) — for sensor items the bound entity is the item
-- itself. The device class is stored so a space's temperature and
-- humidity items can be resolved without HA being reachable.
--
-- Conventions as in V1. Validation lives in Rust (the API rejects a
-- sensor item without an entity, and a non-sensor item with a device
-- class).

ALTER TABLE equipment_item ADD COLUMN ha_entity_id TEXT;
ALTER TABLE equipment_item ADD COLUMN ha_device_class TEXT;

CREATE INDEX idx_equipment_ha_entity ON equipment_item(ha_entity_id);
