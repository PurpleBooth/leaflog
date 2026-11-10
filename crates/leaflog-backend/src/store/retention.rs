//! Sensor-snapshot retention (spec §4.2).
//!
//! Auto-captured snapshots would accumulate forever, so a
//! settings-configurable prune (default 24 months) thins old rows to
//! at most one per hour, always keeping the first and last row of
//! each day and every excursion-linked row.

use rusqlite::Connection;

use super::settings;
use crate::db::StoreError;

/// The settings key holding the retention window in months.
pub const RETENTION_SETTING: &str = "snapshot_retention_months";

/// The default retention window (spec §4.2).
pub const DEFAULT_RETENTION_MONTHS: u32 = 24;

/// Reads the configured retention window in months.
///
/// # Errors
///
/// Returns a [`crate::db::StoreError`] when the settings read fails.
pub fn retention_months(conn: &Connection) -> Result<u32, StoreError> {
    Ok(settings::get(conn, RETENTION_SETTING)
        .ok()
        .and_then(|value| value.as_u64())
        .and_then(|months| u32::try_from(months).ok())
        .unwrap_or(DEFAULT_RETENTION_MONTHS))
}

/// Prunes snapshots captured strictly before `cutoff` (RFC 3339).
///
/// At most one row per (entity, hour) survives — the earliest — and
/// with it every first and last row of an entity's day and every
/// excursion-linked row (a snapshot attached to an auto-logged entry).
/// Rows inside the window are untouched. Returns how many rows went.
///
/// # Errors
///
/// Returns a [`crate::db::StoreError`] on SQLite failures.
pub fn prune(conn: &Connection, cutoff: &str) -> Result<u64, StoreError> {
    let affected = conn.execute(
        "DELETE FROM sensor_snapshot
         WHERE captured_at < ?1
           AND id NOT IN (
               -- One per (entity, hour): the earliest captures the
               -- hour's shape.
               SELECT keep_id FROM (
                   SELECT id AS keep_id,
                          ROW_NUMBER() OVER (
                              PARTITION BY entity_id, substr(captured_at, 1, 13)
                              ORDER BY captured_at, id
                          ) AS rn
                   FROM sensor_snapshot
                   WHERE captured_at < ?1
               ) WHERE rn = 1
           )
           AND id NOT IN (
               -- The first row of each (entity, day).
               SELECT keep_id FROM (
                   SELECT id AS keep_id,
                          ROW_NUMBER() OVER (
                              PARTITION BY entity_id, substr(captured_at, 1, 10)
                              ORDER BY captured_at, id
                          ) AS rn
                   FROM sensor_snapshot
                   WHERE captured_at < ?1
               ) WHERE rn = 1
           )
           AND id NOT IN (
               -- The last row of each (entity, day): the day's close is
               -- its own evidence even when an earlier row won the
               -- hour.
               SELECT keep_id FROM (
                   SELECT id AS keep_id,
                          ROW_NUMBER() OVER (
                              PARTITION BY entity_id, substr(captured_at, 1, 10)
                              ORDER BY captured_at DESC, id
                          ) AS rn
                   FROM sensor_snapshot
                   WHERE captured_at < ?1
               ) WHERE rn = 1
           )
           AND id NOT IN (
               -- Excursion-linked rows: snapshots attached to an
               -- auto-logged entry are evidence of the excursion
               -- itself.
               SELECT snapshot.id
               FROM sensor_snapshot snapshot
               JOIN action_log entry ON entry.id = snapshot.action_log_id
               WHERE entry.notes LIKE 'auto:%'
           )",
        [cutoff],
    )?;
    u64::try_from(affected)
        .map_err(|_| StoreError::Migration("affected row count exceeded u64".to_owned()))
}
