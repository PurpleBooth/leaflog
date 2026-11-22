-- Preserve the old single-plant association while allowing each photo
-- to belong to any number of plants.
CREATE TABLE photo_plant (
    photo_id TEXT NOT NULL REFERENCES photo(id) ON DELETE CASCADE,
    plant_id TEXT NOT NULL REFERENCES plant(id) ON DELETE CASCADE,
    PRIMARY KEY (photo_id, plant_id)
);
INSERT INTO photo_plant (photo_id, plant_id)
SELECT id, plant_id FROM photo WHERE plant_id IS NOT NULL;
CREATE INDEX idx_photo_plant_plant ON photo_plant(plant_id);
