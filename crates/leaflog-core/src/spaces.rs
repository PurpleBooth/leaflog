//! Grow-space domain types (spec §4.1): a named physical setup — a
//! tent or room — that combines equipment catalogue entries and
//! contains grows.

use serde::{Deserialize, Serialize};

/// One grow space: a physical setup (for example `60×60 tent`) that
/// references equipment catalogue entries; grows run inside it.
///
/// Dimensions are stored in centimetres (SI, per the units policy) and
/// may be partial — a cardboard box has no meaningful `depth_cm`
/// distinction from `width_cm`, and a room corner may only have a
/// height.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrowSpace {
    /// ULID.
    pub id: String,
    /// User-chosen name, for example `60×60 tent`.
    pub name: String,
    /// Interior width in centimetres, when known.
    pub width_cm: Option<f64>,
    /// Interior depth in centimetres, when known.
    pub depth_cm: Option<f64>,
    /// Interior height in centimetres, when known.
    pub height_cm: Option<f64>,
    /// Free-form notes.
    pub notes: Option<String>,
    /// Creation instant, RFC 3339 UTC.
    pub created_at: String,
    /// Last modification instant, RFC 3339 UTC.
    pub updated_at: String,
}

/// Payload to create or replace a grow space. All optional fields may
/// be `null`; the backend fills id and timestamps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GrowSpaceInput {
    /// User-chosen name.
    pub name: String,
    /// Interior width in centimetres, when known.
    pub width_cm: Option<f64>,
    /// Interior depth in centimetres, when known.
    pub depth_cm: Option<f64>,
    /// Interior height in centimetres, when known.
    pub height_cm: Option<f64>,
    /// Free-form notes.
    pub notes: Option<String>,
}
