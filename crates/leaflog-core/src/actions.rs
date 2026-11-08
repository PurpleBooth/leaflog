//! Action log domain types (spec §4.2): the quick-add chips, their
//! structured fields, appearance tags, and the M3 phase/day derivation
//! (replaced by the forecast engine in M5).

use serde::{Deserialize, Serialize};

use crate::Phase;

/// One quick-add chip in the action log. Labels are the stable
/// `snake_case` identifiers stored in the database and used by the API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionType {
    /// Lights on/off or a schedule change.
    Lights,
    /// Plain watering.
    Water,
    /// Nutrient feeding.
    Feed,
    /// Trim or defoliation.
    TrimDefoliate,
    /// Training (LST, topping, ...).
    Train,
    /// Transplant to a bigger pot.
    Transplant,
    /// Flip to 12/12 (photoperiods) — moves the forecast.
    Flip,
    /// Pest treatment / IPM.
    PestIpm,
    /// Environment change (fan, humidity, temperature, ...).
    EnvChange,
    /// Light height or dimmer change.
    LightChange,
    /// A plant measurement (height, node count, ...).
    Measurement,
    /// Photo-worthy moment (photos attach separately).
    Photo,
    /// Free-form note.
    Note,
    /// Trichome check — nudges the harvest window.
    TrichomeCheck,
    /// Harvest — ends flower.
    Harvest,
    /// Dry or cure check.
    DryCureCheck,
}

impl ActionType {
    /// All action types in chip order (spec §4.2).
    #[must_use]
    pub const fn all() -> [Self; 16] {
        [
            Self::Lights,
            Self::Water,
            Self::Feed,
            Self::TrimDefoliate,
            Self::Train,
            Self::Transplant,
            Self::Flip,
            Self::PestIpm,
            Self::EnvChange,
            Self::LightChange,
            Self::Measurement,
            Self::Photo,
            Self::Note,
            Self::TrichomeCheck,
            Self::Harvest,
            Self::DryCureCheck,
        ]
    }

    /// The stable `snake_case` label (database and API representation).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Lights => "lights",
            Self::Water => "water",
            Self::Feed => "feed",
            Self::TrimDefoliate => "trim_defoliate",
            Self::Train => "train",
            Self::Transplant => "transplant",
            Self::Flip => "flip",
            Self::PestIpm => "pest_ipm",
            Self::EnvChange => "env_change",
            Self::LightChange => "light_change",
            Self::Measurement => "measurement",
            Self::Photo => "photo",
            Self::Note => "note",
            Self::TrichomeCheck => "trichome_check",
            Self::Harvest => "harvest",
            Self::DryCureCheck => "dry_cure_check",
        }
    }

    /// The short chip caption for the UI.
    #[must_use]
    pub const fn display(self) -> &'static str {
        match self {
            Self::Lights => "Lights",
            Self::Water => "Water",
            Self::Feed => "Feed",
            Self::TrimDefoliate => "Trim",
            Self::Train => "Train",
            Self::Transplant => "Transplant",
            Self::Flip => "Flip 12/12",
            Self::PestIpm => "Pest/IPM",
            Self::EnvChange => "Env change",
            Self::LightChange => "Light change",
            Self::Measurement => "Measure",
            Self::Photo => "Photo",
            Self::Note => "Note",
            Self::TrichomeCheck => "Trichomes",
            Self::Harvest => "Harvest",
            Self::DryCureCheck => "Dry/cure",
        }
    }
}

/// The structured fields of an action entry.
///
/// One flat shape keeps the API and schema simple: fields that are
/// irrelevant for the chosen [`ActionType`] stay `null`. EC is
/// canonical in **µS/cm** (spec §5.3); TDS readouts are converted by
/// the client using the meter scale before submission (unit-aware
/// display lands with the units work).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ActionDetails {
    /// Water or feed volume, millilitres.
    pub volume_ml: Option<f64>,
    /// Input (irrigation) pH.
    pub input_ph: Option<f64>,
    /// Runoff pH.
    pub runoff_ph: Option<f64>,
    /// Input EC, µS/cm.
    pub input_ec_us: Option<u32>,
    /// Runoff EC, µS/cm.
    pub runoff_ec_us: Option<u32>,
    /// Recipe name (feed) or schedule text (lights).
    pub recipe: Option<String>,
    /// Light height above canopy, centimetres (also used for
    /// transplant depth notes).
    pub height_cm: Option<f64>,
    /// Plant height at measurement, centimetres.
    pub plant_height_cm: Option<f64>,
    /// Dimmer percentage.
    pub dim_pct: Option<u8>,
    /// Harvested wet weight, grams.
    pub weight_g: Option<f64>,
    /// Cloudy trichome percentage.
    pub cloudy_pct: Option<u8>,
    /// Amber trichome percentage.
    pub amber_pct: Option<u8>,
}

/// Appearance tags commonly used on log entries (spec §4.2). Free-form
/// tags are allowed; these get chips and translations.
pub const APPEARANCE_TAGS: &[&str] = &[
    "healthy",
    "dark_green",
    "light_green",
    "yellowing",
    "purple_stems",
    "droop",
    "praying",
    "taco",
    "canoe",
    "bleaching",
    "tip_burn",
    "fox_tailing",
    "nutrient_burn",
    "stretchy",
    "short",
];

/// Derives the phase label for an action on `grow_day`.
///
/// Naive on purpose — the M5 forecast engine replaces it (see
/// docs/DECISIONS.md): germination is folded into seedling, `flip_day`
/// is the grow day of the `Flip` entry, `harvested` is true once a
/// `Harvest` entry exists at or before this action.
#[must_use]
pub fn derive_phase(grow_day: u16, flip_day: Option<u16>, harvested: bool) -> Phase {
    if harvested {
        Phase::Drying
    } else if flip_day.is_some_and(|flip| grow_day >= flip) {
        Phase::Flower
    } else if grow_day <= 14 {
        Phase::Seedling
    } else {
        Phase::Vegetative
    }
}

#[cfg(test)]
mod tests {
    use super::{ActionType, Phase, derive_phase};

    #[test]
    fn labels_are_unique_and_stable() {
        let all = ActionType::all();
        let mut labels: Vec<&str> = all.iter().map(|action| action.label()).collect();
        labels.sort_unstable();
        labels.dedup();
        assert_eq!(labels.len(), all.len());
        assert!(labels.contains(&"flip"));
        assert!(labels.contains(&"dry_cure_check"));
    }

    #[test]
    fn phase_derivation() {
        assert_eq!(derive_phase(0, None, false), Phase::Seedling);
        assert_eq!(derive_phase(14, None, false), Phase::Seedling);
        assert_eq!(derive_phase(15, None, false), Phase::Vegetative);
        assert_eq!(derive_phase(40, Some(30), false), Phase::Flower);
        // A flip that has not happened yet does not count.
        assert_eq!(derive_phase(20, Some(30), false), Phase::Vegetative);
        assert_eq!(derive_phase(99, Some(30), true), Phase::Drying);
    }
}
