//! Sensor binding and snapshot persistence (spec §4.2, M4).

use std::collections::BTreeMap;

use rusqlite::{Connection, params};

use crate::db::{StoreError, new_id, now_rfc3339};

/// The binding roles the app understands (spec §4.2 auto-capture list).
pub const ROLES: &[&str] = &[
    "temperature",
    "humidity",
    "co2",
    "soil_moisture",
    "light",
    "reservoir_ph",
    "reservoir_ec",
];

/// A grow's bindings: role → entity id.
pub type Bindings = BTreeMap<String, String>;

/// Payload to replace a grow's bindings.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct BindingsInput {
    /// Role → Home Assistant entity id; roles not present are unbound.
    pub bindings: Bindings,
}

/// Replaces a grow's bindings wholesale.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures or unknown roles.
pub fn replace(conn: &Connection, grow_id: &str, input: &BindingsInput) -> Result<(), StoreError> {
    for role in input.bindings.keys() {
        if !ROLES.contains(&role.as_str()) {
            return Err(StoreError::Sqlite(rusqlite::Error::InvalidParameterName(
                format!("unknown binding role {role}"),
            )));
        }
    }
    conn.execute(
        "DELETE FROM sensor_binding WHERE grow_id = ?1",
        params![grow_id],
    )?;
    for (role, entity_id) in &input.bindings {
        conn.execute(
            "INSERT INTO sensor_binding (grow_id, role, entity_id, updated_at)
             VALUES (?1, ?2, ?3, ?4)",
            params![grow_id, role, entity_id, now_rfc3339()],
        )?;
    }
    Ok(())
}

/// Reads a grow's bindings.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
pub fn list(conn: &Connection, grow_id: &str) -> Result<Bindings, StoreError> {
    let mut stmt = conn.prepare("SELECT role, entity_id FROM sensor_binding WHERE grow_id = ?1")?;
    let rows = stmt.query_map(params![grow_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut map = Bindings::new();
    for row in rows {
        let (role, entity_id) = row?;
        map.insert(role, entity_id);
    }
    Ok(map)
}

/// The union of entity ids bound by any grow (the WS track set).
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
pub fn all_bound_entities(conn: &Connection) -> Result<Vec<String>, StoreError> {
    let mut stmt =
        conn.prepare("SELECT DISTINCT entity_id FROM sensor_binding ORDER BY entity_id")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

/// Every grow binding `entity_id` (for auto-log fan-out).
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
pub fn grow_ids_bound_to(conn: &Connection, entity_id: &str) -> Result<Vec<String>, StoreError> {
    let mut stmt = conn.prepare("SELECT grow_id FROM sensor_binding WHERE entity_id = ?1")?;
    let rows = stmt.query_map(params![entity_id], |row| row.get::<_, String>(0))?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

/// One captured sensor value attached to a log entry.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SensorSnapshot {
    /// ULID.
    pub id: String,
    /// The captured role.
    pub role: String,
    /// The entity it came from.
    pub entity_id: String,
    /// The raw state string.
    pub state: String,
    /// Capture time, RFC 3339 UTC.
    pub captured_at: String,
}

/// Writes one snapshot row.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
pub fn snapshot(
    conn: &Connection,
    action_log_id: &str,
    role: &str,
    entity_id: &str,
    state: &str,
) -> Result<(), StoreError> {
    conn.execute(
        "INSERT INTO sensor_snapshot (id, action_log_id, role, entity_id, state, captured_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            new_id(),
            action_log_id,
            role,
            entity_id,
            state,
            now_rfc3339()
        ],
    )?;
    Ok(())
}

/// Reads a log entry's snapshots.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
pub fn snapshots_for(
    conn: &Connection,
    action_log_id: &str,
) -> Result<Vec<SensorSnapshot>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT id, role, entity_id, state, captured_at
         FROM sensor_snapshot WHERE action_log_id = ?1 ORDER BY role",
    )?;
    let rows = stmt.query_map(params![action_log_id], |row| {
        Ok(SensorSnapshot {
            id: row.get("id")?,
            role: row.get("role")?,
            entity_id: row.get("entity_id")?,
            state: row.get("state")?,
            captured_at: row.get("captured_at")?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}
