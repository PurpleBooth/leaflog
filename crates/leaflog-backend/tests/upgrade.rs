//! Bundled-data upgrade acceptance tests (spec §4.1, §13): on a
//! simulated upgrade, new bundled rows are inserted, existing bundled
//! rows are adopted onto their stable ids, and user entries are never
//! touched. The strain catalogue part of the chain ends in V17's drop:
//! the adopted strain link surfaces as the grow's derived label.

use std::path::PathBuf;

use leaflog_backend::db::{self, Db, StoreError};
use leaflog_backend::seed;
use refinery::Target;
use rusqlite::Connection;

/// A fresh temp directory holding one database file.
struct TempDb {
    dir: PathBuf,
}

impl TempDb {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("leaflog-upgrade-{tag}-{}", db::new_id()));
        std::fs::create_dir_all(&dir).expect("temp dir creates");
        Self { dir }
    }

    fn path(&self) -> PathBuf {
        self.dir.join("leaflog.db")
    }
}

impl Drop for TempDb {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

/// Builds a "pre-stable-id" database at schema V7: bundled rows seeded
/// with random ULIDs (the pre-V8 seeder's behaviour), a grow on the
/// bundled strain, and a grow space referencing the bundled tent. The
/// clip fan is absent — as if the user had deleted it back when
/// builtins were deletable.
fn pre_upgrade_database(path: &std::path::Path) {
    let mut conn = Connection::open(path).expect("open pre-upgrade db");
    conn.pragma_update(None, "foreign_keys", "ON")
        .expect("pragmas");
    db::embedded::migrations::runner()
        .set_target(Target::Version(7))
        .run(&mut conn)
        .expect("schema through V7");

    conn.execute_batch(
        "INSERT INTO strain (id, name, breeder, photoperiod, sex, indica_pct,
             landrace_tags, flower_days_min, flower_days_max, stretch_factor,
             veg_weeks_min, veg_weeks_max, height_class, auto_total_days_min,
             auto_total_days_max, yield_notes, terpene_notes, effect_notes,
             heat_tolerance, cold_tolerance, light_tolerance, user_rating,
             notes, evidence, source_url, source_licence, is_builtin,
             is_phenotype_of, created_at, updated_at)
         VALUES
             ('01STRAINNORTHERN0000000', 'Northern Lights', 'Sensi Seeds',
              'photoperiod', 'feminized', 90, '[]', 45, 55, 1.5,
              NULL, NULL, 'short', NULL, NULL, NULL, NULL, NULL,
              'average', 'average', 'average', NULL, NULL, '{}',
              'https://sensiseeds.com', 'breeder catalogue', 1,
              NULL, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');

         INSERT INTO equipment_item (id, name, category, brand_model,
             real_watts, dimmable, purchase_date, purchase_price, currency,
             notes, source, is_builtin, created_at, updated_at)
         VALUES
             ('01EQUIPTENTULID00000000', 'Grow tent 60x60x180', 'tent_room',
              'Spider Farmer 60x60x180 cm', NULL, 0, NULL, NULL, NULL,
              'user-renamed note', NULL, 0,
              '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'),
             ('01EQUIPUSERLIGHT0000000', 'Borrowed desk lamp', 'light',
              NULL, 40, 0, NULL, NULL, NULL, NULL, NULL, 0,
              '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');

         INSERT INTO grow_space (id, name, width_cm, depth_cm, height_cm,
             notes, created_at, updated_at)
         VALUES ('01SPACETENT00000000000', 'The tent', 60, 60, 180, NULL,
                 '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');

         INSERT INTO grow_space_equipment (grow_space_id, equipment_id) VALUES
             ('01SPACETENT00000000000', '01EQUIPTENTULID00000000');

         INSERT INTO grow (id, name, strain_id, grow_space_id, substrate,
             started_on, status, notes, created_at, updated_at)
         VALUES ('01GROWNORTHERN000000000', 'NL run',
                 '01STRAINNORTHERN0000000', NULL, 'soil', '2026-09-01',
                 'active', NULL, '2026-09-01T00:00:00Z', '2026-09-01T00:00:00Z');",
    )
    .expect("pre-upgrade fixtures");
}

#[tokio::test]
async fn upgrade_adopts_reids_and_inserts_without_touching_user_data() {
    let temp = TempDb::new("adopt");
    let path = temp.path();
    pre_upgrade_database(&path);

    // The upgrade itself: boot applies V8 onward (V17 drops the strain
    // catalogue the adoption repointed), then the seeder runs (the
    // same order `serve` uses).
    let db = Db::open(&path).expect("boot applies the migration chain");
    let seeded_equipment = seed::seed_equipment(&db).await.expect("equipment seeder");

    // "New" means new under the stable id: the tent was already present
    // (adopted), everything else came in fresh.
    assert_eq!(seeded_equipment, 4);

    db.with(|conn| -> Result<(), StoreError> {
        // The bundled tent was adopted (id and read-only flag) and kept
        // its user-edited note; the space reference followed.
        let (id, note, builtin): (String, String, i64) = conn.query_row(
            "SELECT id, notes, is_builtin FROM equipment_item WHERE name = 'Grow tent 60x60x180'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        assert_eq!(id, "seed-equip-grow-tent-60x60x180");
        assert_eq!(note, "user-renamed note", "user edits survive upgrades");
        assert_eq!(builtin, 1);
        let space_ref: i64 = conn.query_row(
            "SELECT COUNT(*) FROM grow_space_equipment
             WHERE grow_space_id = '01SPACETENT00000000000'
               AND equipment_id = 'seed-equip-grow-tent-60x60x180'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(space_ref, 1, "the space reference was repointed");

        // The clip fan was missing pre-upgrade; the seeder inserted it
        // (bundled rows that are new).
        let fan: i64 = conn.query_row(
            "SELECT COUNT(*) FROM equipment_item WHERE id = 'seed-equip-e6-clip-fan-150-mm'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(fan, 1);

        // The user's light is untouched: same id, still a user entry.
        let (id, builtin): (String, i64) = conn.query_row(
            "SELECT id, is_builtin FROM equipment_item WHERE name = 'Borrowed desk lamp'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        assert_eq!(id, "01EQUIPUSERLIGHT0000000");
        assert_eq!(builtin, 0);

        // The strain table is gone (V17) — but the grow it anchored
        // survives, carrying the strain's name as its derived label.
        // The label could only come through the full chain: V8 adopted
        // the grow onto the bundled strain's stable id, V17 backfilled
        // label and photoperiod from that link, then dropped the table.
        let strain_table: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'strain')",
            [],
            |row| row.get(0),
        )?;
        assert!(!strain_table, "the strain catalogue is dropped");
        let (name, label, photoperiod): (String, Option<String>, Option<String>) = conn.query_row(
            "SELECT name, strain_label, photoperiod FROM grow
             WHERE id = '01GROWNORTHERN000000000'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        assert_eq!(name, "NL run");
        assert_eq!(label.as_deref(), Some("Northern Lights"));
        assert_eq!(photoperiod.as_deref(), Some("photoperiod"));
        Ok(())
    })
    .await
    .expect("upgrade assertions");

    // And the whole thing is idempotent: a second boot seeds nothing.
    let again_equipment = seed::seed_equipment(&db).await.expect("re-seed equipment");
    assert_eq!(again_equipment, 0);
}
