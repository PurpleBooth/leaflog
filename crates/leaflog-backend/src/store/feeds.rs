//! Feeding-schedule, frozen-plan and substrate-profile persistence (M6).

use leaflog_core::feed::{
    ActualTimeline, DoseUnit, EcBands, FeedPlan, FeedPlanDay, FeedSchedule, FittedPhase,
    ProductDoses, ScheduleProduct, SubstrateProfile,
};
use rusqlite::{Connection, OptionalExtension, params};

use super::StoreOrNotFound;
use crate::db::{StoreError, new_id, now_rfc3339};

/// A library row: the schedule plus the facts the list view shows.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct ScheduleSummary {
    /// The schedule itself, products included.
    pub schedule: FeedSchedule,
    /// How many product rows the grid has.
    pub product_count: i64,
    /// Distinct group labels, in first-seen order.
    pub groups: Vec<String>,
    /// Distinct dose units, in first-seen order.
    pub units: Vec<String>,
    /// How many grows reference this schedule.
    pub used_by: i64,
}

/// Derives the actual timeline from the forecast windows.
#[must_use]
pub fn actual_from_forecast(
    windows: &[leaflog_core::forecast::PhaseWindow],
) -> Option<ActualTimeline> {
    let seedling = windows
        .iter()
        .find(|w| w.phase == leaflog_core::Phase::Seedling)?;
    let veg = windows
        .iter()
        .find(|w| w.phase == leaflog_core::Phase::Vegetative)?;
    let flower = windows
        .iter()
        .find(|w| w.phase == leaflog_core::Phase::Flower)?;
    let dry = windows
        .iter()
        .find(|w| w.phase == leaflog_core::Phase::Drying)?;
    Some(ActualTimeline {
        seedling_days: u16::try_from((seedling.likely_end - seedling.likely_start).max(0)).ok()?,
        veg_days: u16::try_from((veg.likely_end - veg.likely_start).max(0)).ok()?,
        flower_days: u16::try_from((flower.likely_end - flower.likely_start).max(0)).ok()?,
        flush_days: u16::try_from((dry.likely_end - dry.likely_start).max(0)).ok()?,
    })
}

/// Lists the schedule library with list-view facts.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
#[allow(clippy::missing_panics_doc)] // JSON of plain values cannot fail
pub fn list_schedules(conn: &Connection) -> Result<Vec<ScheduleSummary>, StoreError> {
    let mut stmt = conn.prepare("SELECT id FROM feed_schedule ORDER BY name COLLATE NOCASE")?;
    let ids: Vec<String> = stmt
        .query_map([], |row| row.get("id"))?
        .collect::<Result<Vec<_>, _>>()?;
    let mut summaries = Vec::new();
    for id in ids {
        let schedule = match get_schedule(conn, &id) {
            Ok(schedule) => schedule,
            Err(super::StoreOrNotFound::NotFound(_)) => continue,
            Err(other) => return Err(store_error(other)),
        };
        let product_count = conn.query_row(
            "SELECT COUNT(*) FROM feed_schedule_product WHERE schedule_id = ?1",
            params![&id],
            |r| r.get(0),
        )?;
        let mut groups = Vec::new();
        {
            let mut stmt = conn.prepare(
                "SELECT DISTINCT group_label FROM feed_schedule_product
                 WHERE schedule_id = ?1 ORDER BY position",
            )?;
            for group in stmt.query_map(params![&id], |r| r.get::<_, String>(0))? {
                let group = group?;
                if !groups.contains(&group) {
                    groups.push(group);
                }
            }
        }
        let mut units = Vec::new();
        {
            let mut stmt = conn.prepare(
                "SELECT DISTINCT unit FROM feed_schedule_product WHERE schedule_id = ?1",
            )?;
            for unit in stmt.query_map(params![&id], |r| r.get::<_, String>(0))? {
                let unit = unit?;
                if !units.contains(&unit) {
                    units.push(unit);
                }
            }
        }
        let used_by = conn.query_row(
            "SELECT COUNT(*) FROM grow WHERE feed_schedule_id = ?1",
            params![&id],
            |r| r.get(0),
        )?;
        summaries.push(ScheduleSummary {
            schedule,
            product_count,
            groups,
            units,
            used_by,
        });
    }
    Ok(summaries)
}

fn store_error(other: StoreOrNotFound) -> StoreError {
    match other {
        super::StoreOrNotFound::Store(error) => error,
        super::StoreOrNotFound::NotFound(_) => {
            StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows)
        }
    }
}

/// Fetches one schedule with its product rows.
///
/// # Errors
///
/// Returns [`super::NotFound`] when the id does not exist.
#[allow(clippy::missing_panics_doc)] // JSON of plain values cannot fail
pub fn get_schedule(conn: &Connection, id: &str) -> Result<FeedSchedule, StoreOrNotFound> {
    let mut schedule = conn
        .query_row(
            "SELECT id, name, source, flower_weeks, created_at, updated_at
             FROM feed_schedule WHERE id = ?1",
            params![id],
            |row| {
                Ok(FeedSchedule {
                    id: row.get("id")?,
                    name: row.get("name")?,
                    source: row.get("source")?,
                    flower_weeks: u8::try_from(row.get::<_, i64>("flower_weeks")?).unwrap_or(8),
                    products: Vec::new(),
                    created_at: row.get("created_at")?,
                    updated_at: row.get("updated_at")?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| super::StoreOrNotFound::NotFound(super::NotFound))?;
    let mut stmt = conn.prepare(
        "SELECT id, name, unit, group_label, note, doses
         FROM feed_schedule_product WHERE schedule_id = ?1 ORDER BY position",
    )?;
    let products = stmt
        .query_map(params![id], |row| {
            let doses_json: String = row.get("doses")?;
            let unit_json: String = row.get("unit")?;
            Ok(ScheduleProduct {
                id: row.get("id")?,
                name: row.get("name")?,
                unit: serde_json::from_str(&unit_json).unwrap_or(DoseUnit::Milliliters),
                group: row.get("group_label")?,
                note: row.get("note")?,
                doses: serde_json::from_str::<ProductDoses>(&doses_json).unwrap_or(ProductDoses {
                    propagation: None,
                    veg: None,
                    flower: Vec::new(),
                    water: None,
                }),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    schedule.products = products;
    Ok(schedule)
}

/// Fetches one schedule, or `None` when absent.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
pub fn try_get_schedule(conn: &Connection, id: &str) -> Result<Option<FeedSchedule>, StoreError> {
    match get_schedule(conn, id) {
        Ok(schedule) => Ok(Some(schedule)),
        Err(super::StoreOrNotFound::NotFound(_)) => Ok(None),
        Err(other) => Err(store_error(other)),
    }
}

/// Inserts or updates a schedule (products replaced wholesale).
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
#[allow(clippy::missing_panics_doc)] // JSON of plain values cannot fail
pub fn put_schedule(conn: &Connection, schedule: &FeedSchedule) -> Result<(), StoreError> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "INSERT INTO feed_schedule (id, name, source, flower_weeks, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)
         ON CONFLICT(id) DO UPDATE SET
             name = ?2, source = ?3, flower_weeks = ?4, updated_at = ?6",
        params![
            schedule.id,
            schedule.name,
            schedule.source,
            i64::from(schedule.flower_weeks),
            schedule.created_at,
            schedule.updated_at,
        ],
    )?;
    tx.execute(
        "DELETE FROM feed_schedule_product WHERE schedule_id = ?1",
        params![schedule.id],
    )?;
    let now = now_rfc3339();
    for (position, product) in schedule.products.iter().enumerate() {
        tx.execute(
            "INSERT INTO feed_schedule_product
                 (id, schedule_id, position, name, unit, group_label, note, doses,
                  created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
            params![
                product.id,
                schedule.id,
                i64::try_from(position).unwrap_or(0),
                product.name,
                serde_json::to_string(&product.unit).expect("DoseUnit serializes"),
                product.group,
                product.note,
                serde_json::to_string(&product.doses).expect("ProductDoses serializes"),
                now,
                now,
            ],
        )?;
    }
    tx.commit()?;
    Ok(())
}

/// Deletes a schedule, nulling the grow references first.
///
/// # Errors
///
/// Returns [`super::NotFound`] when the id does not exist.
pub fn delete_schedule(conn: &Connection, id: &str) -> Result<(), StoreOrNotFound> {
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "UPDATE grow SET feed_schedule_id = NULL WHERE feed_schedule_id = ?1",
        params![id],
    )?;
    tx.execute(
        "DELETE FROM feed_schedule_product WHERE schedule_id = ?1",
        params![id],
    )?;
    let affected = tx.execute("DELETE FROM feed_schedule WHERE id = ?1", params![id])?;
    tx.commit()?;
    if affected == 0 {
        return Err(super::StoreOrNotFound::NotFound(super::NotFound));
    }
    Ok(())
}

/// Duplicates a schedule under a fresh id with ` (copy)` appended.
///
/// # Errors
///
/// Returns [`super::NotFound`] when the id does not exist.
pub fn duplicate_schedule(conn: &Connection, id: &str) -> Result<FeedSchedule, StoreOrNotFound> {
    let mut schedule = get_schedule(conn, id)?;
    schedule.id = new_id();
    schedule.name = format!("{} (copy)", schedule.name);
    schedule.created_at = now_rfc3339();
    schedule.updated_at = schedule.created_at.clone();
    for product in &mut schedule.products {
        product.id = new_id();
    }
    put_schedule(conn, &schedule)?;
    Ok(schedule)
}

/// Lists the grow ids that reference a schedule.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
pub fn schedule_grow_ids(conn: &Connection, schedule_id: &str) -> Result<Vec<String>, StoreError> {
    let mut stmt = conn.prepare("SELECT id FROM grow WHERE feed_schedule_id = ?1 ORDER BY name")?;
    let ids = stmt
        .query_map(params![schedule_id], |row| row.get(0))?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ids)
}

/// Fits a schedule onto a grow's actual timeline and stores the frozen
/// plan. Returns the stored plan.
///
/// # Errors
///
/// Returns a [`StoreOrNotFound`] on SQLite failures.
#[allow(clippy::missing_panics_doc)] // JSON of plain values cannot fail
pub fn fit_and_store(
    conn: &Connection,
    grow_id: &str,
    schedule: &FeedSchedule,
    actual: ActualTimeline,
) -> Result<FeedPlan, StoreOrNotFound> {
    let result = leaflog_core::feed::fit_schedule(schedule, &actual);
    let fitted_at = now_rfc3339();
    let plan = FeedPlan {
        schedule_id: schedule.id.clone(),
        schedule_name: schedule.name.clone(),
        flower_weeks: schedule.flower_weeks,
        fitted_at,
        days: result.days,
    };

    // One transaction: a failed INSERT must roll back the DELETE so the
    // grow keeps its previously stored plan (D012 single-connection
    // design, hence `unchecked_transaction` on the shared `&Connection`).
    let tx = conn.unchecked_transaction()?;
    tx.execute(
        "DELETE FROM feed_plan_day WHERE grow_id = ?1",
        params![grow_id],
    )?;
    for day in &plan.days {
        tx.execute(
            "INSERT INTO feed_plan_day (id, grow_id, grow_day, phase, week, doses, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                new_id(),
                grow_id,
                i64::from(day.grow_day),
                day.phase.label(),
                day.week.map(i64::from),
                serde_json::to_string(&day.doses).expect("FittedDose serializes"),
                now_rfc3339(),
            ],
        )?;
    }
    tx.execute(
        "INSERT INTO feed_plan (grow_id, schedule_id, schedule_name, flower_weeks, fitted_at)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(grow_id) DO UPDATE SET
             schedule_id = ?2, schedule_name = ?3, flower_weeks = ?4, fitted_at = ?5",
        params![
            grow_id,
            plan.schedule_id,
            plan.schedule_name,
            i64::from(plan.flower_weeks),
            plan.fitted_at,
        ],
    )?;
    tx.commit()?;
    Ok(plan)
}

/// Reads a grow's stored plan, or `None` when the grow has none.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
#[allow(clippy::missing_panics_doc)] // JSON of plain values cannot fail
pub fn plan(conn: &Connection, grow_id: &str) -> Result<Option<FeedPlan>, StoreError> {
    let header = conn
        .query_row(
            "SELECT schedule_id, schedule_name, flower_weeks, fitted_at
             FROM feed_plan WHERE grow_id = ?1",
            params![grow_id],
            |row| {
                Ok((
                    row.get::<_, String>("schedule_id")?,
                    row.get::<_, String>("schedule_name")?,
                    row.get::<_, i64>("flower_weeks")?,
                    row.get::<_, String>("fitted_at")?,
                ))
            },
        )
        .optional()?;
    let Some((schedule_id, schedule_name, flower_weeks, fitted_at)) = header else {
        return Ok(None);
    };
    let mut stmt = conn.prepare(
        "SELECT grow_day, phase, week, doses
         FROM feed_plan_day WHERE grow_id = ?1 ORDER BY grow_day",
    )?;
    let days = stmt
        .query_map(params![grow_id], |row| {
            let doses_json: String = row.get("doses")?;
            let phase_label: String = row.get("phase")?;
            let week: Option<i64> = row.get("week")?;
            Ok(FeedPlanDay {
                grow_day: u16::try_from(row.get::<_, i64>("grow_day")?).unwrap_or(0),
                phase: FittedPhase::from_label(&phase_label),
                week: week.and_then(|w| u8::try_from(w).ok()),
                doses: serde_json::from_str(&doses_json).unwrap_or_default(),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Some(FeedPlan {
        schedule_id,
        schedule_name,
        flower_weeks: u8::try_from(flower_weeks).unwrap_or(0),
        fitted_at,
        days,
    }))
}

/// Upserts a substrate profile.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
#[allow(clippy::missing_panics_doc)] // JSON of plain values cannot fail
pub fn upsert_profile(conn: &Connection, profile: &SubstrateProfile) -> Result<(), StoreError> {
    conn.execute(
        "INSERT INTO substrate_profile
             (id, label, ph_min, ph_max, watering_style, flush_days_min, flush_days_max,
              runoff_check, ec_bands, source, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
         ON CONFLICT(label) DO UPDATE SET
             ph_min = ?3, ph_max = ?4, watering_style = ?5,
             flush_days_min = ?6, flush_days_max = ?7, runoff_check = ?8,
             ec_bands = ?9, source = ?10, updated_at = ?12",
        params![
            new_id(),
            profile.label,
            profile.ph_min,
            profile.ph_max,
            profile.watering_style,
            i64::from(profile.flush_days_min),
            i64::from(profile.flush_days_max),
            profile.runoff_check,
            serde_json::to_string(&profile.ec_bands).expect("EcBands serializes"),
            profile.source,
            now_rfc3339(),
            now_rfc3339(),
        ],
    )?;
    Ok(())
}

/// Lists substrate profiles.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
pub fn list_profiles(conn: &Connection) -> Result<Vec<SubstrateProfile>, StoreError> {
    let mut stmt = conn.prepare("SELECT * FROM substrate_profile ORDER BY label")?;
    let rows = stmt.query_map([], |row| {
        let ec_json: String = row.get("ec_bands")?;
        Ok(SubstrateProfile {
            label: row.get("label")?,
            ph_min: row.get("ph_min")?,
            ph_max: row.get("ph_max")?,
            watering_style: row.get("watering_style")?,
            flush_days_min: u16::try_from(row.get::<_, i64>("flush_days_min")?).unwrap_or(0),
            flush_days_max: u16::try_from(row.get::<_, i64>("flush_days_max")?).unwrap_or(0),
            runoff_check: row.get("runoff_check")?,
            ec_bands: serde_json::from_str::<EcBands>(&ec_json).unwrap_or(EcBands {
                seedling: (0, 0),
                early_veg: (0, 0),
                late_veg: (0, 0),
                early_flower: (0, 0),
                peak_flower: (0, 0),
                late_flower: (0, 0),
                flush: (0, 0),
            }),
            source: row.get("source")?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

/// Counts substrate profiles (seed check).
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
pub fn profile_count(conn: &Connection) -> Result<i64, StoreError> {
    Ok(conn.query_row("SELECT COUNT(*) FROM substrate_profile", [], |r| r.get(0))?)
}
