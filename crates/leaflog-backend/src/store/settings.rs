//! Settings persistence: a JSON key-value table (spec §8 `settings`).

use rusqlite::{Connection, OptionalExtension, params};
use serde_json::Value;

use super::NotFound;
use crate::db::{StoreError, now_rfc3339};

/// Reads a setting's JSON value.
///
/// # Errors
///
/// Returns [`super::NotFound`] when the key was never set.
pub fn get(conn: &Connection, key: &str) -> Result<Value, super::StoreOrNotFound> {
    conn.query_row(
        "SELECT value FROM settings WHERE key = ?1",
        params![key],
        |row| row.get::<_, String>(0),
    )
    .optional()?
    .and_then(|raw| serde_json::from_str(&raw).ok())
    .ok_or_else(|| NotFound.into())
}

/// Upserts a setting's JSON value.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
///
/// # Panics
///
/// Cannot panic: a `serde_json::Value` always serializes.
pub fn put(conn: &Connection, key: &str, value: &Value) -> Result<(), StoreError> {
    let raw = serde_json::to_string(value).expect("JSON value serializes");
    conn.execute(
        "INSERT INTO settings (key, value, updated_at) VALUES (?1, ?2, ?3)
         ON CONFLICT(key) DO UPDATE SET value = ?2, updated_at = ?3",
        params![key, raw, now_rfc3339()],
    )?;
    Ok(())
}

/// Lists all settings as a JSON object.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
pub fn list(conn: &Connection) -> Result<Value, StoreError> {
    let mut stmt = conn.prepare("SELECT key, value FROM settings ORDER BY key")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
    })?;
    let mut map = serde_json::Map::new();
    for row in rows {
        let (key, raw) = row?;
        if let Ok(value) = serde_json::from_str(&raw) {
            map.insert(key, value);
        }
    }
    Ok(Value::Object(map))
}
