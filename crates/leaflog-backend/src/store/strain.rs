//! Strain persistence (spec §4.4).

use std::collections::BTreeMap;

use leaflog_core::strain::{EvidenceTag, Strain, StrainInput};
use rusqlite::{Connection, OptionalExtension, params};

use super::{NotFound, StoreOrNotFound};
use crate::db::{StoreError, new_id, now_rfc3339};

/// Inserts a strain and returns the stored row.
///
/// # Errors
///
/// Returns a [`crate::db::StoreError`] on SQLite failures.
///
/// # Panics
///
/// Cannot panic: JSON serialization of plain strings and maps always
/// succeeds.
pub fn create(conn: &Connection, strain: &Strain) -> Result<(), StoreError> {
    conn.execute(
        "INSERT INTO strain
             (id, name, breeder, photoperiod, sex, indica_pct, landrace_tags,
              flower_days_min, flower_days_max, stretch_factor,
              veg_weeks_min, veg_weeks_max, height_class,
              auto_total_days_min, auto_total_days_max,
              yield_notes, terpene_notes, effect_notes,
              heat_tolerance, cold_tolerance, light_tolerance,
              user_rating, notes, evidence, source_url, source_licence,
              is_phenotype_of, created_at, updated_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13,
                 ?14, ?15, ?16, ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24,
                 ?25, ?26, ?27, ?28, ?29)",
        params![
            strain.id,
            strain.name,
            strain.breeder,
            photoperiod_label(strain),
            sex_label(strain),
            strain.indica_pct.map(i64::from),
            serde_json::to_string(&strain.landrace_tags).expect("Vec<String> serializes"),
            strain.flower_days_min.map(i64::from),
            strain.flower_days_max.map(i64::from),
            strain.stretch_factor,
            strain.veg_weeks_min.map(i64::from),
            strain.veg_weeks_max.map(i64::from),
            strain.height_class.map(|h| height_label(h).to_owned()),
            strain.auto_total_days_min.map(i64::from),
            strain.auto_total_days_max.map(i64::from),
            strain.yield_notes,
            strain.terpene_notes,
            strain.effect_notes,
            tolerance_label(strain.heat_tolerance),
            tolerance_label(strain.cold_tolerance),
            tolerance_label(strain.light_tolerance),
            strain.user_rating.map(i64::from),
            strain.notes,
            serde_json::to_string(&strain.evidence).expect("BTreeMap serializes"),
            strain.source_url,
            strain.source_licence,
            strain.is_phenotype_of,
            strain.created_at,
            strain.updated_at,
        ],
    )?;
    Ok(())
}

/// Builds a new stored strain from an input payload: fresh ULID,
/// timestamps now, user-entered numbers tagged as grower-reported.
#[must_use]
pub fn from_input(input: &StrainInput) -> Strain {
    let mut evidence = BTreeMap::new();
    for field in [
        "indica_pct",
        "flower_days_min",
        "flower_days_max",
        "stretch_factor",
        "veg_weeks_min",
        "veg_weeks_max",
        "auto_total_days_min",
        "auto_total_days_max",
    ] {
        evidence.insert(field.to_owned(), EvidenceTag::GrowerReported);
    }
    evidence.insert("heat_tolerance".to_owned(), EvidenceTag::Inferred);
    evidence.insert("cold_tolerance".to_owned(), EvidenceTag::Inferred);
    evidence.insert("light_tolerance".to_owned(), EvidenceTag::Inferred);
    Strain {
        id: new_id(),
        created_at: now_rfc3339(),
        updated_at: now_rfc3339(),
        name: input.name.clone(),
        breeder: input.breeder.clone(),
        photoperiod: input.photoperiod,
        sex: input.sex,
        indica_pct: input.indica_pct,
        landrace_tags: input.landrace_tags.clone(),
        flower_days_min: input.flower_days_min,
        flower_days_max: input.flower_days_max,
        stretch_factor: input.stretch_factor,
        veg_weeks_min: input.veg_weeks_min,
        veg_weeks_max: input.veg_weeks_max,
        height_class: input.height_class,
        auto_total_days_min: input.auto_total_days_min,
        auto_total_days_max: input.auto_total_days_max,
        yield_notes: input.yield_notes.clone(),
        terpene_notes: input.terpene_notes.clone(),
        effect_notes: input.effect_notes.clone(),
        heat_tolerance: input.heat_tolerance,
        cold_tolerance: input.cold_tolerance,
        light_tolerance: input.light_tolerance,
        user_rating: input.user_rating,
        notes: input.notes.clone(),
        evidence,
        source_url: None,
        source_licence: None,
        is_phenotype_of: None,
    }
}

/// Lists all strains ordered by name.
///
/// # Errors
///
/// Returns a [`crate::db::StoreError`] on SQLite failures.
pub fn list(conn: &Connection) -> Result<Vec<Strain>, StoreError> {
    let mut stmt = conn.prepare("SELECT * FROM strain ORDER BY name")?;
    let rows = stmt.query_map([], map_row)?;
    rows.collect::<Result<Vec<_>, _>>()
        .map_err(StoreError::from)
}

/// Fetches one strain by id.
///
/// # Errors
///
/// Returns [`super::NotFound`] when the id does not exist.
pub fn get(conn: &Connection, id: &str) -> Result<Strain, StoreOrNotFound> {
    conn.query_row("SELECT * FROM strain WHERE id = ?1", params![id], map_row)
        .optional()?
        .ok_or_else(|| NotFound.into())
}

/// Replaces a strain (full update), preserving id, creation time,
/// provenance and phenotype link.
///
/// # Errors
///
/// Returns [`super::NotFound`] when the id does not exist.
///
/// # Panics
///
/// Cannot panic: JSON serialization of plain strings and maps always
/// succeeds.
pub fn update(conn: &Connection, id: &str, input: &StrainInput) -> Result<Strain, StoreOrNotFound> {
    let mut updated = get(conn, id)?;
    updated.updated_at = now_rfc3339();
    updated.name.clone_from(&input.name);
    updated.breeder.clone_from(&input.breeder);
    updated.photoperiod = input.photoperiod;
    updated.sex = input.sex;
    updated.indica_pct = input.indica_pct;
    updated.landrace_tags.clone_from(&input.landrace_tags);
    updated.flower_days_min = input.flower_days_min;
    updated.flower_days_max = input.flower_days_max;
    updated.stretch_factor = input.stretch_factor;
    updated.veg_weeks_min = input.veg_weeks_min;
    updated.veg_weeks_max = input.veg_weeks_max;
    updated.height_class = input.height_class;
    updated.auto_total_days_min = input.auto_total_days_min;
    updated.auto_total_days_max = input.auto_total_days_max;
    updated.yield_notes.clone_from(&input.yield_notes);
    updated.terpene_notes.clone_from(&input.terpene_notes);
    updated.effect_notes.clone_from(&input.effect_notes);
    updated.heat_tolerance = input.heat_tolerance;
    updated.cold_tolerance = input.cold_tolerance;
    updated.light_tolerance = input.light_tolerance;
    updated.user_rating = input.user_rating;
    updated.notes.clone_from(&input.notes);
    let evidence_json = serde_json::to_string(&updated.evidence).expect("BTreeMap serializes");
    let tags_json = serde_json::to_string(&updated.landrace_tags).expect("Vec<String> serializes");
    conn.execute(
        "UPDATE strain
         SET name = ?2, breeder = ?3, photoperiod = ?4, sex = ?5,
             indica_pct = ?6, landrace_tags = ?7, flower_days_min = ?8,
             flower_days_max = ?9, stretch_factor = ?10, veg_weeks_min = ?11,
             veg_weeks_max = ?12, height_class = ?13,
             auto_total_days_min = ?14, auto_total_days_max = ?15,
             yield_notes = ?16, terpene_notes = ?17, effect_notes = ?18,
             heat_tolerance = ?19, cold_tolerance = ?20,
             light_tolerance = ?21, user_rating = ?22, notes = ?23,
             evidence = ?24, updated_at = ?25
         WHERE id = ?1",
        params![
            updated.id,
            updated.name,
            updated.breeder,
            photoperiod_label(&updated),
            sex_label(&updated),
            updated.indica_pct.map(i64::from),
            tags_json,
            updated.flower_days_min.map(i64::from),
            updated.flower_days_max.map(i64::from),
            updated.stretch_factor,
            updated.veg_weeks_min.map(i64::from),
            updated.veg_weeks_max.map(i64::from),
            updated.height_class.map(|h| height_label(h).to_owned()),
            updated.auto_total_days_min.map(i64::from),
            updated.auto_total_days_max.map(i64::from),
            updated.yield_notes,
            updated.terpene_notes,
            updated.effect_notes,
            tolerance_label(updated.heat_tolerance),
            tolerance_label(updated.cold_tolerance),
            tolerance_label(updated.light_tolerance),
            updated.user_rating.map(i64::from),
            updated.notes,
            evidence_json,
            updated.updated_at,
        ],
    )?;
    Ok(updated)
}

/// Deletes a strain.
///
/// # Errors
///
/// Returns [`super::NotFound`] when the id does not exist.
pub fn delete(conn: &Connection, id: &str) -> Result<(), StoreOrNotFound> {
    let affected = conn.execute("DELETE FROM strain WHERE id = ?1", params![id])?;
    if affected == 0 {
        return Err(NotFound.into());
    }
    Ok(())
}

/// Counts strains (used by the seed loader's "only seed an empty
/// database" rule).
///
/// # Errors
///
/// Returns a [`crate::db::StoreError`] on SQLite failures.
pub fn count(conn: &Connection) -> Result<i64, StoreError> {
    Ok(conn.query_row("SELECT COUNT(*) FROM strain", [], |r| r.get(0))?)
}

/// The stored `snake_case` label of a strain's photoperiod type.
const fn photoperiod_label(strain: &Strain) -> &'static str {
    match strain.photoperiod {
        leaflog_core::strain::PhotoperiodType::Photoperiod => "photoperiod",
        leaflog_core::strain::PhotoperiodType::Auto => "auto",
    }
}

/// The stored `snake_case` label of a strain's seed sex.
const fn sex_label(strain: &Strain) -> &'static str {
    match strain.sex {
        leaflog_core::strain::SeedSex::Feminized => "feminized",
        leaflog_core::strain::SeedSex::Regular => "regular",
    }
}

/// The stored `snake_case` label of a height class.
const fn height_label(height: leaflog_core::strain::HeightClass) -> &'static str {
    match height {
        leaflog_core::strain::HeightClass::Short => "short",
        leaflog_core::strain::HeightClass::Medium => "medium",
        leaflog_core::strain::HeightClass::Tall => "tall",
    }
}

/// The stored `snake_case` label of a tolerance level.
const fn tolerance_label(tolerance: leaflog_core::strain::Tolerance) -> &'static str {
    match tolerance {
        leaflog_core::strain::Tolerance::Sensitive => "sensitive",
        leaflog_core::strain::Tolerance::Average => "average",
        leaflog_core::strain::Tolerance::Tolerant => "tolerant",
    }
}

/// Maps one result row. Column names match the migration's `strain`
/// table. Labels are written only by this module, so unknown values
/// fall back to safe defaults rather than failing the read.
fn map_row(row: &rusqlite::Row<'_>) -> Result<Strain, rusqlite::Error> {
    let photoperiod: String = row.get("photoperiod")?;
    let sex: String = row.get("sex")?;
    let height: Option<String> = row.get("height_class")?;
    let heat: String = row.get("heat_tolerance")?;
    let cold: String = row.get("cold_tolerance")?;
    let light: String = row.get("light_tolerance")?;
    let tags_json: String = row.get("landrace_tags")?;
    let evidence_json: String = row.get("evidence")?;

    Ok(Strain {
        id: row.get("id")?,
        name: row.get("name")?,
        breeder: row.get("breeder")?,
        photoperiod: match photoperiod.as_str() {
            "auto" => leaflog_core::strain::PhotoperiodType::Auto,
            _ => leaflog_core::strain::PhotoperiodType::Photoperiod,
        },
        sex: match sex.as_str() {
            "regular" => leaflog_core::strain::SeedSex::Regular,
            _ => leaflog_core::strain::SeedSex::Feminized,
        },
        indica_pct: row
            .get::<_, Option<i64>>("indica_pct")?
            .and_then(|v| u8::try_from(v).ok()),
        landrace_tags: serde_json::from_str(&tags_json).unwrap_or_default(),
        flower_days_min: opt_u16(row.get("flower_days_min")?),
        flower_days_max: opt_u16(row.get("flower_days_max")?),
        stretch_factor: row.get("stretch_factor")?,
        veg_weeks_min: row
            .get::<_, Option<i64>>("veg_weeks_min")?
            .and_then(|v| u8::try_from(v).ok()),
        veg_weeks_max: row
            .get::<_, Option<i64>>("veg_weeks_max")?
            .and_then(|v| u8::try_from(v).ok()),
        height_class: height.and_then(|label| match label.as_str() {
            "short" => Some(leaflog_core::strain::HeightClass::Short),
            "medium" => Some(leaflog_core::strain::HeightClass::Medium),
            "tall" => Some(leaflog_core::strain::HeightClass::Tall),
            _ => None,
        }),
        auto_total_days_min: opt_u16(row.get("auto_total_days_min")?),
        auto_total_days_max: opt_u16(row.get("auto_total_days_max")?),
        yield_notes: row.get("yield_notes")?,
        terpene_notes: row.get("terpene_notes")?,
        effect_notes: row.get("effect_notes")?,
        heat_tolerance: parse_tolerance(&heat),
        cold_tolerance: parse_tolerance(&cold),
        light_tolerance: parse_tolerance(&light),
        user_rating: row
            .get::<_, Option<i64>>("user_rating")?
            .and_then(|v| u8::try_from(v).ok()),
        notes: row.get("notes")?,
        evidence: serde_json::from_str(&evidence_json).unwrap_or_default(),
        source_url: row.get("source_url")?,
        source_licence: row.get("source_licence")?,
        is_phenotype_of: row.get("is_phenotype_of")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
    })
}

/// Widens an optional stored integer into `Option<u16>`.
fn opt_u16(value: Option<i64>) -> Option<u16> {
    value.and_then(|v| u16::try_from(v).ok())
}

/// Parses a stored tolerance label (defaults to `average` on drift —
/// the alert thresholds then match the unmodified presets).
///
/// # Panics
///
/// Cannot panic: every path returns a `Tolerance`.
fn parse_tolerance(label: &str) -> leaflog_core::strain::Tolerance {
    match label {
        "sensitive" => leaflog_core::strain::Tolerance::Sensitive,
        "tolerant" => leaflog_core::strain::Tolerance::Tolerant,
        _ => leaflog_core::strain::Tolerance::Average,
    }
}
