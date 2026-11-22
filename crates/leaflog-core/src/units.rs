//! Unit-aware display helpers (spec §2, "unit-aware" rule).
//!
//! Storage uses canonical metric units. Convert at the HA sensor and
//! presentation boundaries.

use crate::format::{DecimalStyle, format_decimal};
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
const GRAMS_PER_OZ: f64 = 28.349_523_125;

/// A measurement stored in cm, ml or g, respectively.
#[derive(Debug, Clone, Copy)]
pub enum Measure {
    /// Length (cm or in).
    Length,
    /// Small liquid volume (ml or US fl oz).
    Volume,
    /// Weight (g or oz).
    Weight,
}

impl Measure {
    /// Suffix for the selected system.
    #[must_use]
    pub const fn symbol(self, system: UnitSystem) -> &'static str {
        match (self, system) {
            (Self::Length, UnitSystem::Metric) => "cm",
            (Self::Length, UnitSystem::Imperial) => "in",
            (Self::Volume, UnitSystem::Metric) => "ml",
            (Self::Volume, UnitSystem::Imperial) => "fl oz",
            (Self::Weight, UnitSystem::Metric) => "g",
            (Self::Weight, UnitSystem::Imperial) => "oz",
        }
    }

    const fn scale(self, system: UnitSystem) -> f64 {
        match (self, system) {
            (_, UnitSystem::Metric) => 1.0,
            (Self::Length, UnitSystem::Imperial) => CM_PER_IN,
            (Self::Volume, UnitSystem::Imperial) => ML_PER_FLOZ,
            (Self::Weight, UnitSystem::Imperial) => GRAMS_PER_OZ,
        }
    }

    /// Converts a stored value to the selected unit.
    #[must_use]
    pub fn display_value(self, value: f64, system: UnitSystem) -> f64 {
        value / self.scale(system)
    }

    /// Converts a value in the selected unit to the stored unit.
    #[must_use]
    pub fn canonical(self, value: f64, system: UnitSystem) -> f64 {
        value * self.scale(system)
    }

    /// Formats a stored value in the selected unit and locale.
    #[must_use]
    pub fn format(self, value: f64, system: UnitSystem, style: DecimalStyle) -> String {
        let digits = match system {
            UnitSystem::Metric => 0,
            UnitSystem::Imperial => 2,
        };
        format!(
            "{} {}",
            format_decimal(self.display_value(value, system), digits, style),
            self.symbol(system)
        )
    }
}

/// Reads a sensor temperature into Celsius. A missing unit uses the
/// caller's Celsius default. Skip historical snapshots whose unit is unknown.
#[must_use]
pub fn sensor_celsius(value: f64, unit: Option<&str>) -> Option<f64> {
    if !value.is_finite() {
        return None;
    }
    match unit {
        Some("°F") => Some((value - C_OFFSET) * 5.0 / 9.0),
        Some("K") => Some(value - 273.15),
        None | Some("°C") => Some(value),
        Some(_) => None,
    }
}

/// Temperature symbol for the chosen system.
#[must_use]
pub const fn temperature_symbol(system: UnitSystem) -> &'static str {
    match system {
        UnitSystem::Metric => "°C",
        UnitSystem::Imperial => "°F",
    }
}

/// Converts canonical Celsius for presentation.
#[must_use]
pub fn temperature_value(celsius: f64, system: UnitSystem) -> f64 {
    match system {
        UnitSystem::Metric => celsius,
        UnitSystem::Imperial => celsius * 9.0 / 5.0 + C_OFFSET,
    }
}

/// Formats a temperature with both units, separated per `style`.
#[must_use]
pub fn temp_both(celsius: f64, style: DecimalStyle) -> String {
    let f = celsius * 9.0 / 5.0 + C_OFFSET;
    format!(
        "{} °C / {} °F",
        format_decimal(celsius, 1, style),
        format_decimal(f, 1, style),
    )
}

/// Formats a temperature for the preferred system only, separated per
/// `style`.
#[must_use]
pub fn temp_preferred(celsius: f64, system: UnitSystem, style: DecimalStyle) -> String {
    format!(
        "{} {}",
        format_decimal(temperature_value(celsius, system), 1, style),
        temperature_symbol(system)
    )
}

/// Formats a length with both units, separated per `style`.
#[must_use]
pub fn length_both(cm: f64, style: DecimalStyle) -> String {
    let inches = cm / CM_PER_IN;
    format!(
        "{} cm / {} in",
        format_decimal(cm, 0, style),
        format_decimal(inches, 1, style),
    )
}

/// Formats a length for the preferred system only, separated per
/// `style`.
#[must_use]
pub fn length_preferred(cm: f64, system: UnitSystem, style: DecimalStyle) -> String {
    match system {
        UnitSystem::Metric => format!("{} cm", format_decimal(cm, 0, style)),
        UnitSystem::Imperial => format!("{} in", format_decimal(cm / CM_PER_IN, 1, style)),
    }
}

/// Formats a volume with both units, separated per `style`.
#[must_use]
pub fn volume_both(ml: f64, style: DecimalStyle) -> String {
    let l = ml / 1000.0;
    let gal = ml / ML_PER_GAL;
    format!(
        "{} L / {} gal",
        format_decimal(l, 1, style),
        format_decimal(gal, 2, style),
    )
}

/// Formats a volume for the preferred system only, separated per
/// `style`.
#[must_use]
pub fn volume_preferred(ml: f64, system: UnitSystem, style: DecimalStyle) -> String {
    match system {
        UnitSystem::Metric => format!("{} L", format_decimal(ml / 1000.0, 1, style)),
        UnitSystem::Imperial => format!("{} gal", format_decimal(ml / ML_PER_GAL, 2, style)),
    }
}

/// Formats a small volume (feed amounts) with both units, separated
/// per `style`.
#[must_use]
pub fn small_volume_both(ml: f64, style: DecimalStyle) -> String {
    let floz = ml / ML_PER_FLOZ;
    format!(
        "{} ml / {} fl oz",
        format_decimal(ml, 0, style),
        format_decimal(floz, 1, style),
    )
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
        DecimalStyle, UnitSystem, cm_to_inches, inches_to_cm, length_both, length_preferred,
        small_volume_both, temp_both, temp_preferred, volume_both, volume_preferred,
    };

    #[test]
    fn temp_round_trip() {
        let display = temp_both(25.0, DecimalStyle::Dot);
        assert!(display.contains("25.0 °C"), "metric: {display}");
        assert!(display.contains("77.0 °F"), "imperial: {display}");
    }

    #[test]
    fn temp_separates_per_style() {
        assert_eq!(temp_both(-12.3, DecimalStyle::Comma), "-12,3 °C / 9,9 °F");
        assert_eq!(
            temp_preferred(1234.5, UnitSystem::Imperial, DecimalStyle::Comma),
            "2.254,1 °F"
        );
        assert_eq!(
            temp_preferred(25.0, UnitSystem::Metric, DecimalStyle::Comma),
            "25,0 °C"
        );
        assert_eq!(
            temp_preferred(25.0, UnitSystem::Metric, DecimalStyle::Dot),
            "25.0 °C"
        );
    }

    #[test]
    fn temp_preferred_matches_system() {
        assert_eq!(
            temp_preferred(25.0, UnitSystem::Metric, DecimalStyle::Dot),
            "25.0 °C"
        );
        assert!(temp_preferred(25.0, UnitSystem::Imperial, DecimalStyle::Dot).contains("°F"));
    }

    #[test]
    fn length_both_shows_cm_and_in() {
        let display = length_both(30.0, DecimalStyle::Dot);
        assert!(display.contains("30 cm"), "{display}");
        assert!(display.contains("11.8 in"), "{display}");
    }

    #[test]
    fn volume_both_shows_l_and_gal() {
        let display = volume_both(3785.41, DecimalStyle::Dot);
        assert!(display.contains("3.8 L"), "{display}");
        assert!(display.contains("1.00 gal"), "{display}");
    }

    #[test]
    fn volumes_group_per_style() {
        // 1 234 567 ml → 1 234.6 L / 326.14 gal
        let display = volume_both(1_234_567.0, DecimalStyle::Comma);
        assert!(display.contains("1.234,6 L"), "{display}");
        assert!(display.contains("326,14 gal"), "{display}");
        let display = small_volume_both(1_234_567.0, DecimalStyle::Comma);
        assert!(display.contains("1.234.567 ml"), "{display}");
        assert!(display.contains("41.745,7 fl oz"), "{display}");
        let display = length_both(12_345.6, DecimalStyle::Comma);
        assert!(display.contains("12.346 cm"), "{display}");
        assert!(display.contains("4.860,5 in"), "{display}");
        assert_eq!(
            length_preferred(1234.5, UnitSystem::Metric, DecimalStyle::Comma),
            "1.234 cm"
        );
        assert_eq!(
            volume_preferred(1234.5, UnitSystem::Metric, DecimalStyle::Comma),
            "1,2 L"
        );
    }

    #[test]
    fn volume_preferred_matches_system() {
        assert!(volume_preferred(1000.0, UnitSystem::Metric, DecimalStyle::Dot).contains("1.0 L"));
        assert!(volume_preferred(1000.0, UnitSystem::Imperial, DecimalStyle::Dot).contains("gal"));
    }

    #[test]
    fn inch_cm_round_trip() {
        let cm = inches_to_cm(12.0);
        assert!((cm - 30.48).abs() < 0.01);
        let back = cm_to_inches(cm);
        assert!((back - 12.0).abs() < 0.01);
    }
}
