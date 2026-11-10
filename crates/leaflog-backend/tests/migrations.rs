//! Migration-tooling acceptance tests (spec §13): boot applies pending
//! files, pre-refinery histories are adopted, and a divergent, gapped,
//! or newer-than-binary history refuses to start.

use std::path::PathBuf;

use leaflog_backend::db::{self, Db, StoreError};
use leaflog_backend::store;
use leaflog_core::equipment::EquipmentCategory;
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
        assert_eq!(
            Db::version(conn)?,
            i64::try_from(db::embedded::migrations::runner().get_migrations().len())
                .expect("count fits")
        );
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
        i64::try_from(db::embedded::migrations::runner().get_migrations().len()).ok(),
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
#[tokio::test]
async fn catalogue_trim_recategorizes_prunes_and_parks() {
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
                 ('e-filter', 'Carbon filter 100 mm', 'carbon_filter', NULL, NULL, 0,
                  NULL, NULL, NULL, NULL, 'bundled', 1,
                  '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'),
                 ('e-tray', 'Trim tray set', 'misc', NULL, NULL, 0,
                  NULL, NULL, NULL, NULL, 'bundled', 1,
                  '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'),
                 ('e-dehum', 'My basement dehumidifier', 'dehumidifier', NULL, NULL, 0,
                  NULL, NULL, NULL, NULL, NULL, 0,
                  '2026-01-01T00:00:00Z', '2026-01-01T00:00:00Z'),
                 ('e-light', 'SE1500 LED grow light', 'light', NULL, 150, 1,
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
    }

    let db = Db::open(&path).expect("V6 applies on boot");
    db.with(|conn| -> Result<(), StoreError> {
        let category = |id: &str| -> Result<String, StoreError> {
            Ok(conn.query_row(
                "SELECT category FROM equipment_item WHERE id = ?1",
                [id],
                |row| row.get(0),
            )?)
        };
        assert_eq!(category("e-filter")?, "fan", "carbon filters fold into fan");
        assert_eq!(category("e-light")?, "light", "catalogue rows untouched");
        assert_eq!(
            category("e-dehum")?,
            "misc",
            "user rows in retired categories are parked, not deleted"
        );
        let tray: bool = conn.query_row(
            "SELECT EXISTS(SELECT 1 FROM equipment_item WHERE id = 'e-tray')",
            [],
            |row| row.get(0),
        )?;
        assert!(!tray, "the bundled trim-tray row is removed");
        // The space kept its reference to the recategorized filter and
        // lost (by cascade) the reference to the removed tray.
        let refs: i64 = conn.query_row(
            "SELECT COUNT(*) FROM grow_space_equipment WHERE grow_space_id = 'sp-1'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(refs, 1);
        // Parked rows still deserialize through the store layer.
        let items = store::equipment::list(conn, None)?;
        assert!(
            items
                .iter()
                .any(|item| { item.id == "e-dehum" && item.category == EquipmentCategory::Misc })
        );
        Ok(())
    })
    .await
    .expect("trim assertions");
}
