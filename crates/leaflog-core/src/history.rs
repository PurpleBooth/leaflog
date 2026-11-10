//! Cross-grow history computations (spec §4.15): predicted-vs-actual
//! accuracy and yield aggregation. Everything here is read-only over
//! data the other modules already captured — no new capture flows.

use serde::{Deserialize, Serialize};

use crate::Phase;
use crate::forecast::{Actuals, PhaseWindow};

/// Predicted-vs-actual accuracy for one pinned boundary of a grow.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoundaryAccuracy {
    /// Which boundary: `"flip"` (flower start) or `"harvest"` (flower
    /// end).
    pub boundary: &'static str,
    /// The day the plan predicted (likely path).
    pub predicted_day: i64,
    /// The day the grower actually acted.
    pub actual_day: i64,
    /// `actual - predicted`, in days: positive means later than
    /// planned.
    pub delta_days: i64,
}

impl BoundaryAccuracy {
    /// One comparison row.
    #[must_use]
    pub const fn new(boundary: &'static str, predicted_day: i64, actual_day: i64) -> Self {
        Self {
            boundary,
            predicted_day,
            actual_day,
            delta_days: actual_day - predicted_day,
        }
    }
}

/// Compares a grow's plan against its logged boundaries (spec §4.15):
/// the flip against the predicted flower start, the harvest against
/// the predicted flower end. Only logged boundaries are reported.
#[must_use]
pub fn boundary_accuracy(windows: &[PhaseWindow], actuals: &Actuals) -> Vec<BoundaryAccuracy> {
    let Some(flower) = windows.iter().find(|window| window.phase == Phase::Flower) else {
        return Vec::new();
    };
    let mut boundaries = Vec::new();
    if let Some(actual) = actuals.flip_day {
        boundaries.push(BoundaryAccuracy::new("flip", flower.likely_start, actual));
    }
    if let Some(actual) = actuals.harvest_day {
        boundaries.push(BoundaryAccuracy::new("harvest", flower.likely_end, actual));
    }
    boundaries
}

/// One aggregate row of a yield-by-something view (strain, substrate
/// or feed template): the dry/wet/trimmed totals over every grow that
/// produced a weight, keyed by that something's display label.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct YieldAggregate {
    /// The grouping label (`"unknown"` when a grow has none set).
    pub label: String,
    /// Total dry grams across the group's grows.
    pub total_dry_g: f64,
    /// Total wet grams.
    pub total_wet_g: f64,
    /// Total trimmed grams.
    pub total_trimmed_g: f64,
    /// How many grows contributed a weight.
    pub grows: u32,
}

/// Groups per-grow yield totals by a key extracted with `key_of`
/// (returning a display label per grow), keeping only grows that
/// produced any weight.
#[must_use]
pub fn aggregate_yields(entries: &[(String, crate::grows::YieldSummary)]) -> Vec<YieldAggregate> {
    let mut rows: Vec<YieldAggregate> = Vec::new();
    for (label, summary) in entries {
        if summary.total_dry_g.is_none()
            && summary.total_wet_g.is_none()
            && summary.total_trimmed_g.is_none()
        {
            continue;
        }
        if let Some(row) = rows.iter_mut().find(|row| &row.label == label) {
            row.total_dry_g += summary.total_dry_g.unwrap_or(0.0);
            row.total_wet_g += summary.total_wet_g.unwrap_or(0.0);
            row.total_trimmed_g += summary.total_trimmed_g.unwrap_or(0.0);
            row.grows += 1;
        } else {
            rows.push(YieldAggregate {
                label: label.clone(),
                total_dry_g: summary.total_dry_g.unwrap_or(0.0),
                total_wet_g: summary.total_wet_g.unwrap_or(0.0),
                total_trimmed_g: summary.total_trimmed_g.unwrap_or(0.0),
                grows: 1,
            });
        }
    }
    rows.sort_by(|a, b| a.label.cmp(&b.label));
    rows
}
