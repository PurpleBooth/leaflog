//! Seedbank domain types (spec §4.13, §5.4): owned seed batches with
//! storage-dependent viability windows.

use serde::{Deserialize, Serialize};

/// How a batch of seeds is stored. Labels are the stable `snake_case`
/// identifiers stored in the database and used by the API.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StorageMethod {
    /// Room temperature, dark drawer.
    RoomTempDark,
    /// Cool, dark, dry cupboard (the default).
    CoolDarkDry,
    /// Refrigerated, airtight (~4–8 °C).
    Refrigerated,
    /// Frozen, dried, airtight.
    Frozen,
}

impl StorageMethod {
    /// All storage methods in display order.
    #[must_use]
    pub const fn all() -> [Self; 4] {
        [
            Self::RoomTempDark,
            Self::CoolDarkDry,
            Self::Refrigerated,
            Self::Frozen,
        ]
    }

    /// The stable `snake_case` label (database and API representation).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::RoomTempDark => "room_temp_dark",
            Self::CoolDarkDry => "cool_dark_dry",
            Self::Refrigerated => "refrigerated",
            Self::Frozen => "frozen",
        }
    }

    /// Parses a stored label.
    #[must_use]
    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "room_temp_dark" => Some(Self::RoomTempDark),
            "cool_dark_dry" => Some(Self::CoolDarkDry),
            "refrigerated" => Some(Self::Refrigerated),
            "frozen" => Some(Self::Frozen),
            _ => None,
        }
    }

    /// Human-facing label for UI lists.
    #[must_use]
    pub const fn display(self) -> &'static str {
        match self {
            Self::RoomTempDark => "Room temp, dark drawer",
            Self::CoolDarkDry => "Cool, dark, dry (default)",
            Self::Refrigerated => "Refrigerated, airtight",
            Self::Frozen => "Frozen, dried, airtight",
        }
    }
}

/// One batch of owned seeds (spec §4.13).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeedBatch {
    /// ULID.
    pub id: String,
    /// Strain database link (nullable: unbundled/unnamed seeds).
    pub strain_id: Option<String>,
    /// Seeds remaining. Starting plants from a linked batch decrements
    /// this; manual edits always win.
    pub quantity: u32,
    /// Where the seeds came from: bank, breeder, own harvest, ...
    pub acquired_from: Option<String>,
    /// Acquisition date, `YYYY-MM-DD` (bank packing date when known,
    /// else purchase/receipt date).
    pub acquisition_on: String,
    /// Storage method (drives the §5.4 viability window).
    pub storage_method: StorageMethod,
    /// Free-form notes.
    pub notes: Option<String>,
    /// Creation instant, RFC 3339 UTC.
    pub created_at: String,
    /// Last modification instant, RFC 3339 UTC.
    pub updated_at: String,
}

/// Payload to create or replace a seed batch. The backend fills id and
/// timestamps.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeedBatchInput {
    /// Strain database link.
    pub strain_id: Option<String>,
    /// Seeds in the batch.
    pub quantity: u32,
    /// Where the seeds came from.
    pub acquired_from: Option<String>,
    /// Acquisition date, `YYYY-MM-DD`.
    pub acquisition_on: String,
    /// Storage method.
    pub storage_method: StorageMethod,
    /// Free-form notes.
    pub notes: Option<String>,
}

impl SeedBatchInput {
    /// Extracts the create/replace payload from a stored batch; id and
    /// timestamps are intentionally dropped.
    #[must_use]
    pub fn from_batch(batch: &SeedBatch) -> Self {
        Self {
            strain_id: batch.strain_id.clone(),
            quantity: batch.quantity,
            acquired_from: batch.acquired_from.clone(),
            acquisition_on: batch.acquisition_on.clone(),
            storage_method: batch.storage_method,
            notes: batch.notes.clone(),
        }
    }
}

/// A batch with its computed §5.4 viability window — the API response
/// shape shared by the backend routes and the frontend.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SeedBatchView {
    /// The stored batch.
    #[serde(flatten)]
    pub batch: SeedBatch,
    /// The §5.4 window for the batch's storage method and age.
    pub viability: Viability,
    /// Observed germination rate from the batch's tests (spec §4.13);
    /// `None` until the first test is recorded.
    #[serde(default)]
    pub observed: Option<ObservedRate>,
}

impl SeedBatchView {
    /// Computes the view for a stored batch at `today` (`YYYY-MM-DD`),
    /// without test evidence.
    #[must_use]
    pub fn new(batch: SeedBatch, today: &str) -> Self {
        let viability = viability(&batch.acquisition_on, batch.storage_method, today);
        Self {
            batch,
            viability,
            observed: None,
        }
    }

    /// The view with observed germination evidence attached.
    #[must_use]
    pub fn with_observed(mut self, tests: &[GerminationTest]) -> Self {
        self.observed = ObservedRate::of(tests);
        self
    }
}

/// One germination test of a seed batch (spec §4.13): date, number
/// sown and number sprouted (paper-towel or direct).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GerminationTest {
    /// ULID.
    pub id: String,
    /// The tested batch.
    pub seed_batch_id: String,
    /// Test date, `YYYY-MM-DD`.
    pub tested_on: String,
    /// Seeds sown.
    pub sown: u32,
    /// Seeds that sprouted (never more than sown).
    pub sprouted: u32,
    /// Free-form notes (method, medium, ...).
    pub notes: Option<String>,
    /// Creation instant, RFC 3339 UTC.
    pub created_at: String,
    /// Last modification instant, RFC 3339 UTC.
    pub updated_at: String,
}

/// Payload to create a germination-test entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GerminationTestInput {
    /// Test date, `YYYY-MM-DD`.
    pub tested_on: String,
    /// Seeds sown.
    pub sown: u32,
    /// Seeds that sprouted.
    pub sprouted: u32,
    /// Free-form notes.
    pub notes: Option<String>,
}

/// The observed germination rate across a batch's tests — user
/// evidence shown beside the §5.4 preset window, never overwriting it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedRate {
    /// Total seeds sprouted across all tests.
    pub sprouted: u32,
    /// Total seeds sown across all tests.
    pub sown: u32,
}

impl ObservedRate {
    /// Aggregates the tests into one rate; `None` when no test has
    /// been recorded.
    #[must_use]
    pub fn of(tests: &[GerminationTest]) -> Option<Self> {
        (!tests.is_empty()).then(|| Self {
            sprouted: tests.iter().map(|test| test.sprouted).sum(),
            sown: tests.iter().map(|test| test.sown).sum(),
        })
    }
}

/// The viability assessment of one batch: the §5.4 window for its
/// storage method, whether the batch has passed it, and the source the
/// numbers come from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Viability {
    /// Full-viability window in whole months, lower and upper bound —
    /// sources disagree, so the range is shown. `None` upper bound
    /// means open-ended ("10+ years reported").
    pub window_months: (u32, Option<u32>),
    /// The batch's age in whole months.
    pub age_months: u32,
    /// Whether the age has passed the window's upper bound: falling
    /// germination odds, never a hard block (spec §5.4).
    pub past_prime: bool,
    /// Source label for the preset numbers (seed-data discipline).
    /// Serialize-only: the label is a constant computed server-side, so
    /// deserialization (the frontend reading API responses) restores
    /// the constant instead of borrowing.
    #[serde(skip_deserializing, default = "Viability::source_default")]
    pub source: &'static str,
}

impl Viability {
    /// Source citation carried by every §5.4 preset row.
    pub const SOURCE: &'static str =
        "spec §5.4 table (research snapshot 2026-11-08); sources disagree — range shown";

    /// The serde default for [`Self::source`].
    const fn source_default() -> &'static str {
        Self::SOURCE
    }
}

/// The §5.4 full-viability window (whole months) for a storage method.
#[must_use]
pub const fn viability_months(method: StorageMethod) -> (u32, Option<u32>) {
    match method {
        StorageMethod::RoomTempDark => (6, Some(12)),
        StorageMethod::CoolDarkDry => (12, Some(36)),
        StorageMethod::Refrigerated => (60, Some(120)),
        StorageMethod::Frozen => (120, None),
    }
}

/// Assesses a batch: its storage method's window against its age at
/// `today` (`YYYY-MM-DD`). Storage conditions dominate over seed type
/// (spec §5.4) — no per-strain numbers exist.
#[must_use]
pub fn viability(acquisition_on: &str, storage_method: StorageMethod, today: &str) -> Viability {
    let window = viability_months(storage_method);
    let age_months = age_in_months(acquisition_on, today).unwrap_or(0);
    let past_prime = window.1.is_some_and(|max| age_months > max);
    Viability {
        window_months: window,
        age_months,
        past_prime,
        source: Viability::SOURCE,
    }
}

/// Whole calendar months between two `YYYY-MM-DD` dates.
///
/// `to` minus `from`, floored at zero; `None` when a date does not
/// parse. A batch acquired mid-month is only "one month old" once the
/// same day-of-month has passed.
#[must_use]
pub fn age_in_months(from: &str, to: &str) -> Option<u32> {
    let (from_year, from_month, from_day) = parse_ymd(from)?;
    let (to_year, to_month, to_day) = parse_ymd(to)?;
    // Signed arithmetic: the month difference alone can be negative
    // across a year boundary (November -> January).
    let months = (i32::from(to_year) - i32::from(from_year)) * 12 + i32::from(to_month)
        - i32::from(from_month)
        - i32::from(to_day < from_day);
    // Bounded by the u16 year range, so the cast cannot fail; `.ok()`
    // keeps it panic-free regardless.
    u32::try_from(months.max(0)).ok()
}

/// Whether `date` is a well-formed `YYYY-MM-DD` calendar date.
#[must_use]
pub fn valid_date(date: &str) -> bool {
    parse_ymd(date).is_some()
}

/// Parses `YYYY-MM-DD` into numeric components.
fn parse_ymd(date: &str) -> Option<(u16, u16, u16)> {
    let mut parts = date.split('-');
    let year = parts.next()?.parse().ok()?;
    let month = parts.next()?.parse().ok()?;
    let day = parts.next()?.parse().ok()?;
    (parts.next().is_none() && (1..=12).contains(&month) && (1..=31).contains(&day))
        .then_some((year, month, day))
}

#[cfg(test)]
mod tests {
    use super::{StorageMethod, age_in_months, viability, viability_months};

    #[test]
    fn storage_labels_round_trip() {
        for method in StorageMethod::all() {
            assert_eq!(StorageMethod::from_label(method.label()), Some(method));
        }
        assert!(StorageMethod::from_label("cellar").is_none());
    }

    #[test]
    fn windows_match_the_spec_table() {
        assert_eq!(viability_months(StorageMethod::RoomTempDark), (6, Some(12)));
        assert_eq!(viability_months(StorageMethod::CoolDarkDry), (12, Some(36)));
        assert_eq!(
            viability_months(StorageMethod::Refrigerated),
            (60, Some(120))
        );
        assert_eq!(viability_months(StorageMethod::Frozen), (120, None));
    }

    #[test]
    fn age_counts_whole_calendar_months() {
        assert_eq!(age_in_months("2026-01-15", "2026-02-14"), Some(0));
        assert_eq!(age_in_months("2026-01-15", "2026-02-15"), Some(1));
        assert_eq!(age_in_months("2026-01-15", "2027-01-15"), Some(12));
        assert_eq!(age_in_months("2026-01-15", "2020-01-15"), Some(0));
        assert_eq!(age_in_months("nonsense", "2026-01-15"), None);
    }

    #[test]
    fn past_prime_needs_the_upper_bound_passed() {
        let fresh = viability("2026-01-01", StorageMethod::CoolDarkDry, "2026-06-01");
        assert_eq!(fresh.age_months, 5);
        assert!(!fresh.past_prime);

        let edge = viability("2023-06-01", StorageMethod::CoolDarkDry, "2026-06-01");
        assert_eq!(edge.age_months, 36);
        assert!(!edge.past_prime, "the last day inside the window is fine");

        // May 31 2023 + 37 months lands on June 30 2026 (months without
        // a 31st clamp to their last day), so July 1 is past the window.
        let old = viability("2023-05-31", StorageMethod::CoolDarkDry, "2026-07-01");
        assert_eq!(old.age_months, 37);
        assert!(old.past_prime);

        // Frozen is open-ended: "10+ years reported", never past prime.
        let ancient = viability("2000-01-01", StorageMethod::Frozen, "2026-01-01");
        assert_eq!(ancient.window_months.1, None);
        assert!(!ancient.past_prime);
    }
}
