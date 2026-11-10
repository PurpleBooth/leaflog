//! Grow-space persistence (spec §4.1): a named physical setup whose
//! equipment is a set of catalogue references and which contains grows.

use leaflog_core::spaces::{GrowSpace, GrowSpaceInput};
use rusqlite::{Connection, OptionalExtension, params};

use super::{NotFound, StoreOrNotFound};
use crate::db::{StoreError, new_id, now_rfc3339};

/// Inserts a grow space and returns the stored row.
///
/// # Errors
///
/// Returns a [`crate::db::StoreError`] on SQLite failures.
pub fn create(conn: &Connection, input: &GrowSpaceInput) -> Result<GrowSpace, StoreError> {
    let space = GrowSpace {
        id: new_id(),
        name: input.name.clone(),
        width_cm: input.width_cm,
        depth_cm: input.depth_cm,
        height_cm: input.height_cm,
        notes: input.notes.clone(),
        created_at: now_rfc3339(),
        updated_at: now_rfc3339(),
    };
    conn.execute(
        "INSERT INTO grow_space
             (id, name, width_cm, depth_cm, height_cm, notes, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            space.id,
            space.name,
            space.width_cm,
            space.depth_cm,
            space.height_cm,
            space.notes,
            space.created_at,
            space.updated_at,
        ],
    )?;
    Ok(space)
}

/// Lists grow spaces ordered by name.
///
/// # Errors
///
/// Returns a [`crate::db::StoreError`] on SQLite failures.
pub fn list(conn: &Connection) -> Result<Vec<GrowSpace>, StoreError> {
    let mut stmt = conn.prepare("SELECT * FROM grow_space ORDER BY name")?;
    let rows = stmt.query_map([], map_row)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

/// Fetches one space by id.
///
/// # Errors
///
/// Returns [`super::NotFound`] when the id does not exist.
pub fn get(conn: &Connection, id: &str) -> Result<GrowSpace, StoreOrNotFound> {
    conn.query_row(
        "SELECT * FROM grow_space WHERE id = ?1",
        params![id],
        map_row,
    )
    .optional()?
    .ok_or_else(|| NotFound.into())
}

/// Replaces a space (full update) and returns the updated row.
///
/// # Errors
///
/// Returns [`super::NotFound`] when the id does not exist.
pub fn update(
    conn: &Connection,
    id: &str,
    input: &GrowSpaceInput,
) -> Result<GrowSpace, StoreOrNotFound> {
    let existing = get(conn, id)?;
    let updated = GrowSpace {
        created_at: existing.created_at,
        updated_at: now_rfc3339(),
        id: existing.id,
        name: input.name.clone(),
        width_cm: input.width_cm,
        depth_cm: input.depth_cm,
        height_cm: input.height_cm,
        notes: input.notes.clone(),
    };
    conn.execute(
        "UPDATE grow_space
         SET name = ?2, width_cm = ?3, depth_cm = ?4, height_cm = ?5,
             notes = ?6, updated_at = ?7
         WHERE id = ?1",
        params![
            updated.id,
            updated.name,
            updated.width_cm,
            updated.depth_cm,
            updated.height_cm,
            updated.notes,
            updated.updated_at,
        ],
    )?;
    Ok(updated)
}

/// Deletes a space. Grows inside it survive with a null space
/// (`ON DELETE SET NULL`); its equipment references cascade away.
///
/// # Errors
///
/// Returns [`super::NotFound`] when the id does not exist.
pub fn delete(conn: &Connection, id: &str) -> Result<(), StoreOrNotFound> {
    let affected = conn.execute("DELETE FROM grow_space WHERE id = ?1", params![id])?;
    if affected == 0 {
        return Err(NotFound.into());
    }
    Ok(())
}

/// Adds a catalogue entry to a space. Idempotent: re-adding an entry
/// already in the space is a no-op.
///
/// # Errors
///
/// Returns [`super::NotFound`] when the space or the equipment entry
/// does not exist.
pub fn add_equipment(
    conn: &Connection,
    space_id: &str,
    equipment_id: &str,
) -> Result<(), StoreOrNotFound> {
    get(conn, space_id)?;
    super::equipment::get(conn, equipment_id)?;
    conn.execute(
        "INSERT OR IGNORE INTO grow_space_equipment (grow_space_id, equipment_id)
         VALUES (?1, ?2)",
        params![space_id, equipment_id],
    )?;
    Ok(())
}

/// Removes a catalogue entry from a space. Idempotent: removing an
/// entry that is not in the space is a no-op.
///
/// # Errors
///
/// Returns [`super::NotFound`] when the space or the equipment entry
/// does not exist.
pub fn remove_equipment(
    conn: &Connection,
    space_id: &str,
    equipment_id: &str,
) -> Result<(), StoreOrNotFound> {
    get(conn, space_id)?;
    super::equipment::get(conn, equipment_id)?;
    conn.execute(
        "DELETE FROM grow_space_equipment WHERE grow_space_id = ?1 AND equipment_id = ?2",
        params![space_id, equipment_id],
    )?;
    Ok(())
}

/// Maps one result row. Column order matches the `SELECT *` of the
/// migration's `grow_space` table.
fn map_row(row: &rusqlite::Row<'_>) -> Result<GrowSpace, rusqlite::Error> {
    Ok(GrowSpace {
        id: row.get("id")?,
        name: row.get("name")?,
        width_cm: row.get("width_cm")?,
        depth_cm: row.get("depth_cm")?,
        height_cm: row.get("height_cm")?,
        notes: row.get("notes")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}
