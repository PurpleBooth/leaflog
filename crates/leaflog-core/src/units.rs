//! Unit-aware display helpers (spec §2, "unit-aware" rule).
//!
//! SI is canonical internally; these helpers format for display with
//! the other system alongside. °C/°F, cm/in, L/gal, ml/fl oz.

use serde::{Deserialize, Serialize};

/// Which system the user prefers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnitSystem {
    /// Metric (SI).
    #[default]
    Metric,
    /// US customary.
    Imperial,
}

impl UnitSystem {
    /// The label for settings.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Metric => "metric",
            Self::Imperial => "imperial",
        }
    }
}

// Conversions (SI → imperial)
const CM_PER_IN: f64 = 2.54;
const ML_PER_FLOZ: f64 = 29.5735;
const ML_PER_GAL: f64 = 3785.41;
const C_OFFSET: f64 = 32.0;

/// Formats a temperature with both units.
#[must_use]
pub fn temp_both(celsius: f64) -> String {
    let f = celsius * 9.0 / 5.0 + C_OFFSET;
    format!("{celsius:.1} °C / {f:.1} °F")
}

/// Formats a temperature for the preferred system only.
#[must_use]
pub fn temp_preferred(celsius: f64, system: UnitSystem) -> String {
    match system {
        UnitSystem::Metric => format!("{celsius:.1} °C"),
        UnitSystem::Imperial => format!("{:.1} °F", celsius * 9.0 / 5.0 + C_OFFSET),
    }
}

/// Formats a length with both units.
#[must_use]
pub fn length_both(cm: f64) -> String {
    let inches = cm / CM_PER_IN;
    format!("{cm:.0} cm / {inches:.1} in")
}

/// Formats a length for the preferred system only.
#[must_use]
pub fn length_preferred(cm: f64, system: UnitSystem) -> String {
    match system {
        UnitSystem::Metric => format!("{cm:.0} cm"),
        UnitSystem::Imperial => format!("{:.1} in", cm / CM_PER_IN),
    }
}

/// Formats a volume with both units.
#[must_use]
pub fn volume_both(ml: f64) -> String {
    let l = ml / 1000.0;
    let gal = ml / ML_PER_GAL;
    format!("{l:.1} L / {gal:.2} gal")
}

/// Formats a volume for the preferred system only.
#[must_use]
pub fn volume_preferred(ml: f64, system: UnitSystem) -> String {
    match system {
        UnitSystem::Metric => format!("{:.1} L", ml / 1000.0),
        UnitSystem::Imperial => format!("{:.2} gal", ml / ML_PER_GAL),
    }
}

/// Formats a small volume (feed amounts) with both units.
#[must_use]
pub fn small_volume_both(ml: f64) -> String {
    let floz = ml / ML_PER_FLOZ;
    format!("{ml:.0} ml / {floz:.1} fl oz")
}

/// Converts inches to cm.
#[must_use]
pub fn inches_to_cm(inches: f64) -> f64 {
    inches * CM_PER_IN
}

/// Converts cm to inches.
#[must_use]
pub fn cm_to_inches(cm: f64) -> f64 {
    cm / CM_PER_IN
}

#[cfg(test)]
mod tests {
    use super::{
        UnitSystem, cm_to_inches, inches_to_cm, length_both, temp_both, temp_preferred,
        volume_both, volume_preferred,
    };

    #[test]
    fn temp_round_trip() {
        let display = temp_both(25.0);
        assert!(display.contains("25.0 °C"), "metric: {display}");
        assert!(display.contains("77.0 °F"), "imperial: {display}");
    }

    #[test]
    fn temp_preferred_matches_system() {
        assert_eq!(temp_preferred(25.0, UnitSystem::Metric), "25.0 °C");
        assert!(temp_preferred(25.0, UnitSystem::Imperial).contains("°F"));
    }

    #[test]
    fn length_both_shows_cm_and_in() {
        let display = length_both(30.0);
        assert!(display.contains("30 cm"), "{display}");
        assert!(display.contains("11.8 in"), "{display}");
    }

    #[test]
    fn volume_both_shows_l_and_gal() {
        let display = volume_both(3785.41);
        assert!(display.contains("3.8 L"), "{display}");
        assert!(display.contains("1.00 gal"), "{display}");
    }

    #[test]
    fn volume_preferred_matches_system() {
        assert!(volume_preferred(1000.0, UnitSystem::Metric).contains("1.0 L"));
        assert!(volume_preferred(1000.0, UnitSystem::Imperial).contains("gal"));
    }

    #[test]
    fn inch_cm_round_trip() {
        let cm = inches_to_cm(12.0);
        assert!((cm - 30.48).abs() < 0.01);
        let back = cm_to_inches(cm);
        assert!((back - 12.0).abs() < 0.01);
    }
}
