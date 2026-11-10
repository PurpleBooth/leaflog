//! Grow and plant domain types (spec §4.2, §4.5): a grow groups plants,
//! log entries and photos under one strain and substrate.

use serde::{Deserialize, Serialize};

use crate::Phase;
use crate::actions::{ActionDetails, ActionType};

/// The known substrates (spec §4.8). Anything else is a user-defined
/// label; the UI offers these three plus a free-text field.
pub const SUBSTRATES: &[&str] = &["soil", "coco", "hydro"];

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

/// One grow: a run of one or more plants of a strain in a substrate,
/// optionally inside a grow space.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Grow {
    /// ULID.
    pub id: String,
    /// User-chosen name, for example `NL closet run`.
    pub name: String,
    /// Strain ULID (nullable: a grow may predate the catalogue entry).
    pub strain_id: Option<String>,
    /// Grow-space ULID (nullable: grows may run outside any space, and
    /// deleting a space keeps its grows).
    pub grow_space_id: Option<String>,
    /// Seed batch the plants came from (spec §4.13); nullable and
    /// `None` when absent JSON parses (payloads predate the link).
    #[serde(default)]
    pub seed_batch_id: Option<String>,
    /// The feed template fitted to this grow (spec §4.6/§4.15); copied
    /// by reference. Absent JSON parses as `None`.
    #[serde(default)]
    pub feed_template_id: Option<String>,
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

/// Payload to create or replace a grow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GrowInput {
    /// User-chosen name.
    pub name: String,
    /// Strain ULID.
    pub strain_id: Option<String>,
    /// Grow-space ULID.
    pub grow_space_id: Option<String>,
    /// Seed batch the plants came from (spec §4.13): linked plant
    /// starts decrement the batch count. Absent JSON parses as `None`
    /// (payloads predate the link).
    #[serde(default)]
    pub seed_batch_id: Option<String>,
    /// The fitted feed template, copied by reference (spec §4.6).
    #[serde(default)]
    pub feed_template_id: Option<String>,
    /// Substrate label.
    pub substrate: String,
    /// Seed/clone start date, `YYYY-MM-DD`.
    pub started_on: String,
    /// Life-cycle status.
    pub status: GrowStatus,
    /// Free-form notes.
    pub notes: Option<String>,
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
    /// Plant ULID, when known.
    pub plant_id: Option<String>,
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
