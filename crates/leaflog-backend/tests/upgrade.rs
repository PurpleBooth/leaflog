//! Bundled-data upgrade acceptance tests (spec §4.4, §13): on a
//! simulated upgrade, new bundled rows are inserted and existing bundled
//! rows are adopted onto their stable ids without touching user data.
//! (The bundled equipment catalogue left the product with V16, so the
//! upgrade scenario covers the strain catalogue only.)

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

/// Builds a "pre-stable-id" database at schema V7: bundled strain rows
/// seeded with random ULIDs (the pre-V8 seeder's behaviour), a user
/// phenotype clone pointing at one of them, and a grow on the bundled
/// strain.
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
              NULL, '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'),
             ('01STRAINMYPHENO00000000', 'Northern Lights (my phenotype)',
              'Sensi Seeds', 'photoperiod', 'feminized', 90, '[]', 50, 58, 1.4,
              NULL, NULL, 'short', NULL, NULL, NULL, NULL, NULL,
              'average', 'average', 'average', 4, 'keeps growing',
              '{}', NULL, NULL, 0,
              '01STRAINNORTHERN0000000', '2026-01-02T00:00:00Z', '2026-01-02T00:00:00Z');

         INSERT INTO grow (id, name, strain_id, substrate,
             started_on, status, notes, created_at, updated_at)
         VALUES ('01GROWNORTHERN000000000', 'NL run',
                 '01STRAINNORTHERN0000000', 'soil', '2026-09-01',
                 'active', NULL, '2026-09-01T00:00:00Z', '2026-09-01T00:00:00Z');",
    )
    .expect("pre-upgrade fixtures");
}

#[tokio::test]
async fn upgrade_adopts_reids_and_inserts_without_touching_user_data() {
    let temp = TempDb::new("adopt");
    let path = temp.path();
    pre_upgrade_database(&path);

    // The upgrade itself: boot applies V8, then the seeder runs (the
    // same order `serve` uses).
    let db = Db::open(&path).expect("boot applies V8");
    let seeded_strains = seed::seed_strains(&db).await.expect("strain seeder");

    // "New" means new under the stable id: Northern Lights was already
    // present (adopted), everything else came in fresh.
    assert_eq!(seeded_strains, 14);

    db.with(|conn| -> Result<(), StoreError> {
        // The bundled strain was adopted onto its stable id...
        let (id, builtin): (String, i64) = conn.query_row(
            "SELECT id, is_builtin FROM strain WHERE name = 'Northern Lights'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        assert_eq!(id, "seed-strain-northern-lights");
        assert_eq!(builtin, 1, "adopted rows become read-only presets");
        // ...exactly one such row, and the clone's parent link followed.
        let northern: i64 = conn.query_row(
            "SELECT COUNT(*) FROM strain WHERE name = 'Northern Lights'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(northern, 1, "no duplicate after adoption");
        let parent: String = conn.query_row(
            "SELECT is_phenotype_of FROM strain WHERE id = '01STRAINMYPHENO00000000'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(parent, "seed-strain-northern-lights");
        // The grow's strain reference followed too.
        let grow_strain: String = conn.query_row(
            "SELECT strain_id FROM grow WHERE id = '01GROWNORTHERN000000000'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(grow_strain, "seed-strain-northern-lights");

        // Every other bundled strain inserted as new.
        let bundled_strains: i64 = conn.query_row(
            "SELECT COUNT(*) FROM strain WHERE is_builtin = 1",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(bundled_strains, 15);
        Ok(())
    })
    .await
    .expect("upgrade assertions");

    // And the whole thing is idempotent: a second boot seeds nothing.
    let again_strains = seed::seed_strains(&db).await.expect("re-seed strains");
    assert_eq!(again_strains, 0);
}
