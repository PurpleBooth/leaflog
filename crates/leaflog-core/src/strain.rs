//! Strain database domain types (spec §4.4).
//!
//! Every numeric field is a sourced *range* (or single value where
//! sources agree); `evidence` records where each fact came from, and
//! bundled seed records carry their `source_url` and `source_licence`.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Photoperiod behaviour of a cultivar.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhotoperiodType {
    /// Flowers on a 12/12 light schedule change.
    Photoperiod,
    /// Flowers by age, regardless of light schedule.
    Auto,
}

impl PhotoperiodType {
    /// The stable `snake_case` label (database and API representation).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Photoperiod => "photoperiod",
            Self::Auto => "auto",
        }
    }
}

/// Seed sex type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeedSex {
    /// Feminized seeds.
    Feminized,
    /// Regular (mixed sex) seeds.
    Regular,
}

impl SeedSex {
    /// The stable `snake_case` label (database and API representation).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Feminized => "feminized",
            Self::Regular => "regular",
        }
    }
}

/// Expected height class.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeightClass {
    /// Short (roughly under 100 cm).
    Short,
    /// Medium (roughly 100–160 cm).
    Medium,
    /// Tall (roughly above 160 cm).
    Tall,
}

impl HeightClass {
    /// The stable `snake_case` label (database and API representation).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Short => "short",
            Self::Medium => "medium",
            Self::Tall => "tall",
        }
    }
}

/// A three-level environmental tolerance. These shift *alert
/// thresholds* (spec §4.4), never the species rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tolerance {
    /// More sensitive than average to this stressor.
    Sensitive,
    /// No known deviation from average.
    #[default]
    Average,
    /// Handles this stressor better than average.
    Tolerant,
}

impl Tolerance {
    /// The stable `snake_case` label (database and API representation).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Sensitive => "sensitive",
            Self::Average => "average",
            Self::Tolerant => "tolerant",
        }
    }
}

/// Where a strain fact came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceTag {
    /// Stated by the breeder.
    Breeder,
    /// Aggregated from grower reports.
    GrowerReported,
    /// Inferred from lineage/heuristics; always shown as "inferred".
    Inferred,
    /// Could not be confirmed against two sources (spec §5 rule).
    Unverified,
}

/// A cultivar record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Strain {
    /// ULID.
    pub id: String,
    /// Cultivar name, for example `Northern Lights`.
    pub name: String,
    /// Breeder, when known.
    pub breeder: Option<String>,
    /// Photoperiod or autoflowering.
    pub photoperiod: PhotoperiodType,
    /// Feminized or regular seeds.
    pub sex: SeedSex,
    /// Indica share in percent (0–100); the rest is sativa. `None` when
    /// unknown or genuinely mixed beyond a percentage.
    pub indica_pct: Option<u8>,
    /// Landrace or lineage tags (for example `afghani`, `haze`).
    pub landrace_tags: Vec<String>,
    /// Shortest commonly reported flowering time, days.
    pub flower_days_min: Option<u16>,
    /// Longest commonly reported flowering time, days.
    pub flower_days_max: Option<u16>,
    /// Expected stretch factor during flower (1.5 = +50% height).
    pub stretch_factor: Option<f64>,
    /// Typical vegetative weeks, lower bound (photoperiods).
    pub veg_weeks_min: Option<u8>,
    /// Typical vegetative weeks, upper bound (photoperiods).
    pub veg_weeks_max: Option<u8>,
    /// Expected height class.
    pub height_class: Option<HeightClass>,
    /// Autoflower total days from seed, lower bound.
    pub auto_total_days_min: Option<u16>,
    /// Autoflower total days from seed, upper bound.
    pub auto_total_days_max: Option<u16>,
    /// Yield notes (free text, sources vary wildly).
    pub yield_notes: Option<String>,
    /// Terpene notes.
    pub terpene_notes: Option<String>,
    /// Effect notes.
    pub effect_notes: Option<String>,
    /// Heat tolerance (shifts the temperature alert ceiling).
    pub heat_tolerance: Tolerance,
    /// Cold tolerance (shifts the temperature alert floor).
    pub cold_tolerance: Tolerance,
    /// Light tolerance (cautions the lamp-height advisor).
    pub light_tolerance: Tolerance,
    /// User's own rating, 0–5.
    pub user_rating: Option<u8>,
    /// User's own notes.
    pub notes: Option<String>,
    /// Per-field evidence tags, keyed by field name (for example
    /// `flower_days_min`).
    pub evidence: BTreeMap<String, EvidenceTag>,
    /// Where a bundled record came from (user records: `None`).
    pub source_url: Option<String>,
    /// Licence terms of the bundled record.
    pub source_licence: Option<String>,
    /// Whether this record was bundled with Leaflog (seed data). Set by
    /// the seeder only; user records and clones are always `false`, and
    /// bundled records are read-only (spec §4.4). Absent JSON parses as
    /// `false` (older fixtures and payloads predate the flag).
    #[serde(default)]
    pub is_builtin: bool,
    /// Parent strain ULID when this record is a user's phenotype clone.
    pub is_phenotype_of: Option<String>,
    /// Creation instant, RFC 3339 UTC.
    pub created_at: String,
    /// Last modification instant, RFC 3339 UTC.
    pub updated_at: String,
}

/// Payload to create or replace a strain. `evidence` defaults to
/// marking user-entered numbers as [`EvidenceTag::GrowerReported`] by
/// the backend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrainInput {
    /// Cultivar name.
    pub name: String,
    /// Breeder, when known.
    pub breeder: Option<String>,
    /// Photoperiod or autoflowering.
    pub photoperiod: PhotoperiodType,
    /// Feminized or regular seeds.
    pub sex: SeedSex,
    /// Indica share in percent (0–100).
    pub indica_pct: Option<u8>,
    /// Landrace or lineage tags.
    pub landrace_tags: Vec<String>,
    /// Shortest commonly reported flowering time, days.
    pub flower_days_min: Option<u16>,
    /// Longest commonly reported flowering time, days.
    pub flower_days_max: Option<u16>,
    /// Expected stretch factor during flower.
    pub stretch_factor: Option<f64>,
    /// Typical vegetative weeks, lower bound.
    pub veg_weeks_min: Option<u8>,
    /// Typical vegetative weeks, upper bound.
    pub veg_weeks_max: Option<u8>,
    /// Expected height class.
    pub height_class: Option<HeightClass>,
    /// Autoflower total days from seed, lower bound.
    pub auto_total_days_min: Option<u16>,
    /// Autoflower total days from seed, upper bound.
    pub auto_total_days_max: Option<u16>,
    /// Yield notes.
    pub yield_notes: Option<String>,
    /// Terpene notes.
    pub terpene_notes: Option<String>,
    /// Effect notes.
    pub effect_notes: Option<String>,
    /// Heat tolerance; use [`infer_tolerances`] defaults when unsure.
    pub heat_tolerance: Tolerance,
    /// Cold tolerance.
    pub cold_tolerance: Tolerance,
    /// Light tolerance.
    pub light_tolerance: Tolerance,
    /// User's own rating, 0–5.
    pub user_rating: Option<u8>,
    /// User's own notes.
    pub notes: Option<String>,
}

impl StrainInput {
    /// Extracts the editable payload from a stored strain (drops id,
    /// timestamps, provenance, phenotype link and evidence map).
    #[must_use]
    pub fn from_strain(strain: &Strain) -> Self {
        Self {
            name: strain.name.clone(),
            breeder: strain.breeder.clone(),
            photoperiod: strain.photoperiod,
            sex: strain.sex,
            indica_pct: strain.indica_pct,
            landrace_tags: strain.landrace_tags.clone(),
            flower_days_min: strain.flower_days_min,
            flower_days_max: strain.flower_days_max,
            stretch_factor: strain.stretch_factor,
            veg_weeks_min: strain.veg_weeks_min,
            veg_weeks_max: strain.veg_weeks_max,
            height_class: strain.height_class,
            auto_total_days_min: strain.auto_total_days_min,
            auto_total_days_max: strain.auto_total_days_max,
            yield_notes: strain.yield_notes.clone(),
            terpene_notes: strain.terpene_notes.clone(),
            effect_notes: strain.effect_notes.clone(),
            heat_tolerance: strain.heat_tolerance,
            cold_tolerance: strain.cold_tolerance,
            light_tolerance: strain.light_tolerance,
            user_rating: strain.user_rating,
            notes: strain.notes.clone(),
        }
    }
}

/// The three tolerances, inferred from lineage (spec §4.4 heuristics).
///
/// Heuristics, deliberately coarse and always displayed as "inferred":
/// autoflowers, short bushy broad-leaf indica-leaning plants and
/// cold-climate lineages tend to be more heat-sensitive;
/// sativa/haze/african/afghan/hawaiian lineages tend to be more
/// heat-tolerant but can be shocked by cold. Exceptions exist — these
/// defaults only nudge alert thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tolerances {
    /// Heat tolerance.
    pub heat: Tolerance,
    /// Cold tolerance.
    pub cold: Tolerance,
    /// Light tolerance.
    pub light: Tolerance,
}

/// Infers (heat, cold, light) tolerances from a strain's genetics.
#[must_use]
pub fn infer_tolerances(
    photoperiod: PhotoperiodType,
    indica_pct: Option<u8>,
    landrace_tags: &[String],
) -> Tolerances {
    let tags: Vec<String> = landrace_tags.iter().map(|t| t.to_lowercase()).collect();
    let has = |needle: &str| tags.iter().any(|t| t.contains(needle));

    let heat_lineage = has("haze")
        || has("sativa")
        || has("african")
        || has("afghan")
        || has("hawaiian")
        || has("thai")
        || has("mexican")
        || has("colombian");
    let cold_lineage = has("ruderalis") || has("kush") || has("nepal") || has("mazar");

    let indica_leading = indica_pct.is_some_and(|pct| pct >= 60);

    let heat = if heat_lineage {
        Tolerance::Tolerant
    } else if indica_leaning_auto(photoperiod, indica_leading) || cold_lineage {
        Tolerance::Sensitive
    } else {
        Tolerance::Average
    };

    let cold = if heat_lineage {
        Tolerance::Sensitive
    } else if cold_lineage || photoperiod == PhotoperiodType::Auto {
        Tolerance::Tolerant
    } else {
        Tolerance::Average
    };

    // Light tolerance: no solid published heuristic; default average and
    // let the lamp advisor's caution text carry the uncertainty.
    let light = Tolerance::Average;

    Tolerances { heat, cold, light }
}

/// Short, bushy, broad-leaf, indica-leaning autos run hot (spec §4.4).
const fn indica_leaning_auto(photoperiod: PhotoperiodType, indica_leading: bool) -> bool {
    match photoperiod {
        PhotoperiodType::Auto => true,
        PhotoperiodType::Photoperiod => indica_leading,
    }
}

#[cfg(test)]
mod tests {
    use super::{PhotoperiodType, Tolerance, infer_tolerances};

    #[test]
    fn haze_lineage_runs_hot_but_hates_cold() {
        let tags = vec!["haze".to_owned()];
        let t = infer_tolerances(PhotoperiodType::Photoperiod, Some(20), &tags);
        assert_eq!(t.heat, Tolerance::Tolerant);
        assert_eq!(t.cold, Tolerance::Sensitive);
    }

    #[test]
    fn autos_default_heat_sensitive_cold_tolerant() {
        let t = infer_tolerances(PhotoperiodType::Auto, None, &[]);
        assert_eq!(t.heat, Tolerance::Sensitive);
        assert_eq!(t.cold, Tolerance::Tolerant);
    }

    #[test]
    fn bushy_indica_runs_heat_sensitive() {
        let t = infer_tolerances(PhotoperiodType::Photoperiod, Some(80), &[]);
        assert_eq!(t.heat, Tolerance::Sensitive);
    }

    #[test]
    fn plain_hybrid_is_average() {
        let t = infer_tolerances(PhotoperiodType::Photoperiod, Some(50), &[]);
        assert_eq!(t.heat, Tolerance::Average);
        assert_eq!(t.cold, Tolerance::Average);
        assert_eq!(t.light, Tolerance::Average);
    }

    #[test]
    fn afghan_counts_as_heat_tolerant() {
        let tags = vec!["Afghan".to_owned()];
        let t = infer_tolerances(PhotoperiodType::Photoperiod, Some(70), &tags);
        assert_eq!(t.heat, Tolerance::Tolerant);
    }
}
