//! Seed-batch persistence (spec §4.13).

use leaflog_core::seedbank::{SeedBatch, SeedBatchInput, StorageMethod};
use rusqlite::{Connection, OptionalExtension, params};

use super::{NotFound, StoreOrNotFound};
use crate::db::{StoreError, new_id, now_rfc3339};

/// Inserts a new seed batch and returns the stored row.
///
/// # Errors
///
/// Returns a [`crate::db::StoreError`] on SQLite failures.
pub fn create(conn: &Connection, input: &SeedBatchInput) -> Result<SeedBatch, StoreError> {
    let batch = SeedBatch {
        id: new_id(),
        strain_id: input.strain_id.clone(),
        quantity: input.quantity,
        acquired_from: input.acquired_from.clone(),
        acquisition_on: input.acquisition_on.clone(),
        storage_method: input.storage_method,
        notes: input.notes.clone(),
        created_at: now_rfc3339(),
        updated_at: now_rfc3339(),
    };
    conn.execute(
        "INSERT INTO seed_batch
             (id, strain_id, quantity, acquired_from, acquisition_on,
              storage_method, notes, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            batch.id,
            batch.strain_id,
            i64::from(batch.quantity),
            batch.acquired_from,
            batch.acquisition_on,
            batch.storage_method.label(),
            batch.notes,
            batch.created_at,
            batch.updated_at,
        ],
    )?;
    Ok(batch)
}

/// Lists batches grouped per strain, oldest acquisition first within
/// each strain (use the oldest seeds first, spec §4.13), strains
/// without batches last.
///
/// # Errors
///
/// Returns a [`crate::db::StoreError`] on SQLite failures.
pub fn list(conn: &Connection) -> Result<Vec<SeedBatch>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT * FROM seed_batch
         ORDER BY strain_id IS NULL, strain_id, acquisition_on, created_at",
    )?;
    let rows = stmt.query_map([], map_row)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

/// Counts all batches.
///
/// # Errors
///
/// Returns a [`crate::db::StoreError`] on SQLite failures.
pub fn count(conn: &Connection) -> Result<i64, StoreError> {
    Ok(conn.query_row("SELECT COUNT(*) FROM seed_batch", [], |row| row.get(0))?)
}

/// Fetches one batch by id.
///
/// # Errors
///
/// Returns [`super::NotFound`] when the id does not exist.
pub fn get(conn: &Connection, id: &str) -> Result<SeedBatch, StoreOrNotFound> {
    conn.query_row(
        "SELECT * FROM seed_batch WHERE id = ?1",
        params![id],
        map_row,
    )
    .optional()?
    .ok_or_else(|| NotFound.into())
}

/// Replaces a batch (full update) and returns the updated row.
///
/// # Errors
///
/// Returns [`super::NotFound`] when the id does not exist.
pub fn update(
    conn: &Connection,
    id: &str,
    input: &SeedBatchInput,
) -> Result<SeedBatch, StoreOrNotFound> {
    let existing = get(conn, id)?;
    let updated = SeedBatch {
        id: existing.id,
        strain_id: input.strain_id.clone(),
        quantity: input.quantity,
        acquired_from: input.acquired_from.clone(),
        acquisition_on: input.acquisition_on.clone(),
        storage_method: input.storage_method,
        notes: input.notes.clone(),
        created_at: existing.created_at,
        updated_at: now_rfc3339(),
    };
    conn.execute(
        "UPDATE seed_batch
         SET strain_id = ?2, quantity = ?3, acquired_from = ?4,
             acquisition_on = ?5, storage_method = ?6, notes = ?7,
             updated_at = ?8
         WHERE id = ?1",
        params![
            updated.id,
            updated.strain_id,
            i64::from(updated.quantity),
            updated.acquired_from,
            updated.acquisition_on,
            updated.storage_method.label(),
            updated.notes,
            updated.updated_at,
        ],
    )?;
    Ok(updated)
}

/// Deletes a batch. Grows linked to it keep running (ON DELETE SET
/// NULL).
///
/// # Errors
///
/// Returns [`super::NotFound`] when the id does not exist.
pub fn delete(conn: &Connection, id: &str) -> Result<(), StoreOrNotFound> {
    let affected = conn.execute("DELETE FROM seed_batch WHERE id = ?1", params![id])?;
    if affected == 0 {
        return Err(NotFound.into());
    }
    Ok(())
}

/// Decrements a batch's remaining count (starting plants from a linked
/// batch, spec §4.13); manual edits always win, so this never raises
/// the count and never goes below zero. A no-op for a missing batch.
///
/// # Errors
///
/// Returns a [`crate::db::StoreError`] on SQLite failures.
pub fn decrement(conn: &Connection, id: &str, by: u32) -> Result<(), StoreError> {
    conn.execute(
        "UPDATE seed_batch
         SET quantity = MAX(quantity - ?2, 0), updated_at = ?3
         WHERE id = ?1",
        params![id, i64::from(by), now_rfc3339()],
    )?;
    Ok(())
}

/// The batch a grow is linked to, if any.
///
/// # Errors
///
/// Returns a [`crate::db::StoreError`] on SQLite failures.
pub fn batch_of_grow(conn: &Connection, grow_id: &str) -> Result<Option<String>, StoreError> {
    Ok(conn
        .query_row(
            "SELECT seed_batch_id FROM grow WHERE id = ?1",
            params![grow_id],
            |row| row.get(0),
        )
        .optional()?)
}

/// Maps one result row. Column order matches the `SELECT *` of the
/// V10 `seed_batch` table.
fn map_row(row: &rusqlite::Row<'_>) -> Result<SeedBatch, rusqlite::Error> {
    let storage_label: String = row.get("storage_method")?;
    let storage_method = StorageMethod::from_label(&storage_label).ok_or(
        rusqlite::Error::FromSqlConversionFailure(
            5,
            rusqlite::types::Type::Text,
            Box::from(format!("unknown storage method {storage_label}")),
        ),
    )?;
    let quantity: i64 = row.get("quantity")?;
    Ok(SeedBatch {
        id: row.get("id")?,
        strain_id: row.get("strain_id")?,
        quantity: u32::try_from(quantity).unwrap_or(0),
        acquired_from: row.get("acquired_from")?,
        acquisition_on: row.get("acquisition_on")?,
        storage_method,
        notes: row.get("notes")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}
