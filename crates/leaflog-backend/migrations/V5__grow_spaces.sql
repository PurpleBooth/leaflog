-- Leaflog schema migration 0005 (equipment catalogue + grow spaces).
-- Conventions as in 0001: ULID TEXT ids, RFC 3339 UTC timestamps,
-- snake_case labels of leaflog-core enums validated in Rust.
--
-- equipment_item becomes the equipment catalogue: `is_builtin` marks
-- entries bundled by the seeder (fresh seeds only; rows already present
-- in upgraded databases stay unflagged, see docs/DECISIONS.md D020).

ALTER TABLE equipment_item ADD COLUMN source TEXT;
ALTER TABLE equipment_item ADD COLUMN is_builtin INTEGER NOT NULL DEFAULT 0;

-- One physical setup (tent/room): catalogue references plus grows.
CREATE TABLE grow_space (
    id          TEXT PRIMARY KEY,
    name        TEXT NOT NULL,
    width_cm    REAL,                     -- interior, SI centimetres
    depth_cm    REAL,
    height_cm   REAL,
    notes       TEXT,
    created_at  TEXT NOT NULL,
    updated_at  TEXT NOT NULL
);

-- Catalogue entries referenced by a space. A shared item (a meter
-- carried between tents) may appear in several spaces; two identical
-- lights are two catalogue entries.
CREATE TABLE grow_space_equipment (
    grow_space_id TEXT NOT NULL REFERENCES grow_space(id) ON DELETE CASCADE,
    equipment_id  TEXT NOT NULL REFERENCES equipment_item(id) ON DELETE CASCADE,
    PRIMARY KEY (grow_space_id, equipment_id)
);

CREATE INDEX idx_grow_space_equipment_equipment ON grow_space_equipment(equipment_id);

-- Grows live inside a space when one is chosen; deleting the space
-- keeps the grow (strain_id uses the same ON DELETE SET NULL pattern).
ALTER TABLE grow ADD COLUMN grow_space_id TEXT REFERENCES grow_space(id) ON DELETE SET NULL;
