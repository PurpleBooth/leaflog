//! Action-log persistence (spec §4.2).

use leaflog_core::Phase;
use leaflog_core::actions::{ActionDetails, ActionType};
use leaflog_core::grows::ActionEntry;
use leaflog_core::grows::ActionInput;
use rusqlite::{Connection, OptionalExtension, params};

use super::{NotFound, StoreOrNotFound};
use crate::db::{StoreError, new_id, now_rfc3339};

/// Re-dates every entry of `grow_id` under `day_of`.
///
/// `day_of` maps a stored `occurred_at` to its grow day under a new
/// anchor — the Germination pick's recompute (ticket 04). Callers
/// run this inside the pick's transaction; an unmappable timestamp
/// aborts it before any row moves.
///
/// # Errors
///
/// Returns [`StoreError::Unparsable`] when a stored `occurred_at`
/// does not parse, and SQLite failures as-is.
pub fn redate(
    conn: &Connection,
    grow_id: &str,
    day_of: &dyn Fn(&str) -> Option<u16>,
) -> Result<usize, StoreError> {
    let rows: Vec<(String, String)> = conn
        .prepare("SELECT id, occurred_at FROM action_log WHERE grow_id = ?1")?
        .query_map(params![grow_id], |row| {
            Ok((
                row.get::<_, String>("id")?,
                row.get::<_, String>("occurred_at")?,
            ))
        })?
        .collect::<Result<_, _>>()?;
    let stamp = now_rfc3339();
    let mut moved = 0;
    for (row_id, occurred_at) in rows {
        let day =
            day_of(&occurred_at).ok_or_else(|| StoreError::Unparsable(occurred_at.clone()))?;
        conn.execute(
            "UPDATE action_log SET grow_day = ?2, updated_at = ?3 WHERE id = ?1",
            params![row_id, i64::from(day), stamp],
        )?;
        moved += 1;
    }
    Ok(moved)
}

/// Builds a stored entry from a payload; the caller supplies the
/// derived phase and grow day.
#[must_use]
pub fn from_input(grow_id: &str, input: &ActionInput, phase: Phase, grow_day: u16) -> ActionEntry {
    ActionEntry {
        id: new_id(),
        grow_id: grow_id.to_owned(),
        created_at: now_rfc3339(),
        updated_at: now_rfc3339(),
        plant_ids: input.plant_ids.clone(),
        occurred_at: input.occurred_at.clone(),
        action_type: input.action_type,
        phase: Some(phase),
        grow_day,
        details: input.details.clone(),
        notes: input.notes.clone(),
        appearance_tags: input.appearance_tags.clone(),
    }
}

/// Inserts a log entry.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
#[allow(clippy::missing_panics_doc)] // JSON of plain values cannot fail
pub fn create(conn: &Connection, entry: &ActionEntry) -> Result<(), StoreError> {
    conn.execute(
        "INSERT INTO action_log
             (id, grow_id, plant_ids, occurred_at, action_type, phase, grow_day,
              details, notes, appearance_tags, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            entry.id,
            entry.grow_id,
            serde_json::to_string(&entry.plant_ids).expect("list serializes"),
            entry.occurred_at,
            entry.action_type.label(),
            entry.phase.map(Phase::label),
            i64::from(entry.grow_day),
            serde_json::to_string(&entry.details).expect("struct serializes"),
            entry.notes,
            serde_json::to_string(&entry.appearance_tags).expect("list serializes"),
            entry.created_at,
            entry.updated_at,
        ],
    )?;
    Ok(())
}

/// Lists the entries that name one plant, newest first. `plant_ids`
/// is a JSON array, so the match is the id between the array's quotes.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
pub fn list_for_plant(conn: &Connection, plant_id: &str) -> Result<Vec<ActionEntry>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT * FROM action_log
         WHERE plant_ids LIKE ?1
         ORDER BY occurred_at DESC, created_at DESC",
    )?;
    let rows = stmt.query_map(params![format!("%\"{plant_id}\"%")], map_row)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

/// Lists a grow's entries, newest first, optionally filtered by type.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
pub fn list(
    conn: &Connection,
    grow_id: &str,
    action_type: Option<ActionType>,
    limit: u32,
) -> Result<Vec<ActionEntry>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT * FROM action_log
         WHERE grow_id = ?1 AND (?2 IS NULL OR action_type = ?2)
         ORDER BY occurred_at DESC, created_at DESC
         LIMIT ?3",
    )?;
    let rows = stmt.query_map(
        params![
            grow_id,
            action_type.map(ActionType::label),
            i64::from(limit)
        ],
        map_row,
    )?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

/// Lists a grow's entries inside a window (both ends inclusive),
/// newest first, capped at `limit` rows.
///
/// The window bounds compare as instants (`julianday`), so entries
/// stored with a non-UTC offset or fractional seconds still match;
/// the text sort only orders the read. A stamp SQLite cannot parse
/// can never be a marker (the epoch filter drops it), so the scan
/// may skip it.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
pub fn list_in_window(
    conn: &Connection,
    grow_id: &str,
    from: &str,
    to: &str,
    limit: u32,
) -> Result<Vec<ActionEntry>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT * FROM action_log
         WHERE grow_id = ?1
           AND julianday(occurred_at) >= julianday(?2)
           AND julianday(occurred_at) <= julianday(?3)
         ORDER BY occurred_at DESC, created_at DESC
         LIMIT ?4",
    )?;
    let rows = stmt.query_map(params![grow_id, from, to, i64::from(limit)], map_row)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

/// Fetches one entry by id.
///
/// # Errors
///
/// Returns [`super::NotFound`] when the id does not exist.
pub fn get(conn: &Connection, id: &str) -> Result<ActionEntry, StoreOrNotFound> {
    conn.query_row(
        "SELECT * FROM action_log WHERE id = ?1",
        params![id],
        map_row,
    )
    .optional()?
    .ok_or_else(|| NotFound.into())
}

/// Replaces an entry (full update), recomputing nothing (phase and day
/// were derived at creation; edits keep them unless the API says
/// otherwise).
///
/// # Errors
///
/// Returns [`super::NotFound`] when the id does not exist.
#[allow(clippy::missing_panics_doc)] // JSON of plain values cannot fail
pub fn update(
    conn: &Connection,
    id: &str,
    input: &ActionInput,
    phase: Phase,
    grow_day: u16,
) -> Result<ActionEntry, StoreOrNotFound> {
    let existing = get(conn, id)?;
    let mut updated = existing;
    updated.updated_at = now_rfc3339();
    updated.plant_ids.clone_from(&input.plant_ids);
    updated.occurred_at.clone_from(&input.occurred_at);
    updated.action_type = input.action_type;
    updated.phase = Some(phase);
    updated.grow_day = grow_day;
    updated.details.clone_from(&input.details);
    updated.notes.clone_from(&input.notes);
    updated.appearance_tags.clone_from(&input.appearance_tags);
    conn.execute(
        "UPDATE action_log SET plant_ids = ?2, occurred_at = ?3, action_type = ?4,
             phase = ?5, grow_day = ?6, details = ?7, notes = ?8,
             appearance_tags = ?9, updated_at = ?10
         WHERE id = ?1",
        params![
            updated.id,
            serde_json::to_string(&updated.plant_ids).expect("list serializes"),
            updated.occurred_at,
            updated.action_type.label(),
            updated.phase.map(Phase::label),
            i64::from(updated.grow_day),
            serde_json::to_string(&updated.details).expect("struct serializes"),
            updated.notes,
            serde_json::to_string(&updated.appearance_tags).expect("list serializes"),
            updated.updated_at,
        ],
    )?;
    Ok(updated)
}

/// Deletes an entry.
///
/// # Errors
///
/// Returns [`super::NotFound`] when the id does not exist.
pub fn delete(conn: &Connection, id: &str) -> Result<(), StoreOrNotFound> {
    let affected = conn.execute("DELETE FROM action_log WHERE id = ?1", params![id])?;
    if affected == 0 {
        return Err(NotFound.into());
    }
    Ok(())
}

/// The earliest grow day of the given action type in a grow, at or
/// before `up_to_day` (used to derive phase from logged flips and
/// harvests).
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
pub fn first_event_day(
    conn: &Connection,
    grow_id: &str,
    action_type: ActionType,
    up_to_day: u16,
) -> Result<Option<u16>, StoreError> {
    let day: Option<i64> = conn.query_row(
        "SELECT MIN(grow_day) FROM action_log
         WHERE grow_id = ?1 AND action_type = ?2 AND grow_day <= ?3",
        params![grow_id, action_type.label(), i64::from(up_to_day)],
        |row| row.get(0),
    )?;
    Ok(day.and_then(|value| u16::try_from(value).ok()))
}

/// The latest grow day of a Phase-change entry that moved the grow to
/// `target_phase` (D073): those rows carry the target label in the
/// `phase` column.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
pub fn latest_phase_change_day(
    conn: &Connection,
    grow_id: &str,
    target_phase: &str,
) -> Result<Option<u16>, StoreError> {
    let day: Option<i64> = conn.query_row(
        "SELECT MAX(grow_day) FROM action_log
         WHERE grow_id = ?1 AND action_type = 'phase_change' AND phase = ?2",
        params![grow_id, target_phase],
        |row| row.get(0),
    )?;
    Ok(day.and_then(|value| u16::try_from(value).ok()))
}

/// Maps one `action_log` row. Labels are written only by this module;
/// unknown values fall back safely rather than failing the read.
fn map_row(row: &rusqlite::Row<'_>) -> Result<ActionEntry, rusqlite::Error> {
    let action_type: String = row.get("action_type")?;
    let phase: Option<String> = row.get("phase")?;
    let plant_ids: String = row.get("plant_ids")?;
    let details: String = row.get("details")?;
    let tags: String = row.get("appearance_tags")?;
    let grow_day: i64 = row.get("grow_day")?;
    Ok(ActionEntry {
        id: row.get("id")?,
        grow_id: row.get("grow_id")?,
        plant_ids: serde_json::from_str(&plant_ids).unwrap_or_default(),
        occurred_at: row.get("occurred_at")?,
        action_type: ActionType::all()
            .into_iter()
            .find(|kind| kind.label() == action_type)
            .unwrap_or(ActionType::Note),
        phase: phase.and_then(|label| phase_from_label(&label)),
        grow_day: u16::try_from(grow_day.max(0)).unwrap_or(0),
        details: serde_json::from_str::<ActionDetails>(&details).unwrap_or_default(),
        notes: row.get("notes")?,
        appearance_tags: serde_json::from_str(&tags).unwrap_or_default(),
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

/// Parses a stored phase label.
fn phase_from_label(label: &str) -> Option<Phase> {
    Phase::all()
        .into_iter()
        .find(|phase| phase.label() == label)
}

/// Occurred-at + details JSON for the grow's feed and water entries
/// (the pH/EC event fields ride those entries), oldest first.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
pub fn detail_rows(conn: &Connection, grow_id: &str) -> Result<Vec<(String, String)>, StoreError> {
    let mut statement = conn.prepare(
        "SELECT occurred_at, details FROM action_log
         WHERE grow_id = ?1 AND action_type IN ('feed', 'water')
         ORDER BY occurred_at",
    )?;
    let rows = statement.query_map(params![grow_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}
