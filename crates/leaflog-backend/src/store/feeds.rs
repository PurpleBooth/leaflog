//! Feed template, plan-day and substrate-profile persistence (M6).

use leaflog_core::feed::{
    ActualTimeline, EcBands, FeedPhase, FeedPlanDay, FeedRow, FitConfig, SubstrateProfile,
    TemplateTimeline, fit_feed,
};
use rusqlite::{Connection, OptionalExtension, params};

use super::StoreOrNotFound;
use crate::db::{StoreError, new_id, now_rfc3339};

/// A stored feed template with its rows.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct FeedTemplate {
    /// ULID.
    pub id: String,
    /// User-facing name.
    pub name: String,
    /// Substrate label.
    pub substrate: String,
    /// Provenance.
    pub source: Option<String>,
    /// True for the bundled generic templates.
    pub is_builtin: bool,
    /// Rows in order.
    pub rows: Vec<FeedRow>,
    /// Creation instant.
    pub created_at: String,
    /// Last modification instant.
    pub updated_at: String,
}

/// Derives the template timeline from the rows (segment spans).
#[must_use]
pub fn timeline_from_rows(rows: &[FeedRow]) -> TemplateTimeline {
    let span = |phase_check: fn(FeedPhase) -> bool| -> u16 {
        rows.iter()
            .filter(|row| phase_check(row.phase))
            .map(|row| row.day_offset + 7) // assume 1 week per row
            .max()
            .unwrap_or(0)
    };
    TemplateTimeline {
        seedling_days: span(|p| p == FeedPhase::Seedling),
        veg_days: span(|p| matches!(p, FeedPhase::EarlyVeg | FeedPhase::LateVeg)),
        flower_days: span(|p| {
            matches!(
                p,
                FeedPhase::EarlyFlower | FeedPhase::MidFlower | FeedPhase::LateFlower
            )
        }),
        ripen_flush_days: span(FeedPhase::is_protected),
    }
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

/// Fits a template onto a grow's actual timeline and stores the plan
/// days. Returns the fit result.
///
/// # Errors
///
/// Returns a [`StoreOrNotFound`] on SQLite failures or missing rows.
#[allow(clippy::missing_panics_doc)] // JSON of plain values cannot fail
pub fn fit_and_store(
    conn: &Connection,
    grow_id: &str,
    template: &FeedTemplate,
    actual: ActualTimeline,
    config: FitConfig,
) -> Result<leaflog_core::feed::FitResult, StoreOrNotFound> {
    let template_timeline = timeline_from_rows(&template.rows);
    let result = fit_feed(&template.rows, template_timeline, actual, config);

    conn.execute(
        "DELETE FROM feed_plan_day WHERE grow_id = ?1",
        params![grow_id],
    )?;
    for day in &result.days {
        conn.execute(
            "INSERT INTO feed_plan_day (id, grow_id, grow_day, phase, ec_target_us, ph_min, ph_max, protected, created_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            params![
                new_id(),
                grow_id,
                i64::from(day.grow_day),
                day.phase.label(),
                i64::from(day.ec_target_us),
                day.ph_min,
                day.ph_max,
                i64::from(day.protected),
                now_rfc3339(),
            ],
        )?;
    }
    Ok(result)
}

/// Lists a grow's stored plan days.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
pub fn plan_days(conn: &Connection, grow_id: &str) -> Result<Vec<FeedPlanDay>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT grow_day, phase, ec_target_us, ph_min, ph_max, protected
         FROM feed_plan_day WHERE grow_id = ?1 ORDER BY grow_day",
    )?;
    let days = stmt
        .query_map(params![grow_id], |row| {
            let grow_day: i64 = row.get("grow_day")?;
            let phase: String = row.get("phase")?;
            let ec: i64 = row.get("ec_target_us")?;
            let protected: i64 = row.get("protected")?;
            Ok(FeedPlanDay {
                grow_day: u16::try_from(grow_day).unwrap_or(0),
                phase: FeedPhase::all()
                    .into_iter()
                    .find(|p| p.label() == phase)
                    .unwrap_or(FeedPhase::Seedling),
                ec_target_us: u32::try_from(ec).unwrap_or(0),
                ph_min: row.get("ph_min")?,
                ph_max: row.get("ph_max")?,
                protected: protected != 0,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(days)
}

/// Lists feed templates, optionally filtered by substrate.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
pub fn list_templates(
    conn: &Connection,
    substrate: Option<&str>,
) -> Result<Vec<FeedTemplate>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT * FROM feed_template WHERE (?1 IS NULL OR substrate = ?1) ORDER BY name",
    )?;
    let rows = stmt.query_map(params![substrate], |row| {
        Ok(FeedTemplate {
            id: row.get("id")?,
            name: row.get("name")?,
            substrate: row.get("substrate")?,
            source: row.get("source")?,
            is_builtin: row.get::<_, i64>("is_builtin")? != 0,
            rows: Vec::new(),
            created_at: row.get("created_at")?,
            updated_at: row.get("updated_at")?,
        })
    })?;
    let mut templates = Vec::new();
    for row in rows {
        templates.push(row?);
    }
    // Load rows for each template.
    for template in &mut templates {
        template.rows = template_rows(conn, &template.id)?;
    }
    Ok(templates)
}

/// Fetches one template with rows.
///
/// # Errors
///
/// Returns [`super::NotFound`] when the id does not exist.
pub fn get_template(conn: &Connection, id: &str) -> Result<FeedTemplate, StoreOrNotFound> {
    let mut template = conn
        .query_row(
            "SELECT * FROM feed_template WHERE id = ?1",
            params![id],
            |row| {
                Ok(FeedTemplate {
                    id: row.get("id")?,
                    name: row.get("name")?,
                    substrate: row.get("substrate")?,
                    source: row.get("source")?,
                    is_builtin: row.get::<_, i64>("is_builtin")? != 0,
                    rows: Vec::new(),
                    created_at: row.get("created_at")?,
                    updated_at: row.get("updated_at")?,
                })
            },
        )
        .optional()?
        .ok_or_else(|| super::StoreOrNotFound::NotFound(super::NotFound))?;
    template.rows = template_rows(conn, id)?;
    Ok(template)
}

/// Loads a template's rows.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
pub fn template_rows(conn: &Connection, template_id: &str) -> Result<Vec<FeedRow>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT phase, day_offset, ec_target_us, ph_min, ph_max
         FROM feed_template_row WHERE template_id = ?1 ORDER BY day_offset",
    )?;
    let rows = stmt.query_map(params![template_id], |row| {
        let phase: String = row.get("phase")?;
        Ok(FeedRow {
            phase: FeedPhase::all()
                .into_iter()
                .find(|p| p.label() == phase)
                .unwrap_or(FeedPhase::Seedling),
            day_offset: u16::try_from(row.get::<_, i64>("day_offset")?).unwrap_or(0),
            ec_target_us: u32::try_from(row.get::<_, i64>("ec_target_us")?).unwrap_or(0),
            ph_min: row.get("ph_min")?,
            ph_max: row.get("ph_max")?,
        })
    })?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
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
