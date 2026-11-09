//! Equipment inventory persistence (spec §4.1).

use leaflog_core::equipment::{EquipmentCategory, EquipmentInput, EquipmentItem};
use rusqlite::{Connection, OptionalExtension, params};

use super::{NotFound, StoreOrNotFound};
use crate::db::{StoreError, new_id, now_rfc3339};

/// Inserts a new equipment item and returns the stored row.
///
/// # Errors
///
/// Returns a [`crate::db::StoreError`] on SQLite failures.
pub fn create(conn: &Connection, input: &EquipmentInput) -> Result<EquipmentItem, StoreError> {
    let item = EquipmentItem {
        id: new_id(),
        name: input.name.clone(),
        category: input.category,
        brand_model: input.brand_model.clone(),
        real_watts: input.real_watts,
        dimmable: input.dimmable,
        purchase_date: input.purchase_date.clone(),
        purchase_price: input.purchase_price,
        currency: input.currency.clone(),
        notes: input.notes.clone(),
        created_at: now_rfc3339(),
        updated_at: now_rfc3339(),
    };
    conn.execute(
        "INSERT INTO equipment_item
             (id, name, category, brand_model, real_watts, dimmable,
              purchase_date, purchase_price, currency, notes,
              created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
        params![
            item.id,
            item.name,
            item.category.label(),
            item.brand_model,
            item.real_watts,
            i64::from(item.dimmable),
            item.purchase_date,
            item.purchase_price,
            item.currency,
            item.notes,
            item.created_at,
            item.updated_at,
        ],
    )?;
    Ok(item)
}

/// Counts all equipment rows.
///
/// # Errors
///
/// Returns a [`crate::db::StoreError`] on SQLite failures.
pub fn count(conn: &Connection) -> Result<i64, StoreError> {
    Ok(conn.query_row("SELECT COUNT(*) FROM equipment_item", [], |r| r.get(0))?)
}

/// Lists equipment, optionally filtered by category, ordered by name.
///
/// # Errors
///
/// Returns a [`crate::db::StoreError`] on SQLite failures.
pub fn list(
    conn: &Connection,
    category: Option<EquipmentCategory>,
) -> Result<Vec<EquipmentItem>, StoreError> {
    if let Some(category) = category {
        let mut stmt =
            conn.prepare("SELECT * FROM equipment_item WHERE category = ?1 ORDER BY name")?;
        let rows = stmt.query_map(params![category.label()], map_row)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    } else {
        let mut stmt = conn.prepare("SELECT * FROM equipment_item ORDER BY name")?;
        let rows = stmt.query_map([], map_row)?;
        rows.collect::<Result<Vec<_>, _>>()
            .map_err(StoreError::from)
    }
}

/// Fetches one item by id.
///
/// # Errors
///
/// Returns [`super::NotFound`] when the id does not exist.
pub fn get(conn: &Connection, id: &str) -> Result<EquipmentItem, StoreOrNotFound> {
    conn.query_row(
        "SELECT * FROM equipment_item WHERE id = ?1",
        params![id],
        map_row,
    )
    .optional()?
    .ok_or_else(|| NotFound.into())
}

/// Replaces an item (full update) and returns the updated row.
///
/// # Errors
///
/// Returns [`super::NotFound`] when the id does not exist.
pub fn update(
    conn: &Connection,
    id: &str,
    input: &EquipmentInput,
) -> Result<EquipmentItem, StoreOrNotFound> {
    let existing = get(conn, id)?;
    let updated = EquipmentItem {
        created_at: existing.created_at,
        updated_at: now_rfc3339(),
        id: existing.id,
        name: input.name.clone(),
        category: input.category,
        brand_model: input.brand_model.clone(),
        real_watts: input.real_watts,
        dimmable: input.dimmable,
        purchase_date: input.purchase_date.clone(),
        purchase_price: input.purchase_price,
        currency: input.currency.clone(),
        notes: input.notes.clone(),
    };
    conn.execute(
        "UPDATE equipment_item
         SET name = ?2, category = ?3, brand_model = ?4, real_watts = ?5,
             dimmable = ?6, purchase_date = ?7, purchase_price = ?8,
             currency = ?9, notes = ?10, updated_at = ?11
         WHERE id = ?1",
        params![
            updated.id,
            updated.name,
            updated.category.label(),
            updated.brand_model,
            updated.real_watts,
            i64::from(updated.dimmable),
            updated.purchase_date,
            updated.purchase_price,
            updated.currency,
            updated.notes,
            updated.updated_at,
        ],
    )?;
    Ok(updated)
}

/// Deletes an item.
///
/// # Errors
///
/// Returns [`super::NotFound`] when the id does not exist.
pub fn delete(conn: &Connection, id: &str) -> Result<(), StoreOrNotFound> {
    let affected = conn.execute("DELETE FROM equipment_item WHERE id = ?1", params![id])?;
    if affected == 0 {
        return Err(NotFound.into());
    }
    Ok(())
}

/// Maps one result row. Column order matches the `SELECT *` of the
/// migration's `equipment_item` table.
fn map_row(row: &rusqlite::Row<'_>) -> Result<EquipmentItem, rusqlite::Error> {
    let category_label: String = row.get("category")?;
    let category = EquipmentCategory::all()
        .into_iter()
        .find(|candidate| candidate.label() == category_label)
        .ok_or(rusqlite::Error::FromSqlConversionFailure(
            2,
            rusqlite::types::Type::Text,
            Box::from(format!("unknown equipment category {category_label}")),
        ))?;
    let dimmable: i64 = row.get("dimmable")?;
    Ok(EquipmentItem {
        id: row.get("id")?,
        name: row.get("name")?,
        category,
        brand_model: row.get("brand_model")?,
        real_watts: row.get("real_watts")?,
        dimmable: dimmable != 0,
        purchase_date: row.get("purchase_date")?,
        purchase_price: row.get("purchase_price")?,
        currency: row.get("currency")?,
        notes: row.get("notes")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}
