//! Phase forecast engine (spec §4.5, milestone M5).
//!
//! A pure function from a grow's inputs (start date, strain, planned
//! veg weeks or auto age, and any logged actual events) to a list of
//! phase windows — each phase with an **earliest / likely / latest**
//! start and end. Logged events (flip, transplant, trichome check,
//! harvest) pin their phase boundary to reality; everything after the
//! last actual re-forecasts from it.
//!
//! Seed defaults carry the spec §4.5 ranges; the strain's
//! flower-days and stretch override them when present. Autos skip the
//! veg choice and flower by age.

use serde::{Deserialize, Serialize};

use crate::Phase;
use crate::strain::{PhotoperiodType, Strain};

/// A date (days since the grow start, 0-based), so the engine stays
/// calendar-free and testable.
pub type Day = i64;

/// The default phase durations in days (spec §4.5, sourced ranges).
/// Germination 3–10, seedling 14–21 (2–3 weeks), flower 56–77
/// (8–11 weeks; the strain wins when present), dry 7–14, cure 14–30.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseDefaults {
    /// Germination minimum days.
    pub germination_min: Day,
    /// Germination maximum days.
    pub germination_max: Day,
    /// Seedling minimum days (2 weeks).
    pub seedling_min: Day,
    /// Seedling maximum days (3 weeks).
    pub seedling_max: Day,
    /// Flower minimum days (8 weeks; strain overrides).
    pub flower_min: Day,
    /// Flower maximum days (11 weeks; strain overrides).
    pub flower_max: Day,
    /// Dry minimum days (until stem snap).
    pub dry_min: Day,
    /// Dry maximum days.
    pub dry_max: Day,
    /// Cure minimum days (14, ideally ~30).
    pub cure_min: Day,
    /// Cure maximum days.
    pub cure_max: Day,
}

impl Default for PhaseDefaults {
    fn default() -> Self {
        Self {
            germination_min: 3,
            germination_max: 10,
            seedling_min: 14,
            seedling_max: 21,
            flower_min: 56,
            flower_max: 77,
            dry_min: 7,
            dry_max: 14,
            cure_min: 14,
            cure_max: 30,
        }
    }
}

/// What actually happened (logged events, spec §4.5 re-forecast).
///
/// The boundary fields are pins from recorded phase events (D073): a
/// pin moves its window's start and the chain back-fills around it.
/// The backend resolves "latest event per boundary wins" before
/// building this — the engine stays dumb and pure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Actuals {
    /// Grow day of the flip to 12/12 (photoperiods).
    pub flip_day: Option<Day>,
    /// Grow day of the harvest.
    pub harvest_day: Option<Day>,
    /// Grow day of the last trichome check (nudges harvest).
    pub trichome_check_day: Option<Day>,
    /// True when germination is confirmed logged (else the engine
    /// still reserves the window).
    pub germinated: bool,
    /// Pinned germination start (a "back to Germination" pick).
    pub germination_start: Option<Day>,
    /// Pinned germination → seedling boundary.
    pub seedling_start: Option<Day>,
    /// Pinned seedling → vegetative boundary.
    pub vegetative_start: Option<Day>,
    /// Pinned vegetative → flower boundary (a Flowering pick; also
    /// fills in when a photoperiod has no flip).
    pub flower_start: Option<Day>,
    /// Pinned drying → curing boundary.
    pub curing_start: Option<Day>,
}

/// The inputs to [`forecast`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ForecastInput {
    /// Photoperiod or autoflowering (from the strain).
    pub photoperiod: PhotoperiodType,
    /// Planned vegetative weeks for photoperiods (grower's choice);
    /// ignored for autos. When absent and no flip is logged, the
    /// photoperiod veg window is open-ended and no flower, drying or
    /// curing window is produced.
    pub planned_veg_weeks: Option<u8>,
    /// Autoflower total days from seed (overrides flower timing when
    /// present).
    pub auto_total_days: Option<(Day, Day)>,
    /// Strain flower days (min, max); overrides the defaults.
    pub strain_flower_days: Option<(Day, Day)>,
    /// The sourced default ranges (editable).
    pub defaults: PhaseDefaults,
    /// Logged actual events.
    pub actuals: Actuals,
}

/// One phase window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseWindow {
    /// Which phase.
    pub phase: Phase,
    /// Earliest start (grow day).
    pub earliest_start: Day,
    /// Likely start.
    pub likely_start: Day,
    /// Latest start.
    pub latest_start: Day,
    /// Earliest end.
    pub earliest_end: Day,
    /// Likely end.
    pub likely_end: Day,
    /// Latest end.
    pub latest_end: Day,
    /// True when the start is pinned by a logged event.
    pub actual_start: bool,
}

/// Computes the phase windows for a grow.
///
/// Phases are contiguous and ordered (the property tests check this):
/// germination → seedling → veg → flower → dry → cure. Actual events
/// pin boundaries; the likely path threads through the pinned points
/// and the default ranges after them.
///
/// A photoperiod with neither a logged flip nor planned veg weeks has
/// an open-ended vegetative window: the chain stops there and no
/// flower, drying or curing window is produced.
///
/// # Panics
///
/// Never panics: nonsensical inputs clamp to the nearest sensible
/// window instead.
#[must_use]
pub fn forecast(input: &ForecastInput) -> Vec<PhaseWindow> {
    let d = input.defaults;
    let mut windows: Vec<PhaseWindow> = Vec::with_capacity(6);

    // --- Germination: pinned or 0 .. (likely mid, 3–10 range) ---
    let germ_likely = i64::midpoint(d.germination_min, d.germination_max);
    add(
        &mut windows,
        Phase::Germination,
        input.actuals.germination_start.unwrap_or(0),
        d.germination_min,
        germ_likely,
        d.germination_max,
        true,
        None,
    );

    // --- Seedling: 14–21 ---
    let seedling_pin = input.actuals.seedling_start;
    if let Some(pin) = seedling_pin {
        pin_start(&mut windows, pin);
    }
    let seed_start = seedling_pin.unwrap_or(windows[0].likely_end);
    add(
        &mut windows,
        Phase::Seedling,
        seed_start,
        d.seedling_min,
        i64::midpoint(d.seedling_min, d.seedling_max),
        d.seedling_max,
        seedling_pin.is_some(),
        None,
    );

    // --- Veg (photoperiod: grower weeks; auto: by age) ---
    let Some((flip_day, flower_len)) = add_vegetative(&mut windows, input, d) else {
        // Open-ended veg: nothing past it is projectable.
        normalize(&mut windows);
        return windows;
    };

    let (f_min, f_max) = flower_len;
    let flower_end = flower_end_for(input, flip_day, flower_len);
    let flower_actual_end = input.actuals.harvest_day;
    let flower_window = add(
        &mut windows,
        Phase::Flower,
        flip_day,
        f_min,
        flower_end - flip_day,
        f_max,
        input.actuals.flip_day.is_some() || input.photoperiod == PhotoperiodType::Auto,
        flower_actual_end,
    );

    // --- Dry: after flower end ---
    let dry_window = add(
        &mut windows,
        Phase::Drying,
        flower_window.likely_end,
        d.dry_min,
        i64::midpoint(d.dry_min, d.dry_max),
        d.dry_max,
        input.actuals.harvest_day.is_some(),
        None,
    );

    // --- Cure: after dry ---
    add(
        &mut windows,
        Phase::Curing,
        dry_window.likely_end,
        d.cure_min,
        i64::midpoint(d.cure_min, d.cure_max),
        d.cure_max,
        false,
        None,
    );

    normalize(&mut windows);
    windows
}

/// Forward compression and the final monotonicity pass, shared by the
/// full chain and the open-ended prefix.
fn normalize(windows: &mut [PhaseWindow]) {
    // Forward compression: an early actual (flip or harvest) can land
    // before the chain predicted; the phases before it are historical,
    // so they compress proportionally to fit.
    for index in 0..windows.len() {
        if windows[index].likely_start > windows[index].likely_end {
            // Zero-length-or-inverted: clamp to a point.
            windows[index].likely_end = windows[index].likely_start;
        }
        if index > 0 && windows[index].likely_start < windows[index - 1].likely_end {
            // This phase starts before the previous ends: compress the
            // previous phase to end where this one starts.
            let start = windows[index].likely_start;
            let previous = &mut windows[index - 1];
            if previous.likely_start < start {
                previous.likely_end = start;
            } else {
                // Both must collapse to the same point.
                previous.likely_end = previous.likely_start;
                windows[index].likely_start = previous.likely_start;
            }
        }
    }
    // Final monotonicity pass on the end bands.
    for window in windows {
        window.earliest_end = window
            .earliest_end
            .clamp(window.likely_start, window.likely_end);
        window.latest_end = window.latest_end.max(window.likely_end);
    }
}

/// Builds the forecast input from a strain + grow facts (used by the
/// backend API).
#[must_use]
pub fn input_for(
    strain: Option<&Strain>,
    planned_veg_weeks: Option<u8>,
    actuals: Actuals,
) -> ForecastInput {
    let photoperiod = strain.map_or(PhotoperiodType::Photoperiod, |s| s.photoperiod);
    let strain_flower_days = strain.and_then(|s| match (s.flower_days_min, s.flower_days_max) {
        (Some(min), Some(max)) => Some((Day::from(min), Day::from(max))),
        _ => None,
    });
    let auto_total_days =
        strain.and_then(|s| match (s.auto_total_days_min, s.auto_total_days_max) {
            (Some(min), Some(max)) => Some((Day::from(min), Day::from(max))),
            _ => None,
        });
    ForecastInput {
        photoperiod,
        planned_veg_weeks,
        auto_total_days,
        strain_flower_days,
        defaults: PhaseDefaults::default(),
        actuals,
    }
}

/// The likely flower end, adjusting for the auto total cap and a
/// trichome-check nudge.
fn flower_end_for(input: &ForecastInput, flower_start: Day, (f_min, f_max): (Day, Day)) -> Day {
    let base = flower_start + i64::midpoint(f_min, f_max);
    let auto_capped = match (input.photoperiod, input.auto_total_days) {
        (PhotoperiodType::Auto, Some((auto_min, auto_max))) => {
            i64::midpoint(auto_min, auto_max).max(flower_start + f_min)
        }
        _ => base,
    };
    input
        .actuals
        .trichome_check_day
        .map_or(auto_capped, |check| {
            i64::midpoint(auto_capped, check).max(flower_start + f_min)
        })
}

/// Appends the vegetative window; returns the flip day and the flower
/// length range (strain overrides). `None` when nothing past veg is
/// projectable: a photoperiod with neither a logged flip nor planned
/// veg weeks leaves the window open-ended.
/// Pins the boundary between the last window and the one about to be
/// appended: the previous phase ends where the pinned one begins.
/// History wins in both directions — a later-than-expected pin extends
/// the previous phase, an earlier one truncates it; a pin at or before
/// the previous start collapses it to a point.
const fn pin_start(windows: &mut [PhaseWindow], start: Day) {
    if let Some(previous) = windows.last_mut() {
        previous.likely_end = if start > previous.likely_start {
            start
        } else {
            previous.likely_start
        };
    }
}

fn add_vegetative(
    windows: &mut Vec<PhaseWindow>,
    input: &ForecastInput,
    d: PhaseDefaults,
) -> Option<(Day, (Day, Day))> {
    if let Some(pin) = input.actuals.vegetative_start {
        pin_start(windows, pin);
    }
    let start = input
        .actuals
        .vegetative_start
        .unwrap_or_else(|| windows[1].likely_end);
    match input.photoperiod {
        PhotoperiodType::Photoperiod => {
            let flower = input
                .strain_flower_days
                .unwrap_or((d.flower_min, d.flower_max));
            let pinned = input.actuals.vegetative_start.is_some();
            let Some(weeks) = input.planned_veg_weeks else {
                // No plan: veg runs until the grower flips it. A
                // logged flip pins its end; without one there is no
                // projected end and no downstream windows.
                let flip = input.actuals.flip_day;
                add(
                    windows,
                    Phase::Vegetative,
                    start,
                    0,
                    0,
                    0,
                    flip.is_some() || pinned,
                    flip,
                );
                return flip.map(|flip| (flip, flower));
            };
            let veg_days = Day::from(weeks.max(1)) * 7;
            let flip = input.actuals.flip_day.unwrap_or(start + veg_days);
            add(
                windows,
                Phase::Vegetative,
                start,
                veg_days,
                veg_days,
                veg_days + 3,
                input.actuals.flip_day.is_some() || pinned,
                Some(flip),
            );
            Some((flip, flower))
        }
        PhotoperiodType::Auto => {
            let auto_flip = 28.max(start);
            add(
                windows,
                Phase::Vegetative,
                start,
                14,
                21,
                28,
                input.actuals.vegetative_start.is_some(),
                Some(auto_flip),
            );
            Some((auto_flip, (d.flower_min, d.flower_max)))
        }
    }
}

/// Appends a window and returns it; retro-truncates the previous phase
/// when an actual start lands earlier than its predicted end.
#[allow(clippy::too_many_arguments)] // grouped below in future cleanup; see docs/DECISIONS.md
fn add(
    windows: &mut Vec<PhaseWindow>,
    phase: Phase,
    start: Day,
    len_min: Day,
    len_likely: Day,
    len_max: Day,
    actual_start: bool,
    actual_end: Option<Day>,
) -> PhaseWindow {
    // When the actual end contradicts the natural start (an early
    // flip/harvest), reality wins: the phase compresses to start
    // at the actual end of the phase BEFORE it.
    let start = actual_end.map_or(start, |end| end.min(start));
    if let Some(previous) = windows.last_mut()
        && start < previous.likely_end
    {
        previous.likely_end = start;
        previous.earliest_end = previous.earliest_end.min(start);
        previous.latest_end = previous.latest_end.min(start.max(previous.earliest_end));
    }
    let end_likely = actual_end.map_or(start + len_likely, |end| end.max(start));
    let end_min = start + len_min.min(end_likely - start);
    let end_max = start + len_max.max(end_likely - start);
    let window = PhaseWindow {
        phase,
        earliest_start: start,
        likely_start: start,
        latest_start: start,
        earliest_end: end_min,
        likely_end: end_likely,
        latest_end: end_max,
        actual_start,
    };
    windows.push(window);
    window
}

#[cfg(test)]
mod tests {
    use super::{Actuals, Day, ForecastInput, Phase, PhaseDefaults, forecast, input_for};
    use crate::forecast::PhaseWindow;

    fn phases_ordered_and_contiguous(windows: &[PhaseWindow]) -> Result<(), String> {
        let expected = [
            Phase::Germination,
            Phase::Seedling,
            Phase::Vegetative,
            Phase::Flower,
            Phase::Drying,
            Phase::Curing,
        ];
        // The chain may stop early (open-ended photoperiod veg) but it
        // never skips or reorders a phase.
        if windows.len() > expected.len() {
            return Err(format!(
                "expected at most {} phases, got {}",
                expected.len(),
                windows.len()
            ));
        }
        for (index, (window, phase)) in windows.iter().zip(expected.iter()).enumerate() {
            if window.phase != *phase {
                return Err(format!(
                    "phase {index}: expected {:?}, got {:?}",
                    phase, window.phase
                ));
            }
            if window.earliest_start > window.likely_start
                || window.likely_start > window.latest_start
            {
                return Err(format!(
                    "phase {index} ({:?}): starts not ordered {:?} ≤ {:?} ≤ {:?}",
                    phase, window.earliest_start, window.likely_start, window.latest_start
                ));
            }
            if window.earliest_end > window.likely_end || window.likely_end > window.latest_end {
                return Err(format!("phase {index} ({phase:?}): ends not ordered"));
            }
            if window.likely_end < window.likely_start {
                return Err(format!(
                    "phase {index} ({:?}): end {:?} before start {:?}",
                    phase, window.likely_end, window.likely_start
                ));
            }
            if index > 0 && window.likely_start < windows[index - 1].likely_end {
                return Err(format!(
                    "phase {index} ({:?}): likely start {:?} before previous likely end {:?}",
                    phase,
                    window.likely_start,
                    windows[index - 1].likely_end
                ));
            }
            if index > 0 && window.earliest_start < windows[index - 1].earliest_start {
                return Err(format!(
                    "phase {index} ({phase:?}): earliest start goes backwards"
                ));
            }
        }
        Ok(())
    }

    fn base_input() -> ForecastInput {
        ForecastInput {
            photoperiod: crate::strain::PhotoperiodType::Photoperiod,
            planned_veg_weeks: Some(4),
            auto_total_days: None,
            strain_flower_days: None,
            defaults: PhaseDefaults::default(),
            actuals: Actuals::default(),
        }
    }

    // --- Property-style: many input combinations stay ordered ---

    #[test]
    fn contiguity_across_veg_weeks_and_flower_ranges() {
        for veg in 1_u8..=12 {
            for flower_min in [49_u16, 56, 63] {
                for flower_max in [flower_min, flower_min + 21] {
                    let input = ForecastInput {
                        planned_veg_weeks: Some(veg),
                        strain_flower_days: Some((Day::from(flower_min), Day::from(flower_max))),
                        ..base_input()
                    };
                    let windows = forecast(&input);
                    phases_ordered_and_contiguous(&windows).unwrap_or_else(|error| {
                        panic!("veg={veg} flower={flower_min}-{flower_max}: {error}")
                    });
                }
            }
        }
    }

    #[test]
    fn contiguity_with_actuals_pinned() {
        for flip in [20_i64, 30, 45, 60] {
            for harvest in [None, Some(flip + 56), Some(flip + 77)] {
                let input = ForecastInput {
                    actuals: Actuals {
                        flip_day: Some(flip),
                        harvest_day: harvest,
                        trichome_check_day: harvest.map(|h| h - 3),
                        germinated: true,
                        ..Actuals::default()
                    },
                    ..base_input()
                };
                let windows = forecast(&input);
                phases_ordered_and_contiguous(&windows)
                    .unwrap_or_else(|error| panic!("flip={flip} harvest={harvest:?}: {error}"));
            }
        }
    }

    #[test]
    fn contiguity_with_open_ended_veg() {
        for plan in [None, Some(1_u8), Some(8)] {
            for flip in [None, Some(20_i64), Some(50)] {
                let input = ForecastInput {
                    planned_veg_weeks: plan,
                    actuals: Actuals {
                        flip_day: flip,
                        ..Actuals::default()
                    },
                    ..base_input()
                };
                let windows = forecast(&input);
                phases_ordered_and_contiguous(&windows).unwrap_or_else(|error| {
                    panic!("plan={plan:?} flip={flip:?}: {error}");
                });
            }
        }
    }

    #[test]
    fn photoperiod_without_plan_or_pin_is_open_ended() {
        let input = ForecastInput {
            planned_veg_weeks: None,
            ..base_input()
        };
        let windows = forecast(&input);
        // Only the front of the chain is projected: no flower, drying
        // or curing window exists.
        assert_eq!(windows.len(), 3);
        assert_eq!(windows[2].phase, Phase::Vegetative);
        // Defaults: germ 3–10 (likely 6), seedling 14–21 (likely 17),
        // so veg starts at day 23 and has no projected end.
        assert_eq!(windows[2].likely_start, 23);
        assert_eq!(windows[2].likely_end, 23);
        assert!(!windows[2].actual_start);
        assert!(
            windows
                .iter()
                .all(|w| !matches!(w.phase, Phase::Flower | Phase::Drying | Phase::Curing))
        );
    }

    #[test]
    fn planned_veg_projects_the_flip() {
        let input = ForecastInput {
            planned_veg_weeks: Some(2),
            ..base_input()
        };
        let windows = forecast(&input);
        // 2 planned weeks = 14 days of veg from day 23 → flip at 37,
        // and the full chain follows.
        assert_eq!(windows.len(), 6);
        let veg = &windows[2];
        assert_eq!(veg.phase, Phase::Vegetative);
        assert_eq!(veg.likely_start, 23);
        assert_eq!(veg.likely_end, 37);
        let flower = &windows[3];
        assert_eq!(flower.phase, Phase::Flower);
        assert_eq!(flower.likely_start, 37);
        assert!(!flower.actual_start);
    }

    #[test]
    fn flip_pin_without_plan_derives_downstream() {
        let input = ForecastInput {
            planned_veg_weeks: None,
            actuals: Actuals {
                flip_day: Some(45),
                ..Actuals::default()
            },
            ..base_input()
        };
        let windows = forecast(&input);
        assert_eq!(windows.len(), 6);
        // The pinned flip ends veg and starts flower.
        let veg = &windows[2];
        assert_eq!(veg.likely_end, 45);
        assert_eq!(veg.latest_end, 45);
        let flower = &windows[3];
        assert_eq!(flower.likely_start, 45);
        assert!(flower.actual_start);
        // Defaults 56–77 (likely 66) → dry starts at 111.
        assert_eq!(flower.likely_end, 111);
        assert_eq!(windows[4].phase, Phase::Drying);
        assert_eq!(windows[4].likely_start, 111);
    }

    #[test]
    fn germination_pin_moves_germ_start() {
        // A "back to Germination" pick restarts the chain at the pinned
        // day.
        let input = ForecastInput {
            actuals: Actuals {
                germination_start: Some(5),
                ..Actuals::default()
            },
            ..base_input()
        };
        let windows = forecast(&input);
        assert_eq!(windows[0].phase, Phase::Germination);
        assert_eq!(windows[0].likely_start, 5);
        assert!(windows[0].actual_start);
        phases_ordered_and_contiguous(&windows).unwrap();
    }

    #[test]
    fn seedling_pin_moves_seedling_and_compresses_germ() {
        let input = ForecastInput {
            actuals: Actuals {
                seedling_start: Some(10),
                ..Actuals::default()
            },
            ..base_input()
        };
        let windows = forecast(&input);
        assert_eq!(windows[1].phase, Phase::Seedling);
        assert_eq!(windows[1].likely_start, 10);
        assert!(windows[1].actual_start);
        // Germination is history: it ends where seedling began.
        assert_eq!(windows[0].likely_end, 10);
        phases_ordered_and_contiguous(&windows).unwrap();
    }

    #[test]
    fn vegetative_pin_moves_veg_start() {
        let input = ForecastInput {
            planned_veg_weeks: None,
            actuals: Actuals {
                vegetative_start: Some(30),
                ..Actuals::default()
            },
            ..base_input()
        };
        let windows = forecast(&input);
        assert_eq!(windows[2].phase, Phase::Vegetative);
        assert_eq!(windows[2].likely_start, 30);
        assert!(windows[2].actual_start);
        assert_eq!(windows[1].likely_end, 30);
        // Still open-ended: the pin only moved the boundary.
        assert_eq!(windows.len(), 3);
        phases_ordered_and_contiguous(&windows).unwrap();
    }

    #[test]
    fn autoflower_forecasts_stay_ordered() {
        for total_min in [63_i64, 70, 77] {
            for total_max in [total_min, total_min + 14] {
                let input = ForecastInput {
                    photoperiod: crate::strain::PhotoperiodType::Auto,
                    planned_veg_weeks: None,
                    auto_total_days: Some((total_min, total_max)),
                    ..base_input()
                };
                let windows = forecast(&input);
                phases_ordered_and_contiguous(&windows)
                    .unwrap_or_else(|error| panic!("auto={total_min}-{total_max}: {error}"));
            }
        }
    }

    #[test]
    fn re_forecast_from_flip_is_consistent() {
        // Without a flip, flower starts after the planned veg.
        let planned = forecast(&base_input());
        let planned_flower = planned
            .iter()
            .find(|w| w.phase == Phase::Flower)
            .expect("flower window");

        // With the flip pinned to that same day, the result matches.
        let pinned = forecast(&ForecastInput {
            actuals: Actuals {
                flip_day: Some(planned_flower.likely_start),
                ..Actuals::default()
            },
            ..base_input()
        });
        let pinned_flower = pinned
            .iter()
            .find(|w| w.phase == Phase::Flower)
            .expect("flower window");
        assert_eq!(planned_flower.likely_start, pinned_flower.likely_start);
        assert_eq!(planned_flower.likely_end, pinned_flower.likely_end);
    }

    // --- Golden tests (spec §13) ---

    #[test]
    fn golden_photoperiod_northern_lights() {
        // Northern Lights: 45–63 flower days (seed catalogue), 4 veg
        // weeks planned. Defaults: germ 3–10 (likely 6), seedling
        // 14–21 (likely 17).
        let strain_data = r#"{
            "id": "x", "name": "Northern Lights", "breeder": "Sensi Seeds",
            "photoperiod": "photoperiod", "sex": "feminized",
            "indica_pct": 90, "landrace_tags": ["afghani"],
            "flower_days_min": 45, "flower_days_max": 63,
            "stretch_factor": 1.4, "veg_weeks_min": null, "veg_weeks_max": null,
            "height_class": "short", "auto_total_days_min": null,
            "auto_total_days_max": null, "yield_notes": null,
            "terpene_notes": null, "effect_notes": null,
            "heat_tolerance": "sensitive", "cold_tolerance": "average",
            "light_tolerance": "average", "user_rating": null, "notes": null,
            "evidence": {}, "source_url": null, "source_licence": null,
            "is_phenotype_of": null, "created_at": "t", "updated_at": "t"
        }"#;
        let strain: crate::strain::Strain =
            serde_json::from_str(strain_data).expect("strain parses");
        let input = input_for(Some(&strain), Some(4), Actuals::default());
        let windows = forecast(&input);

        let by_phase = |phase: Phase| {
            windows
                .iter()
                .find(|w| w.phase == phase)
                .unwrap_or_else(|| panic!("{phase:?} window missing"))
        };

        // Germ: day 0, likely end 6 (mid of 3–10).
        assert_eq!(by_phase(Phase::Germination).likely_end, 6);
        // Seedling: 17 days (mid of 14–21), so veg starts at 6+17=23.
        assert_eq!(by_phase(Phase::Seedling).likely_start, 6);
        assert_eq!(by_phase(Phase::Seedling).likely_end, 23);
        // Veg: 4 weeks = 28 days → flower starts at 51.
        assert_eq!(by_phase(Phase::Vegetative).likely_start, 23);
        assert_eq!(by_phase(Phase::Vegetative).likely_end, 51);
        // Flower: strain 45–63 days, likely 54 → ends at 105.
        let flower = by_phase(Phase::Flower);
        assert_eq!(flower.likely_start, 51);
        assert_eq!(flower.earliest_end, 51 + 45);
        assert_eq!(flower.likely_end, 51 + 54);
        assert_eq!(flower.latest_end, 51 + 63);
        // Dry: 7–14 (likely 10) → 115; cure: 14–30 (likely 22) → 137.
        assert_eq!(by_phase(Phase::Drying).likely_start, 105);
        assert_eq!(by_phase(Phase::Drying).likely_end, 115);
        assert_eq!(by_phase(Phase::Curing).likely_start, 115);
        assert_eq!(by_phase(Phase::Curing).likely_end, 137);
    }

    #[test]
    fn golden_autoflower_gg4_auto() {
        // GG4 Auto: 70–84 days from seed. Autos flip at day 28.
        let strain_data = r#"{
            "id": "x", "name": "GG4 Auto", "breeder": null,
            "photoperiod": "auto", "sex": "feminized",
            "indica_pct": 60, "landrace_tags": ["chem", "sour_diesel", "ruderalis"],
            "flower_days_min": null, "flower_days_max": null,
            "stretch_factor": 1.4, "veg_weeks_min": null, "veg_weeks_max": null,
            "height_class": "short", "auto_total_days_min": 70,
            "auto_total_days_max": 84, "yield_notes": null,
            "terpene_notes": null, "effect_notes": null,
            "heat_tolerance": "sensitive", "cold_tolerance": "tolerant",
            "light_tolerance": "average", "user_rating": null, "notes": null,
            "evidence": {}, "source_url": null, "source_licence": null,
            "is_phenotype_of": null, "created_at": "t", "updated_at": "t"
        }"#;
        let strain: crate::strain::Strain =
            serde_json::from_str(strain_data).expect("strain parses");
        let input = input_for(Some(&strain), None, Actuals::default());
        let windows = forecast(&input);

        let flower = windows
            .iter()
            .find(|w| w.phase == Phase::Flower)
            .expect("flower window");
        // Autos flip by age (day 28), flower actual_start is true.
        assert!(flower.actual_start);
        // The total window lands within the seed-to-harvest range
        // (70–84 days from seed).
        let harvest = flower.likely_end;
        assert!(
            (70..=84).contains(&harvest),
            "autoflower harvest day {harvest} should be within 70–84"
        );
    }

    #[test]
    fn strain_flower_days_override_defaults() {
        let with_strain = forecast(&ForecastInput {
            strain_flower_days: Some((50, 60)),
            ..base_input()
        });
        let without = forecast(&base_input());
        let with = with_strain
            .iter()
            .find(|w| w.phase == Phase::Flower)
            .expect("flower");
        let plain = without
            .iter()
            .find(|w| w.phase == Phase::Flower)
            .expect("flower");
        assert_eq!(with.earliest_end - with.likely_start, 50);
        assert_eq!(with.latest_end - with.likely_start, 60);
        assert_eq!(
            plain.earliest_end - plain.likely_start,
            super::PhaseDefaults::default().flower_min
        );
    }
}
