//! Feeding schedules (ADR 0004): product–dose grids, not EC plans.
//!
//! A feeding schedule is a reusable chart: free-text product rows
//! against fixed columns — PROPAGATION (one dose), VEG (one dose, any
//! length), one column per flowering week (the user picks how many),
//! and a WATER tail. Fitting stretches the chart onto a grow's actual
//! timeline: the propagation column covers the seedling segment, the
//! veg dose covers the whole vegetative segment, the chart's
//! flowering weeks spread across the real flowering days week by
//! week, and the WATER tail stays pinned to the substrate flush —
//! never dropped, whatever the compression.
//!
//! Doses are min–max ranges stored in SI-internal integer steps:
//! µL/L for ml/L products and mg/L for g/L products (a factor of
//! 1000), so locale decimal display never touches stored data.

#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)] // SI scaling rounds before casting (substrate/TDS graft)

use crate::format::{DecimalStyle, format_decimal};
use serde::{Deserialize, Serialize};

/// The dose unit of one product row (spec: per-litre dosing only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DoseUnit {
    /// Millilitres per litre; stored in µL/L.
    #[serde(rename = "ml_per_l")]
    Milliliters,
    /// Grams per litre; stored in mg/L.
    #[serde(rename = "g_per_l")]
    Grams,
}

impl DoseUnit {
    /// SI steps per display unit (µL per ml, mg per g).
    pub const SI_STEPS_PER_UNIT: u32 = 1000;

    /// The stable wire/database label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Milliliters => "ml_per_l",
            Self::Grams => "g_per_l",
        }
    }

    /// The display suffix (`ml/L`, `g/L`).
    #[must_use]
    pub const fn display(self) -> &'static str {
        match self {
            Self::Milliliters => "ml/L",
            Self::Grams => "g/L",
        }
    }

    /// Parses a stored/wire label; unknown labels read as ml/L
    /// (the tolerant default every other label parser here uses).
    #[must_use]
    pub fn from_label(label: &str) -> Self {
        if label == Self::Grams.label() {
            Self::Grams
        } else {
            Self::Milliliters
        }
    }
}

/// One dose cell: a min–max range in SI-internal integer steps
/// (µL/L or mg/L, depending on the product's [`DoseUnit`]). A constant
/// dose has `min == max`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Dose {
    /// Range minimum, SI steps.
    pub min: u32,
    /// Range maximum, SI steps.
    pub max: u32,
}

impl Dose {
    /// Builds a dose, ordering the bounds so `min <= max` always
    /// holds (a reversed input is a user typo, not a rejection).
    #[must_use]
    pub const fn new(min: u32, max: u32) -> Self {
        if min <= max {
            Self { min, max }
        } else {
            Self { min: max, max: min }
        }
    }
}

/// Parses one dose cell of chart text (`2`, `0,25–2`, `0.25-2`) into
/// an SI dose in `unit`'s steps. Empty text, a lone dash, or anything
/// non-numeric reads as "no dose" (`None`) — the grid's `—` cells.
#[must_use]
pub fn parse_dose_text(text: &str, unit: DoseUnit) -> Option<Dose> {
    let normalized = text
        .trim()
        .replace(',', ".")
        .replace(['–', '—', '−'], "-")
        .replace("..", "-");
    if normalized.is_empty() || normalized == "-" {
        return None;
    }
    let (min_text, max_text) = match normalized.split_once('-') {
        Some((low, high)) => (low.trim(), high.trim()),
        None => (normalized.as_str(), normalized.as_str()),
    };
    if min_text.is_empty() || max_text.is_empty() {
        return None;
    }
    let min: f64 = min_text.parse().ok()?;
    let max: f64 = max_text.parse().ok()?;
    if !min.is_finite() || !max.is_finite() || min < 0.0 {
        return None;
    }
    // Swapped chart cells ("3–1") normalize to ascending.
    let (min, max) = if max < min { (max, min) } else { (min, max) };
    scale_to_si(min, max, unit)
}

/// Rounds display-unit values into SI steps. The floats are already
/// validated finite and non-negative, so the u64 cast is exact
/// enough and the `try_from` rejects the absurd.
// f64 -> u64 after `.round()` on validated non-negative finite input;
// no truncation or sign loss can occur (docs/DECISIONS.md register).
fn scale_to_si(min: f64, max: f64, unit: DoseUnit) -> Option<Dose> {
    let scale = f64::from(DoseUnit::SI_STEPS_PER_UNIT);
    let min_si = (min * scale).round() as u64;
    let max_si = (max * scale).round() as u64;
    let (min_si, max_si) = (u32::try_from(min_si).ok()?, u32::try_from(max_si).ok()?);
    let _ = unit; // both units scale by 1000; the unit only picks the label
    Some(Dose::new(min_si, max_si))
}

/// Formats a dose for display in `unit` ("2", "0,25–2"), using
/// `style`'s decimal separator and trimming trailing zeros.
#[must_use]
pub fn format_dose(dose: Dose, style: DecimalStyle) -> String {
    let render = |si: u32| {
        let display = f64::from(si) / f64::from(DoseUnit::SI_STEPS_PER_UNIT);
        let formatted = format_decimal(display, 3, style);
        let separator = style.decimal_separator();
        if formatted.contains(separator) {
            formatted
                .trim_end_matches('0')
                .trim_end_matches(separator)
                .to_owned()
        } else {
            formatted
        }
    };
    if dose.min == dose.max {
        render(dose.max)
    } else {
        format!("{}–{}", render(dose.min), render(dose.max))
    }
}

/// One product row of a schedule.
///
/// A free-text name, a dosing unit, a free-text group label (the
/// chart's section headers, e.g. Feeds), an optional note, and the
/// dose cells. No product catalogue exists — rows are owned by
/// their schedule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduleProduct {
    /// ULID; empty on the write path (the server generates it).
    #[serde(default)]
    pub id: String,
    /// Product name as printed on the chart (`Bio·Grow`).
    pub name: String,
    /// The dosing unit of every dose cell in this row.
    pub unit: DoseUnit,
    /// Free-text group label; empty string when ungrouped.
    #[serde(default)]
    pub group: String,
    /// Free-text note under the row name; `None` when plain.
    #[serde(default)]
    pub note: Option<String>,
    /// The dose cells across the chart's columns.
    pub doses: ProductDoses,
}

/// The dose cells of one product row: one optional dose per fixed
/// column (PROPAGATION, VEG, WATER) and one per flowering week.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProductDoses {
    /// The PROPAGATION column dose.
    #[serde(default)]
    pub propagation: Option<Dose>,
    /// The VEG column dose (repeated across the whole veg segment).
    #[serde(default)]
    pub veg: Option<Dose>,
    /// One dose per flowering week, 0-based (index 0 = WK 1). Missing
    /// weeks read as "no dose"; the editor pads to the week count.
    #[serde(default)]
    pub flower: Vec<Option<Dose>>,
    /// The WATER tail dose (usually none: the tail is plain water).
    #[serde(default)]
    pub water: Option<Dose>,
}

impl ProductDoses {
    /// Empty cells (every column blank).
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            propagation: None,
            veg: None,
            flower: Vec::new(),
            water: None,
        }
    }

    /// The dose of one flowering week (`week` is 1-based).
    #[must_use]
    pub fn week(&self, week: usize) -> Option<Dose> {
        if week == 0 {
            return None;
        }
        self.flower.get(week - 1).copied().flatten()
    }

    /// Pads or truncates the flower cells to `weeks` entries.
    pub fn resize_flower(&mut self, weeks: usize) {
        self.flower.resize(weeks, None);
    }

    /// True when every cell is blank (a row with no advice).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.propagation.is_none()
            && self.veg.is_none()
            && self.water.is_none()
            && self.flower.iter().all(Option::is_none)
    }
}

/// A reusable feeding schedule (ADR 0004): the product–dose grid one
/// assigns to grows. Medium-independent by design — any schedule may
/// be assigned to any grow.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedSchedule {
    /// ULID; empty on the write path (the server generates it).
    #[serde(default)]
    pub id: String,
    /// User-facing name (`BioBizz — ALL·MIX`).
    pub name: String,
    /// Provenance note (the chart the grid came from); `None` when
    /// self-made.
    #[serde(default)]
    pub source: Option<String>,
    /// How many flowering weeks the chart carries (the WK 1…N span).
    pub flower_weeks: u8,
    /// The product rows, in chart order.
    pub products: Vec<ScheduleProduct>,
    /// Creation instant, RFC 3339 UTC.
    #[serde(default)]
    pub created_at: String,
    /// Last modification instant, RFC 3339 UTC.
    #[serde(default)]
    pub updated_at: String,
}

impl FeedSchedule {
    /// The hard limits of one schedule.
    pub const MAX_FLOWER_WEEKS: u8 = 24;
    /// The hard limit of product rows.
    pub const MAX_PRODUCTS: usize = 64;

    /// Validates the schedule for storage: a non-empty name, a
    /// flowering-week count in `1..=[Self::MAX_FLOWER_WEEKS]`, and
    /// every product named. `Err` carries the i18n-neutral reason.
    ///
    /// # Errors
    ///
    /// Returns a static reason string for the API to translate.
    pub fn validate(&self) -> Result<(), ScheduleValidationError> {
        if self.name.trim().is_empty() {
            return Err(ScheduleValidationError::Name);
        }
        if self.flower_weeks == 0 || self.flower_weeks > Self::MAX_FLOWER_WEEKS {
            return Err(ScheduleValidationError::FlowerWeeks);
        }
        if self.products.len() > Self::MAX_PRODUCTS {
            return Err(ScheduleValidationError::TooManyProducts);
        }
        if self
            .products
            .iter()
            .any(|product| product.name.trim().is_empty())
        {
            return Err(ScheduleValidationError::ProductName);
        }
        Ok(())
    }

    /// Returns an edit-ready copy: trimmed texts, blank-name products
    /// dropped, the flower cells padded/truncated to the week count.
    /// Call after [`Self::validate`] on the write path.
    #[must_use]
    pub fn normalized(&self) -> Self {
        let flower_weeks = self.flower_weeks.clamp(1, Self::MAX_FLOWER_WEEKS);
        let mut products: Vec<ScheduleProduct> = self
            .products
            .iter()
            .filter(|product| !product.name.trim().is_empty())
            .map(|product| {
                let mut product = product.clone();
                product.name = product.name.trim().to_owned();
                product.group = product.group.trim().to_owned();
                product.note = product
                    .note
                    .as_deref()
                    .map(str::trim)
                    .filter(|note| !note.is_empty())
                    .map(ToOwned::to_owned);
                product.doses.resize_flower(usize::from(flower_weeks));
                product
            })
            .collect();
        products.truncate(Self::MAX_PRODUCTS);
        Self {
            id: self.id.clone(),
            name: self.name.trim().to_owned(),
            source: self
                .source
                .as_deref()
                .map(str::trim)
                .filter(|source| !source.is_empty())
                .map(ToOwned::to_owned),
            flower_weeks,
            products,
            created_at: self.created_at.clone(),
            updated_at: self.updated_at.clone(),
        }
    }

    /// The distinct non-empty group labels, in first-seen order.
    #[must_use]
    pub fn groups(&self) -> Vec<String> {
        let mut groups: Vec<String> = Vec::new();
        for group in self.products.iter().map(|product| product.group.as_str()) {
            if !group.is_empty() && !groups.iter().any(|seen| seen == group) {
                groups.push(group.to_owned());
            }
        }
        groups
    }

    /// The distinct dose units in use, in first-seen order.
    #[must_use]
    pub fn units(&self) -> Vec<DoseUnit> {
        let mut units: Vec<DoseUnit> = Vec::new();
        for unit in self.products.iter().map(|product| product.unit) {
            if !units.contains(&unit) {
                units.push(unit);
            }
        }
        units
    }
}

/// Why a schedule failed validation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScheduleValidationError {
    /// The name is blank.
    Name,
    /// The flowering-week count is zero or over the cap.
    FlowerWeeks,
    /// More products than the cap allows.
    TooManyProducts,
    /// A product row has a blank name.
    ProductName,
}

/// The fitted phases of the plan, one per timeline segment: the chart
/// columns mapped onto real grow days.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FittedPhase {
    /// PROPAGATION column on the seedling segment.
    Propagation,
    /// VEG column on the vegetative segment.
    Veg,
    /// WK 1…N columns on the flowering segment.
    Flower,
    /// WATER tail pinned to the substrate flush — never dropped.
    Water,
}

impl FittedPhase {
    /// The stable `snake_case` label (wire and database form).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Propagation => "propagation",
            Self::Veg => "veg",
            Self::Flower => "flower",
            Self::Water => "water",
        }
    }

    /// Parses a stored label; unknown labels read as veg (the
    /// tolerant default the old phase parser used).
    #[must_use]
    pub fn from_label(label: &str) -> Self {
        match label {
            "propagation" => Self::Propagation,
            "flower" => Self::Flower,
            "water" => Self::Water,
            _ => Self::Veg,
        }
    }

    /// True for the never-dropped tail (WATER).
    #[must_use]
    pub fn is_protected(self) -> bool {
        self == Self::Water
    }
}

/// One dose line on a fitted day.
///
/// The product snapshot (name, unit, group, note) plus the due
/// range. A frozen copy of the chart cell — renaming or deleting a
/// product in the schedule never rewrites a stored plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FittedDose {
    /// Product name at fit time.
    pub name: String,
    /// Dose unit at fit time.
    pub unit: DoseUnit,
    /// Group label at fit time; empty when ungrouped.
    #[serde(default)]
    pub group: String,
    /// Product note at fit time.
    #[serde(default)]
    pub note: Option<String>,
    /// The due range.
    pub dose: Dose,
}

/// One fitted day of a grow's plan.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedPlanDay {
    /// Grow day (0-based from grow start).
    pub grow_day: u16,
    /// The chart column this day came from.
    pub phase: FittedPhase,
    /// The 1-based flowering week, when `phase` is flower.
    pub week: Option<u8>,
    /// The doses due this day.
    pub doses: Vec<FittedDose>,
}

impl FeedPlanDay {
    /// True on the protected WATER tail.
    #[must_use]
    pub fn is_protected(&self) -> bool {
        self.phase.is_protected()
    }
}

/// A grow's fitted plan, frozen at fit time: the schedule snapshot
/// header plus one row per grow day. Survives the schedule's later
/// edit or deletion — refitting replaces it wholesale.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FeedPlan {
    /// The schedule the plan was fitted from.
    pub schedule_id: String,
    /// The schedule's name at fit time (the attribution line).
    pub schedule_name: String,
    /// The chart's flowering weeks at fit time.
    pub flower_weeks: u8,
    /// Fit instant, RFC 3339 UTC.
    pub fitted_at: String,
    /// The fitted days, ordered by grow day.
    pub days: Vec<FeedPlanDay>,
}

/// The fit result handed back after fitting: the same days a
/// [`FeedPlan`] stores, plus the shape summary.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FitResult {
    /// The fitted days, ordered by grow day.
    pub days: Vec<FeedPlanDay>,
    /// The chart's flowering weeks the fit spread.
    pub flower_weeks: u8,
    /// The total actual days covered.
    pub actual_total_days: u16,
}

/// The plant's actual phase timeline (days, 0-based from grow start).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActualTimeline {
    /// Days the seedling phase lasts.
    pub seedling_days: u16,
    /// Days the veg phase lasts (grower's choice).
    pub veg_days: u16,
    /// Days the flower phase lasts (as logged).
    pub flower_days: u16,
    /// Days the flush lasts (substrate-dependent).
    pub flush_days: u16,
}

/// Fits a schedule onto the actual timeline (ADR 0004).
///
/// Piecewise, column by column: the PROPAGATION dose covers the
/// seedling segment, the VEG dose covers the whole vegetative segment
/// (veg length is the grower's choice — one dose repeats, never
/// stretches), the chart's `flower_weeks` spread across the real
/// flowering days (`week = day × weeks / flower_days`, so a 9-week
/// chart on a 56-day flower lands WK 1–8 evenly), and the WATER tail
/// fills the flush days — pinned to the end, never dropped or
/// interpolated away, whatever the compression.
#[must_use]
pub fn fit_schedule(schedule: &FeedSchedule, actual: &ActualTimeline) -> FitResult {
    let mut days: Vec<FeedPlanDay> = Vec::new();
    let mut cursor: u32 = 0;

    let push_segment = |len: u16,
                        phase: FittedPhase,
                        week: Option<u8>,
                        doses: Vec<FittedDose>,
                        days: &mut Vec<FeedPlanDay>,
                        cursor: &mut u32| {
        for offset in 0..u32::from(len) {
            let grow_day = u16::try_from(*cursor + offset).unwrap_or(u16::MAX);
            days.push(FeedPlanDay {
                grow_day,
                phase,
                week,
                doses: doses.clone(),
            });
        }
        *cursor += u32::from(len);
    };

    let doses_of = |pick: &dyn Fn(&ScheduleProduct) -> Option<Dose>| -> Vec<FittedDose> {
        schedule
            .products
            .iter()
            .filter_map(|product| {
                pick(product).map(|dose| FittedDose {
                    name: product.name.clone(),
                    unit: product.unit,
                    group: product.group.clone(),
                    note: product.note.clone(),
                    dose,
                })
            })
            .collect()
    };

    // PROPAGATION on the seedling segment.
    push_segment(
        actual.seedling_days,
        FittedPhase::Propagation,
        None,
        doses_of(&|product| product.doses.propagation),
        &mut days,
        &mut cursor,
    );
    // VEG, one dose repeated across the whole segment.
    push_segment(
        actual.veg_days,
        FittedPhase::Veg,
        None,
        doses_of(&|product| product.doses.veg),
        &mut days,
        &mut cursor,
    );
    // Flowering: the chart weeks warp onto the real days.
    let flower_weeks = u32::from(schedule.flower_weeks);
    if actual.flower_days > 0 && flower_weeks > 0 {
        let flower_days = u32::from(actual.flower_days);
        let week_of = |day: u32| -> u8 {
            let index = day * flower_weeks / flower_days;
            u8::try_from(index + 1).unwrap_or(schedule.flower_weeks)
        };
        for offset in 0..actual.flower_days {
            let week = week_of(u32::from(offset));
            let doses = doses_of(&|product| product.doses.week(usize::from(week)));
            let grow_day = u16::try_from(cursor + u32::from(offset)).unwrap_or(u16::MAX);
            days.push(FeedPlanDay {
                grow_day,
                phase: FittedPhase::Flower,
                week: Some(week),
                doses,
            });
        }
        cursor += flower_days;
    }
    // WATER tail: pinned to the flush, never dropped.
    push_segment(
        actual.flush_days,
        FittedPhase::Water,
        None,
        doses_of(&|product| product.doses.water),
        &mut days,
        &mut cursor,
    );

    FitResult {
        days,
        flower_weeks: schedule.flower_weeks,
        actual_total_days: u16::try_from(cursor).unwrap_or(u16::MAX),
    }
}

/// The feed context of one grow week, distilled from a plan: what
/// phase the grow sits in today, which chart week that is, the due
/// doses, and the flush countdown for the tail.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThisWeek {
    /// The phase today sits in.
    pub phase: FittedPhase,
    /// The 1-based chart week, when in the flowering segment.
    pub week: Option<u8>,
    /// The plan's total chart weeks; `None` when none of the days
    /// carry weeks.
    pub total_weeks: Option<u8>,
    /// The distinct doses due this week (deduped by product name,
    /// chart order).
    pub doses: Vec<FittedDose>,
    /// The first flush (WATER) day, when the plan has a tail.
    pub flush_start_day: Option<u16>,
    /// Days from today until the flush starts; `None` once inside it
    /// (or when the plan has no tail).
    pub days_until_flush: Option<u16>,
}

/// Distils the week context at `today` from a plan's days. `None`
/// when the plan is empty or `today` lies outside it.
#[must_use]
pub fn this_week(days: &[FeedPlanDay], today: u16) -> Option<ThisWeek> {
    let total_weeks = days.iter().filter_map(|day| day.week).max();
    let flush_start_day = days
        .iter()
        .find(|day| day.phase == FittedPhase::Water)
        .map(|day| day.grow_day);
    let today_day = days.iter().find(|day| day.grow_day == today)?;

    let week_doses = |predicate: &dyn Fn(&FeedPlanDay) -> bool| -> Vec<FittedDose> {
        let mut doses: Vec<FittedDose> = Vec::new();
        for dose in days
            .iter()
            .filter(|day| predicate(day))
            .flat_map(|day| day.doses.iter())
        {
            if !doses.iter().any(|seen| seen.name == dose.name) {
                doses.push(dose.clone());
            }
        }
        doses
    };

    let doses = match today_day.phase {
        FittedPhase::Propagation => week_doses(&|day| day.phase == FittedPhase::Propagation),
        FittedPhase::Veg => week_doses(&|day| day.phase == FittedPhase::Veg),
        FittedPhase::Flower => week_doses(&|day| day.week == today_day.week),
        FittedPhase::Water => today_day.doses.clone(),
    };
    let days_until_flush = flush_start_day
        .and_then(|start| start.checked_sub(today))
        .filter(|remaining| *remaining > 0);

    Some(ThisWeek {
        phase: today_day.phase,
        week: today_day.week,
        total_weeks,
        doses,
        flush_start_day,
        days_until_flush,
    })
}

/// A substrate profile (spec §5.3, §4.8): pH, EC bands, watering
/// style, flush length — all sourced, editable ranges.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubstrateProfile {
    /// Label: `soil`, `coco`, `hydro`, or a user-defined name.
    pub label: String,
    /// Typical pH minimum.
    pub ph_min: f64,
    /// Typical pH maximum.
    pub ph_max: f64,
    /// Watering style summary (for the coach).
    pub watering_style: String,
    /// Pre-harvest flush minimum days.
    pub flush_days_min: u16,
    /// Pre-harvest flush maximum days.
    pub flush_days_max: u16,
    /// Runoff check frequency.
    pub runoff_check: String,
    /// Seed EC bands per phase (µS/cm, from spec §5.3; contested —
    /// the user should default to the lower half).
    pub ec_bands: EcBands,
    /// Source citation.
    pub source: String,
}

/// EC bands per feed phase (µS/cm), each (min, max).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct EcBands {
    /// Seedling band.
    pub seedling: (u32, u32),
    /// Early veg band.
    pub early_veg: (u32, u32),
    /// Late veg band.
    pub late_veg: (u32, u32),
    /// Early flower band.
    pub early_flower: (u32, u32),
    /// Peak flower band.
    pub peak_flower: (u32, u32),
    /// Late flower band.
    pub late_flower: (u32, u32),
    /// Flush band (near zero).
    pub flush: (u32, u32),
}

/// TDS meter scales (spec §5.3): the same solution reads different
/// ppm on different scales. EC (µS/cm) is canonical; TDS is derived.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TdsScale {
    /// `NaCl` / "500" scale: `ppm = EC × 0.5`.
    Half,
    /// "442" scale: `ppm = EC × 0.64`.
    Scale442,
    /// "700" scale: `ppm = EC × 0.7`.
    Seven,
}

impl TdsScale {
    /// Converts EC (µS/cm) to TDS (ppm) on this scale.
    #[must_use]
    pub fn ec_to_ppm(self, ec_us: u32) -> u32 {
        let factor = match self {
            Self::Half => 0.5,
            Self::Scale442 => 0.64,
            Self::Seven => 0.7,
        };
        (f64::from(ec_us) * factor).round() as u32
    }

    /// Converts TDS (ppm) back to EC (µS/cm) on this scale.
    #[must_use]
    pub fn ppm_to_ec(self, ppm: u32) -> u32 {
        let factor = match self {
            Self::Half => 0.5,
            Self::Scale442 => 0.64,
            Self::Seven => 0.7,
        };
        (f64::from(ppm) / factor).round() as u32
    }

    /// The display label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Half => "0.5 (NaCl / 500)",
            Self::Scale442 => "0.64 (442)",
            Self::Seven => "0.7 (700)",
        }
    }
}

/// The built-in substrate presets (spec §5.3, sourced ranges).
#[must_use]
pub fn substrate_presets() -> Vec<SubstrateProfile> {
    vec![
        SubstrateProfile {
            label: "soil".to_owned(),
            ph_min: 6.2,
            ph_max: 6.8,
            watering_style: "wet–dry cycle (water when top 1–2 in is dry)"
                .to_owned(),
            flush_days_min: 7,
            flush_days_max: 10,
            runoff_check: "occasional".to_owned(),
            ec_bands: EcBands {
                seedling: (100, 200),
                early_veg: (300, 500),
                late_veg: (600, 900),
                early_flower: (800, 1100),
                peak_flower: (1000, 1400),
                late_flower: (700, 1000),
                flush: (0, 50),
            },
            source: "spec §5.3 seed table (one 500-scale source; contested — sources give only ~600–800 ppm for soil flowering)".to_owned(),
        },
        SubstrateProfile {
            label: "coco".to_owned(),
            ph_min: 5.8,
            ph_max: 6.2,
            watering_style: "keep ~90–100% saturated, fertigate ≥1×/day, 10–20% runoff every time".to_owned(),
            flush_days_min: 5,
            flush_days_max: 7,
            runoff_check: "every feed (compare in vs out)".to_owned(),
            ec_bands: EcBands {
                seedling: (250, 350),
                early_veg: (500, 700),
                late_veg: (800, 1100),
                early_flower: (1000, 1300),
                peak_flower: (1200, 1600),
                late_flower: (900, 1100),
                flush: (0, 50),
            },
            source: "spec §5.3 seed table (one 500-scale source; contested)".to_owned(),
        },
        SubstrateProfile {
            label: "hydro".to_owned(),
            ph_min: 5.5,
            ph_max: 6.0,
            watering_style: "reservoir management, log pH/EC trend".to_owned(),
            flush_days_min: 3,
            flush_days_max: 5,
            runoff_check: "n/a (reservoir)".to_owned(),
            ec_bands: EcBands {
                seedling: (200, 300),
                early_veg: (400, 600),
                late_veg: (700, 1000),
                early_flower: (900, 1200),
                peak_flower: (1100, 1500),
                late_flower: (800, 1000),
                flush: (0, 50),
            },
            source: "spec §5.3 seed table (one 500-scale source; contested)".to_owned(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::{
        ActualTimeline, DecimalStyle, Dose, DoseUnit, FeedPlanDay, FeedSchedule, FittedDose,
        FittedPhase, ProductDoses, ScheduleProduct, ScheduleValidationError, fit_schedule,
        format_dose, parse_dose_text, this_week,
    };

    /// The BioBizz-style fixture: one feeds product with per-week
    /// doses (2,2,3,4 for WK 1–4), one constant biology product, and
    /// no water dose.
    fn two_product_schedule(flower_weeks: u8) -> FeedSchedule {
        let doses = |cells: &[Option<u32>]| -> ProductDoses {
            ProductDoses {
                propagation: None,
                veg: None,
                flower: cells.iter().map(|cell| cell.map(Dose::symmetric)).collect(),
                water: None,
            }
        };
        FeedSchedule {
            id: "sched-1".to_owned(),
            name: "Test chart".to_owned(),
            source: Some("unit-test fixture".to_owned()),
            flower_weeks,
            products: vec![
                ScheduleProduct {
                    id: "prod-feed".to_owned(),
                    name: "BIO·BLOOM".to_owned(),
                    unit: DoseUnit::Milliliters,
                    group: "Feeds".to_owned(),
                    note: None,
                    doses: doses(&[Some(2), Some(2), Some(3), Some(4)]),
                },
                ScheduleProduct {
                    id: "prod-bio".to_owned(),
                    name: "MICROBES".to_owned(),
                    unit: DoseUnit::Grams,
                    group: "Rooting & biology".to_owned(),
                    note: Some("once a week".to_owned()),
                    doses: ProductDoses {
                        propagation: Some(Dose::symmetric(400)),
                        veg: Some(Dose::symmetric(400)),
                        flower: (0..usize::from(flower_weeks))
                            .map(|week| {
                                Some(if week % 4 == 3 {
                                    Dose::symmetric(200)
                                } else {
                                    Dose::symmetric(400)
                                })
                            })
                            .collect(),
                        water: None,
                    },
                },
            ],
            created_at: "2026-01-01T00:00:00Z".to_owned(),
            updated_at: "2026-01-01T00:00:00Z".to_owned(),
        }
    }

    /// An actual timeline of `flower_days` real flowering days.
    fn timeline(seedling: u16, veg: u16, flower: u16, flush: u16) -> ActualTimeline {
        ActualTimeline {
            seedling_days: seedling,
            veg_days: veg,
            flower_days: flower,
            flush_days: flush,
        }
    }

    impl Dose {
        /// A constant dose (`min == max`).
        fn symmetric(si: u32) -> Self {
            Self { min: si, max: si }
        }
    }

    #[test]
    fn flower_weeks_warp_onto_real_days() {
        // An 8-week chart over 56 real flower days: exactly 8 weeks,
        // 7 days each, in order.
        let schedule = two_product_schedule(8);
        let result = fit_schedule(&schedule, &timeline(7, 21, 56, 7));
        let flower: Vec<&FeedPlanDay> = result
            .days
            .iter()
            .filter(|day| day.phase == FittedPhase::Flower)
            .collect();
        assert_eq!(flower.len(), 56);
        let weeks: Vec<u8> = flower.iter().filter_map(|day| day.week).collect();
        assert_eq!(weeks.first().copied(), Some(1));
        assert_eq!(weeks.last().copied(), Some(8));
        for pair in weeks.windows(2) {
            assert!(
                pair[1] == pair[0] || pair[1] == pair[0] + 1,
                "weeks must advance one at a time, got {weeks:?}"
            );
        }
        for (index, week) in weeks.iter().enumerate() {
            assert_eq!(
                usize::from(*week),
                index * 8 / 56 + 1,
                "day {index} maps to the wrong chart week"
            );
        }
    }

    #[test]
    fn compressed_and_stretched_flowers_both_cover_every_week() {
        // A 9-week chart squeezed into 42 days and stretched across
        // 77: every chart week lands at least once.
        let mut schedule = two_product_schedule(9);
        schedule.products[0].doses.flower = (0_u32..9)
            .map(|week| Some(Dose::symmetric(week + 1)))
            .collect();
        for flower_days in [42_u16, 49, 56, 63, 70, 77, 84] {
            let result = fit_schedule(&schedule, &timeline(7, 14, flower_days, 5));
            let seen: Vec<u8> = result.days.iter().filter_map(|day| day.week).collect();
            for week in 1..=9 {
                assert!(
                    seen.contains(&week),
                    "chart week {week} dropped at {flower_days} days"
                );
            }
        }
    }

    #[test]
    fn water_tail_never_dropped_and_always_pinned_to_the_end() {
        for flower_days in [14_u16, 40, 77] {
            let schedule = two_product_schedule(4);
            let result = fit_schedule(&schedule, &timeline(7, 14, flower_days, 10));
            assert_eq!(
                result
                    .days
                    .iter()
                    .filter(|day| day.phase == FittedPhase::Water)
                    .count(),
                10,
                "flush days dropped at {flower_days}"
            );
            let last = result.days.last().expect("days exist");
            assert_eq!(last.phase, FittedPhase::Water);
            assert!(last.is_protected());
            assert_eq!(
                last.grow_day,
                7 + 14 + flower_days + 9,
                "the tail ends the plan"
            );
        }
    }

    #[test]
    fn veg_dose_repeats_instead_of_stretching() {
        let schedule = two_product_schedule(4);
        let result = fit_schedule(&schedule, &timeline(3, 42, 49, 7));
        let veg: Vec<&FeedPlanDay> = result
            .days
            .iter()
            .filter(|day| day.phase == FittedPhase::Veg)
            .collect();
        assert_eq!(veg.len(), 42);
        // MICROBES carries the veg dose on every veg day.
        for day in veg {
            assert!(
                day.doses
                    .iter()
                    .any(|dose| dose.name == "MICROBES" && dose.dose.max == 400),
                "the veg dose must repeat on every veg day"
            );
        }
    }

    #[test]
    fn degenerate_timeline_produces_only_the_segments_that_exist() {
        let schedule = two_product_schedule(4);
        let result = fit_schedule(&schedule, &timeline(0, 0, 0, 0));
        assert_eq!(result.days, [] as [FeedPlanDay; 0]);
        assert_eq!(result.actual_total_days, 0);

        let flush_only = fit_schedule(&schedule, &timeline(0, 0, 0, 5));
        assert_eq!(flush_only.days.len(), 5);
        assert!(
            flush_only
                .days
                .iter()
                .all(|day| day.phase == FittedPhase::Water)
        );
    }

    #[test]
    fn doses_follow_the_chart_cells() {
        // BIO·BLOOM: 2 ml/L in WK 1, 4 ml/L in WK 4 — the 4-week
        // chart compressed to 16 days gives each week 4 days: WK 1 is
        // days 0–3 and WK 4 days 12–15 of the flower segment.
        let schedule = two_product_schedule(4);
        let result = fit_schedule(&schedule, &timeline(0, 0, 16, 0));
        let week1 = &result.days[0];
        let week4 = &result.days[12];
        let dose_of = |day: &FeedPlanDay, name: &str| {
            day.doses
                .iter()
                .find(|dose| dose.name == name)
                .map(|dose| dose.dose.max)
        };
        assert_eq!(week1.week, Some(1));
        assert_eq!(dose_of(week1, "BIO·BLOOM"), Some(2));
        assert_eq!(week4.week, Some(4));
        assert_eq!(dose_of(week4, "BIO·BLOOM"), Some(4));
        // MICROBES: 0.4 g/L weekly except every fourth WK (0.2).
        assert_eq!(dose_of(&result.days[3], "MICROBES"), Some(400));
        assert_eq!(dose_of(&result.days[4], "MICROBES"), Some(400));
        assert_eq!(dose_of(&result.days[12], "MICROBES"), Some(200));
    }

    #[test]
    fn this_week_reports_position_doses_and_flush_countdown() {
        let schedule = two_product_schedule(4);
        let plan = fit_schedule(&schedule, &timeline(7, 21, 28, 7));
        // Grow day 30 is flower day 2 — chart WK 1 of 4; the flush
        // starts at day 56, 26 days out.
        let week = this_week(&plan.days, 30).expect("day 30 is on the plan");
        assert_eq!(week.phase, FittedPhase::Flower);
        assert_eq!(week.week, Some(1));
        assert_eq!(week.total_weeks, Some(4));
        let feed_dose = week
            .doses
            .iter()
            .find(|dose| dose.name == "BIO·BLOOM")
            .expect("the week's feed dose");
        assert_eq!(feed_dose.dose.max, 2);
        assert_eq!(week.flush_start_day, Some(56));
        assert_eq!(week.days_until_flush, Some(26));

        // Inside the flush: today is water, no countdown left.
        let flush_week = this_week(&plan.days, 58).expect("day 58 is on the plan");
        assert_eq!(flush_week.phase, FittedPhase::Water);
        assert_eq!(flush_week.days_until_flush, None);

        // Off-plan days read as None.
        assert_eq!(this_week(&plan.days, 999), None);
        assert_eq!(this_week(&[], 0), None);
    }

    #[test]
    fn dose_text_parses_ranges_and_locales() {
        let ml = DoseUnit::Milliliters;
        assert_eq!(
            parse_dose_text("0,25–2", ml),
            Some(Dose::new(250, 2000)),
            "comma decimals and en dash"
        );
        assert_eq!(parse_dose_text("0.25-2", ml), Some(Dose::new(250, 2000)));
        assert_eq!(parse_dose_text("2", ml), Some(Dose::new(2000, 2000)));
        assert_eq!(
            parse_dose_text("2", DoseUnit::Grams),
            Some(Dose::new(2000, 2000))
        );
        assert_eq!(parse_dose_text("—", ml), None, "the dash cell");
        assert_eq!(parse_dose_text("", ml), None);
        assert_eq!(parse_dose_text("n/a", ml), None);
        assert_eq!(parse_dose_text("-2", ml), None);
        assert_eq!(
            parse_dose_text("3–1", ml),
            Some(Dose::new(1000, 3000)),
            "swapped"
        );
    }

    #[test]
    fn dose_formatting_trims_zeros_and_localizes() {
        let comma = DecimalStyle::Comma;
        let dot = DecimalStyle::Dot;
        assert_eq!(format_dose(Dose::new(250, 2000), comma), "0,25–2");
        assert_eq!(format_dose(Dose::new(250, 2000), dot), "0.25–2");
        assert_eq!(format_dose(Dose::new(2000, 2000), comma), "2");
        assert_eq!(format_dose(Dose::new(1500, 1500), dot), "1.5");
        assert_eq!(format_dose(Dose::new(0, 0), comma), "0");
    }

    #[test]
    fn schedule_validation_and_normalization() {
        let mut schedule = two_product_schedule(8);
        assert_eq!(schedule.validate(), Ok(()));

        schedule.name = "  ".to_owned();
        assert_eq!(schedule.validate(), Err(ScheduleValidationError::Name));
        schedule.name = "Renamed".to_owned();

        schedule.flower_weeks = 0;
        assert_eq!(
            schedule.validate(),
            Err(ScheduleValidationError::FlowerWeeks)
        );
        schedule.flower_weeks = FeedSchedule::MAX_FLOWER_WEEKS + 1;
        assert_eq!(
            schedule.validate(),
            Err(ScheduleValidationError::FlowerWeeks)
        );
        schedule.flower_weeks = 8;

        schedule.products.push(ScheduleProduct {
            id: "prod-blank".to_owned(),
            name: "   ".to_owned(),
            unit: DoseUnit::Milliliters,
            group: String::new(),
            note: None,
            doses: ProductDoses::empty(),
        });
        assert_eq!(
            schedule.validate(),
            Err(ScheduleValidationError::ProductName)
        );

        // Normalization drops the blank row, pads the cells to the
        // week count and trims the texts.
        let normalized = schedule.normalized();
        assert_eq!(normalized.validate(), Ok(()));
        assert_eq!(normalized.products.len(), 2);
        assert!(
            normalized
                .products
                .iter()
                .all(|product| product.doses.flower.len() == 8)
        );
        assert_eq!(normalized.name, "Renamed");

        assert_eq!(
            normalized.groups(),
            vec!["Feeds".to_owned(), "Rooting & biology".to_owned()]
        );
        assert_eq!(
            normalized.units(),
            vec![DoseUnit::Milliliters, DoseUnit::Grams]
        );
    }

    /// Compile-time check that plans round-trip through the JSON form
    /// the database stores (doses column, day rows).
    #[test]
    fn plan_and_schedule_serial_round_trip() {
        let schedule = two_product_schedule(4);
        let result = fit_schedule(&schedule, &timeline(7, 21, 28, 7));
        let plan = FeedPlanJsonFixture::from_days(&result);
        let json = serde_json::to_string(&plan.days).expect("days serialize");
        let back: Vec<FeedPlanDay> = serde_json::from_str(&json).expect("days parse");
        assert_eq!(back, plan.days);
        let schedule_json = serde_json::to_string(&schedule).expect("schedule serializes");
        let schedule_back: FeedSchedule =
            serde_json::from_str(&schedule_json).expect("schedule parses");
        assert_eq!(schedule_back, schedule);
    }

    /// Test-local alias so the round-trip test reads without reaching
    /// into store types.
    struct FeedPlanJsonFixture;
    impl FeedPlanJsonFixture {
        fn from_days(result: &super::FitResult) -> super::FitResult {
            result.clone()
        }
    }

    /// A dose line snapshot must not change shape silently — the
    /// frozen-plan contract depends on it.
    #[test]
    fn fitted_dose_snapshot_round_trips() {
        let dose = FittedDose {
            name: "BIO·GROW".to_owned(),
            unit: DoseUnit::Milliliters,
            group: "Feeds".to_owned(),
            note: Some("switch from Fish·Mix to flower".to_owned()),
            dose: Dose::new(2000, 4000),
        };
        let json = serde_json::to_string(&dose).expect("serializes");
        let back: FittedDose = serde_json::from_str(&json).expect("parses");
        assert_eq!(back, dose);
    }
}
#[test]
fn substrate_presets_differ_in_ph_ec_flush() {
    let presets = substrate_presets();
    assert!(presets.len() >= 3);
    let soil = presets.iter().find(|p| p.label == "soil").unwrap();
    let coco = presets.iter().find(|p| p.label == "coco").unwrap();
    let hydro = presets.iter().find(|p| p.label == "hydro").unwrap();

    // pH targets differ.
    assert!(
        soil.ph_min > coco.ph_min,
        "soil pH should be higher than coco"
    );
    assert!(
        coco.ph_min > hydro.ph_min,
        "coco pH should be higher than hydro"
    );

    // Flush lengths differ.
    assert!(soil.flush_days_min > coco.flush_days_min);
    assert!(coco.flush_days_min > hydro.flush_days_min);

    // EC bands differ.
    assert_ne!(soil.ec_bands.peak_flower, coco.ec_bands.peak_flower);
    assert_ne!(coco.ec_bands.peak_flower, hydro.ec_bands.peak_flower);

    // Watering styles differ.
    assert_ne!(soil.watering_style, coco.watering_style);
}

// --- TDS/EC round-trip (spec §13) ---

#[test]
fn tds_ec_round_trip_all_three_scales() {
    for scale in [TdsScale::Half, TdsScale::Scale442, TdsScale::Seven] {
        for ec in [0_u32, 50, 100, 350, 500, 700, 1000, 1200, 1400, 1600] {
            let ppm = scale.ec_to_ppm(ec);
            let back = scale.ppm_to_ec(ppm);
            // Round-trip within 1 µS/cm (rounding on both ends).
            assert!(
                back.abs_diff(ec) <= 1,
                "scale {scale:?}: ec {ec} → ppm {ppm} → ec {back}"
            );
        }
    }
}

#[test]
fn tds_scales_read_differently() {
    let ec = 1000_u32;
    let p500 = TdsScale::Half.ec_to_ppm(ec);
    let p442 = TdsScale::Scale442.ec_to_ppm(ec);
    let p700 = TdsScale::Seven.ec_to_ppm(ec);
    assert_eq!(p500, 500);
    assert_eq!(p442, 640);
    assert_eq!(p700, 700);
}

/// Compile-time check that `SubstrateProfile` is deserializable
/// (used by the seeding path).
#[test]
fn substrate_profile_serializes() {
    let presets: Vec<SubstrateProfile> = substrate_presets();
    let json = serde_json::to_string(&presets).expect("serializes");
    let back: Vec<SubstrateProfile> = serde_json::from_str(&json).expect("deserializes");
    assert_eq!(presets, back);
}
