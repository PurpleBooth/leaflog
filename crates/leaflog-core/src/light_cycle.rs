//! Light Cycle domain (spec §4.9, §5.1).
//!
//! A grow carries one daily lights-on local time plus hours-on per
//! lit phase, stored nullable: every missing part falls back to a
//! cited preset default, and reset-to-default means "clear back to
//! those defaults". Pure domain only: parsing, validation and the
//! strain-aware defaults. Period expansion against a timezone is
//! later work (ticket 07) and keeps living in the chart engine.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::Phase;
use crate::strain::PhotoperiodType;

/// The phases a Light Cycle speaks about: the ones with grow lights.
/// Drying and curing run without lights, so no schedule applies.
pub const LIT_PHASES: &[Phase] = &[
    Phase::Germination,
    Phase::Seedling,
    Phase::Vegetative,
    Phase::Flower,
];

/// Smallest accepted hours-on per phase: a lit phase with no light
/// hours cannot grow a plant.
pub const MIN_HOURS_ON: u8 = 1;

/// Largest accepted hours-on per phase (24-0 schedules included).
pub const MAX_HOURS_ON: u8 = 24;

/// A wall-clock time of day, `HH:MM` local to the grow's canonical
/// timezone (D059). Serializes as the `"HH:MM"` string the HTML time
/// input speaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ClockTime {
    /// Hour of day, 0–23.
    pub hour: u8,
    /// Minute of the hour, 0–59.
    pub minute: u8,
}

impl ClockTime {
    /// Parses `"H:MM"` or `"HH:MM"` (what `<input type="time">`
    /// yields).
    ///
    /// # Errors
    ///
    /// [`LightCycleError::BadLightsOn`] when the text is not a valid
    /// `HH:MM` wall-clock time.
    pub fn parse(text: &str) -> Result<Self, LightCycleError> {
        let invalid = |text: &str| LightCycleError::BadLightsOn(text.to_owned());
        let (hour_text, minute_text) = text.split_once(':').ok_or_else(|| invalid(text))?;
        let hour: u8 = hour_text.parse().map_err(|_| invalid(text))?;
        let minute: u8 = minute_text.parse().map_err(|_| invalid(text))?;
        if hour_text.len() > 2 || minute_text.len() != 2 {
            return Err(invalid(text));
        }
        if hour > 23 || minute > 59 {
            return Err(invalid(text));
        }
        Ok(Self { hour, minute })
    }

    /// Minutes after local midnight; the lights-off instant of a
    /// schedule is `(minutes + hours_on × 60) mod day`.
    #[must_use]
    pub const fn minutes_since_midnight(self) -> u32 {
        // u8 -> u32 widens without loss; From is not const-stable.
        (self.hour as u32) * 60 + (self.minute as u32)
    }
}

impl fmt::Display for ClockTime {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{:02}:{:02}", self.hour, self.minute)
    }
}

impl Serialize for ClockTime {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ClockTime {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let text = String::deserialize(deserializer)?;
        Self::parse(&text).map_err(serde::de::Error::custom)
    }
}

/// Hours the lights stay on, per lit phase. A `None` phase uses the
/// preset default for the grow's strain type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PhaseHours {
    /// Lights-on hours while germinating.
    #[serde(default)]
    pub germination: Option<u8>,
    /// Lights-on hours while a seedling.
    #[serde(default)]
    pub seedling: Option<u8>,
    /// Lights-on hours in vegetative growth.
    #[serde(default)]
    pub vegetative: Option<u8>,
    /// Lights-on hours in flower.
    #[serde(default)]
    pub flower: Option<u8>,
}

impl PhaseHours {
    /// The override for one phase (`None` for the unlit phases).
    #[must_use]
    pub const fn get(self, phase: Phase) -> Option<u8> {
        match phase {
            Phase::Germination => self.germination,
            Phase::Seedling => self.seedling,
            Phase::Vegetative => self.vegetative,
            Phase::Flower => self.flower,
            Phase::Drying | Phase::Curing => None,
        }
    }
}

/// A grow's Light Cycle override (spec §4.9). Every part is nullable;
/// the stored default value — all `None` — means "the preset defaults
/// apply", which is also what reset-to-default restores.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LightCycle {
    /// Daily lights-on local time; `None` uses
    /// [`DEFAULT_LIGHTS_ON`].
    #[serde(default)]
    pub lights_on: Option<ClockTime>,
    /// Hours-on per lit phase; unset phases use
    /// [`default_hours_on`].
    #[serde(default)]
    pub hours_on: PhaseHours,
}

impl LightCycle {
    /// Whether this cycle overrides anything (the reset button shows
    /// exactly when this holds).
    #[must_use]
    pub fn has_override(&self) -> bool {
        *self != Self::default()
    }

    /// The lights-on time in effect for the grow.
    #[must_use]
    pub fn lights_on_resolved(&self) -> ClockTime {
        self.lights_on.unwrap_or(DEFAULT_LIGHTS_ON)
    }

    /// The hours-on in effect for one phase of a grow of `photoperiod`.
    /// `None` for the unlit phases (drying, curing): no schedule
    /// applies there.
    #[must_use]
    pub fn hours_on_for(&self, phase: Phase, photoperiod: PhotoperiodType) -> Option<u8> {
        self.hours_on
            .get(phase)
            .or_else(|| default_hours_on(phase, photoperiod).map(|default| default.hours))
    }
}

/// The raw wire form of a Light Cycle (`PUT` body): text fields not
/// yet validated. Hours arrive as JSON numbers; a `None` field leaves
/// that part on its default (and resetting sends nothing at all).
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LightCycleInput {
    /// `"HH:MM"` or `null`.
    #[serde(default)]
    pub lights_on: Option<String>,
    /// Hours-on per lit phase or `null`s.
    #[serde(default)]
    pub hours_on: PhaseHoursInput,
}

/// Raw per-phase hours of a [`LightCycleInput`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct PhaseHoursInput {
    /// While germinating.
    #[serde(default)]
    pub germination: Option<i64>,
    /// While a seedling.
    #[serde(default)]
    pub seedling: Option<i64>,
    /// In vegetative growth.
    #[serde(default)]
    pub vegetative: Option<i64>,
    /// In flower.
    #[serde(default)]
    pub flower: Option<i64>,
}

impl LightCycleInput {
    /// Validates the raw form into a stored [`LightCycle`].
    ///
    /// # Errors
    ///
    /// [`LightCycleError::BadLightsOn`] for a time that is not
    /// `HH:MM`; [`LightCycleError::HoursOnRange`] for hours outside
    /// [`MIN_HOURS_ON`]..=[`MAX_HOURS_ON`].
    pub fn validate(&self) -> Result<LightCycle, LightCycleError> {
        let lights_on = match &self.lights_on {
            Some(text) => Some(ClockTime::parse(text)?),
            None => None,
        };
        let PhaseHoursInput {
            germination,
            seedling,
            vegetative,
            flower,
        } = &self.hours_on;
        let phase_hours = |phase: Phase, raw: Option<i64>| -> Result<Option<u8>, LightCycleError> {
            raw.map_or(Ok(None), |hours| match u8::try_from(hours) {
                Ok(parsed) if (MIN_HOURS_ON..=MAX_HOURS_ON).contains(&parsed) => Ok(Some(parsed)),
                _ => Err(LightCycleError::HoursOnRange { phase, value: raw }),
            })
        };
        Ok(LightCycle {
            lights_on,
            hours_on: PhaseHours {
                germination: phase_hours(Phase::Germination, *germination)?,
                seedling: phase_hours(Phase::Seedling, *seedling)?,
                vegetative: phase_hours(Phase::Vegetative, *vegetative)?,
                flower: phase_hours(Phase::Flower, *flower)?,
            },
        })
    }
}

/// Why a Light Cycle input was rejected; the message is what the API
/// returns to the editor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LightCycleError {
    /// The lights-on time was not a valid `HH:MM`.
    BadLightsOn(String),
    /// Per-phase hours were outside [`MIN_HOURS_ON`]..=[`MAX_HOURS_ON`].
    HoursOnRange {
        /// The phase whose hours were rejected.
        phase: Phase,
        /// The raw value as sent (`None` when it was not a whole
        /// number at all).
        value: Option<i64>,
    },
}

impl fmt::Display for LightCycleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BadLightsOn(text) => write!(
                formatter,
                "lights_on must be HH:MM between 00:00 and 23:59, got {text:?}"
            ),
            Self::HoursOnRange { phase, value } => write!(
                formatter,
                "hours_on for {} must be {}–{} whole hours, got {}",
                phase.label(),
                MIN_HOURS_ON,
                MAX_HOURS_ON,
                value.map_or_else(
                    || "a non-whole number".to_owned(),
                    |hours| hours.to_string()
                )
            ),
        }
    }
}

impl std::error::Error for LightCycleError {}

/// The default lights-on time: 06:00 local.
///
/// No published standard anchors a wall-clock on-time — it is a
/// grower preference — so this default only anchors the day/night
/// split and is marked unverified per the seed-data discipline.
pub const DEFAULT_LIGHTS_ON: ClockTime = ClockTime { hour: 6, minute: 0 };

/// The source label for [`DEFAULT_LIGHTS_ON`].
pub const DEFAULT_LIGHTS_ON_SOURCE: &str = "grower convention, unverified — no published standard for the \
     wall-clock on-time; 06:00 keeps the dark period in the evening";

/// One phase's preset hours-on for a strain type, with its source
/// label (seed-data discipline: a range-backed default, labelled).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HoursOnDefault {
    /// Preset hours the lights stay on.
    pub hours: u8,
    /// Where the number comes from.
    pub source: &'static str,
}

/// The preset hours-on per lit phase for a strain type (spec §5.1).
///
/// Photoperiod: 18/6 from germination through veg (spec has no
/// seedling row; seedlings run the vegetative schedule), 12/12 in
/// flower. Autoflower: long days throughout at the low end of the
/// 18–20 h range so the dark period stays bounded.
///
/// `None` for the unlit phases (drying, curing).
#[must_use]
pub const fn default_hours_on(
    phase: Phase,
    photoperiod: PhotoperiodType,
) -> Option<HoursOnDefault> {
    let photoperiod_veg = HoursOnDefault {
        hours: 18,
        source: "spec §5.1 18/6 veg; GrowWeedEasy light schedules — 18-6 is the \
                 common indoor vegetative schedule, seedlings included",
    };
    let photoperiod_flower = HoursOnDefault {
        hours: 12,
        source: "spec §5.1 12/12 flower; GrowWeedEasy light schedules — 12/12 \
                 initiates and holds flowering",
    };
    let auto = HoursOnDefault {
        hours: 18,
        source: "spec §5.1 autos commonly 18–20 h (low end kept so a dark \
                 period remains); research lights-off caveat C6",
    };
    match (phase, photoperiod) {
        (
            Phase::Germination | Phase::Seedling | Phase::Vegetative,
            PhotoperiodType::Photoperiod,
        ) => Some(photoperiod_veg),
        (Phase::Flower, PhotoperiodType::Photoperiod) => Some(photoperiod_flower),
        (Phase::Germination | Phase::Seedling | Phase::Vegetative | Phase::Flower, _) => Some(auto),
        (Phase::Drying | Phase::Curing, _) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DEFAULT_LIGHTS_ON, DEFAULT_LIGHTS_ON_SOURCE, LIT_PHASES, LightCycle, LightCycleError,
        LightCycleInput, MAX_HOURS_ON, MIN_HOURS_ON, PhaseHours, PhaseHoursInput, default_hours_on,
    };
    use crate::Phase;
    use crate::strain::PhotoperiodType;

    #[test]
    fn clock_time_parses_the_shapes_an_html_time_input_yields() {
        assert_eq!(
            super::ClockTime::parse("06:00").unwrap(),
            super::ClockTime { hour: 6, minute: 0 }
        );
        assert_eq!(
            super::ClockTime::parse("23:59").unwrap(),
            super::ClockTime {
                hour: 23,
                minute: 59
            }
        );
        assert_eq!(super::ClockTime::parse("6:00").unwrap().hour, 6);
    }

    #[test]
    fn clock_time_rejects_bad_values_with_clear_text() {
        for text in ["24:00", "12:60", "abc", "", "12", "0123:00", "12:005"] {
            let error = super::ClockTime::parse(text).unwrap_err();
            assert!(
                matches!(error, LightCycleError::BadLightsOn(_)),
                "{text} must parse as a bad time"
            );
            assert!(
                error.to_string().contains("HH:MM"),
                "message for {text} must say what a good time looks like"
            );
        }
    }

    #[test]
    fn clock_time_round_trips_through_the_wire_string() {
        let time = super::ClockTime {
            hour: 7,
            minute: 30,
        };
        let json = serde_json::to_string(&time).unwrap();
        assert_eq!(json, "\"07:30\"");
        let back: super::ClockTime = serde_json::from_str(&json).unwrap();
        assert_eq!(back, time);
        assert_eq!(back.to_string(), "07:30");
    }

    #[test]
    fn minutes_since_midnight_anchors_the_schedule() {
        assert_eq!(DEFAULT_LIGHTS_ON.minutes_since_midnight(), 360);
    }

    /// The acceptance contract for defaults: photoperiod 18/6 through
    /// veg, 12/12 flower; autos long days everywhere; the unlit phases
    /// get no schedule at all.
    #[test]
    fn photoperiod_defaults_are_18_veg_and_12_flower() {
        for phase in [Phase::Germination, Phase::Seedling, Phase::Vegetative] {
            let default = default_hours_on(phase, PhotoperiodType::Photoperiod)
                .unwrap_or_else(|| panic!("{} must have a photoperiod default", phase.label()));
            assert_eq!(default.hours, 18, "{} photoperiod default", phase.label());
            assert!(
                !default.source.is_empty(),
                "{} must carry a source",
                phase.label()
            );
        }
        let flower = default_hours_on(Phase::Flower, PhotoperiodType::Photoperiod).unwrap();
        assert_eq!(flower.hours, 12);
        assert!(flower.source.contains("12/12"));
    }

    #[test]
    fn autoflower_defaults_are_long_days_through_flower() {
        for phase in [
            Phase::Germination,
            Phase::Seedling,
            Phase::Vegetative,
            Phase::Flower,
        ] {
            let default = default_hours_on(phase, PhotoperiodType::Auto)
                .unwrap_or_else(|| panic!("{} must have an autoflower default", phase.label()));
            assert_eq!(default.hours, 18, "{} autoflower default", phase.label());
            assert!(
                default.source.contains("18–20"),
                "{} autoflower source must cite the 18–20 h range",
                phase.label()
            );
        }
    }

    #[test]
    fn unlit_phases_have_no_schedule_default() {
        for phase in [Phase::Drying, Phase::Curing] {
            assert!(default_hours_on(phase, PhotoperiodType::Photoperiod).is_none());
            assert!(default_hours_on(phase, PhotoperiodType::Auto).is_none());
        }
        assert!(
            LIT_PHASES
                .iter()
                .all(|phase| { default_hours_on(*phase, PhotoperiodType::Photoperiod).is_some() })
        );
    }

    #[test]
    fn the_default_lights_on_time_carries_a_source() {
        assert_eq!(DEFAULT_LIGHTS_ON.hour, 6);
        assert_eq!(DEFAULT_LIGHTS_ON.minute, 0);
        assert!(DEFAULT_LIGHTS_ON_SOURCE.contains("unverified"));
    }

    #[test]
    fn overrides_win_and_unset_parts_fall_back_to_defaults() {
        let cycle = LightCycle {
            lights_on: Some(super::ClockTime {
                hour: 21,
                minute: 15,
            }),
            hours_on: PhaseHours {
                flower: Some(11),
                ..PhaseHours::default()
            },
        };
        assert_eq!(cycle.lights_on_resolved().hour, 21);
        assert_eq!(
            cycle.hours_on_for(Phase::Flower, PhotoperiodType::Photoperiod),
            Some(11)
        );
        // Unset parts resolve to the strain-aware defaults.
        assert_eq!(
            cycle.hours_on_for(Phase::Vegetative, PhotoperiodType::Photoperiod),
            Some(18)
        );
        // The autoflower default applies to an unset phase; a set
        // override wins whatever the strain type is.
        assert_eq!(
            cycle.hours_on_for(Phase::Vegetative, PhotoperiodType::Auto),
            Some(18)
        );
        assert_eq!(
            cycle.hours_on_for(Phase::Flower, PhotoperiodType::Auto),
            Some(11)
        );
        // A default cycle resolves to pure defaults.
        let empty = LightCycle::default();
        assert_eq!(empty.lights_on_resolved(), DEFAULT_LIGHTS_ON);
        assert_eq!(
            empty.hours_on_for(Phase::Flower, PhotoperiodType::Photoperiod),
            Some(12)
        );
    }

    #[test]
    fn the_unlit_phases_resolve_to_no_schedule() {
        let cycle = LightCycle::default();
        assert_eq!(
            cycle.hours_on_for(Phase::Drying, PhotoperiodType::Auto),
            None
        );
        assert_eq!(
            cycle.hours_on_for(Phase::Curing, PhotoperiodType::Photoperiod),
            None
        );
    }

    #[test]
    fn override_presence_drives_the_reset_button() {
        assert!(!LightCycle::default().has_override());
        let one = LightCycle {
            hours_on: PhaseHours {
                vegetative: Some(20),
                ..PhaseHours::default()
            },
            ..LightCycle::default()
        };
        assert!(one.has_override());
    }

    #[test]
    fn input_validation_accepts_the_full_wire_form() {
        let input = LightCycleInput {
            lights_on: Some("07:30".to_owned()),
            hours_on: PhaseHoursInput {
                germination: Some(6),
                seedling: None,
                vegetative: Some(MAX_HOURS_ON.into()),
                flower: Some(MIN_HOURS_ON.into()),
            },
        };
        let cycle = input.validate().unwrap();
        assert_eq!(
            cycle.lights_on.map(|time| time.to_string()).as_deref(),
            Some("07:30")
        );
        assert_eq!(cycle.hours_on.germination, Some(6));
        assert_eq!(cycle.hours_on.seedling, None);
        assert_eq!(cycle.hours_on.vegetative, Some(24));
        assert_eq!(cycle.hours_on.flower, Some(1));
    }

    #[test]
    fn input_validation_rejects_bad_times_and_out_of_range_hours() {
        let bad_time = LightCycleInput {
            lights_on: Some("25:00".to_owned()),
            hours_on: PhaseHoursInput::default(),
        };
        assert_eq!(
            bad_time.validate(),
            Err(LightCycleError::BadLightsOn("25:00".to_owned()))
        );
        assert!(
            bad_time
                .validate()
                .unwrap_err()
                .to_string()
                .contains("HH:MM")
        );

        let low = LightCycleInput {
            lights_on: None,
            hours_on: PhaseHoursInput {
                vegetative: Some(0),
                ..PhaseHoursInput::default()
            },
        };
        assert_eq!(
            low.validate(),
            Err(LightCycleError::HoursOnRange {
                phase: Phase::Vegetative,
                value: Some(0),
            })
        );
        assert!(
            low.validate().unwrap_err().to_string().contains("1–24"),
            "the message must name the accepted range"
        );

        let high = LightCycleInput {
            lights_on: None,
            hours_on: PhaseHoursInput {
                flower: Some(30),
                ..PhaseHoursInput::default()
            },
        };
        assert_eq!(
            high.validate(),
            Err(LightCycleError::HoursOnRange {
                phase: Phase::Flower,
                value: Some(30),
            })
        );
        // A non-whole hour arrives as a number serde already rejected,
        // so the raw value is `None` here — still a clear rejection.
        let fractional = LightCycleInput {
            lights_on: None,
            hours_on: PhaseHoursInput {
                flower: Some(-3),
                ..PhaseHoursInput::default()
            },
        };
        assert_eq!(
            fractional.validate(),
            Err(LightCycleError::HoursOnRange {
                phase: Phase::Flower,
                value: Some(-3),
            })
        );
    }

    #[test]
    fn the_wire_form_round_trips_for_storage() {
        let input = LightCycleInput {
            lights_on: Some("06:00".to_owned()),
            hours_on: PhaseHoursInput {
                flower: Some(12),
                ..PhaseHoursInput::default()
            },
        };
        let cycle = input.validate().unwrap();
        let json = serde_json::to_string(&cycle).unwrap();
        assert!(json.contains("\"lights_on\":\"06:00\""));
        assert!(json.contains("\"flower\":12"));
        let back: LightCycle = serde_json::from_str(&json).unwrap();
        assert_eq!(back, cycle);
    }
}
