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
    /// Local dates carrying at least one photo across the whole grow,
    /// regardless of the returned window, newest first.
    pub photo_dates: Vec<String>,
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
    /// Hours of the elapsed part of the day in which the temperature
    /// sensor reported nothing; `None` when it reported all along or
    /// not at all that day.
    pub temperature_unavailable_hours: Option<f64>,
    /// The same for the humidity sensor.
    pub humidity_unavailable_hours: Option<f64>,
    /// The same for the light: hours its state is unknown, because the
    /// device reported `unavailable` while the grow does not read
    /// dead hours as off (D159), or no observation covers them.
    pub light_unavailable_hours: Option<f64>,
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
}
