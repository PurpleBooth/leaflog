//! Grow and plant persistence (spec §4.2, §4.5).

use leaflog_core::grows::{Grow, GrowInput, GrowStatus, Plant, PlantInput};
use rusqlite::{Connection, OptionalExtension, params};

use super::{NotFound, StoreOrNotFound};
use crate::db::{StoreError, new_id, now_rfc3339};

/// Inserts a grow and returns the stored row.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
#[allow(clippy::missing_panics_doc)] // JSON of plain lists cannot fail
pub fn create(conn: &Connection, input: &GrowInput) -> Result<Grow, StoreError> {
    let grow = Grow {
        id: new_id(),
        created_at: now_rfc3339(),
        updated_at: now_rfc3339(),
        name: input.name.clone(),
        strain_id: input.strain_id.clone(),
        grow_space_id: input.grow_space_id.clone(),
        substrate: input.substrate.clone(),
        started_on: input.started_on.clone(),
        status: input.status,
        notes: input.notes.clone(),
    };
    conn.execute(
        "INSERT INTO grow (id, name, strain_id, grow_space_id, substrate, started_on, status, notes, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
        params![
            grow.id,
            grow.name,
            grow.strain_id,
            grow.grow_space_id,
            grow.substrate,
            grow.started_on,
            grow.status.label(),
            grow.notes,
            grow.created_at,
            grow.updated_at,
        ],
    )?;
    Ok(grow)
}

/// Lists grows, active first, newest start date first.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
pub fn list(conn: &Connection) -> Result<Vec<Grow>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT * FROM grow ORDER BY CASE status WHEN 'active' THEN 0 ELSE 1 END, started_on DESC",
    )?;
    let rows = stmt.query_map([], map_grow)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

/// Fetches one grow by id.
///
/// # Errors
///
/// Returns [`super::NotFound`] when the id does not exist.
pub fn get(conn: &Connection, id: &str) -> Result<Grow, StoreOrNotFound> {
    conn.query_row("SELECT * FROM grow WHERE id = ?1", params![id], map_grow)
        .optional()?
        .ok_or_else(|| NotFound.into())
}

/// Lists the grows inside one grow space, active first, newest start
/// date first.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
pub fn list_for_space(conn: &Connection, space_id: &str) -> Result<Vec<Grow>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT * FROM grow WHERE grow_space_id = ?1
         ORDER BY CASE status WHEN 'active' THEN 0 ELSE 1 END, started_on DESC",
    )?;
    let rows = stmt.query_map(params![space_id], map_grow)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

/// Replaces a grow (full update), preserving id and creation time.
///
/// # Errors
///
/// Returns [`super::NotFound`] when the id does not exist.
pub fn update(conn: &Connection, id: &str, input: &GrowInput) -> Result<Grow, StoreOrNotFound> {
    let existing = get(conn, id)?;
    let mut updated = existing;
    updated.updated_at = now_rfc3339();
    updated.name.clone_from(&input.name);
    updated.strain_id.clone_from(&input.strain_id);
    updated.grow_space_id.clone_from(&input.grow_space_id);
    updated.substrate.clone_from(&input.substrate);
    updated.started_on.clone_from(&input.started_on);
    updated.status = input.status;
    updated.notes.clone_from(&input.notes);
    conn.execute(
        "UPDATE grow SET name = ?2, strain_id = ?3, grow_space_id = ?4, substrate = ?5,
             started_on = ?6, status = ?7, notes = ?8, updated_at = ?9
         WHERE id = ?1",
        params![
            updated.id,
            updated.name,
            updated.strain_id,
            updated.grow_space_id,
            updated.substrate,
            updated.started_on,
            updated.status.label(),
            updated.notes,
            updated.updated_at,
        ],
    )?;
    Ok(updated)
}

/// Deletes a grow (cascades to plants, log entries and photo rows).
///
/// # Errors
///
/// Returns [`super::NotFound`] when the id does not exist.
pub fn delete(conn: &Connection, id: &str) -> Result<(), StoreOrNotFound> {
    let affected = conn.execute("DELETE FROM grow WHERE id = ?1", params![id])?;
    if affected == 0 {
        return Err(NotFound.into());
    }
    Ok(())
}

/// Inserts a plant into a grow.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
pub fn create_plant(
    conn: &Connection,
    grow_id: &str,
    input: &PlantInput,
) -> Result<Plant, StoreError> {
    let plant = Plant {
        id: new_id(),
        grow_id: grow_id.to_owned(),
        created_at: now_rfc3339(),
        updated_at: now_rfc3339(),
        name: input.name.clone(),
    };
    conn.execute(
        "INSERT INTO plant (id, grow_id, name, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            plant.id,
            plant.grow_id,
            plant.name,
            plant.created_at,
            plant.updated_at
        ],
    )?;
    Ok(plant)
}

/// Lists the plants of a grow by creation order.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
pub fn list_plants(conn: &Connection, grow_id: &str) -> Result<Vec<Plant>, StoreError> {
    let mut stmt =
        conn.prepare("SELECT * FROM plant WHERE grow_id = ?1 ORDER BY created_at, name")?;
    let rows = stmt.query_map(params![grow_id], map_plant)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

/// Deletes a plant.
///
/// # Errors
///
/// Returns [`super::NotFound`] when the id does not exist.
pub fn delete_plant(conn: &Connection, id: &str) -> Result<(), StoreOrNotFound> {
    let affected = conn.execute("DELETE FROM plant WHERE id = ?1", params![id])?;
    if affected == 0 {
        return Err(NotFound.into());
    }
    Ok(())
}

/// Maps one `grow` row.
fn map_grow(row: &rusqlite::Row<'_>) -> Result<Grow, rusqlite::Error> {
    let status: String = row.get("status")?;
    Ok(Grow {
        id: row.get("id")?,
        name: row.get("name")?,
        strain_id: row.get("strain_id")?,
        grow_space_id: row.get("grow_space_id")?,
        substrate: row.get("substrate")?,
        started_on: row.get("started_on")?,
        status: if status == "archived" {
            GrowStatus::Archived
        } else {
            GrowStatus::Active
        },
        notes: row.get("notes")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

/// Maps one `plant` row.
fn map_plant(row: &rusqlite::Row<'_>) -> Result<Plant, rusqlite::Error> {
    Ok(Plant {
        id: row.get("id")?,
        grow_id: row.get("grow_id")?,
        name: row.get("name")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}
