//! Grow and plant persistence (spec §4.2, §4.5).

use leaflog_core::grows::{
    Grow, GrowInput, GrowStatus, PhotoperiodType, Plant, PlantInput, Tolerance,
};
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
        grow_space_id: input.grow_space_id.clone(),
        feed_template_id: input.feed_template_id.clone(),
        planned_veg_weeks: input.planned_veg_weeks,
        strain_label: input.strain_label.clone(),
        photoperiod: input.photoperiod,
        heat_tolerance: input.heat_tolerance,
        cold_tolerance: input.cold_tolerance,
        light_tolerance: input.light_tolerance,
        real_watts: input.real_watts,
        dimmable: input.dimmable,
        substrate: input.substrate.clone(),
        started_on: input.started_on.clone(),
        status: input.status,
        notes: input.notes.clone(),
    };
    conn.execute(
        "INSERT INTO grow (id, name, grow_space_id, feed_template_id, planned_veg_weeks, strain_label, photoperiod, heat_tolerance, cold_tolerance, light_tolerance, real_watts, dimmable, substrate, started_on, status, notes, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18)",
        params![
            grow.id,
            grow.name,
            grow.grow_space_id,
            grow.feed_template_id,
            grow.planned_veg_weeks.map(i64::from),
            non_empty_label(&grow.strain_label),
            grow.photoperiod.label(),
            grow.heat_tolerance.label(),
            grow.cold_tolerance.label(),
            grow.light_tolerance.label(),
            grow.real_watts,
            i64::from(grow.dimmable),
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
    updated.grow_space_id.clone_from(&input.grow_space_id);
    updated.feed_template_id.clone_from(&input.feed_template_id);
    updated.planned_veg_weeks = input.planned_veg_weeks;
    updated.strain_label.clone_from(&input.strain_label);
    updated.photoperiod = input.photoperiod;
    updated.heat_tolerance = input.heat_tolerance;
    updated.cold_tolerance = input.cold_tolerance;
    updated.light_tolerance = input.light_tolerance;
    updated.real_watts = input.real_watts;
    updated.dimmable = input.dimmable;
    updated.substrate.clone_from(&input.substrate);
    updated.started_on.clone_from(&input.started_on);
    updated.status = input.status;
    updated.notes.clone_from(&input.notes);
    conn.execute(
        "UPDATE grow SET name = ?2, grow_space_id = ?3,
             feed_template_id = ?4, planned_veg_weeks = ?5, strain_label = ?6, photoperiod = ?7,
             heat_tolerance = ?8, cold_tolerance = ?9, light_tolerance = ?10,
             real_watts = ?11, dimmable = ?12, substrate = ?13, started_on = ?14,
             status = ?15, notes = ?16, updated_at = ?17
         WHERE id = ?1",
        params![
            updated.id,
            updated.name,
            updated.grow_space_id,
            updated.feed_template_id,
            updated.planned_veg_weeks.map(i64::from),
            non_empty_label(&updated.strain_label),
            updated.photoperiod.label(),
            updated.heat_tolerance.label(),
            updated.cold_tolerance.label(),
            updated.light_tolerance.label(),
            updated.real_watts,
            i64::from(updated.dimmable),
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
        wet_g: input.wet_g,
        dry_g: input.dry_g,
        trimmed_g: input.trimmed_g,
    };
    conn.execute(
        "INSERT INTO plant (id, grow_id, name, wet_g, dry_g, trimmed_g, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            plant.id,
            plant.grow_id,
            plant.name,
            plant.wet_g,
            plant.dry_g,
            plant.trimmed_g,
            plant.created_at,
            plant.updated_at
        ],
    )?;
    Ok(plant)
}

/// Replaces a plant (full update: name and harvest weights) and
/// returns the updated row.
///
/// # Errors
///
/// Returns [`super::NotFound`] when the id does not exist.
pub fn update_plant(
    conn: &Connection,
    id: &str,
    input: &PlantInput,
) -> Result<Plant, StoreOrNotFound> {
    let existing = get_plant(conn, id)?;
    let updated = Plant {
        id: existing.id,
        grow_id: existing.grow_id,
        name: input.name.clone(),
        wet_g: input.wet_g,
        dry_g: input.dry_g,
        trimmed_g: input.trimmed_g,
        created_at: existing.created_at,
        updated_at: now_rfc3339(),
    };
    conn.execute(
        "UPDATE plant
         SET name = ?2, wet_g = ?3, dry_g = ?4, trimmed_g = ?5, updated_at = ?6
         WHERE id = ?1",
        params![
            updated.id,
            updated.name,
            updated.wet_g,
            updated.dry_g,
            updated.trimmed_g,
            updated.updated_at
        ],
    )?;
    Ok(updated)
}

/// Fetches one plant by id.
///
/// # Errors
///
/// Returns [`super::NotFound`] when the id does not exist.
pub fn get_plant(conn: &Connection, id: &str) -> Result<Plant, StoreOrNotFound> {
    conn.query_row("SELECT * FROM plant WHERE id = ?1", params![id], map_plant)
        .optional()?
        .ok_or_else(|| NotFound.into())
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
#[allow(clippy::too_many_lines)] // flat row-to-struct mapping, not logic
fn map_grow(row: &rusqlite::Row<'_>) -> Result<Grow, rusqlite::Error> {
    let status: String = row.get("status")?;
    // The V14 columns are nullable: a row written before them (or with
    // an unknown label) reads back as the field's default (D075).
    let dimmable: i64 = row.get("dimmable")?;
    Ok(Grow {
        id: row.get("id")?,
        name: row.get("name")?,
        grow_space_id: row.get("grow_space_id")?,
        feed_template_id: row.get("feed_template_id")?,
        planned_veg_weeks: row
            .get::<_, Option<i64>>("planned_veg_weeks")?
            .and_then(|value| u8::try_from(value).ok()),
        strain_label: row
            .get::<_, Option<String>>("strain_label")?
            .unwrap_or_default(),
        photoperiod: label_or_default(row, "photoperiod", PhotoperiodType::from_label)?,
        heat_tolerance: label_or_default(row, "heat_tolerance", Tolerance::from_label)?,
        cold_tolerance: label_or_default(row, "cold_tolerance", Tolerance::from_label)?,
        light_tolerance: label_or_default(row, "light_tolerance", Tolerance::from_label)?,
        real_watts: row.get("real_watts")?,
        dimmable: dimmable != 0,
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

/// Reads one nullable enum-label column and parses it; `NULL` (a row
/// predating V14) and an unknown label fall back to the type's
/// default, so old rows keep loading.
fn label_or_default<T>(
    row: &rusqlite::Row<'_>,
    column: &str,
    parse: fn(&str) -> Option<T>,
) -> Result<T, rusqlite::Error>
where
    T: Default,
{
    let label: Option<String> = row.get(column)?;
    Ok(label.and_then(|text| parse(&text)).unwrap_or_default())
}

/// Stores an empty strain label as `NULL` so "unlabelled" has one
/// representation in the database.
const fn non_empty_label(label: &str) -> Option<&str> {
    if label.is_empty() { None } else { Some(label) }
}

/// Maps one `plant` row.
fn map_plant(row: &rusqlite::Row<'_>) -> Result<Plant, rusqlite::Error> {
    Ok(Plant {
        id: row.get("id")?,
        grow_id: row.get("grow_id")?,
        name: row.get("name")?,
        wet_g: row.get("wet_g")?,
        dry_g: row.get("dry_g")?,
        trimmed_g: row.get("trimmed_g")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}
