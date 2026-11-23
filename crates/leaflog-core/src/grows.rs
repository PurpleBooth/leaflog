//! Grow and plant domain types (spec §4.2, §4.5).
//!
//! A grow groups plants, log entries and photos under one strain
//! label and substrate, and carries the photoperiod and tolerance
//! facts the recommendation engines read (D075).

use serde::{Deserialize, Serialize};

use crate::Phase;
use crate::actions::{ActionDetails, ActionType};

/// Photoperiod behaviour of a plant (D075): a grow attribute — the
/// recommendation engines read it from the grow, never from a strain
/// record.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PhotoperiodType {
    /// Flowers on a 12/12 light schedule change.
    #[default]
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

    /// Parses a stored label. Returns `None` for labels this build
    /// does not know.
    #[must_use]
    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "photoperiod" => Some(Self::Photoperiod),
            "auto" => Some(Self::Auto),
            _ => None,
        }
    }
}

/// A three-level environmental tolerance. These shift *alert
/// thresholds* (spec §4.4), never the species rules. A grow attribute
/// (D075): unset parses as [`Tolerance::Average`].
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

    /// Parses a stored label. Returns `None` for labels this build
    /// does not know.
    #[must_use]
    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "sensitive" => Some(Self::Sensitive),
            "average" => Some(Self::Average),
            "tolerant" => Some(Self::Tolerant),
            _ => None,
        }
    }
}

/// The known substrates (spec §4.8). Anything else is a user-defined
/// label; the UI offers these three plus a free-text field.
pub const SUBSTRATES: &[&str] = &["soil", "coco", "hydro"];

/// Serde default for `dimmable` (D075): a lamp is dimmable unless the
/// grower says otherwise.
const fn default_true() -> bool {
    true
}

/// Life-cycle status of a grow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrowStatus {
    /// Currently running.
    #[default]
    Active,
    /// Finished and moved out of the default view.
    Archived,
}

impl GrowStatus {
    /// The stable `snake_case` label (database and API representation).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Archived => "archived",
        }
    }
}

/// One grow: a run of one or more plants under one strain label in a
/// substrate.
///
/// `Eq` is not derived: `real_watts` is a `f64` (measured wall draw),
/// and floats carry no `Eq`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Grow {
    /// ULID.
    pub id: String,
    /// User-chosen name, for example `NL closet run`.
    pub name: String,
    /// The feed template fitted to this grow (spec §4.6/§4.15); copied
    /// by reference. Absent JSON parses as `None`.
    #[serde(default)]
    pub feed_schedule_id: Option<String>,
    /// Planned vegetative weeks for photoperiods (spec §4.5): the
    /// plan projects the flip until one is logged. Absent JSON parses
    /// as `None`.
    #[serde(default)]
    pub planned_veg_weeks: Option<u8>,
    /// Free-text strain label (D075): what this grow was grown from.
    /// Labels the diary and groups yield history; never a
    /// recommendation input. Absent JSON parses as empty.
    #[serde(default)]
    pub strain_label: String,
    /// Photoperiod behaviour (D075): flowers when light hours drop,
    /// or by age regardless of light. Absent JSON parses as
    /// [`PhotoperiodType::Photoperiod`].
    #[serde(default)]
    pub photoperiod: PhotoperiodType,
    /// Heat tolerance level (D075); shifts the heat alert threshold.
    /// Absent JSON parses as [`Tolerance::Average`].
    #[serde(default)]
    pub heat_tolerance: Tolerance,
    /// Cold tolerance level (D075); shifts the cold alert threshold.
    /// Absent JSON parses as [`Tolerance::Average`].
    #[serde(default)]
    pub cold_tolerance: Tolerance,
    /// Light tolerance level (D075); shifts the light advice.
    /// Absent JSON parses as [`Tolerance::Average`].
    #[serde(default)]
    pub light_tolerance: Tolerance,
    /// The lamp's measured wall watts (D075); `None` = unknown, and the lamp
    /// advisor falls back to its default fixture. Absent JSON parses
    /// as `None`.
    #[serde(default)]
    pub real_watts: Option<f64>,
    /// Whether the lamp's output is dimmable (D075). Absent JSON
    /// parses as `true`.
    #[serde(default = "default_true")]
    pub dimmable: bool,
    /// The grow's Light Cycle override (spec §4.9): lights-on time and
    /// hours-on per lit phase. `None` — or any `None` part inside —
    /// falls back to the cited preset defaults
    /// ([`crate::light_cycle::default_hours_on`]). Absent JSON parses
    /// as `None`.
    #[serde(default)]
    pub light_cycle: Option<crate::light_cycle::LightCycle>,
    /// Substrate label: one of [`SUBSTRATES`] or a user-defined name.
    pub substrate: String,
    /// Seed/clone start date, `YYYY-MM-DD`. Day numbers count from here.
    pub started_on: String,
    /// Life-cycle status.
    pub status: GrowStatus,
    /// Free-form notes.
    pub notes: Option<String>,
    /// Creation instant, RFC 3339 UTC.
    pub created_at: String,
    /// Last modification instant, RFC 3339 UTC.
    pub updated_at: String,
}

/// Payload to create or replace a grow (`Eq` is not derived, same
/// reason as [`Grow`]).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrowInput {
    /// User-chosen name.
    pub name: String,
    /// The fitted feed template, copied by reference (spec §4.6).
    #[serde(default)]
    pub feed_schedule_id: Option<String>,
    /// Planned vegetative weeks for photoperiods (spec §4.5).
    #[serde(default)]
    pub planned_veg_weeks: Option<u8>,
    /// Free-text strain label (D075); empty when unlabelled.
    #[serde(default)]
    pub strain_label: String,
    /// Photoperiod behaviour (D075); defaults to photoperiod.
    #[serde(default)]
    pub photoperiod: PhotoperiodType,
    /// Heat tolerance level (D075); defaults to average.
    #[serde(default)]
    pub heat_tolerance: Tolerance,
    /// Cold tolerance level (D075); defaults to average.
    #[serde(default)]
    pub cold_tolerance: Tolerance,
    /// Light tolerance level (D075); defaults to average.
    #[serde(default)]
    pub light_tolerance: Tolerance,
    /// The lamp's measured wall watts (D075); `None` = unknown.
    #[serde(default)]
    pub real_watts: Option<f64>,
    /// Whether the lamp is dimmable (D075); defaults to `true`.
    #[serde(default = "default_true")]
    pub dimmable: bool,
    /// Substrate label.
    pub substrate: String,
    /// Seed/clone start date, `YYYY-MM-DD`.
    pub started_on: String,
    /// Life-cycle status.
    pub status: GrowStatus,
    /// Free-form notes.
    pub notes: Option<String>,
}

impl GrowInput {
    /// Extracts the create/replace payload from a stored grow.
    ///
    /// Used by the edit path; id and timestamps are intentionally
    /// dropped.
    #[must_use]
    pub fn from_grow(grow: &Grow) -> Self {
        Self {
            name: grow.name.clone(),
            feed_schedule_id: grow.feed_schedule_id.clone(),
            planned_veg_weeks: grow.planned_veg_weeks,
            strain_label: grow.strain_label.clone(),
            photoperiod: grow.photoperiod,
            heat_tolerance: grow.heat_tolerance,
            cold_tolerance: grow.cold_tolerance,
            light_tolerance: grow.light_tolerance,
            real_watts: grow.real_watts,
            dimmable: grow.dimmable,
            substrate: grow.substrate.clone(),
            started_on: grow.started_on.clone(),
            status: grow.status,
            notes: grow.notes.clone(),
        }
    }
}

/// One plant inside a grow. Strain and timeline come from the grow.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Plant {
    /// ULID.
    pub id: String,
    /// Owning grow ULID.
    pub grow_id: String,
    /// User-chosen name, for example `NL #1`.
    pub name: String,
    /// Harvest wet weight in grams (spec §4.14); `None` = not weighed.
    #[serde(default)]
    pub wet_g: Option<f64>,
    /// Harvest dry weight in grams; `None` = not weighed.
    #[serde(default)]
    pub dry_g: Option<f64>,
    /// Harvest trimmed weight in grams; `None` = not weighed.
    #[serde(default)]
    pub trimmed_g: Option<f64>,
    /// Creation instant, RFC 3339 UTC.
    pub created_at: String,
    /// Last modification instant, RFC 3339 UTC.
    pub updated_at: String,
}

/// Payload to create a plant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlantInput {
    /// User-chosen name.
    pub name: String,
    /// Harvest weights in grams (spec §4.14); `None` on plain plants.
    #[serde(default)]
    pub wet_g: Option<f64>,
    /// Harvest dry weight in grams.
    #[serde(default)]
    pub dry_g: Option<f64>,
    /// Harvest trimmed weight in grams.
    #[serde(default)]
    pub trimmed_g: Option<f64>,
}

impl PlantInput {
    /// Extracts the editable payload from a stored plant.
    #[must_use]
    pub fn from_plant(plant: &Plant) -> Self {
        Self {
            name: plant.name.clone(),
            wet_g: plant.wet_g,
            dry_g: plant.dry_g,
            trimmed_g: plant.trimmed_g,
        }
    }
}

/// A grow's yield summary (spec §4.14): totals, per-plant breakdown
/// and the wet→dry ratio. All grams; a total is `None` when no plant
/// carries that weight.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct YieldSummary {
    /// The grow the summary belongs to.
    pub grow_id: String,
    /// Every plant of the grow with its weights (unset weights are
    /// `None`, not zero).
    pub plants: Vec<Plant>,
    /// Sum of the plants' wet weights; `None` when nothing was weighed.
    pub total_wet_g: Option<f64>,
    /// Sum of the plants' dry weights; `None` when nothing was weighed.
    pub total_dry_g: Option<f64>,
    /// Sum of the plants' trimmed weights; `None` when not weighed.
    pub total_trimmed_g: Option<f64>,
    /// `total_wet / total_dry` when both exist and the dry weight is
    /// positive.
    pub wet_to_dry_ratio: Option<f64>,
}

impl YieldSummary {
    /// Aggregates a grow's plants into their yield summary.
    #[must_use]
    pub fn of(grow_id: &str, plants: Vec<Plant>) -> Self {
        let total = |weight: fn(&Plant) -> Option<f64>| -> Option<f64> {
            let values: Vec<f64> = plants.iter().filter_map(weight).collect();
            (!values.is_empty()).then(|| values.iter().sum())
        };
        let total_wet_g = total(|plant| plant.wet_g);
        let total_dry_g = total(|plant| plant.dry_g);
        let total_trimmed_g = total(|plant| plant.trimmed_g);
        let wet_to_dry_ratio = match (total_wet_g, total_dry_g) {
            (Some(wet), Some(dry)) if dry > 0.0 => Some(wet / dry),
            _ => None,
        };
        Self {
            grow_id: grow_id.to_owned(),
            plants,
            total_wet_g,
            total_dry_g,
            total_trimmed_g,
            wet_to_dry_ratio,
        }
    }
}

/// One action-log entry (spec §4.2). Phase and grow day are filled by
/// the backend from the grow start and the logged flip/harvest events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionEntry {
    /// ULID.
    pub id: String,
    /// Owning grow ULID.
    pub grow_id: String,
    /// Plants this entry applies to (empty = the whole grow).
    pub plant_ids: Vec<String>,
    /// When it happened, RFC 3339 UTC.
    pub occurred_at: String,
    /// What happened.
    pub action_type: ActionType,
    /// Auto-filled phase label at the time of the entry.
    pub phase: Option<Phase>,
    /// Days since the grow start (0-based).
    pub grow_day: u16,
    /// Structured, type-specific fields.
    pub details: ActionDetails,
    /// Free text.
    pub notes: Option<String>,
    /// Appearance tags (see [`crate::actions::APPEARANCE_TAGS`] and
    /// free-form values).
    pub appearance_tags: Vec<String>,
    /// Creation instant, RFC 3339 UTC.
    pub created_at: String,
    /// Last modification instant, RFC 3339 UTC.
    pub updated_at: String,
}

/// Payload to create or replace a log entry; the backend fills id,
/// phase, grow day and timestamps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionInput {
    /// Plants this entry applies to.
    pub plant_ids: Vec<String>,
    /// When it happened, RFC 3339 UTC.
    pub occurred_at: String,
    /// What happened.
    pub action_type: ActionType,
    /// Structured, type-specific fields.
    pub details: ActionDetails,
    /// Free text.
    pub notes: Option<String>,
    /// Appearance tags.
    pub appearance_tags: Vec<String>,
}

/// One stored photo (spec §4.3). Original and thumbnail bytes live on
/// disk under the photo store; this row is the metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Photo {
    /// ULID.
    pub id: String,
    /// Owning grow ULID.
    pub grow_id: String,
    /// Legacy first plant ULID, when known.
    pub plant_id: Option<String>,
    /// Plants in the photo; empty means the whole grow.
    #[serde(default)]
    pub plant_ids: Vec<String>,
    /// Linked log entry, when the photo was attached to one.
    pub action_log_id: Option<String>,
    /// Capture time (EXIF `DateTimeOriginal` when present, else upload
    /// time), RFC 3339 UTC.
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
    /// Size of the stored (stripped) original in bytes.
    pub bytes_stored: u64,
    /// Creation instant, RFC 3339 UTC.
    pub created_at: String,
    /// Last modification instant, RFC 3339 UTC.
    pub updated_at: String,
}
