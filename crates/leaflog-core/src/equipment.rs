//! Equipment catalogue domain types (spec §4.1).

use serde::{Deserialize, Serialize};

/// Category of a piece of grow equipment. Labels are the stable
/// `snake_case` identifiers stored in the database and used by the API.
///
/// The catalogue covers durable hardware only — lights, tents/rooms,
/// fans (incl. carbon filters) and sensors. Sensors are never bundled:
/// each is created from a Home Assistant entity (spec §4.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EquipmentCategory {
    /// Grow light (LED, HPS, CMH, ...).
    Light,
    /// Tent or grow room.
    TentRoom,
    /// Inline, oscillating or clip fan, including carbon filters.
    Fan,
    /// Environmental sensor, created from a bound HA entity.
    Sensor,
    /// Legacy bucket for rows stored under a category the trimmed
    /// catalogue no longer offers (pre-trim databases; V6 recategorizes
    /// or parks them here). Never valid in create/update payloads.
    Misc,
}

impl EquipmentCategory {
    /// The first-class catalogue categories (spec §4.1), in display
    /// order. The API accepts exactly these for create and update; the
    /// UI offers exactly these in pickers and filters. [`Self::Misc`]
    /// is deliberately absent — it exists only so stored legacy rows
    /// keep deserializing.
    #[must_use]
    pub const fn catalogue() -> [Self; 4] {
        [Self::Light, Self::TentRoom, Self::Fan, Self::Sensor]
    }

    /// Parses a stored category label, including the legacy `misc`
    /// bucket. Returns `None` for labels this build does not know.
    #[must_use]
    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "light" => Some(Self::Light),
            "tent_room" => Some(Self::TentRoom),
            "fan" => Some(Self::Fan),
            "sensor" => Some(Self::Sensor),
            "misc" => Some(Self::Misc),
            _ => None,
        }
    }

    /// The stable `snake_case` label (database and API representation).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::TentRoom => "tent_room",
            Self::Fan => "fan",
            Self::Sensor => "sensor",
            Self::Misc => "misc",
        }
    }

    /// Human-facing label for UI lists.
    #[must_use]
    pub const fn display(self) -> &'static str {
        match self {
            Self::Light => "Light",
            Self::TentRoom => "Tent / room",
            Self::Fan => "Fan (incl. carbon filter)",
            Self::Sensor => "Sensor",
            Self::Misc => "Legacy / misc",
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

#[cfg(test)]
mod tests {
    use super::EquipmentCategory;

    #[test]
    fn catalogue_offers_exactly_the_durable_hardware_categories() {
        assert_eq!(
            EquipmentCategory::catalogue(),
            [
                EquipmentCategory::Light,
                EquipmentCategory::TentRoom,
                EquipmentCategory::Fan,
                EquipmentCategory::Sensor,
            ],
        );
    }

    #[test]
    fn labels_round_trip_through_from_label() {
        for category in EquipmentCategory::catalogue() {
            assert_eq!(
                EquipmentCategory::from_label(category.label()),
                Some(category)
            );
        }
        // The legacy bucket parses too — stored rows must deserialize —
        // but it is not part of the catalogue offered by the API/UI.
        assert_eq!(
            EquipmentCategory::from_label(EquipmentCategory::Misc.label()),
            Some(EquipmentCategory::Misc),
        );
    }

    #[test]
    fn unknown_labels_do_not_parse() {
        assert!(EquipmentCategory::from_label("carbon_filter").is_none());
        assert!(EquipmentCategory::from_label("meter").is_none());
        assert!(EquipmentCategory::from_label("humidifier").is_none());
        assert!(EquipmentCategory::from_label("").is_none());
    }
}
