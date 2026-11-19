//! Bundled-data upgrade acceptance tests (spec §13): on a simulated
//! upgrade, the pre-stable-id rows are adopted onto their stable ids
//! without touching user data — and the adopted strain link then
//! surfaces as the grow's derived label when V17 drops the catalogue.

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
/// strain. (The bundled equipment catalogue left the product with V16,
/// so the upgrade scenario covers the strain catalogue only.)
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
async fn upgrade_adopts_reids_and_the_drop_surfaces_the_label() {
    let temp = TempDb::new("adopt");
    let path = temp.path();
    pre_upgrade_database(&path);

    // Stage one of the upgrade: apply V8 through V16 only. The bundled
    // strain was adopted onto its stable id (exactly one row — no
    // duplicate), the clone's parent link and the grow's reference
    // followed, and adoption marked it read-only.
    {
        let mut conn = Connection::open(&path).expect("reopen pre-drop db");
        conn.pragma_update(None, "foreign_keys", "ON")
            .expect("pragmas");
        db::embedded::migrations::runner()
            .set_target(Target::Version(16))
            .run(&mut conn)
            .expect("apply through the last pre-drop migration");

        let (id, builtin): (String, i64) = conn
            .query_row(
                "SELECT id, is_builtin FROM strain WHERE name = 'Northern Lights'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("adopted bundled strain");
        assert_eq!(id, "seed-strain-northern-lights");
        assert_eq!(builtin, 1, "adopted rows become read-only presets");
        let northern: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM strain WHERE name = 'Northern Lights'",
                [],
                |row| row.get(0),
            )
            .expect("adoption count");
        assert_eq!(northern, 1, "no duplicate after adoption");
        let parent: String = conn
            .query_row(
                "SELECT is_phenotype_of FROM strain WHERE id = '01STRAINMYPHENO00000000'",
                [],
                |row| row.get(0),
            )
            .expect("clone parent link");
        assert_eq!(parent, "seed-strain-northern-lights");
        let grow_strain: String = conn
            .query_row(
                "SELECT strain_id FROM grow WHERE id = '01GROWNORTHERN000000000'",
                [],
                |row| row.get(0),
            )
            .expect("grow reference repointed");
        assert_eq!(grow_strain, "seed-strain-northern-lights");
    }

    // Stage two: a plain boot applies V17. The catalogue drops — but
    // the grow keeps its identity: the label and photoperiod are
    // backfilled from the adopted link before the table goes.
    let db = Db::open(&path).expect("boot applies the strain drop");
    db.with(|conn| -> Result<(), StoreError> {
        let strain_table: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'strain')",
            [],
            |row| row.get(0),
        )?;
        assert!(!strain_table, "the strain catalogue is dropped");
        let (name, label, photoperiod): (String, Option<String>, Option<String>) = conn
            .query_row(
                "SELECT name, strain_label, photoperiod FROM grow
                 WHERE id = '01GROWNORTHERN000000000'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("the upgraded grow survives");
        assert_eq!(name, "NL run");
        assert_eq!(label.as_deref(), Some("Northern Lights"));
        assert_eq!(photoperiod.as_deref(), Some("photoperiod"));
        Ok(())
    })
    .await
    .expect("upgrade assertions");

    // First boot after the upgrade seeds the one thing that is still
    // bundled data: the substrate/feed presets.
    seed::seed_feed_presets(&db).await.expect("feed presets");
    let profiles: i64 = db
        .with(|conn| {
            conn.query_row("SELECT COUNT(*) FROM substrate_profile", [], |r| r.get(0))
                .map_err(StoreError::from)
        })
        .await
        .expect("preset count");
    assert!(profiles >= 3, "the presets seeded once: {profiles}");
    // The feeding-schedule library ships empty (ADR 0004): the bundled
    // grow data is substrate guidance only.
    let schedules: i64 = db
        .with(|conn| {
            conn.query_row("SELECT COUNT(*) FROM feed_schedule", [], |r| r.get(0))
                .map_err(StoreError::from)
        })
        .await
        .expect("schedule count");
    assert_eq!(schedules, 0, "the schedule library ships empty");
}
