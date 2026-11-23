//! Photo metadata persistence (spec §4.3). Bytes live on disk in the
//! photo store; these rows index them.

use leaflog_core::Phase;
use leaflog_core::grows::Photo;
use rusqlite::{Connection, OptionalExtension, params};

use super::{NotFound, StoreOrNotFound};
use crate::db::{StoreError, new_id, now_rfc3339};

/// Re-dates every photo of `grow_id` under `day_of`.
///
/// `day_of` maps a stored `captured_at` to its grow day under a new
/// anchor — the Germination pick's recompute (ticket 04). Callers
/// run this inside the pick's transaction; an unmappable timestamp
/// aborts it before any row moves.
///
/// # Errors
///
/// Returns [`StoreError::Unparsable`] when a stored `captured_at`
/// does not parse, and SQLite failures as-is.
pub fn redate(
    conn: &Connection,
    grow_id: &str,
    day_of: &dyn Fn(&str) -> Option<u16>,
) -> Result<usize, StoreError> {
    let rows: Vec<(String, String)> = conn
        .prepare("SELECT id, captured_at FROM photo WHERE grow_id = ?1")?
        .query_map(params![grow_id], |row| {
            Ok((
                row.get::<_, String>("id")?,
                row.get::<_, String>("captured_at")?,
            ))
        })?
        .collect::<Result<_, _>>()?;
    let stamp = now_rfc3339();
    let mut moved = 0;
    for (row_id, captured_at) in rows {
        let day =
            day_of(&captured_at).ok_or_else(|| StoreError::Unparsable(captured_at.clone()))?;
        conn.execute(
            "UPDATE photo SET grow_day = ?2, updated_at = ?3 WHERE id = ?1",
            params![row_id, i64::from(day), stamp],
        )?;
        moved += 1;
    }
    Ok(moved)
}

/// The facts the API layer derives before inserting a photo row.
#[derive(Debug, Clone)]
pub struct NewPhoto {
    /// Owning grow.
    pub grow_id: String,
    /// Plants in the photo; empty means the whole grow.
    pub plant_ids: Vec<String>,
    /// Linked log entry, when attached to one.
    pub action_log_id: Option<String>,
    /// Capture time, RFC 3339 UTC.
    pub captured_at: String,
    /// Days since the grow start at capture.
    pub grow_day: u16,
    /// Phase label at capture.
    pub phase: Option<Phase>,
    /// Free-form tags.
    pub tags: Vec<String>,
    /// File name under `photos/originals/<grow>/`.
    pub original_name: String,
    /// File name under `photos/thumbnails/<grow>/`.
    pub thumbnail_name: Option<String>,
    /// Size of the stored original in bytes.
    pub bytes_stored: u64,
}

/// Inserts a photo row and returns it.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
#[allow(clippy::missing_panics_doc)] // JSON of plain lists cannot fail
pub fn create(conn: &Connection, new: &NewPhoto) -> Result<Photo, StoreError> {
    let photo = Photo {
        id: new_id(),
        grow_id: new.grow_id.clone(),
        plant_id: new.plant_ids.first().cloned(),
        plant_ids: new.plant_ids.clone(),
        action_log_id: new.action_log_id.clone(),
        captured_at: new.captured_at.clone(),
        grow_day: new.grow_day,
        phase: new.phase,
        tags: new.tags.clone(),
        original_name: new.original_name.clone(),
        thumbnail_name: new.thumbnail_name.clone(),
        bytes_stored: new.bytes_stored,
        created_at: now_rfc3339(),
        updated_at: now_rfc3339(),
    };
    conn.execute(
        "INSERT INTO photo
             (id, grow_id, plant_id, action_log_id, captured_at, grow_day, phase,
              tags, original_name, thumbnail_name, bytes_stored, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)",
        params![
            photo.id,
            photo.grow_id,
            photo.plant_id,
            photo.action_log_id,
            photo.captured_at,
            i64::from(photo.grow_day),
            photo.phase.map(Phase::label),
            serde_json::to_string(&photo.tags).expect("list serializes"),
            photo.original_name,
            photo.thumbnail_name,
            clamp_to_i64(photo.bytes_stored),
            photo.created_at,
            photo.updated_at,
        ],
    )?;
    for plant_id in &photo.plant_ids {
        conn.execute(
            "INSERT INTO photo_plant (photo_id, plant_id) VALUES (?1, ?2)",
            params![photo.id, plant_id],
        )?;
    }
    Ok(photo)
}

/// Lists the photos linked to one plant, newest first.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
pub fn list_for_plant(conn: &Connection, plant_id: &str) -> Result<Vec<Photo>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT photo.*, (SELECT json_group_array(plant_id) FROM photo_plant
         WHERE photo_id = photo.id) AS plant_ids FROM photo
         JOIN photo_plant AS filter ON photo.id = filter.photo_id
         WHERE filter.plant_id = ?1 ORDER BY photo.created_at DESC",
    )?;
    let rows = stmt.query_map(params![plant_id], map_row)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

/// Lists a grow's photos, newest capture first.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
pub fn list(conn: &Connection, grow_id: &str) -> Result<Vec<Photo>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT photo.*, (SELECT json_group_array(plant_id) FROM photo_plant
         WHERE photo_id = photo.id) AS plant_ids FROM photo
         WHERE grow_id = ?1 ORDER BY captured_at DESC",
    )?;
    let rows = stmt.query_map(params![grow_id], map_row)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

/// The oldest capture timestamp of a grow (`MIN(captured_at)`), or
/// `None` when it has no photos.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
pub fn earliest_timestamp(conn: &Connection, grow_id: &str) -> Result<Option<String>, StoreError> {
    Ok(conn.query_row(
        "SELECT MIN(captured_at) FROM photo WHERE grow_id = ?1",
        params![grow_id],
        |row| row.get(0),
    )?)
}

/// Lists photos captured within a half-open UTC interval.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
pub fn list_in_window(
    conn: &Connection,
    grow_id: &str,
    from: jiff::Timestamp,
    to: jiff::Timestamp,
) -> Result<Vec<Photo>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT photo.*, (SELECT json_group_array(plant_id) FROM photo_plant
         WHERE photo_id = photo.id) AS plant_ids FROM photo
         WHERE grow_id = ?1
           AND julianday(captured_at) >= julianday(?2)
           AND julianday(captured_at) < julianday(?3) + 1.0 / 86400.0
         ORDER BY captured_at DESC, created_at DESC",
    )?;
    let rows = stmt.query_map(params![grow_id, from.to_string(), to.to_string()], |row| {
        let photo = map_row(row)?;
        // Photo capture timestamps can carry subsecond precision too,
        // so the exact half-open check follows the SQL pre-filter.
        let in_window = photo
            .captured_at
            .parse::<jiff::Timestamp>()
            .is_ok_and(|at| (from..to).contains(&at));
        Ok(in_window.then_some(photo))
    })?;
    Ok(rows
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect())
}

/// Fetches one photo by id.
///
/// # Errors
///
/// Returns [`super::NotFound`] when the id does not exist.
pub fn get(conn: &Connection, id: &str) -> Result<Photo, StoreOrNotFound> {
    conn.query_row(
        "SELECT photo.*, (SELECT json_group_array(plant_id) FROM photo_plant
         WHERE photo_id = photo.id) AS plant_ids FROM photo WHERE id = ?1",
        params![id],
        map_row,
    )
    .optional()?
    .ok_or_else(|| NotFound.into())
}

/// Marks the thumbnail as regenerated (or missing) after a lazy
/// rebuild.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
pub fn set_thumbnail(
    conn: &Connection,
    id: &str,
    thumbnail_name: Option<&str>,
) -> Result<(), StoreError> {
    conn.execute(
        "UPDATE photo SET thumbnail_name = ?2, updated_at = ?3 WHERE id = ?1",
        params![id, thumbnail_name, now_rfc3339()],
    )?;
    Ok(())
}

/// Deletes a photo row (the caller removes the files).
///
/// # Errors
///
/// Returns [`super::NotFound`] when the id does not exist.
pub fn delete(conn: &Connection, id: &str) -> Result<(), StoreOrNotFound> {
    let affected = conn.execute("DELETE FROM photo WHERE id = ?1", params![id])?;
    if affected == 0 {
        return Err(NotFound.into());
    }
    Ok(())
}

/// Total stored original bytes (for the storage-size warning).
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
pub fn total_bytes(conn: &Connection) -> Result<u64, StoreError> {
    let total: Option<i64> =
        conn.query_row("SELECT SUM(bytes_stored) FROM photo", [], |row| row.get(0))?;
    Ok(u64::try_from(total.unwrap_or(0).max(0)).unwrap_or(0))
}

/// Clamps a byte count into the SQLite integer range.
fn clamp_to_i64(bytes: u64) -> i64 {
    i64::try_from(bytes).unwrap_or(i64::MAX)
}

/// Maps one `photo` row.
fn map_row(row: &rusqlite::Row<'_>) -> Result<Photo, rusqlite::Error> {
    let phase: Option<String> = row.get("phase")?;
    let tags: String = row.get("tags")?;
    let grow_day: i64 = row.get("grow_day")?;
    let bytes: i64 = row.get("bytes_stored")?;
    Ok(Photo {
        id: row.get("id")?,
        grow_id: row.get("grow_id")?,
        plant_id: row.get("plant_id")?,
        plant_ids: serde_json::from_str(&row.get::<_, String>("plant_ids")?).unwrap_or_default(),
        action_log_id: row.get("action_log_id")?,
        captured_at: row.get("captured_at")?,
        grow_day: u16::try_from(grow_day.max(0)).unwrap_or(0),
        phase: phase.and_then(|label| {
            Phase::all()
                .into_iter()
                .find(|phase| phase.label() == label)
        }),
        tags: serde_json::from_str(&tags).unwrap_or_default(),
        original_name: row.get("original_name")?,
        thumbnail_name: row.get("thumbnail_name")?,
        bytes_stored: u64::try_from(bytes.max(0)).unwrap_or(0),
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}
