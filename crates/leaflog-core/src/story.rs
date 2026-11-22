//! Wire types for the grow's calendar-day diary.

use serde::{Deserialize, Serialize};

use crate::Phase;
use crate::grows::{ActionEntry, Photo};

/// A window of the diary, newest day first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoryPage {
    /// Canonical IANA timezone for dates and wall-clock times.
    pub timezone: String,
    /// Days with diary items or sensor evidence.
    pub days: Vec<StoryDay>,
    /// Exclusive local date cursor for the next window.
    pub before: Option<String>,
}

/// The diary and available observations for a local calendar date.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StoryDay {
    /// Local ISO calendar date.
    pub date: String,
    /// Day number relative to the grow start.
    pub grow_day: u16,
    /// Phase on this day.
    pub phase: Phase,
    /// Manual entries, newest first.
    pub actions: Vec<ActionEntry>,
    /// Independent photos, newest first.
    pub photos: Vec<Photo>,
    /// At least one automatic light event was recorded.
    pub automatic: bool,
    /// Environmental readings or light-state observations exist.
    pub has_observations: bool,
    /// Time for which the lights were known to be on, in hours.
    pub light_hours: Option<f64>,
    /// Light history did not cover the whole day.
    pub light_partial: bool,
    /// Measurements classified by light state, on then off.
    pub periods: [Period; 2],
}

/// Averages over available measurements in one light period.
#[derive(Debug, Default, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Period {
    /// Air temperature (°C).
    pub temperature: Option<f64>,
    /// Relative humidity (%).
    pub humidity: Option<f64>,
    /// Vapour pressure deficit (kPa).
    pub vpd: Option<f64>,
    /// Some measurements have gaps or are based only on spot readings.
    pub sparse: bool,
}
