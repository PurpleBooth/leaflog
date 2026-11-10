//! Germination-test persistence (spec §4.13).

use leaflog_core::seedbank::GerminationTest;
use leaflog_core::seedbank::GerminationTestInput;
use rusqlite::Connection;
use rusqlite::params;

use super::NotFound;
use super::StoreOrNotFound;
use crate::db::StoreError;
use crate::db::new_id;
use crate::db::now_rfc3339;

/// Inserts a germination-test entry for a batch.
///
/// # Errors
///
/// Returns a [`crate::db::StoreError`] on SQLite failures.
pub fn create(
    conn: &Connection,
    seed_batch_id: &str,
    input: &GerminationTestInput,
) -> Result<GerminationTest, StoreError> {
    let test = GerminationTest {
        id: new_id(),
        seed_batch_id: seed_batch_id.to_owned(),
        tested_on: input.tested_on.clone(),
        sown: input.sown,
        sprouted: input.sprouted,
        notes: input.notes.clone(),
        created_at: now_rfc3339(),
        updated_at: now_rfc3339(),
    };
    conn.execute(
        "INSERT INTO germination_log
             (id, seed_batch_id, tested_on, sown, sprouted, notes,
              created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            test.id,
            test.seed_batch_id,
            test.tested_on,
            i64::from(test.sown),
            i64::from(test.sprouted),
            test.notes,
            test.created_at,
            test.updated_at,
        ],
    )?;
    Ok(test)
}

/// Lists a batch's tests, oldest test date first.
///
/// # Errors
///
/// Returns a [`crate::db::StoreError`] on SQLite failures.
pub fn list_for_batch(
    conn: &Connection,
    seed_batch_id: &str,
) -> Result<Vec<GerminationTest>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT * FROM germination_log
         WHERE seed_batch_id = ?1 ORDER BY tested_on, created_at",
    )?;
    let rows = stmt.query_map(params![seed_batch_id], map_row)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

/// Fetches one test by id.
///
/// # Errors
///
/// Returns [`super::NotFound`] when the id does not exist.
pub fn get(conn: &Connection, id: &str) -> Result<GerminationTest, StoreOrNotFound> {
    use rusqlite::OptionalExtension;
    conn.query_row(
        "SELECT * FROM germination_log WHERE id = ?1",
        params![id],
        map_row,
    )
    .optional()?
    .ok_or_else(|| NotFound.into())
}

/// Deletes a test entry.
///
/// # Errors
///
/// Returns [`super::NotFound`] when the id does not exist.
pub fn delete(conn: &Connection, id: &str) -> Result<(), StoreOrNotFound> {
    let affected = conn.execute("DELETE FROM germination_log WHERE id = ?1", params![id])?;
    if affected == 0 {
        return Err(NotFound.into());
    }
    Ok(())
}

/// Maps one result row. Column order matches the `SELECT *` of the
/// V12 `germination_log` table.
fn map_row(row: &rusqlite::Row<'_>) -> Result<GerminationTest, rusqlite::Error> {
    let sown: i64 = row.get("sown")?;
    let sprouted: i64 = row.get("sprouted")?;
    Ok(GerminationTest {
        id: row.get("id")?,
        seed_batch_id: row.get("seed_batch_id")?,
        tested_on: row.get("tested_on")?,
        sown: u32::try_from(sown).unwrap_or(0),
        sprouted: u32::try_from(sprouted).unwrap_or(0),
        notes: row.get("notes")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}
