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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plant {
    /// ULID.
    pub id: String,
    /// Owning grow ULID.
    pub grow_id: String,
    /// User-chosen name, for example `NL #1`.
    pub name: String,
    /// Creation instant, RFC 3339 UTC.
    pub created_at: String,
    /// Last modification instant, RFC 3339 UTC.
    pub updated_at: String,
}

/// Payload to create a plant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlantInput {
    /// User-chosen name.
    pub name: String,
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
