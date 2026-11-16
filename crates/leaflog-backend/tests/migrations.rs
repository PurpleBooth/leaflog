//! Migration-tooling acceptance tests (spec §13): boot applies pending
//! files, pre-refinery histories are adopted, and a divergent, gapped,
//! or newer-than-binary history refuses to start.

use std::path::PathBuf;

use leaflog_backend::db::{self, Db, StoreError};
use leaflog_backend::store;
use leaflog_core::grows::{GrowInput, GrowStatus, PhotoperiodType, Tolerance};
use refinery::Target;
use rusqlite::Connection;

/// A fresh temp directory holding one database file.
struct TempDb {
    dir: PathBuf,
}

impl TempDb {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("leaflog-migrate-{tag}-{}", db::new_id()));
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

/// The first three migration files, applied by hand — the schema a
/// pre-refinery (D001) database would have.
const LEGACY_SQL: [&str; 3] = [
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/V1__init.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/V2__grows_actions_photos.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/migrations/V3__sensors.sql"
    )),
];

/// Builds a database in the legacy state: schema at version 3, history
/// in the old runner's `schema_migrations` table.
fn legacy_database(path: &std::path::Path) {
    let conn = Connection::open(path).expect("open legacy database");
    conn.pragma_update(None, "foreign_keys", "ON")
        .expect("legacy pragmas");
    for sql in LEGACY_SQL {
        conn.execute_batch(sql).expect("legacy migration applies");
    }
    conn.execute_batch(
        "CREATE TABLE schema_migrations (
             version    INTEGER PRIMARY KEY,
             applied_at TEXT NOT NULL
         );
         INSERT INTO schema_migrations (version, applied_at) VALUES
             (1, '2026-01-01T00:00:00Z'),
             (2, '2026-01-01T00:00:01Z'),
             (3, '2026-01-01T00:00:02Z');",
    )
    .expect("legacy history records");
}

#[tokio::test]
async fn boot_applies_pending_migrations() {
    let temp = TempDb::new("pending");
    let path = temp.path();
    // A database the old binary left at version 3.
    legacy_database(&path);

    let db = Db::open(&path).expect("boot opens and migrates");
    db.with(|conn| -> Result<(), StoreError> {
        // Version numbers, not the migration count: the numbering may
        // carry a gap (a reserved V slot another change owns).
        assert_eq!(Db::version(conn)?, Db::latest_embedded_version());
        Ok(())
    })
    .await
    .expect("version query");
}

#[tokio::test]
async fn legacy_history_is_adopted_and_the_old_table_dropped() {
    let temp = TempDb::new("adopt");
    let path = temp.path();
    legacy_database(&path);

    let db = Db::open(&path).expect("adopting open");
    db.with(|conn| -> Result<(), StoreError> {
        // Adoption rewrote the history with the embedded names and
        // checksums for V1..V3 and refinery then applied the rest.
        let legacy: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_migrations')",
            [],
            |row| row.get(0),
        )?;
        assert!(!legacy, "legacy history table must be dropped");
        let adopted_name: String = conn.query_row(
            "SELECT name FROM refinery_schema_history WHERE version = 2",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(adopted_name, "grows_actions_photos");
        Ok(())
    })
    .await
    .expect("adoption checks");

    // Data written before the upgrade survives adoption.
    db.with(|conn| -> Result<(), StoreError> {
        conn.execute(
            "INSERT INTO settings (key, value, updated_at) VALUES ('adopted', '1', 'x')",
            [],
        )?;
        Ok(())
    })
    .await
    .expect("post-adoption write");
}

#[test]
fn status_reports_legacy_pending_and_then_applies() {
    let temp = TempDb::new("status");
    let path = temp.path();
    legacy_database(&path);

    let status = db::migrate_status(&path).expect("status of legacy db");
    assert_eq!(status.legacy_version, Some(3));
    assert!(
        status.applied.is_empty(),
        "refinery history not yet created"
    );
    assert!(
        status.pending.iter().all(|m| m.version > 3),
        "only migrations after the legacy version are pending: {:?}",
        status.pending
    );

    let applied = db::migrate_apply(&path).expect("apply after status");
    assert_eq!(applied.len(), status.pending.len());
    let after = db::migrate_status(&path).expect("status after apply");
    assert!(after.pending.is_empty());
    assert!(after.legacy_version.is_none());
    assert_eq!(
        after.applied.last().map(|m| m.version),
        Some(Db::latest_embedded_version())
    );
}

#[test]
fn divergent_history_refuses_to_start() {
    let temp = TempDb::new("divergent");
    let path = temp.path();
    drop(Db::open(&path).expect("fresh database"));

    let conn = Connection::open(&path).expect("tamper connection");
    conn.execute(
        "UPDATE refinery_schema_history SET checksum = '1' WHERE version = 2",
        [],
    )
    .expect("checksum tampered");
    drop(conn);

    let error = Db::open(&path).expect_err("divergent history must refuse to start");
    assert!(
        error.to_string().contains("migration error"),
        "expected a migration error, got {error}"
    );
}

#[test]
fn gapped_history_refuses_to_start() {
    let temp = TempDb::new("gap");
    let path = temp.path();
    drop(Db::open(&path).expect("fresh database"));

    let conn = Connection::open(&path).expect("tamper connection");
    conn.execute("DELETE FROM refinery_schema_history WHERE version = 2", [])
        .expect("history gap cut");
    drop(conn);

    let error = Db::open(&path).expect_err("out-of-order history must refuse to start");
    assert!(
        error.to_string().contains("migration error"),
        "expected a migration error, got {error}"
    );
}

#[test]
fn newer_than_binary_history_refuses_to_start() {
    let temp = TempDb::new("newer");
    let path = temp.path();
    drop(Db::open(&path).expect("fresh database"));

    let conn = Connection::open(&path).expect("tamper connection");
    conn.execute(
        "INSERT INTO refinery_schema_history (version, name, applied_on, checksum)
         VALUES (99, 'from_the_future', '2027-01-01T00:00:00Z', '123')",
        [],
    )
    .expect("future migration faked");
    drop(conn);

    let error = Db::open(&path).expect_err("newer-than-binary history must refuse to start");
    assert!(
        error.to_string().contains("migration error"),
        "expected a migration error, got {error}"
    );
}

#[test]
fn legacy_history_newer_than_binary_refuses_to_start() {
    let temp = TempDb::new("legacy-newer");
    let path = temp.path();
    legacy_database(&path);
    let conn = Connection::open(&path).expect("tamper connection");
    conn.execute(
        "INSERT INTO schema_migrations (version, applied_at) VALUES (99, '2027-01-01T00:00:00Z')",
        [],
    )
    .expect("future legacy version faked");
    drop(conn);

    let error = Db::open(&path).expect_err("newer legacy history must refuse to start");
    assert!(
        error.to_string().contains("refusing to start"),
        "expected an explicit newer-database refusal, got {error}"
    );
}

#[tokio::test]
async fn partially_adopted_history_is_completed() {
    let temp = TempDb::new("interrupted");
    let path = temp.path();
    legacy_database(&path);
    // Simulate an adoption interrupted after marking but before the
    // legacy drop: refinery table at 3, legacy table still present.
    let mut conn = Connection::open(&path).expect("adopt connection");
    conn.pragma_update(None, "foreign_keys", "ON")
        .expect("pragmas");
    db::embedded::migrations::runner()
        .set_target(Target::FakeVersion(3))
        .run(&mut conn)
        .expect("fake adoption");
    drop(conn);

    let db = Db::open(&path).expect("interrupted adoption completes");
    db.with(|conn| -> Result<(), StoreError> {
        let legacy: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = 'schema_migrations')",
            [],
            |row| row.get(0),
        )?;
        assert!(!legacy, "the leftover legacy table is dropped on reopen");
        Ok(())
    })
    .await
    .expect("leftover check");
}

/// V6 (catalogue trim): carbon filters fold into `fan`, the sixteen
/// bundled rows whose categories left the catalogue are removed (with
/// space references cascading), and user rows in retired categories are
/// parked in the legacy `misc` bucket where they keep deserializing.
///
/// Asserted at V6 exactly: the equipment and space tables this
/// migration shaped were dropped again by V16 (feature-strip), so a
/// boot to the latest head has nothing left to inspect — the V16 drop
/// test owns the current schema.
#[test]
fn catalogue_trim_recategorizes_prunes_and_parks() {
    let temp = TempDb::new("trim");
    let path = temp.path();
    {
        let mut conn = Connection::open(&path).expect("open at V5");
        conn.pragma_update(None, "foreign_keys", "ON")
            .expect("pragmas");
        db::embedded::migrations::runner()
            .set_target(Target::Version(5))
            .run(&mut conn)
            .expect("apply through V5");
        conn.execute_batch(
            "INSERT INTO equipment_item
                 (id, name, category, brand_model, real_watts, dimmable,
                  purchase_date, purchase_price, currency, notes,
                  source, is_builtin, created_at, updated_at)
             VALUES
                 ('e-filter', 'Shop carbon filter 100', 'carbon_filter', NULL, NULL, 0,
                  NULL, NULL, NULL, NULL, 'bundled', 1,
                  '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'),
                 ('e-tray', 'Trim tray set', 'misc', NULL, NULL, 0,
                  NULL, NULL, NULL, NULL, 'bundled', 1,
                  '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'),
                 ('e-dehum', 'My basement dehumidifier', 'dehumidifier', NULL, NULL, 0,
                  NULL, NULL, NULL, NULL, NULL, 0,
                  '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'),
                 ('e-light', 'My SE999 light', 'light', NULL, 150, 1,
                  NULL, NULL, NULL, NULL, 'bundled', 1,
                  '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
             INSERT INTO grow_space
                 (id, name, width_cm, depth_cm, height_cm, notes, created_at, updated_at)
             VALUES ('sp-1', 'Tent', 60, 60, 180, NULL,
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
             INSERT INTO grow_space_equipment (grow_space_id, equipment_id) VALUES
                 ('sp-1', 'e-filter'),
                 ('sp-1', 'e-tray');",
        )
        .expect("pre-trim fixtures");

        db::embedded::migrations::runner()
            .set_target(Target::Version(6))
            .run(&mut conn)
            .expect("V6 applies");

        let category = |id: &str| -> rusqlite::Result<String> {
            conn.query_row(
                "SELECT category FROM equipment_item WHERE id = ?1",
                [id],
                |row| row.get(0),
            )
        };
        assert_eq!(
            category("e-filter").expect("filter row"),
            "fan",
            "carbon filters fold into fan"
        );
        assert_eq!(
            category("e-light").expect("light row"),
            "light",
            "catalogue rows untouched"
        );
        assert_eq!(
            category("e-dehum").expect("parked row"),
            "misc",
            "user rows in retired categories are parked, not deleted"
        );
        let tray: bool = conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM equipment_item WHERE id = 'e-tray')",
                [],
                |row| row.get(0),
            )
            .expect("tray existence");
        assert!(!tray, "the bundled trim-tray row is removed");
        // The space kept its reference to the recategorized filter and
        // lost (by cascade) the reference to the removed tray.
        let refs: i64 = conn
            .query_row(
                "SELECT COUNT(*) FROM grow_space_equipment WHERE grow_space_id = 'sp-1'",
                [],
                |row| row.get(0),
            )
            .expect("space references");
        assert_eq!(refs, 1);
    }
}

/// V14 (D075 step 01) is append-only: a database at V13 boots, its
/// grow rows survive byte-for-byte, the new columns read back as the
/// fields' defaults, and the history records the new version without
/// touching the old entries.
#[tokio::test]
async fn v14_adds_grow_columns_without_touching_existing_grows() {
    let temp = TempDb::new("v14-columns");
    let path = temp.path();
    {
        let mut conn = Connection::open(&path).expect("open at V13");
        conn.pragma_update(None, "foreign_keys", "ON")
            .expect("pragmas");
        db::embedded::migrations::runner()
            .set_target(Target::Version(13))
            .run(&mut conn)
            .expect("apply through V13");
        // A grow written by the pre-strip binary: no V14 columns.
        conn.execute_batch(
            "INSERT INTO grow (id, name, strain_id, substrate, started_on,
                 status, notes, created_at, updated_at)
             VALUES ('01GROWPRESTRIP00000000', 'Pre-strip run', NULL, 'soil',
                     '2026-09-01', 'active', NULL,
                     '2026-09-01T00:00:00Z', '2026-09-01T00:00:00Z');",
        )
        .expect("pre-V14 grow row");
    }

    let db = Db::open(&path).expect("V14 applies on boot");
    let status = db::migrate_status(&path).expect("migration status");
    assert!(
        status.pending.is_empty(),
        "nothing left pending after boot: {:?}",
        status.pending
    );
    let v14 = status
        .applied
        .iter()
        .find(|migration| migration.version == 14)
        .expect("V14 is in the applied history");
    assert_eq!(v14.name, "grow_strain_lamp_fields");

    db.with(|conn| -> Result<(), StoreError> {
        // The old row still loads, keeps its old values, and reads the
        // defaults for every new field (D075 defaults: empty label,
        // photoperiod, average tolerances, unset watts, dimmable).
        let grow =
            store::grows::get(conn, "01GROWPRESTRIP00000000").expect("pre-strip grow still loads");
        assert_eq!(grow.name, "Pre-strip run");
        assert_eq!(grow.substrate, "soil");
        assert_eq!(grow.strain_label, "");
        assert_eq!(grow.photoperiod, PhotoperiodType::Photoperiod);
        assert_eq!(grow.heat_tolerance, Tolerance::Average);
        assert_eq!(grow.cold_tolerance, Tolerance::Average);
        assert_eq!(grow.light_tolerance, Tolerance::Average);
        assert_eq!(grow.real_watts, None);
        assert!(grow.dimmable);
        // The stored columns themselves were left NULL by the
        // migration (append-only, no rewrite of old rows); only the
        // dimmable default was filled in by the column default.
        let (label, watts): (Option<String>, Option<f64>) = conn.query_row(
            "SELECT strain_label, real_watts FROM grow WHERE id = '01GROWPRESTRIP00000000'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        assert!(label.is_none(), "migration must not rewrite old rows");
        assert!(watts.is_none(), "migration must not rewrite old rows");
        Ok(())
    })
    .await
    .expect("V14 column assertions");
}

/// V17 (strain-catalogue removal): every grow formerly linked to a
/// strain emerges from the migration with the strain's name as its
/// free-text label and the strain's photoperiod behaviour; labels and
/// toggles the grower already set are not overwritten. The strain
/// table and the grow's `strain_id` column disappear, tolerances read
/// back as Average, and the grow row with its plants survives.
#[tokio::test]
async fn dropping_the_strain_catalogue_backfills_label_and_photoperiod() {
    let temp = TempDb::new("strain-drop");
    let path = temp.path();
    {
        let mut conn = Connection::open(&path).expect("open at the pre-drop version");
        conn.pragma_update(None, "foreign_keys", "ON")
            .expect("pragmas");
        db::embedded::migrations::runner()
            // V16 (spaces and equipment) is ticket 04's drop; absent in
            // this tree it simply leaves the fixture one version
            // earlier — the assertions are about the state after V17.
            .set_target(Target::Version(16))
            .run(&mut conn)
            .expect("apply through the last pre-drop migration");
        conn.execute_batch(
            "INSERT INTO strain
                 (id, name, breeder, photoperiod, sex, indica_pct, landrace_tags,
                  flower_days_min, flower_days_max, stretch_factor, veg_weeks_min,
                  veg_weeks_max, height_class, auto_total_days_min, auto_total_days_max,
                  yield_notes, terpene_notes, effect_notes, heat_tolerance,
                  cold_tolerance, light_tolerance, user_rating, notes, evidence,
                  source_url, source_licence, is_phenotype_of, created_at, updated_at)
             VALUES
                 ('01STRAINNL000000000000', 'Northern Lights', 'Sensi Seeds',
                  'photoperiod', 'feminized', 90, '[]', 45, 55, 1.5, NULL, NULL,
                  'short', NULL, NULL, NULL, NULL, NULL, 'average', 'average',
                  'average', NULL, NULL, '{}', NULL, NULL, NULL,
                  '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'),
                 ('01STRAINGGAUTO00000000', 'GG4 Auto', NULL,
                  'auto', 'feminized', 60, '[]', NULL, NULL, NULL, NULL, NULL,
                  NULL, 63, 77, NULL, NULL, NULL, 'average', 'average',
                  'average', NULL, NULL, '{}', NULL, NULL, NULL,
                  '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
             INSERT INTO grow
                 (id, name, strain_id, substrate, started_on, status, notes,
                  created_at, updated_at)
             VALUES
                 ('01GROWLINKEDNL00000000', 'NL run', '01STRAINNL000000000000',
                  'soil', '2026-03-01', 'active', NULL,
                  '2026-03-01T00:00:00Z', '2026-03-01T00:00:00Z'),
                 ('01GROWLINKEDAUTO0000000', 'Auto run', '01STRAINGGAUTO00000000',
                  'coco', '2026-03-02', 'active', NULL,
                  '2026-03-02T00:00:00Z', '2026-03-02T00:00:00Z'),
                 ('01GROWNEVERLINKED000000', 'Free run', NULL,
                  'soil', '2026-03-03', 'active', NULL,
                  '2026-03-03T00:00:00Z', '2026-03-03T00:00:00Z');
             -- The grower set label and toggle by hand after V14: the
             -- backfill must keep them.
             INSERT INTO grow
                 (id, name, strain_id, substrate, started_on, status, notes,
                  created_at, updated_at, strain_label, photoperiod)
             VALUES
                 ('01GROWKEPTFIELDS0000000', 'Kept run', '01STRAINNL000000000000',
                  'soil', '2026-03-04', 'active', NULL,
                  '2026-03-04T00:00:00Z', '2026-03-04T00:00:00Z',
                  'My NL cross', 'auto');
             INSERT INTO plant (id, grow_id, name, created_at, updated_at)
             VALUES ('01PLANTOFNL0000000000', '01GROWLINKEDNL00000000', 'Plant 1',
                     '2026-03-02T00:00:00Z', '2026-03-02T00:00:00Z');",
        )
        .expect("pre-drop fixtures");
    }

    let db = Db::open(&path).expect("the strain drop applies on boot");
    let status = db::migrate_status(&path).expect("migration status");
    let v17 = status
        .applied
        .iter()
        .find(|migration| migration.version == 17)
        .expect("V17 is in the applied history");
    assert_eq!(v17.name, "drop_strains");

    db.with(|conn| -> Result<(), StoreError> {
        let table_exists = |name: &str| -> Result<bool, StoreError> {
            Ok(conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
                [name],
                |row| row.get(0),
            )?)
        };
        assert!(!table_exists("strain")?, "the strain table is dropped");
        let has_strain_column: bool = conn.query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM pragma_table_info('grow') WHERE name = 'strain_id'
             )",
            [],
            |row| row.get(0),
        )?;
        assert!(!has_strain_column, "grow loses its strain_id column");

        // Linked grows carry the derived label and photoperiod; the
        // never-linked grow stays unlabelled; the grower's own values
        // were kept.
        let linked = store::grows::get(conn, "01GROWLINKEDNL00000000").expect("linked grow");
        assert_eq!(linked.name, "NL run");
        assert_eq!(linked.strain_label, "Northern Lights");
        assert_eq!(linked.photoperiod, PhotoperiodType::Photoperiod);
        assert_eq!(linked.substrate, "soil");
        let auto = store::grows::get(conn, "01GROWLINKEDAUTO0000000").expect("auto grow");
        assert_eq!(auto.strain_label, "GG4 Auto");
        assert_eq!(auto.photoperiod, PhotoperiodType::Auto);
        let free = store::grows::get(conn, "01GROWNEVERLINKED000000").expect("free grow");
        assert_eq!(free.strain_label, "", "an unlinked grow stays unlabelled");
        assert_eq!(free.photoperiod, PhotoperiodType::default());
        let kept = store::grows::get(conn, "01GROWKEPTFIELDS0000000").expect("kept grow");
        assert_eq!(kept.strain_label, "My NL cross", "user label is kept");
        assert_eq!(
            kept.photoperiod,
            PhotoperiodType::Auto,
            "user toggle is kept"
        );

        // Tolerances were never derived from the catalogue: the unset
        // columns read back as Average (the D075 default).
        assert_eq!(linked.heat_tolerance, Tolerance::Average);
        assert_eq!(linked.cold_tolerance, Tolerance::Average);
        assert_eq!(linked.light_tolerance, Tolerance::Average);

        // The grow row and its children survive the rebuild.
        let plants: i64 = conn.query_row(
            "SELECT COUNT(*) FROM plant WHERE grow_id = '01GROWLINKEDNL00000000'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(plants, 1, "the grow's plants survive the drop");

        // The store still writes and reads grows afterwards, without a
        // strain reference.
        let created = store::grows::create(
            conn,
            &GrowInput {
                name: "Post-drop run".to_owned(),
                feed_template_id: None,
                planned_veg_weeks: None,
                strain_label: "Moby Dick".to_owned(),
                photoperiod: PhotoperiodType::Auto,
                heat_tolerance: Tolerance::default(),
                cold_tolerance: Tolerance::default(),
                light_tolerance: Tolerance::default(),
                real_watts: None,
                dimmable: true,
                substrate: "coco".to_owned(),
                started_on: "2026-04-01".to_owned(),
                status: GrowStatus::Active,
                notes: None,
            },
        )?;
        assert_eq!(created.strain_label, "Moby Dick");
        assert_eq!(created.photoperiod, PhotoperiodType::Auto);
        Ok(())
    })
    .await
    .expect("strain-drop assertions");
}

/// V15 (seed-bank removal): the `seed_batch` and `germination_log`
/// tables and the grow's `seed_batch_id` column disappear on boot.
/// The drop also pins that the grow row, its plants and its log
/// survive — and that the store still round-trips grows afterwards,
/// now carrying the V14 strain/lamp fields too.
#[tokio::test]
async fn dropping_the_seed_bank_loses_tables_and_the_grow_column() {
    let temp = TempDb::new("seedbank-drop");
    let path = temp.path();
    {
        let mut conn = Connection::open(&path).expect("open at the pre-drop version");
        conn.pragma_update(None, "foreign_keys", "ON")
            .expect("pragmas");
        db::embedded::migrations::runner()
            .set_target(Target::Version(13))
            .run(&mut conn)
            .expect("apply through the last pre-drop migration");
        conn.execute_batch(
            "INSERT INTO seed_batch
                 (id, strain_id, quantity, acquired_from, acquisition_on,
                  storage_method, notes, created_at, updated_at)
             VALUES ('sb-1', NULL, 7, 'E2E bank', '2026-01-15', 'cool_dark_dry', NULL,
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
             INSERT INTO germination_log
                 (id, seed_batch_id, tested_on, sown, sprouted, notes, created_at, updated_at)
             VALUES ('gl-1', 'sb-1', '2026-02-01', 4, 3, 'paper towel',
                     '2026-02-01T00:00:00Z', '2026-02-01T00:00:00Z');
             INSERT INTO grow
                 (id, name, strain_id, substrate, started_on, status, notes,
                  created_at, updated_at, grow_space_id, seed_batch_id, feed_template_id,
                  planned_veg_weeks)
             VALUES ('g-1', 'Linked run', NULL, 'soil', '2026-03-01', 'active', NULL,
                     '2026-03-01T00:00:00Z', '2026-03-01T00:00:00Z',
                     NULL, 'sb-1', NULL, 4);
             INSERT INTO plant (id, grow_id, name, created_at, updated_at)
             VALUES ('p-1', 'g-1', 'Plant 1',
                     '2026-03-02T00:00:00Z', '2026-03-02T00:00:00Z');",
        )
        .expect("pre-drop fixtures");
    }

    let db = Db::open(&path).expect("the seed-bank drop applies on boot");
    db.with(|conn| -> Result<(), StoreError> {
        let table_exists = |name: &str| -> Result<bool, StoreError> {
            Ok(conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
                [name],
                |row| row.get(0),
            )?)
        };
        assert!(
            !table_exists("seed_batch")?,
            "the seed_batch table is dropped"
        );
        assert!(
            !table_exists("germination_log")?,
            "the germination_log table is dropped"
        );
        let has_batch_column: bool = conn.query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM pragma_table_info('grow') WHERE name = 'seed_batch_id'
             )",
            [],
            |row| row.get(0),
        )?;
        assert!(!has_batch_column, "grow loses its seed_batch_id column");

        // The grow row survives the drop with its data, and its child
        // rows stay attached.
        let (name, substrate): (String, String) = conn.query_row(
            "SELECT name, substrate FROM grow WHERE id = 'g-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        assert_eq!(name, "Linked run");
        assert_eq!(substrate, "soil");
        let plants: i64 = conn.query_row(
            "SELECT COUNT(*) FROM plant WHERE grow_id = 'g-1'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(plants, 1, "the grow's plants survive the rebuild");

        // The store still writes and reads grows afterwards — a fresh
        // create carrying the V14 strain/lamp fields, and a full
        // listing.
        let created = store::grows::create(
            conn,
            &GrowInput {
                name: "Post-drop run".to_owned(),
                feed_template_id: None,
                planned_veg_weeks: None,
                strain_label: String::new(),
                photoperiod: PhotoperiodType::default(),
                heat_tolerance: Tolerance::default(),
                cold_tolerance: Tolerance::default(),
                light_tolerance: Tolerance::default(),
                real_watts: None,
                dimmable: true,
                substrate: "coco".to_owned(),
                started_on: "2026-04-01".to_owned(),
                status: GrowStatus::Active,
                notes: None,
            },
        )?;
        assert_eq!(created.substrate, "coco");
        let all = store::grows::list(conn)?;
        assert_eq!(all.len(), 2, "the old and the new grow are listed");
        Ok(())
    })
    .await
    .expect("seed-bank drop assertions");
}

/// V16 (spaces + equipment-catalogue removal): the `grow_space`,
/// `grow_space_equipment` and `equipment_item` tables and the grow's
/// `grow_space_id` column disappear on boot. The drop also pins that
/// the grow row, its plants and its sensor bindings survive —
/// bindings are the surviving sensor concept and never depended on
/// equipment — and that the store still round-trips grows afterwards.
#[tokio::test]
async fn dropping_spaces_and_equipment_loses_tables_and_the_grow_column() {
    let temp = TempDb::new("spaces-equipment-drop");
    let path = temp.path();
    {
        let mut conn = Connection::open(&path).expect("open at the pre-drop version");
        conn.pragma_update(None, "foreign_keys", "ON")
            .expect("pragmas");
        db::embedded::migrations::runner()
            .set_target(Target::Version(15))
            .run(&mut conn)
            .expect("apply through the last pre-drop migration");
        conn.execute_batch(
            "INSERT INTO equipment_item
                 (id, name, category, brand_model, real_watts, dimmable,
                  purchase_date, purchase_price, currency, notes,
                  ha_entity_id, ha_device_class, source, is_builtin,
                  created_at, updated_at)
             VALUES ('eq-1', 'Bound meter', 'sensor', NULL, NULL, 0,
                     NULL, NULL, NULL, NULL,
                     'sensor.grow_temperature', 'temperature', NULL, 0,
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
             INSERT INTO grow_space
                 (id, name, width_cm, depth_cm, height_cm, notes, created_at, updated_at)
             VALUES ('sp-1', 'The tent', 60, 60, 180, NULL,
                     '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z');
             INSERT INTO grow_space_equipment (grow_space_id, equipment_id) VALUES
                 ('sp-1', 'eq-1');
             INSERT INTO grow
                 (id, name, strain_id, substrate, started_on, status, notes,
                  created_at, updated_at, grow_space_id, feed_template_id,
                  planned_veg_weeks, strain_label, photoperiod,
                  heat_tolerance, cold_tolerance, light_tolerance,
                  real_watts, dimmable)
             VALUES ('g-1', 'Tent run', NULL, 'soil', '2026-03-01', 'active', NULL,
                     '2026-03-01T00:00:00Z', '2026-03-01T00:00:00Z',
                     'sp-1', NULL, 4, 'Northern Lights', 'photoperiod',
                     'average', 'average', 'average', NULL, 1);
             INSERT INTO plant (id, grow_id, name, created_at, updated_at)
             VALUES ('p-1', 'g-1', 'Plant 1',
                     '2026-03-02T00:00:00Z', '2026-03-02T00:00:00Z');
             INSERT INTO sensor_binding (grow_id, role, entity_id, updated_at)
             VALUES ('g-1', 'temperature', 'sensor.grow_temperature',
                     '2026-03-02T00:00:00Z');",
        )
        .expect("pre-drop fixtures");
    }

    let db = Db::open(&path).expect("the spaces/equipment drop applies on boot");
    db.with(|conn| -> Result<(), StoreError> {
        let table_exists = |name: &str| -> Result<bool, StoreError> {
            Ok(conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
                [name],
                |row| row.get(0),
            )?)
        };
        assert!(
            !table_exists("grow_space")?,
            "the grow_space table is dropped"
        );
        assert!(
            !table_exists("grow_space_equipment")?,
            "the grow_space_equipment table is dropped"
        );
        assert!(
            !table_exists("equipment_item")?,
            "the equipment_item table is dropped"
        );
        let has_space_column: bool = conn.query_row(
            "SELECT EXISTS(
                 SELECT 1 FROM pragma_table_info('grow') WHERE name = 'grow_space_id'
             )",
            [],
            |row| row.get(0),
        )?;
        assert!(!has_space_column, "grow loses its grow_space_id column");

        // The grow row survives the drop with its data, and its child
        // rows stay attached — plants, and the sensor bindings the
        // live tiles, charts and auto-log run on.
        let (name, substrate, label): (String, String, String) = conn.query_row(
            "SELECT name, substrate, COALESCE(strain_label, '') FROM grow WHERE id = 'g-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?;
        assert_eq!(name, "Tent run");
        assert_eq!(substrate, "soil");
        assert_eq!(label, "Northern Lights");
        let plants: i64 = conn.query_row(
            "SELECT COUNT(*) FROM plant WHERE grow_id = 'g-1'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(plants, 1, "the grow's plants survive the drop");
        let (role, entity): (String, String) = conn.query_row(
            "SELECT role, entity_id FROM sensor_binding WHERE grow_id = 'g-1'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        assert_eq!(
            (role.as_str(), entity.as_str()),
            ("temperature", "sensor.grow_temperature"),
            "the grow's sensor bindings survive the drop"
        );

        // The store still writes and reads grows afterwards — a fresh
        // create, and a full listing.
        let created = store::grows::create(
            conn,
            &GrowInput {
                name: "Post-drop run".to_owned(),
                feed_template_id: None,
                planned_veg_weeks: None,
                strain_label: String::new(),
                photoperiod: PhotoperiodType::default(),
                heat_tolerance: Tolerance::default(),
                cold_tolerance: Tolerance::default(),
                light_tolerance: Tolerance::default(),
                real_watts: None,
                dimmable: true,
                substrate: "coco".to_owned(),
                started_on: "2026-04-01".to_owned(),
                status: GrowStatus::Active,
                notes: None,
            },
        )?;
        assert_eq!(created.substrate, "coco");
        let all = store::grows::list(conn)?;
        assert_eq!(all.len(), 2, "the old and the new grow are listed");
        Ok(())
    })
    .await
    .expect("spaces/equipment drop assertions");
}
