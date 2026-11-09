// The whole module converts float interpolation results (EC targets)
// to integer µS/cm — the precision-limited casts are inherent.
#![allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]

//! Feed-fit engine (spec §4.6, milestone M6).
//!
//! A piecewise time-warp that stretches or shrinks feed template rows
//! onto the plant's actual phase timeline. Flower length is
//! genetics-bound and veg length is the grower's choice — never
//! uniformly scaled. Ripen/flush rows are always preserved, and EC
//! steps between consecutive days are clamped to a configurable
//! maximum.

use serde::{Deserialize, Serialize};

/// The template phase segment a row belongs to (spec §4.6).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedPhase {
    /// Seedling rows.
    Seedling,
    /// Early vegetative rows.
    EarlyVeg,
    /// Late vegetative rows.
    LateVeg,
    /// Early flowering rows.
    EarlyFlower,
    /// Mid flowering rows.
    MidFlower,
    /// Late flowering rows.
    LateFlower,
    /// Ripen (last feed before flush) — never dropped.
    Ripen,
    /// Flush — never dropped.
    Flush,
}

impl FeedPhase {
    /// The stable `snake_case` label.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Seedling => "seedling",
            Self::EarlyVeg => "early_veg",
            Self::LateVeg => "late_veg",
            Self::EarlyFlower => "early_flower",
            Self::MidFlower => "mid_flower",
            Self::LateFlower => "late_flower",
            Self::Ripen => "ripen",
            Self::Flush => "flush",
        }
    }

    /// All phases in template order.
    #[must_use]
    pub const fn all() -> [Self; 8] {
        [
            Self::Seedling,
            Self::EarlyVeg,
            Self::LateVeg,
            Self::EarlyFlower,
            Self::MidFlower,
            Self::LateFlower,
            Self::Ripen,
            Self::Flush,
        ]
    }

    /// True for the never-dropped tail phases.
    #[must_use]
    pub const fn is_protected(self) -> bool {
        matches!(self, Self::Ripen | Self::Flush)
    }
}

/// One row of a feed template (week- or fraction-of-phase based).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FeedRow {
    /// The template phase this row belongs to.
    pub phase: FeedPhase,
    /// Template-relative position (0-based, in days from the template
    /// start of this phase).
    pub day_offset: u16,
    /// Target EC, **µS/cm** (spec §5.3: EC is canonical).
    pub ec_target_us: u32,
    /// pH range (min, max).
    pub ph_min: f64,
    /// pH range max.
    pub ph_max: f64,
}

/// The plant's actual phase timeline (days, 0-based from grow start).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActualTimeline {
    /// Days the seedling phase lasts.
    pub seedling_days: u16,
    /// Days the veg phase lasts (grower's choice).
    pub veg_days: u16,
    /// Days the flower phase lasts (strain).
    pub flower_days: u16,
    /// Days the flush lasts (substrate-dependent).
    pub flush_days: u16,
}

/// The template's own phase lengths (what the rows assume).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemplateTimeline {
    /// Days the seedling rows span.
    pub seedling_days: u16,
    /// Days the veg rows span.
    pub veg_days: u16,
    /// Days the flower rows span (ripen+flush excluded).
    pub flower_days: u16,
    /// Days the ripen+flush rows span.
    pub ripen_flush_days: u16,
}

/// Fit configuration clamps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct FitConfig {
    /// Maximum EC step (µS/cm) between consecutive fitted days.
    pub max_ec_step_us: u32,
}

impl Default for FitConfig {
    fn default() -> Self {
        Self {
            max_ec_step_us: 200,
        }
    }
}

/// One fitted day in the result.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct FeedPlanDay {
    /// Grow day (0-based from grow start).
    pub grow_day: u16,
    /// The template row this day was derived from.
    pub phase: FeedPhase,
    /// Interpolated EC target, µS/cm.
    pub ec_target_us: u32,
    /// pH min carried from the source row.
    pub ph_min: f64,
    /// pH max carried from the source row.
    pub ph_max: f64,
    /// True when this day came from a protected (ripen/flush) row.
    pub protected: bool,
}

/// The fit result: fitted days plus the before/after summary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FitResult {
    /// The fitted days, ordered by grow day.
    pub days: Vec<FeedPlanDay>,
    /// The total template days before fitting.
    pub template_total_days: u16,
    /// The total actual days after fitting.
    pub actual_total_days: u16,
}

/// Fits a feed template onto the actual timeline (spec §4.6).
///
/// Piecewise per phase: seedling rows stretch/shrink onto the actual
/// seedling days, veg rows onto the actual veg days, flower rows onto
/// the actual flower days. Ripen and flush rows are pinned to the end
/// and **never dropped or interpolated away**. EC values interpolate
/// linearly within each phase segment; consecutive-day EC steps are
/// clamped to [`FitConfig::max_ec_step_us`].
#[must_use]
pub fn fit_feed(
    rows: &[FeedRow],
    template: TemplateTimeline,
    actual: ActualTimeline,
    config: FitConfig,
) -> FitResult {
    let template_total = template.seedling_days
        + template.veg_days
        + template.flower_days
        + template.ripen_flush_days;
    let actual_total =
        actual.seedling_days + actual.veg_days + actual.flower_days + actual.flush_days;

    let mut days: Vec<FeedPlanDay> = Vec::with_capacity(usize::from(actual_total));

    // --- Piecewise mapping: (template segment) → (actual segment) ---
    let segments = [
        (
            FeedPhase::Seedling,
            template.seedling_days,
            actual.seedling_days,
            0_u16,
        ),
        (
            FeedPhase::EarlyVeg,
            template.veg_days,
            actual.veg_days,
            actual.seedling_days,
        ),
        (
            FeedPhase::EarlyFlower,
            template.flower_days,
            actual.flower_days,
            actual.seedling_days + actual.veg_days,
        ),
    ];

    for (segment_phase, template_len, actual_len, actual_offset) in segments {
        if template_len == 0 || actual_len == 0 {
            continue;
        }
        let segment_rows: Vec<&FeedRow> = rows
            .iter()
            .filter(|row| same_segment(row.phase, segment_phase))
            .collect();
        if segment_rows.is_empty() {
            continue;
        }
        // For each actual day in the segment, find the interpolated
        // template row (template-day = actual-day × template_len/actual_len).
        let scale = f64::from(template_len) / f64::from(actual_len);
        for day in 0..actual_len {
            let template_day = f64::from(day) * scale;
            let (ec, ph_min, ph_max, source_phase) =
                interpolate_row(&segment_rows, template_day, template_len);
            days.push(FeedPlanDay {
                grow_day: actual_offset + day,
                phase: source_phase,
                ec_target_us: ec,
                ph_min,
                ph_max,
                protected: false,
            });
        }
    }

    // --- Ripen + flush: pinned to the end, never dropped ---
    // Flush occupies the last `flush_days` days; `flush_days` is part of
    // `actual_total`, so this subtraction never underflows. The ripen
    // marker takes the day just before the flush and replaces any
    // interpolated day there — the store enforces UNIQUE
    // (grow_id, grow_day), and ripen, the last feed before flush, is
    // the value that day must carry.
    let flush_start = actual_total - actual.flush_days;
    let ripen_rows: Vec<&FeedRow> = rows
        .iter()
        .filter(|row| row.phase == FeedPhase::Ripen)
        .collect();
    let flush_rows: Vec<&FeedRow> = rows
        .iter()
        .filter(|row| row.phase == FeedPhase::Flush)
        .collect();

    // One ripen day at the start of the tail (if the template has one
    // and a day exists before the flush).
    if let (Some(ripen), Some(ripen_day)) = (ripen_rows.first(), flush_start.checked_sub(1)) {
        // Segment grow-day ranges are disjoint, so this drops at most
        // the one interpolated day colliding with the ripen slot.
        days.retain(|day| day.grow_day != ripen_day);
        days.push(FeedPlanDay {
            grow_day: ripen_day,
            phase: FeedPhase::Ripen,
            ec_target_us: ripen.ec_target_us,
            ph_min: ripen.ph_min,
            ph_max: ripen.ph_max,
            protected: true,
        });
    }
    // Flush fills the remaining tail days.
    if let Some(flush) = flush_rows.first() {
        for day in 0..actual.flush_days {
            days.push(FeedPlanDay {
                grow_day: flush_start + day,
                phase: FeedPhase::Flush,
                ec_target_us: flush.ec_target_us,
                ph_min: flush.ph_min,
                ph_max: flush.ph_max,
                protected: true,
            });
        }
    }

    // --- Sort by grow day, then apply EC step clamps forward ---
    days.sort_by_key(|day| day.grow_day);
    clamp_ec_steps(&mut days, config.max_ec_step_us);

    FitResult {
        days,
        template_total_days: template_total,
        actual_total_days: actual_total,
    }
}

/// Maps a fine-grained row phase into its coarse segment.
fn same_segment(row_phase: FeedPhase, segment: FeedPhase) -> bool {
    match segment {
        FeedPhase::Seedling => row_phase == FeedPhase::Seedling,
        FeedPhase::EarlyVeg => {
            matches!(row_phase, FeedPhase::EarlyVeg | FeedPhase::LateVeg)
        }
        FeedPhase::EarlyFlower => matches!(
            row_phase,
            FeedPhase::EarlyFlower | FeedPhase::MidFlower | FeedPhase::LateFlower
        ),
        _ => false,
    }
}

/// Interpolates EC linearly between the rows bracketing `template_day`.
fn interpolate_row(
    rows: &[&FeedRow],
    template_day: f64,
    segment_len: u16,
) -> (u32, f64, f64, FeedPhase) {
    let rows = {
        let mut sorted = rows.to_vec();
        sorted.sort_by_key(|row| row.day_offset);
        sorted
    };
    if rows.is_empty() {
        return (0, 6.0, 6.5, FeedPhase::Seedling);
    }
    // Clamp to segment bounds.
    let day = template_day.clamp(0.0, f64::from(segment_len.saturating_sub(1)));
    // Find the bracketing pair.
    let (before, after) = {
        let mut pair = (rows[0], rows[0]);
        for (index, row) in rows.iter().enumerate() {
            if f64::from(row.day_offset) <= day {
                pair.0 = *row;
                pair.1 = rows.get(index + 1).copied().unwrap_or(*row);
            }
        }
        pair
    };
    let span = f64::from(after.day_offset.saturating_sub(before.day_offset));
    if span <= 0.0 || after.day_offset == before.day_offset {
        return (
            before.ec_target_us,
            before.ph_min,
            before.ph_max,
            before.phase,
        );
    }
    let t = (day - f64::from(before.day_offset)) / span;
    let ec_base = f64::from(before.ec_target_us);
    let ec_delta = f64::from(after.ec_target_us) - ec_base;
    let ec = t.mul_add(ec_delta, ec_base);
    (
        ec.max(0.0).round() as u32,
        before.ph_min,
        before.ph_max,
        before.phase,
    )
}

/// Clamps EC steps between consecutive days (protected days are
/// exempt: they carry the template's explicit values).
fn clamp_ec_steps(days: &mut [FeedPlanDay], max_step: u32) {
    for index in 1..days.len() {
        if days[index].protected || days[index - 1].protected {
            continue;
        }
        let previous = days[index - 1].ec_target_us;
        let current = days[index].ec_target_us;
        let step = current.abs_diff(previous);
        if step > max_step {
            let direction = if current > previous { 1 } else { -1 };
            let clamped = i64::from(previous) + i64::from(max_step) * i64::from(direction);
            days[index].ec_target_us = u32::try_from(clamped.max(0)).unwrap_or(0);
        }
    }
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
        ActualTimeline, FeedPhase, FeedRow, FitConfig, SubstrateProfile, TdsScale,
        TemplateTimeline, fit_feed, substrate_presets,
    };

    /// A 9-week-flower template (63 days flower) with EC ramping
    /// 500→1400 µS/cm and a ripen+flush tail.
    fn nine_week_template() -> (Vec<FeedRow>, TemplateTimeline) {
        let rows = vec![
            FeedRow {
                phase: FeedPhase::Seedling,
                day_offset: 0,
                ec_target_us: 150,
                ph_min: 6.0,
                ph_max: 6.5,
            },
            FeedRow {
                phase: FeedPhase::EarlyVeg,
                day_offset: 0,
                ec_target_us: 400,
                ph_min: 6.0,
                ph_max: 6.5,
            },
            FeedRow {
                phase: FeedPhase::LateVeg,
                day_offset: 14,
                ec_target_us: 700,
                ph_min: 6.0,
                ph_max: 6.5,
            },
            FeedRow {
                phase: FeedPhase::EarlyFlower,
                day_offset: 0,
                ec_target_us: 900,
                ph_min: 6.0,
                ph_max: 6.3,
            },
            FeedRow {
                phase: FeedPhase::MidFlower,
                day_offset: 21,
                ec_target_us: 1200,
                ph_min: 6.0,
                ph_max: 6.3,
            },
            FeedRow {
                phase: FeedPhase::LateFlower,
                day_offset: 42,
                ec_target_us: 1000,
                ph_min: 6.0,
                ph_max: 6.3,
            },
            FeedRow {
                phase: FeedPhase::Ripen,
                day_offset: 0,
                ec_target_us: 400,
                ph_min: 6.0,
                ph_max: 6.3,
            },
            FeedRow {
                phase: FeedPhase::Flush,
                day_offset: 0,
                ec_target_us: 0,
                ph_min: 6.0,
                ph_max: 6.3,
            },
        ];
        let timeline = TemplateTimeline {
            seedling_days: 14,
            veg_days: 28,
            flower_days: 56,     // 8 weeks of ramp + 1 wk late = 9 wk total
            ripen_flush_days: 7, // ripen 1d + flush 6d
        };
        (rows, timeline)
    }

    #[test]
    fn nine_week_fits_eight_weeks() {
        let (rows, template) = nine_week_template();
        let actual = ActualTimeline {
            seedling_days: 14,
            veg_days: 28,
            flower_days: 56, // 8 weeks
            flush_days: 6,
        };
        let result = fit_feed(&rows, template, actual, FitConfig::default());

        // Ripen and flush are preserved.
        assert!(
            result
                .days
                .iter()
                .any(|d| d.phase == FeedPhase::Ripen && d.protected)
        );
        assert!(
            result
                .days
                .iter()
                .filter(|d| d.phase == FeedPhase::Flush && d.protected)
                .count()
                >= 5
        );

        // EC steps between non-protected consecutive days stay within
        // the clamp.
        let max_step = FitConfig::default().max_ec_step_us;
        for pair in result.days.windows(2) {
            if pair[0].protected || pair[1].protected {
                continue;
            }
            let step = pair[1].ec_target_us.abs_diff(pair[0].ec_target_us);
            assert!(
                step <= max_step,
                "EC step {} exceeds clamp {} at day {}",
                step,
                max_step,
                pair[1].grow_day
            );
        }

        // The timeline is shorter than the template.
        assert!(result.actual_total_days < result.template_total_days);
    }

    #[test]
    fn nine_week_fits_eleven_weeks() {
        let (rows, template) = nine_week_template();
        let actual = ActualTimeline {
            seedling_days: 14,
            veg_days: 28,
            flower_days: 77, // 11 weeks
            flush_days: 7,
        };
        let result = fit_feed(&rows, template, actual, FitConfig::default());

        assert!(result.actual_total_days > result.template_total_days);
        assert!(
            result
                .days
                .iter()
                .any(|d| d.phase == FeedPhase::Ripen && d.protected)
        );
        // Flower days filled the stretched window.
        let flower_days = result
            .days
            .iter()
            .filter(|d| {
                !d.protected
                    && d.grow_day >= actual.seedling_days + actual.veg_days
                    && d.grow_day < actual.seedling_days + actual.veg_days + actual.flower_days
            })
            .count();
        assert!(
            flower_days >= 70,
            "stretched flower should have ~77 days, got {flower_days}"
        );
    }

    #[test]
    fn ripen_flush_never_dropped_regardless_of_compression() {
        let (rows, template) = nine_week_template();
        for flower_days in [42_u16, 49, 56, 63, 70, 77, 84] {
            let actual = ActualTimeline {
                seedling_days: 7,
                veg_days: 14,
                flower_days,
                flush_days: 3,
            };
            let result = fit_feed(&rows, template, actual, FitConfig::default());
            let ripen = result
                .days
                .iter()
                .filter(|d| d.phase == FeedPhase::Ripen)
                .count();
            let flush = result
                .days
                .iter()
                .filter(|d| d.phase == FeedPhase::Flush)
                .count();
            assert!(ripen >= 1, "ripen dropped at flower={flower_days}");
            assert!(flush >= 3, "flush dropped at flower={flower_days}");
        }
    }

    #[test]
    fn ec_steps_within_clamps_across_stretches() {
        let (rows, template) = nine_week_template();
        for flower in [42_u16, 56, 63, 77] {
            let actual = ActualTimeline {
                seedling_days: 14,
                veg_days: 28,
                flower_days: flower,
                flush_days: 7,
            };
            let result = fit_feed(&rows, template, actual, FitConfig::default());
            let clamp = FitConfig::default().max_ec_step_us;
            for pair in result.days.windows(2) {
                if pair[0].protected || pair[1].protected {
                    continue;
                }
                assert!(
                    pair[1].ec_target_us.abs_diff(pair[0].ec_target_us) <= clamp,
                    "step exceeds clamp at flower={flower}, day={}",
                    pair[1].grow_day
                );
            }
        }
    }

    // --- Substrate profiles (spec §13: substrate switch changes recommendations) ---

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

    /// Every fitted day must occupy a distinct grow day: the store has
    /// `UNIQUE (grow_id, grow_day)`, so a duplicate day (the ripen
    /// marker colliding with the last interpolated segment day) fails
    /// the whole fit insert.
    #[test]
    fn fitted_days_never_share_a_grow_day() {
        for actual in [
            ActualTimeline {
                seedling_days: 7,
                veg_days: 21,
                flower_days: 63,
                flush_days: 7,
            },
            ActualTimeline {
                seedling_days: 0,
                veg_days: 21,
                flower_days: 63,
                flush_days: 0,
            },
            ActualTimeline {
                seedling_days: 7,
                veg_days: 0,
                flower_days: 40,
                flush_days: 10,
            },
            ActualTimeline {
                seedling_days: 0,
                veg_days: 0,
                flower_days: 0,
                flush_days: 14,
            },
        ] {
            let (rows, template) = nine_week_template();
            let result = fit_feed(&rows, template, actual, FitConfig::default());
            let mut days: Vec<u16> = result.days.iter().map(|day| day.grow_day).collect();
            let total = days.len();
            days.sort_unstable();
            days.dedup();
            assert_eq!(
                days.len(),
                total,
                "fitted days must be unique for actual timeline {actual:?}"
            );
        }
    }

    /// Degenerate timelines (every segment zero) must not underflow or
    /// panic; the fit simply yields at most the protected tail.
    #[test]
    fn degenerate_timeline_does_not_underflow() {
        let (rows, template) = nine_week_template();
        let result = fit_feed(
            &rows,
            template,
            ActualTimeline {
                seedling_days: 0,
                veg_days: 0,
                flower_days: 0,
                flush_days: 0,
            },
            FitConfig::default(),
        );
        for day in result.days {
            assert!(day.grow_day < result.actual_total_days.max(1));
        }
    }
}
