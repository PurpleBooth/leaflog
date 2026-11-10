//! Snapshot-retention acceptance test (spec §4.2, §13): the prune
//! keeps at most one snapshot per hour, the first and last row of each
//! day, and every excursion-linked row inside the window.

use leaflog_backend::db::{Db, StoreError};
use leaflog_backend::store::retention;
use leaflog_backend::store::{actions, grows};
use leaflog_core::actions::ActionType;
use leaflog_core::grows::{GrowInput, GrowStatus};

/// Inserts one log entry (returning its id) and one snapshot of it.
fn snapshot(conn: &rusqlite::Connection, grow_id: &str, at: &str, auto: bool) -> String {
    let input = leaflog_core::grows::ActionInput {
        plant_ids: Vec::new(),
        occurred_at: at.to_owned(),
        action_type: ActionType::Water,
        details: leaflog_core::actions::ActionDetails::default(),
        notes: auto.then(|| "auto: excursion".to_owned()),
        appearance_tags: Vec::new(),
    };
    // Build the stored shape via the store's own from_input + create.
    let entry = actions::from_input(grow_id, &input, leaflog_core::Phase::Vegetative, 0);
    actions::create(conn, &entry).expect("entry");
    let snapshot_id = leaflog_backend::db::new_id();
    conn.execute(
        "INSERT INTO sensor_snapshot (id, action_log_id, role, entity_id, state, captured_at)
         VALUES (?1, ?2, 'temperature', 'sensor.grow_temperature', '24.1', ?3)",
        rusqlite::params![snapshot_id, entry.id, at],
    )
    .expect("snapshot");
    snapshot_id
}

fn remaining(conn: &rusqlite::Connection) -> Vec<String> {
    let mut stmt = conn
        .prepare("SELECT id FROM sensor_snapshot ORDER BY captured_at")
        .unwrap();
    let rows = stmt.query_map([], |row| row.get::<_, String>(0)).unwrap();
    rows.collect::<Result<Vec<_>, _>>().unwrap()
}

#[tokio::test]
async fn prune_thins_to_one_per_hour_keeping_day_edges_and_excursions() {
    let db = Db::open_in_memory().expect("migrated db");
    let grow = db
        .with(|conn| {
            grows::create(
                conn,
                &GrowInput {
                    name: "Retention run".to_owned(),
                    strain_id: None,
                    grow_space_id: None,
                    seed_batch_id: None,
                    feed_template_id: None,
                    substrate: "soil".to_owned(),
                    started_on: "2026-01-01".to_owned(),
                    status: GrowStatus::Active,
                    notes: None,
                },
            )
        })
        .await
        .expect("grow");

    db.with(move |conn| -> Result<(), StoreError> {
        let id = grow.id;
        // Day 1: four snapshots across two hours plus a mid-hour
        // duplicate; day 2: two snapshots in one hour, one of them an
        // excursion; and one recent snapshot (inside the window).
        let d1_open = snapshot(conn, &id, "2026-01-01T08:00:00Z", false);
        let d1_dup = snapshot(conn, &id, "2026-01-01T08:20:00Z", false);
        let d1_late = snapshot(conn, &id, "2026-01-01T09:40:00Z", false);
        let d1_close = snapshot(conn, &id, "2026-01-01T23:50:00Z", false);
        let d2_open = snapshot(conn, &id, "2026-01-02T07:00:00Z", false);
        let d2_excursion = snapshot(conn, &id, "2026-01-02T07:30:00Z", true);
        let recent = snapshot(conn, &id, "2026-11-01T07:00:00Z", false);

        let pruned = retention::prune(conn, "2026-02-01T00:00:00Z")?;
        assert_eq!(pruned, 1, "only the mid-hour duplicate goes");

        let survivors = remaining(conn);
        for kept in [
            &d1_open,
            &d1_late,
            &d1_close,
            &d2_open,
            &d2_excursion,
            &recent,
        ] {
            assert!(survivors.contains(kept), "{kept} must survive");
        }
        assert!(
            !survivors.contains(&d1_dup),
            "the hour keeps only its first"
        );
        assert_eq!(survivors.len(), 6);

        // Idempotent: a second run finds nothing to do, and the
        // configured window is read from settings (default 24).
        assert_eq!(retention::prune(conn, "2026-02-01T00:00:00Z")?, 0);
        assert_eq!(retention::retention_months(conn)?, 24);

        // A configured window overrides the default.
        put_setting(conn, "snapshot_retention_months", 6);
        assert_eq!(retention::retention_months(conn)?, 6);
        Ok(())
    })
    .await
    .expect("retention assertions");
}

/// Writes one settings key (plain JSON number, the settings-store
/// format).
fn put_setting(conn: &rusqlite::Connection, key: &str, value: u32) {
    conn.execute(
        "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, '2026-01-01T00:00:00Z')
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        rusqlite::params![key, value],
    )
    .unwrap();
}
