-- Leaflog schema migration V8 (stable bundled IDs, spec §4.1 / D051).
--
-- Every bundled record (equipment presets, strains) carries a stable ID
-- from its source dataset. On upgrade the app inserts bundled rows that
-- are new and never modifies or deletes existing ones; a corrected
-- bundled record ships as a new revision (new stable ID), and removing
-- one requires an explicit migration (V6 did that for the trim).
--
-- This one-time migration re-keys the bundled rows of pre-V8 databases
-- (seeded with random ULIDs) onto their stable IDs so the insert-only
-- seeder can tell "already bundled" from "new in this release" apart.
-- Foreign keys are ON while it runs, so each record is adopted with the
-- FK-safe insert -> repoint children -> delete-old pattern rather than a
-- primary-key UPDATE. Children follow: grow_space_equipment for
-- equipment; grow.strain_id and strain.is_phenotype_of (clone parents)
-- for strains.
--
-- Adoption matches by (name, category) for equipment and (name, breeder)
-- for strains, and only when exactly one row matches: a missing row is
-- left for the seeder to insert fresh (it is "new" under its stable ID),
-- and an ambiguous pair (e.g. a user entry that coincidentally shares
-- the bundled name) is left untouched to never destroy user data.
-- Adopted rows are marked is_builtin = 1 whatever the old flag said:
-- they are the bundled records, and bundled records are read-only.

-- Grow tent 60x60x180 (tent_room)
INSERT INTO equipment_item
    (id, name, category, brand_model, real_watts, dimmable,
     purchase_date, purchase_price, currency, notes, source,
     is_builtin, created_at, updated_at)
SELECT 'seed-equip-grow-tent-60x60x180', name, category, brand_model, real_watts, dimmable,
       purchase_date, purchase_price, currency, notes, source,
       1, created_at, updated_at
FROM equipment_item
WHERE name = 'Grow tent 60x60x180' AND category = 'tent_room'
  AND (SELECT COUNT(*) FROM equipment_item
       WHERE name = 'Grow tent 60x60x180' AND category = 'tent_room') = 1;

UPDATE grow_space_equipment
SET equipment_id = 'seed-equip-grow-tent-60x60x180'
WHERE EXISTS(SELECT 1 FROM equipment_item WHERE id = 'seed-equip-grow-tent-60x60x180')
  AND equipment_id IN (SELECT id FROM equipment_item
                       WHERE name = 'Grow tent 60x60x180' AND category = 'tent_room');

DELETE FROM equipment_item
WHERE name = 'Grow tent 60x60x180' AND category = 'tent_room' AND id <> 'seed-equip-grow-tent-60x60x180'
  AND EXISTS(SELECT 1 FROM equipment_item WHERE id = 'seed-equip-grow-tent-60x60x180');

-- SE1500 LED grow light (light)
INSERT INTO equipment_item
    (id, name, category, brand_model, real_watts, dimmable,
     purchase_date, purchase_price, currency, notes, source,
     is_builtin, created_at, updated_at)
SELECT 'seed-equip-se1500-led-grow-light', name, category, brand_model, real_watts, dimmable,
       purchase_date, purchase_price, currency, notes, source,
       1, created_at, updated_at
FROM equipment_item
WHERE name = 'SE1500 LED grow light' AND category = 'light'
  AND (SELECT COUNT(*) FROM equipment_item
       WHERE name = 'SE1500 LED grow light' AND category = 'light') = 1;

UPDATE grow_space_equipment
SET equipment_id = 'seed-equip-se1500-led-grow-light'
WHERE EXISTS(SELECT 1 FROM equipment_item WHERE id = 'seed-equip-se1500-led-grow-light')
  AND equipment_id IN (SELECT id FROM equipment_item
                       WHERE name = 'SE1500 LED grow light' AND category = 'light');

DELETE FROM equipment_item
WHERE name = 'SE1500 LED grow light' AND category = 'light' AND id <> 'seed-equip-se1500-led-grow-light'
  AND EXISTS(SELECT 1 FROM equipment_item WHERE id = 'seed-equip-se1500-led-grow-light');

-- E6 clip fan 150 mm (fan)
INSERT INTO equipment_item
    (id, name, category, brand_model, real_watts, dimmable,
     purchase_date, purchase_price, currency, notes, source,
     is_builtin, created_at, updated_at)
SELECT 'seed-equip-e6-clip-fan-150-mm', name, category, brand_model, real_watts, dimmable,
       purchase_date, purchase_price, currency, notes, source,
       1, created_at, updated_at
FROM equipment_item
WHERE name = 'E6 clip fan 150 mm' AND category = 'fan'
  AND (SELECT COUNT(*) FROM equipment_item
       WHERE name = 'E6 clip fan 150 mm' AND category = 'fan') = 1;

UPDATE grow_space_equipment
SET equipment_id = 'seed-equip-e6-clip-fan-150-mm'
WHERE EXISTS(SELECT 1 FROM equipment_item WHERE id = 'seed-equip-e6-clip-fan-150-mm')
  AND equipment_id IN (SELECT id FROM equipment_item
                       WHERE name = 'E6 clip fan 150 mm' AND category = 'fan');

DELETE FROM equipment_item
WHERE name = 'E6 clip fan 150 mm' AND category = 'fan' AND id <> 'seed-equip-e6-clip-fan-150-mm'
  AND EXISTS(SELECT 1 FROM equipment_item WHERE id = 'seed-equip-e6-clip-fan-150-mm');

-- Carbon filter 100 mm (fan)
INSERT INTO equipment_item
    (id, name, category, brand_model, real_watts, dimmable,
     purchase_date, purchase_price, currency, notes, source,
     is_builtin, created_at, updated_at)
SELECT 'seed-equip-carbon-filter-100-mm', name, category, brand_model, real_watts, dimmable,
       purchase_date, purchase_price, currency, notes, source,
       1, created_at, updated_at
FROM equipment_item
WHERE name = 'Carbon filter 100 mm' AND category = 'fan'
  AND (SELECT COUNT(*) FROM equipment_item
       WHERE name = 'Carbon filter 100 mm' AND category = 'fan') = 1;

UPDATE grow_space_equipment
SET equipment_id = 'seed-equip-carbon-filter-100-mm'
WHERE EXISTS(SELECT 1 FROM equipment_item WHERE id = 'seed-equip-carbon-filter-100-mm')
  AND equipment_id IN (SELECT id FROM equipment_item
                       WHERE name = 'Carbon filter 100 mm' AND category = 'fan');

DELETE FROM equipment_item
WHERE name = 'Carbon filter 100 mm' AND category = 'fan' AND id <> 'seed-equip-carbon-filter-100-mm'
  AND EXISTS(SELECT 1 FROM equipment_item WHERE id = 'seed-equip-carbon-filter-100-mm');

-- Carbon filter 150 mm (fan)
INSERT INTO equipment_item
    (id, name, category, brand_model, real_watts, dimmable,
     purchase_date, purchase_price, currency, notes, source,
     is_builtin, created_at, updated_at)
SELECT 'seed-equip-carbon-filter-150-mm', name, category, brand_model, real_watts, dimmable,
       purchase_date, purchase_price, currency, notes, source,
       1, created_at, updated_at
FROM equipment_item
WHERE name = 'Carbon filter 150 mm' AND category = 'fan'
  AND (SELECT COUNT(*) FROM equipment_item
       WHERE name = 'Carbon filter 150 mm' AND category = 'fan') = 1;

UPDATE grow_space_equipment
SET equipment_id = 'seed-equip-carbon-filter-150-mm'
WHERE EXISTS(SELECT 1 FROM equipment_item WHERE id = 'seed-equip-carbon-filter-150-mm')
  AND equipment_id IN (SELECT id FROM equipment_item
                       WHERE name = 'Carbon filter 150 mm' AND category = 'fan');

DELETE FROM equipment_item
WHERE name = 'Carbon filter 150 mm' AND category = 'fan' AND id <> 'seed-equip-carbon-filter-150-mm'
  AND EXISTS(SELECT 1 FROM equipment_item WHERE id = 'seed-equip-carbon-filter-150-mm');

-- Northern Lights (Sensi Seeds)
INSERT INTO strain
    (id, name, breeder, photoperiod, sex, indica_pct, landrace_tags,
     flower_days_min, flower_days_max, stretch_factor, veg_weeks_min,
     veg_weeks_max, height_class, auto_total_days_min,
     auto_total_days_max, yield_notes, terpene_notes, effect_notes,
     heat_tolerance, cold_tolerance, light_tolerance, user_rating,
     notes, evidence, source_url, source_licence, is_builtin,
     is_phenotype_of, created_at, updated_at)
SELECT 'seed-strain-northern-lights', name, breeder, photoperiod, sex, indica_pct,
       landrace_tags, flower_days_min, flower_days_max, stretch_factor,
       veg_weeks_min, veg_weeks_max, height_class, auto_total_days_min,
       auto_total_days_max, yield_notes, terpene_notes, effect_notes,
       heat_tolerance, cold_tolerance, light_tolerance, user_rating,
       notes, evidence, source_url, source_licence, 1,
       is_phenotype_of, created_at, updated_at
FROM strain
WHERE name = 'Northern Lights' AND breeder = 'Sensi Seeds'
  AND (SELECT COUNT(*) FROM strain WHERE name = 'Northern Lights' AND breeder = 'Sensi Seeds') = 1;

UPDATE grow
SET strain_id = 'seed-strain-northern-lights'
WHERE EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-northern-lights')
  AND strain_id IN (SELECT id FROM strain WHERE name = 'Northern Lights' AND breeder = 'Sensi Seeds');

UPDATE strain
SET is_phenotype_of = 'seed-strain-northern-lights'
WHERE EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-northern-lights')
  AND is_phenotype_of IN (SELECT id FROM strain WHERE name = 'Northern Lights' AND breeder = 'Sensi Seeds');

DELETE FROM strain
WHERE name = 'Northern Lights' AND breeder = 'Sensi Seeds' AND id <> 'seed-strain-northern-lights'
  AND EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-northern-lights');

-- Amnesia Haze (Barney's Farm)
INSERT INTO strain
    (id, name, breeder, photoperiod, sex, indica_pct, landrace_tags,
     flower_days_min, flower_days_max, stretch_factor, veg_weeks_min,
     veg_weeks_max, height_class, auto_total_days_min,
     auto_total_days_max, yield_notes, terpene_notes, effect_notes,
     heat_tolerance, cold_tolerance, light_tolerance, user_rating,
     notes, evidence, source_url, source_licence, is_builtin,
     is_phenotype_of, created_at, updated_at)
SELECT 'seed-strain-amnesia-haze', name, breeder, photoperiod, sex, indica_pct,
       landrace_tags, flower_days_min, flower_days_max, stretch_factor,
       veg_weeks_min, veg_weeks_max, height_class, auto_total_days_min,
       auto_total_days_max, yield_notes, terpene_notes, effect_notes,
       heat_tolerance, cold_tolerance, light_tolerance, user_rating,
       notes, evidence, source_url, source_licence, 1,
       is_phenotype_of, created_at, updated_at
FROM strain
WHERE name = 'Amnesia Haze' AND breeder = 'Barney''s Farm'
  AND (SELECT COUNT(*) FROM strain WHERE name = 'Amnesia Haze' AND breeder = 'Barney''s Farm') = 1;

UPDATE grow
SET strain_id = 'seed-strain-amnesia-haze'
WHERE EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-amnesia-haze')
  AND strain_id IN (SELECT id FROM strain WHERE name = 'Amnesia Haze' AND breeder = 'Barney''s Farm');

UPDATE strain
SET is_phenotype_of = 'seed-strain-amnesia-haze'
WHERE EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-amnesia-haze')
  AND is_phenotype_of IN (SELECT id FROM strain WHERE name = 'Amnesia Haze' AND breeder = 'Barney''s Farm');

DELETE FROM strain
WHERE name = 'Amnesia Haze' AND breeder = 'Barney''s Farm' AND id <> 'seed-strain-amnesia-haze'
  AND EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-amnesia-haze');

-- Blue Dream
INSERT INTO strain
    (id, name, breeder, photoperiod, sex, indica_pct, landrace_tags,
     flower_days_min, flower_days_max, stretch_factor, veg_weeks_min,
     veg_weeks_max, height_class, auto_total_days_min,
     auto_total_days_max, yield_notes, terpene_notes, effect_notes,
     heat_tolerance, cold_tolerance, light_tolerance, user_rating,
     notes, evidence, source_url, source_licence, is_builtin,
     is_phenotype_of, created_at, updated_at)
SELECT 'seed-strain-blue-dream', name, breeder, photoperiod, sex, indica_pct,
       landrace_tags, flower_days_min, flower_days_max, stretch_factor,
       veg_weeks_min, veg_weeks_max, height_class, auto_total_days_min,
       auto_total_days_max, yield_notes, terpene_notes, effect_notes,
       heat_tolerance, cold_tolerance, light_tolerance, user_rating,
       notes, evidence, source_url, source_licence, 1,
       is_phenotype_of, created_at, updated_at
FROM strain
WHERE name = 'Blue Dream' AND breeder IS NULL
  AND (SELECT COUNT(*) FROM strain WHERE name = 'Blue Dream' AND breeder IS NULL) = 1;

UPDATE grow
SET strain_id = 'seed-strain-blue-dream'
WHERE EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-blue-dream')
  AND strain_id IN (SELECT id FROM strain WHERE name = 'Blue Dream' AND breeder IS NULL);

UPDATE strain
SET is_phenotype_of = 'seed-strain-blue-dream'
WHERE EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-blue-dream')
  AND is_phenotype_of IN (SELECT id FROM strain WHERE name = 'Blue Dream' AND breeder IS NULL);

DELETE FROM strain
WHERE name = 'Blue Dream' AND breeder IS NULL AND id <> 'seed-strain-blue-dream'
  AND EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-blue-dream');

-- Gorilla Glue #4
INSERT INTO strain
    (id, name, breeder, photoperiod, sex, indica_pct, landrace_tags,
     flower_days_min, flower_days_max, stretch_factor, veg_weeks_min,
     veg_weeks_max, height_class, auto_total_days_min,
     auto_total_days_max, yield_notes, terpene_notes, effect_notes,
     heat_tolerance, cold_tolerance, light_tolerance, user_rating,
     notes, evidence, source_url, source_licence, is_builtin,
     is_phenotype_of, created_at, updated_at)
SELECT 'seed-strain-gorilla-glue-4', name, breeder, photoperiod, sex, indica_pct,
       landrace_tags, flower_days_min, flower_days_max, stretch_factor,
       veg_weeks_min, veg_weeks_max, height_class, auto_total_days_min,
       auto_total_days_max, yield_notes, terpene_notes, effect_notes,
       heat_tolerance, cold_tolerance, light_tolerance, user_rating,
       notes, evidence, source_url, source_licence, 1,
       is_phenotype_of, created_at, updated_at
FROM strain
WHERE name = 'Gorilla Glue #4' AND breeder IS NULL
  AND (SELECT COUNT(*) FROM strain WHERE name = 'Gorilla Glue #4' AND breeder IS NULL) = 1;

UPDATE grow
SET strain_id = 'seed-strain-gorilla-glue-4'
WHERE EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-gorilla-glue-4')
  AND strain_id IN (SELECT id FROM strain WHERE name = 'Gorilla Glue #4' AND breeder IS NULL);

UPDATE strain
SET is_phenotype_of = 'seed-strain-gorilla-glue-4'
WHERE EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-gorilla-glue-4')
  AND is_phenotype_of IN (SELECT id FROM strain WHERE name = 'Gorilla Glue #4' AND breeder IS NULL);

DELETE FROM strain
WHERE name = 'Gorilla Glue #4' AND breeder IS NULL AND id <> 'seed-strain-gorilla-glue-4'
  AND EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-gorilla-glue-4');

-- GG4 Auto
INSERT INTO strain
    (id, name, breeder, photoperiod, sex, indica_pct, landrace_tags,
     flower_days_min, flower_days_max, stretch_factor, veg_weeks_min,
     veg_weeks_max, height_class, auto_total_days_min,
     auto_total_days_max, yield_notes, terpene_notes, effect_notes,
     heat_tolerance, cold_tolerance, light_tolerance, user_rating,
     notes, evidence, source_url, source_licence, is_builtin,
     is_phenotype_of, created_at, updated_at)
SELECT 'seed-strain-gg4-auto', name, breeder, photoperiod, sex, indica_pct,
       landrace_tags, flower_days_min, flower_days_max, stretch_factor,
       veg_weeks_min, veg_weeks_max, height_class, auto_total_days_min,
       auto_total_days_max, yield_notes, terpene_notes, effect_notes,
       heat_tolerance, cold_tolerance, light_tolerance, user_rating,
       notes, evidence, source_url, source_licence, 1,
       is_phenotype_of, created_at, updated_at
FROM strain
WHERE name = 'GG4 Auto' AND breeder IS NULL
  AND (SELECT COUNT(*) FROM strain WHERE name = 'GG4 Auto' AND breeder IS NULL) = 1;

UPDATE grow
SET strain_id = 'seed-strain-gg4-auto'
WHERE EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-gg4-auto')
  AND strain_id IN (SELECT id FROM strain WHERE name = 'GG4 Auto' AND breeder IS NULL);

UPDATE strain
SET is_phenotype_of = 'seed-strain-gg4-auto'
WHERE EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-gg4-auto')
  AND is_phenotype_of IN (SELECT id FROM strain WHERE name = 'GG4 Auto' AND breeder IS NULL);

DELETE FROM strain
WHERE name = 'GG4 Auto' AND breeder IS NULL AND id <> 'seed-strain-gg4-auto'
  AND EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-gg4-auto');

-- Mokum's Tulip (Dutch Passion)
INSERT INTO strain
    (id, name, breeder, photoperiod, sex, indica_pct, landrace_tags,
     flower_days_min, flower_days_max, stretch_factor, veg_weeks_min,
     veg_weeks_max, height_class, auto_total_days_min,
     auto_total_days_max, yield_notes, terpene_notes, effect_notes,
     heat_tolerance, cold_tolerance, light_tolerance, user_rating,
     notes, evidence, source_url, source_licence, is_builtin,
     is_phenotype_of, created_at, updated_at)
SELECT 'seed-strain-mokum-s-tulip', name, breeder, photoperiod, sex, indica_pct,
       landrace_tags, flower_days_min, flower_days_max, stretch_factor,
       veg_weeks_min, veg_weeks_max, height_class, auto_total_days_min,
       auto_total_days_max, yield_notes, terpene_notes, effect_notes,
       heat_tolerance, cold_tolerance, light_tolerance, user_rating,
       notes, evidence, source_url, source_licence, 1,
       is_phenotype_of, created_at, updated_at
FROM strain
WHERE name = 'Mokum''s Tulip' AND breeder = 'Dutch Passion'
  AND (SELECT COUNT(*) FROM strain WHERE name = 'Mokum''s Tulip' AND breeder = 'Dutch Passion') = 1;

UPDATE grow
SET strain_id = 'seed-strain-mokum-s-tulip'
WHERE EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-mokum-s-tulip')
  AND strain_id IN (SELECT id FROM strain WHERE name = 'Mokum''s Tulip' AND breeder = 'Dutch Passion');

UPDATE strain
SET is_phenotype_of = 'seed-strain-mokum-s-tulip'
WHERE EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-mokum-s-tulip')
  AND is_phenotype_of IN (SELECT id FROM strain WHERE name = 'Mokum''s Tulip' AND breeder = 'Dutch Passion');

DELETE FROM strain
WHERE name = 'Mokum''s Tulip' AND breeder = 'Dutch Passion' AND id <> 'seed-strain-mokum-s-tulip'
  AND EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-mokum-s-tulip');

-- Euforia (Dutch Passion)
INSERT INTO strain
    (id, name, breeder, photoperiod, sex, indica_pct, landrace_tags,
     flower_days_min, flower_days_max, stretch_factor, veg_weeks_min,
     veg_weeks_max, height_class, auto_total_days_min,
     auto_total_days_max, yield_notes, terpene_notes, effect_notes,
     heat_tolerance, cold_tolerance, light_tolerance, user_rating,
     notes, evidence, source_url, source_licence, is_builtin,
     is_phenotype_of, created_at, updated_at)
SELECT 'seed-strain-euforia', name, breeder, photoperiod, sex, indica_pct,
       landrace_tags, flower_days_min, flower_days_max, stretch_factor,
       veg_weeks_min, veg_weeks_max, height_class, auto_total_days_min,
       auto_total_days_max, yield_notes, terpene_notes, effect_notes,
       heat_tolerance, cold_tolerance, light_tolerance, user_rating,
       notes, evidence, source_url, source_licence, 1,
       is_phenotype_of, created_at, updated_at
FROM strain
WHERE name = 'Euforia' AND breeder = 'Dutch Passion'
  AND (SELECT COUNT(*) FROM strain WHERE name = 'Euforia' AND breeder = 'Dutch Passion') = 1;

UPDATE grow
SET strain_id = 'seed-strain-euforia'
WHERE EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-euforia')
  AND strain_id IN (SELECT id FROM strain WHERE name = 'Euforia' AND breeder = 'Dutch Passion');

UPDATE strain
SET is_phenotype_of = 'seed-strain-euforia'
WHERE EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-euforia')
  AND is_phenotype_of IN (SELECT id FROM strain WHERE name = 'Euforia' AND breeder = 'Dutch Passion');

DELETE FROM strain
WHERE name = 'Euforia' AND breeder = 'Dutch Passion' AND id <> 'seed-strain-euforia'
  AND EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-euforia');

-- Melonade Runtz (Dutch Passion)
INSERT INTO strain
    (id, name, breeder, photoperiod, sex, indica_pct, landrace_tags,
     flower_days_min, flower_days_max, stretch_factor, veg_weeks_min,
     veg_weeks_max, height_class, auto_total_days_min,
     auto_total_days_max, yield_notes, terpene_notes, effect_notes,
     heat_tolerance, cold_tolerance, light_tolerance, user_rating,
     notes, evidence, source_url, source_licence, is_builtin,
     is_phenotype_of, created_at, updated_at)
SELECT 'seed-strain-melonade-runtz', name, breeder, photoperiod, sex, indica_pct,
       landrace_tags, flower_days_min, flower_days_max, stretch_factor,
       veg_weeks_min, veg_weeks_max, height_class, auto_total_days_min,
       auto_total_days_max, yield_notes, terpene_notes, effect_notes,
       heat_tolerance, cold_tolerance, light_tolerance, user_rating,
       notes, evidence, source_url, source_licence, 1,
       is_phenotype_of, created_at, updated_at
FROM strain
WHERE name = 'Melonade Runtz' AND breeder = 'Dutch Passion'
  AND (SELECT COUNT(*) FROM strain WHERE name = 'Melonade Runtz' AND breeder = 'Dutch Passion') = 1;

UPDATE grow
SET strain_id = 'seed-strain-melonade-runtz'
WHERE EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-melonade-runtz')
  AND strain_id IN (SELECT id FROM strain WHERE name = 'Melonade Runtz' AND breeder = 'Dutch Passion');

UPDATE strain
SET is_phenotype_of = 'seed-strain-melonade-runtz'
WHERE EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-melonade-runtz')
  AND is_phenotype_of IN (SELECT id FROM strain WHERE name = 'Melonade Runtz' AND breeder = 'Dutch Passion');

DELETE FROM strain
WHERE name = 'Melonade Runtz' AND breeder = 'Dutch Passion' AND id <> 'seed-strain-melonade-runtz'
  AND EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-melonade-runtz');

-- Frozen Biscotti (Dutch Passion)
INSERT INTO strain
    (id, name, breeder, photoperiod, sex, indica_pct, landrace_tags,
     flower_days_min, flower_days_max, stretch_factor, veg_weeks_min,
     veg_weeks_max, height_class, auto_total_days_min,
     auto_total_days_max, yield_notes, terpene_notes, effect_notes,
     heat_tolerance, cold_tolerance, light_tolerance, user_rating,
     notes, evidence, source_url, source_licence, is_builtin,
     is_phenotype_of, created_at, updated_at)
SELECT 'seed-strain-frozen-biscotti', name, breeder, photoperiod, sex, indica_pct,
       landrace_tags, flower_days_min, flower_days_max, stretch_factor,
       veg_weeks_min, veg_weeks_max, height_class, auto_total_days_min,
       auto_total_days_max, yield_notes, terpene_notes, effect_notes,
       heat_tolerance, cold_tolerance, light_tolerance, user_rating,
       notes, evidence, source_url, source_licence, 1,
       is_phenotype_of, created_at, updated_at
FROM strain
WHERE name = 'Frozen Biscotti' AND breeder = 'Dutch Passion'
  AND (SELECT COUNT(*) FROM strain WHERE name = 'Frozen Biscotti' AND breeder = 'Dutch Passion') = 1;

UPDATE grow
SET strain_id = 'seed-strain-frozen-biscotti'
WHERE EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-frozen-biscotti')
  AND strain_id IN (SELECT id FROM strain WHERE name = 'Frozen Biscotti' AND breeder = 'Dutch Passion');

UPDATE strain
SET is_phenotype_of = 'seed-strain-frozen-biscotti'
WHERE EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-frozen-biscotti')
  AND is_phenotype_of IN (SELECT id FROM strain WHERE name = 'Frozen Biscotti' AND breeder = 'Dutch Passion');

DELETE FROM strain
WHERE name = 'Frozen Biscotti' AND breeder = 'Dutch Passion' AND id <> 'seed-strain-frozen-biscotti'
  AND EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-frozen-biscotti');

-- Tropical Tangie (Dutch Passion)
INSERT INTO strain
    (id, name, breeder, photoperiod, sex, indica_pct, landrace_tags,
     flower_days_min, flower_days_max, stretch_factor, veg_weeks_min,
     veg_weeks_max, height_class, auto_total_days_min,
     auto_total_days_max, yield_notes, terpene_notes, effect_notes,
     heat_tolerance, cold_tolerance, light_tolerance, user_rating,
     notes, evidence, source_url, source_licence, is_builtin,
     is_phenotype_of, created_at, updated_at)
SELECT 'seed-strain-tropical-tangie', name, breeder, photoperiod, sex, indica_pct,
       landrace_tags, flower_days_min, flower_days_max, stretch_factor,
       veg_weeks_min, veg_weeks_max, height_class, auto_total_days_min,
       auto_total_days_max, yield_notes, terpene_notes, effect_notes,
       heat_tolerance, cold_tolerance, light_tolerance, user_rating,
       notes, evidence, source_url, source_licence, 1,
       is_phenotype_of, created_at, updated_at
FROM strain
WHERE name = 'Tropical Tangie' AND breeder = 'Dutch Passion'
  AND (SELECT COUNT(*) FROM strain WHERE name = 'Tropical Tangie' AND breeder = 'Dutch Passion') = 1;

UPDATE grow
SET strain_id = 'seed-strain-tropical-tangie'
WHERE EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-tropical-tangie')
  AND strain_id IN (SELECT id FROM strain WHERE name = 'Tropical Tangie' AND breeder = 'Dutch Passion');

UPDATE strain
SET is_phenotype_of = 'seed-strain-tropical-tangie'
WHERE EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-tropical-tangie')
  AND is_phenotype_of IN (SELECT id FROM strain WHERE name = 'Tropical Tangie' AND breeder = 'Dutch Passion');

DELETE FROM strain
WHERE name = 'Tropical Tangie' AND breeder = 'Dutch Passion' AND id <> 'seed-strain-tropical-tangie'
  AND EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-tropical-tangie');

-- Royal Cookies (Royal Queen Seeds)
INSERT INTO strain
    (id, name, breeder, photoperiod, sex, indica_pct, landrace_tags,
     flower_days_min, flower_days_max, stretch_factor, veg_weeks_min,
     veg_weeks_max, height_class, auto_total_days_min,
     auto_total_days_max, yield_notes, terpene_notes, effect_notes,
     heat_tolerance, cold_tolerance, light_tolerance, user_rating,
     notes, evidence, source_url, source_licence, is_builtin,
     is_phenotype_of, created_at, updated_at)
SELECT 'seed-strain-royal-cookies', name, breeder, photoperiod, sex, indica_pct,
       landrace_tags, flower_days_min, flower_days_max, stretch_factor,
       veg_weeks_min, veg_weeks_max, height_class, auto_total_days_min,
       auto_total_days_max, yield_notes, terpene_notes, effect_notes,
       heat_tolerance, cold_tolerance, light_tolerance, user_rating,
       notes, evidence, source_url, source_licence, 1,
       is_phenotype_of, created_at, updated_at
FROM strain
WHERE name = 'Royal Cookies' AND breeder = 'Royal Queen Seeds'
  AND (SELECT COUNT(*) FROM strain WHERE name = 'Royal Cookies' AND breeder = 'Royal Queen Seeds') = 1;

UPDATE grow
SET strain_id = 'seed-strain-royal-cookies'
WHERE EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-royal-cookies')
  AND strain_id IN (SELECT id FROM strain WHERE name = 'Royal Cookies' AND breeder = 'Royal Queen Seeds');

UPDATE strain
SET is_phenotype_of = 'seed-strain-royal-cookies'
WHERE EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-royal-cookies')
  AND is_phenotype_of IN (SELECT id FROM strain WHERE name = 'Royal Cookies' AND breeder = 'Royal Queen Seeds');

DELETE FROM strain
WHERE name = 'Royal Cookies' AND breeder = 'Royal Queen Seeds' AND id <> 'seed-strain-royal-cookies'
  AND EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-royal-cookies');

-- Fat Banana Auto (Royal Queen Seeds)
INSERT INTO strain
    (id, name, breeder, photoperiod, sex, indica_pct, landrace_tags,
     flower_days_min, flower_days_max, stretch_factor, veg_weeks_min,
     veg_weeks_max, height_class, auto_total_days_min,
     auto_total_days_max, yield_notes, terpene_notes, effect_notes,
     heat_tolerance, cold_tolerance, light_tolerance, user_rating,
     notes, evidence, source_url, source_licence, is_builtin,
     is_phenotype_of, created_at, updated_at)
SELECT 'seed-strain-fat-banana-auto', name, breeder, photoperiod, sex, indica_pct,
       landrace_tags, flower_days_min, flower_days_max, stretch_factor,
       veg_weeks_min, veg_weeks_max, height_class, auto_total_days_min,
       auto_total_days_max, yield_notes, terpene_notes, effect_notes,
       heat_tolerance, cold_tolerance, light_tolerance, user_rating,
       notes, evidence, source_url, source_licence, 1,
       is_phenotype_of, created_at, updated_at
FROM strain
WHERE name = 'Fat Banana Auto' AND breeder = 'Royal Queen Seeds'
  AND (SELECT COUNT(*) FROM strain WHERE name = 'Fat Banana Auto' AND breeder = 'Royal Queen Seeds') = 1;

UPDATE grow
SET strain_id = 'seed-strain-fat-banana-auto'
WHERE EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-fat-banana-auto')
  AND strain_id IN (SELECT id FROM strain WHERE name = 'Fat Banana Auto' AND breeder = 'Royal Queen Seeds');

UPDATE strain
SET is_phenotype_of = 'seed-strain-fat-banana-auto'
WHERE EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-fat-banana-auto')
  AND is_phenotype_of IN (SELECT id FROM strain WHERE name = 'Fat Banana Auto' AND breeder = 'Royal Queen Seeds');

DELETE FROM strain
WHERE name = 'Fat Banana Auto' AND breeder = 'Royal Queen Seeds' AND id <> 'seed-strain-fat-banana-auto'
  AND EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-fat-banana-auto');

-- Green Gelato Auto (Royal Queen Seeds)
INSERT INTO strain
    (id, name, breeder, photoperiod, sex, indica_pct, landrace_tags,
     flower_days_min, flower_days_max, stretch_factor, veg_weeks_min,
     veg_weeks_max, height_class, auto_total_days_min,
     auto_total_days_max, yield_notes, terpene_notes, effect_notes,
     heat_tolerance, cold_tolerance, light_tolerance, user_rating,
     notes, evidence, source_url, source_licence, is_builtin,
     is_phenotype_of, created_at, updated_at)
SELECT 'seed-strain-green-gelato-auto', name, breeder, photoperiod, sex, indica_pct,
       landrace_tags, flower_days_min, flower_days_max, stretch_factor,
       veg_weeks_min, veg_weeks_max, height_class, auto_total_days_min,
       auto_total_days_max, yield_notes, terpene_notes, effect_notes,
       heat_tolerance, cold_tolerance, light_tolerance, user_rating,
       notes, evidence, source_url, source_licence, 1,
       is_phenotype_of, created_at, updated_at
FROM strain
WHERE name = 'Green Gelato Auto' AND breeder = 'Royal Queen Seeds'
  AND (SELECT COUNT(*) FROM strain WHERE name = 'Green Gelato Auto' AND breeder = 'Royal Queen Seeds') = 1;

UPDATE grow
SET strain_id = 'seed-strain-green-gelato-auto'
WHERE EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-green-gelato-auto')
  AND strain_id IN (SELECT id FROM strain WHERE name = 'Green Gelato Auto' AND breeder = 'Royal Queen Seeds');

UPDATE strain
SET is_phenotype_of = 'seed-strain-green-gelato-auto'
WHERE EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-green-gelato-auto')
  AND is_phenotype_of IN (SELECT id FROM strain WHERE name = 'Green Gelato Auto' AND breeder = 'Royal Queen Seeds');

DELETE FROM strain
WHERE name = 'Green Gelato Auto' AND breeder = 'Royal Queen Seeds' AND id <> 'seed-strain-green-gelato-auto'
  AND EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-green-gelato-auto');

-- Cookies Gelato Auto (Royal Queen Seeds)
INSERT INTO strain
    (id, name, breeder, photoperiod, sex, indica_pct, landrace_tags,
     flower_days_min, flower_days_max, stretch_factor, veg_weeks_min,
     veg_weeks_max, height_class, auto_total_days_min,
     auto_total_days_max, yield_notes, terpene_notes, effect_notes,
     heat_tolerance, cold_tolerance, light_tolerance, user_rating,
     notes, evidence, source_url, source_licence, is_builtin,
     is_phenotype_of, created_at, updated_at)
SELECT 'seed-strain-cookies-gelato-auto', name, breeder, photoperiod, sex, indica_pct,
       landrace_tags, flower_days_min, flower_days_max, stretch_factor,
       veg_weeks_min, veg_weeks_max, height_class, auto_total_days_min,
       auto_total_days_max, yield_notes, terpene_notes, effect_notes,
       heat_tolerance, cold_tolerance, light_tolerance, user_rating,
       notes, evidence, source_url, source_licence, 1,
       is_phenotype_of, created_at, updated_at
FROM strain
WHERE name = 'Cookies Gelato Auto' AND breeder = 'Royal Queen Seeds'
  AND (SELECT COUNT(*) FROM strain WHERE name = 'Cookies Gelato Auto' AND breeder = 'Royal Queen Seeds') = 1;

UPDATE grow
SET strain_id = 'seed-strain-cookies-gelato-auto'
WHERE EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-cookies-gelato-auto')
  AND strain_id IN (SELECT id FROM strain WHERE name = 'Cookies Gelato Auto' AND breeder = 'Royal Queen Seeds');

UPDATE strain
SET is_phenotype_of = 'seed-strain-cookies-gelato-auto'
WHERE EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-cookies-gelato-auto')
  AND is_phenotype_of IN (SELECT id FROM strain WHERE name = 'Cookies Gelato Auto' AND breeder = 'Royal Queen Seeds');

DELETE FROM strain
WHERE name = 'Cookies Gelato Auto' AND breeder = 'Royal Queen Seeds' AND id <> 'seed-strain-cookies-gelato-auto'
  AND EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-cookies-gelato-auto');

-- Special Queen #1 (Royal Queen Seeds)
INSERT INTO strain
    (id, name, breeder, photoperiod, sex, indica_pct, landrace_tags,
     flower_days_min, flower_days_max, stretch_factor, veg_weeks_min,
     veg_weeks_max, height_class, auto_total_days_min,
     auto_total_days_max, yield_notes, terpene_notes, effect_notes,
     heat_tolerance, cold_tolerance, light_tolerance, user_rating,
     notes, evidence, source_url, source_licence, is_builtin,
     is_phenotype_of, created_at, updated_at)
SELECT 'seed-strain-special-queen-1', name, breeder, photoperiod, sex, indica_pct,
       landrace_tags, flower_days_min, flower_days_max, stretch_factor,
       veg_weeks_min, veg_weeks_max, height_class, auto_total_days_min,
       auto_total_days_max, yield_notes, terpene_notes, effect_notes,
       heat_tolerance, cold_tolerance, light_tolerance, user_rating,
       notes, evidence, source_url, source_licence, 1,
       is_phenotype_of, created_at, updated_at
FROM strain
WHERE name = 'Special Queen #1' AND breeder = 'Royal Queen Seeds'
  AND (SELECT COUNT(*) FROM strain WHERE name = 'Special Queen #1' AND breeder = 'Royal Queen Seeds') = 1;

UPDATE grow
SET strain_id = 'seed-strain-special-queen-1'
WHERE EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-special-queen-1')
  AND strain_id IN (SELECT id FROM strain WHERE name = 'Special Queen #1' AND breeder = 'Royal Queen Seeds');

UPDATE strain
SET is_phenotype_of = 'seed-strain-special-queen-1'
WHERE EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-special-queen-1')
  AND is_phenotype_of IN (SELECT id FROM strain WHERE name = 'Special Queen #1' AND breeder = 'Royal Queen Seeds');

DELETE FROM strain
WHERE name = 'Special Queen #1' AND breeder = 'Royal Queen Seeds' AND id <> 'seed-strain-special-queen-1'
  AND EXISTS(SELECT 1 FROM strain WHERE id = 'seed-strain-special-queen-1');
