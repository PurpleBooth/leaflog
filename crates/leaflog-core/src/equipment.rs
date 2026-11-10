//! Equipment inventory domain types (spec §4.1).

use serde::{Deserialize, Serialize};

/// Category of a piece of grow equipment. Labels are the stable
/// `snake_case` identifiers stored in the database and used by the API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EquipmentCategory {
    /// Grow light (LED, HPS, CMH, ...).
    Light,
    /// Tent or grow room.
    TentRoom,
    /// Inline, oscillating or clip fan.
    Fan,
    /// Carbon filter / odour control.
    CarbonFilter,
    /// Humidifier.
    Humidifier,
    /// Dehumidifier.
    Dehumidifier,
    /// Heater or air conditioning.
    HeaterAc,
    /// Pots and growing media.
    PotsMedia,
    /// Pumps and reservoirs.
    PumpsReservoir,
    /// Measurement meters: pH, EC/TDS, PPFD/PAR.
    Meter,
    /// Environmental sensors.
    Sensor,
    /// Controllers (timers, climate controllers, ...).
    Controller,
    /// Everything else.
    Misc,
}

impl EquipmentCategory {
    /// All categories in display order.
    #[must_use]
    pub const fn all() -> [Self; 13] {
        [
            Self::Light,
            Self::TentRoom,
            Self::Fan,
            Self::CarbonFilter,
            Self::Humidifier,
            Self::Dehumidifier,
            Self::HeaterAc,
            Self::PotsMedia,
            Self::PumpsReservoir,
            Self::Meter,
            Self::Sensor,
            Self::Controller,
            Self::Misc,
        ]
    }

    /// The stable `snake_case` label (database and API representation).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::TentRoom => "tent_room",
            Self::Fan => "fan",
            Self::CarbonFilter => "carbon_filter",
            Self::Humidifier => "humidifier",
            Self::Dehumidifier => "dehumidifier",
            Self::HeaterAc => "heater_ac",
            Self::PotsMedia => "pots_media",
            Self::PumpsReservoir => "pumps_reservoir",
            Self::Meter => "meter",
            Self::Sensor => "sensor",
            Self::Controller => "controller",
            Self::Misc => "misc",
        }
    }

    /// Human-facing label for UI lists.
    #[must_use]
    pub const fn display(self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::TentRoom => "Tent / room",
            Self::Fan => "Fan",
            Self::CarbonFilter => "Carbon filter",
            Self::Humidifier => "Humidifier",
            Self::Dehumidifier => "Dehumidifier",
            Self::HeaterAc => "Heater / AC",
            Self::PotsMedia => "Pots & media",
            Self::PumpsReservoir => "Pumps & reservoir",
            Self::Meter => "Meter",
            Self::Sensor => "Sensor",
            Self::Controller => "Controller",
            Self::Misc => "Misc",
        }
    }
}

/// One entry in the equipment catalogue: bundled defaults plus
/// user-added items (the strain-catalogue pattern, spec §4.1). Grow
/// spaces reference entries from here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EquipmentItem {
    /// ULID.
    pub id: String,
    /// User-chosen name.
    pub name: String,
    /// What kind of equipment this is.
    pub category: EquipmentCategory,
    /// Brand and model, free text.
    pub brand_model: Option<String>,
    /// Real measured wall wattage — never the marketing "equivalent".
    pub real_watts: Option<f64>,
    /// Whether the output (light intensity) is dimmable.
    pub dimmable: bool,
    /// Purchase date, `YYYY-MM-DD`.
    pub purchase_date: Option<String>,
    /// Purchase price in [`Self::currency`].
    pub purchase_price: Option<f64>,
    /// ISO 4217 currency code of the price (for example `EUR`).
    pub currency: Option<String>,
    /// Free-form notes.
    pub notes: Option<String>,
    /// Provenance note for bundled entries; `None` on user-added items.
    pub source: Option<String>,
    /// Whether this entry was bundled with Leaflog (seed data). Set by
    /// the seeder only; API-created items are always user entries.
    pub is_builtin: bool,
    /// Creation instant, RFC 3339 UTC.
    pub created_at: String,
    /// Last modification instant, RFC 3339 UTC.
    pub updated_at: String,
}

/// Payload to create or replace an equipment item. All optional fields
/// may be `null`; the backend fills id and timestamps.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EquipmentInput {
    /// User-chosen name.
    pub name: String,
    /// What kind of equipment this is.
    pub category: EquipmentCategory,
    /// Brand and model, free text.
    pub brand_model: Option<String>,
    /// Real measured wall wattage — never the marketing "equivalent".
    pub real_watts: Option<f64>,
    /// Whether the output (light intensity) is dimmable.
    pub dimmable: bool,
    /// Purchase date, `YYYY-MM-DD`.
    pub purchase_date: Option<String>,
    /// Purchase price in [`Self::currency`].
    pub purchase_price: Option<f64>,
    /// ISO 4217 currency code of the price (for example `EUR`).
    pub currency: Option<String>,
    /// Free-form notes.
    pub notes: Option<String>,
}

impl EquipmentInput {
    /// Extracts the create/replace payload from a stored item.
    ///
    /// Used by the clone and round-trip paths; id and timestamps are
    /// intentionally dropped.
    #[must_use]
    pub fn from_item(item: &EquipmentItem) -> Self {
        Self {
            name: item.name.clone(),
            category: item.category,
            brand_model: item.brand_model.clone(),
            real_watts: item.real_watts,
            dimmable: item.dimmable,
            purchase_date: item.purchase_date.clone(),
            purchase_price: item.purchase_price,
            currency: item.currency.clone(),
            notes: item.notes.clone(),
        }
    }
}
// `from_item` intentionally drops `source`/`is_builtin`: inputs describe
// user-shaped payloads, and both flags are preserved by the store on
// update and set only by the seeder on create.
