//! Phase forecast engine (spec §4.5, milestone M5).
//!
//! A pure function from a grow's inputs (start date, photoperiod
//! toggle, planned veg weeks or auto age, and any logged actual
//! events) to a list of phase windows — each phase with an
//! **earliest / likely / latest** start and end. Logged events (flip,
//! transplant, trichome check, harvest) pin their phase boundary to
//! reality; everything after the last actual re-forecasts from it.
//!
//! Seed defaults carry the spec §4.5 ranges; an explicit flower-days
//! override replaces them when present ([`ForecastInput`
//! construction is the extension point, not the engine]). Autos skip
//! the veg choice and flower by age. All inputs come from the grow
//! (D075) — no strain record is consulted.

use serde::{Deserialize, Serialize};

use crate::Phase;
use crate::grows::PhotoperiodType;

/// A date (days since the grow start, 0-based), so the engine stays
/// calendar-free and testable.
pub type Day = i64;

/// The default phase durations in days (spec §4.5, sourced ranges).
/// Germination 3–10, seedling 14–21 (2–3 weeks), flower 56–77
/// (8–11 weeks; an explicit flower-days override wins), dry 7–14,
/// cure 14–30.
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
    /// Flower minimum days (8 weeks; the override wins when present).
    pub flower_min: Day,
    /// Flower maximum days (11 weeks; the override wins when
    /// present).
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
    /// Photoperiod or autoflowering (from the grow, D075).
    pub photoperiod: PhotoperiodType,
    /// Planned vegetative weeks for photoperiods (grower's choice);
    /// ignored for autos. When absent and no flip is logged, the
    /// photoperiod veg window is open-ended and no flower, drying or
    /// curing window is produced.
    pub planned_veg_weeks: Option<u8>,
    /// Autoflower total days from seed (overrides flower timing when
    /// present).
    pub auto_total_days: Option<(Day, Day)>,
    /// Flower days (min, max); overrides the defaults. The name is
    /// historical — any source may supply the override; the grow
    /// carries no flower data, so [`input_for`] only fills it for the
    /// auto default window.
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
    let germ_end = windows[0].likely_end;
    let seed_start = seedling_pin.map_or(germ_end, |pin| pin_start(&mut windows, pin));
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
        finish_open_ended(&mut windows, input, d);
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
        input.actuals.flip_day.is_some()
            || input.actuals.flower_start.is_some()
            || input.photoperiod == PhotoperiodType::Auto,
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

    // --- Cure: after dry, or pinned ---
    let cure_start = input
        .actuals
        .curing_start
        .map_or(dry_window.likely_end, |pin| pin_start(&mut windows, pin));
    add(
        &mut windows,
        Phase::Curing,
        cure_start,
        d.cure_min,
        i64::midpoint(d.cure_min, d.cure_max),
        d.cure_max,
        input.actuals.curing_start.is_some(),
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
                // Both must collapse to the same point; the start band
                // of this window clamps up with its likely start.
                let collapsed = previous.likely_start;
                previous.likely_end = collapsed;
                windows[index].likely_start = collapsed;
                windows[index].earliest_start = windows[index].earliest_start.max(collapsed);
                windows[index].latest_start = windows[index].latest_start.max(collapsed);
                windows[index].likely_end = windows[index].likely_end.max(collapsed);
            }
        }
    }
    // Final monotonicity pass on the end bands.
    for window in windows {
        window.earliest_end = window.earliest_end.clamp(
            window.likely_start,
            window.likely_end.max(window.likely_start),
        );
        window.latest_end = window.latest_end.max(window.likely_end);
    }
}

/// The grow day an autoflower starts flowering by age (spec §4.5:
/// autos start ~3–5 weeks from seed; the engine anchors the age flip
/// at week 4).
pub const AUTO_FLIP_DAY: Day = 28;

/// The default autoflower seed-to-harvest window in days (spec §4.5:
/// ~9–11 weeks from seed). Unchanged default window — the grow's
/// photoperiod toggle alone selects it.
pub const AUTO_TOTAL_DAYS: (Day, Day) = (63, 77);

/// Builds the forecast input from a grow's facts (used by the backend
/// API, D075): the photoperiod toggle decides the windows.
///
/// Unset parses as a photoperiod grow on the default ranges; an
/// autoflower grow finishes within [`AUTO_TOTAL_DAYS`] from seed, so
/// its flower window is the total minus the age flip.
#[must_use]
pub fn input_for(
    photoperiod: PhotoperiodType,
    planned_veg_weeks: Option<u8>,
    actuals: Actuals,
) -> ForecastInput {
    let (strain_flower_days, auto_total_days) = match photoperiod {
        PhotoperiodType::Photoperiod => (None, None),
        PhotoperiodType::Auto => (
            Some((
                AUTO_TOTAL_DAYS.0 - AUTO_FLIP_DAY,
                AUTO_TOTAL_DAYS.1 - AUTO_FLIP_DAY,
            )),
            Some(AUTO_TOTAL_DAYS),
        ),
    };
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

/// Completes an open-ended chain: nothing past veg is projectable,
/// unless a curing pin says the grow is already curing, in which case
/// the skipped phases collapse into history at the pin.
fn finish_open_ended(windows: &mut Vec<PhaseWindow>, input: &ForecastInput, d: PhaseDefaults) {
    if let Some(pin) = input.actuals.curing_start {
        let start = pin_start(windows, pin);
        add(windows, Phase::Flower, start, 0, 0, 0, true, Some(start));
        add(windows, Phase::Drying, start, 0, 0, 0, true, Some(start));
        add(
            windows,
            Phase::Curing,
            start,
            d.cure_min,
            i64::midpoint(d.cure_min, d.cure_max),
            d.cure_max,
            true,
            None,
        );
    }
}

/// Pins the boundary between the last window and the one about to be
/// appended: the previous phase ends where the pinned one begins.
/// History wins in both directions — a later-than-expected pin extends
/// the previous phase; a pin earlier than the previous phase's start
/// clamps to that start (nearest sensible window). Returns the
/// effective pinned start.
fn pin_start(windows: &mut [PhaseWindow], requested: Day) -> Day {
    let floor = windows
        .last()
        .map_or(requested, |previous| previous.likely_start);
    let start = requested.max(floor);
    if let Some(previous) = windows.last_mut() {
        previous.likely_end = start;
    }
    start
}

/// Appends the vegetative window; returns the flip day and the flower
/// length range (override or defaults). `None` when nothing past veg
/// is projectable: a photoperiod with neither a logged flip nor
/// planned veg weeks leaves the window open-ended.
fn add_vegetative(
    windows: &mut Vec<PhaseWindow>,
    input: &ForecastInput,
    d: PhaseDefaults,
) -> Option<(Day, (Day, Day))> {
    let seedling_end = windows[1].likely_end;
    let start = input
        .actuals
        .vegetative_start
        .map_or_else(|| seedling_end, |pin| pin_start(windows, pin));
    match input.photoperiod {
        PhotoperiodType::Photoperiod => {
            let flower = input
                .strain_flower_days
                .unwrap_or((d.flower_min, d.flower_max));
            let pinned = input.actuals.vegetative_start.is_some();
            let Some(weeks) = input.planned_veg_weeks else {
                // No plan: veg runs until the grower flips it. A
                // logged flip (or Flowering pick) pins its end; without
                // one there is no projected end and no downstream
                // windows.
                let flip = input.actuals.flip_day.or(input.actuals.flower_start);
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
            let flip = input
                .actuals
                .flip_day
                .or(input.actuals.flower_start)
                .unwrap_or(start + veg_days);
            add(
                windows,
                Phase::Vegetative,
                start,
                veg_days,
                veg_days,
                veg_days + 3,
                input.actuals.flip_day.is_some() || input.actuals.flower_start.is_some() || pinned,
                Some(flip),
            );
            Some((flip, flower))
        }
        PhotoperiodType::Auto => {
            // A Flowering pick pins the auto's flower start; otherwise
            // the age estimate stands, never earlier than the veg
            // start.
            let auto_flip = input
                .actuals
                .flower_start
                .map_or_else(|| AUTO_FLIP_DAY.max(start), |pin| pin.max(start));
            // The flower window: an explicit override (the adapter's
            // derived default total) or the default ranges.
            let flower = input
                .strain_flower_days
                .unwrap_or((d.flower_min, d.flower_max));
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
            Some((auto_flip, flower))
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
    // A pinned end can predate the natural start; the boundary never
    // moves before the previous phase began.
    let floor = windows
        .last()
        .map_or(start, |previous| previous.likely_start);
    let start = start.max(floor);
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
            photoperiod: crate::grows::PhotoperiodType::Photoperiod,
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
    fn flower_pin_pins_open_photoperiod() {
        // A Flowering pick on a photoperiod with no flip and no plan
        // still projects the whole downstream chain.
        let input = ForecastInput {
            planned_veg_weeks: None,
            actuals: Actuals {
                flower_start: Some(45),
                ..Actuals::default()
            },
            ..base_input()
        };
        let windows = forecast(&input);
        assert_eq!(windows.len(), 6);
        assert_eq!(windows[2].likely_end, 45);
        let flower = &windows[3];
        assert_eq!(flower.likely_start, 45);
        assert!(flower.actual_start);
        phases_ordered_and_contiguous(&windows).unwrap();
    }

    #[test]
    fn autoflower_flowering_pin_beats_age_estimate() {
        let input = ForecastInput {
            photoperiod: crate::grows::PhotoperiodType::Auto,
            planned_veg_weeks: None,
            auto_total_days: Some((70, 84)),
            actuals: Actuals {
                flower_start: Some(35),
                ..Actuals::default()
            },
            ..base_input()
        };
        let windows = forecast(&input);
        assert_eq!(windows[2].likely_end, 35);
        let flower = &windows[3];
        assert_eq!(flower.likely_start, 35);
        assert!(flower.actual_start);
        phases_ordered_and_contiguous(&windows).unwrap();
    }

    #[test]
    fn flip_beats_flower_pin_on_photoperiod() {
        // A logged flip is the more precise record for the same
        // boundary.
        let input = ForecastInput {
            planned_veg_weeks: None,
            actuals: Actuals {
                flip_day: Some(45),
                flower_start: Some(40),
                ..Actuals::default()
            },
            ..base_input()
        };
        let windows = forecast(&input);
        assert_eq!(windows[2].likely_end, 45);
        assert_eq!(windows[3].likely_start, 45);
    }

    #[test]
    fn curing_pin_starts_cure_and_extends_dry() {
        let input = ForecastInput {
            actuals: Actuals {
                flip_day: Some(45),
                curing_start: Some(130),
                ..Actuals::default()
            },
            ..base_input()
        };
        let windows = forecast(&input);
        // Dry ran long (reality wins) and cure starts at the pin.
        assert_eq!(windows[4].likely_end, 130);
        let cure = &windows[5];
        assert_eq!(cure.phase, Phase::Curing);
        assert_eq!(cure.likely_start, 130);
        assert!(cure.actual_start);
        phases_ordered_and_contiguous(&windows).unwrap();
    }

    #[test]
    fn curing_pin_on_open_chain_skips_history() {
        // Curing pinned without a flip or plan: the skipped flower and
        // drying phases collapse into history at the pin.
        let input = ForecastInput {
            planned_veg_weeks: None,
            actuals: Actuals {
                curing_start: Some(90),
                ..Actuals::default()
            },
            ..base_input()
        };
        let windows = forecast(&input);
        assert_eq!(windows.len(), 6);
        assert_eq!(windows[2].likely_end, 90);
        let flower = &windows[3];
        assert_eq!(flower.phase, Phase::Flower);
        assert_eq!(flower.likely_start, 90);
        assert_eq!(flower.likely_end, 90);
        let drying = &windows[4];
        assert_eq!(drying.likely_start, 90);
        assert_eq!(drying.likely_end, 90);
        let cure = &windows[5];
        assert_eq!(cure.likely_start, 90);
        assert!(cure.likely_end > 90);
        assert!(cure.actual_start);
        phases_ordered_and_contiguous(&windows).unwrap();
    }

    #[test]
    fn contiguity_with_arbitrary_boundary_pins() {
        for germ in [None, Some(0), Some(6), Some(40)] {
            for seed in [None, Some(2), Some(12), Some(50)] {
                for veg in [None, Some(5), Some(28), Some(70)] {
                    for flower in [None, Some(10), Some(45), Some(90)] {
                        for cure in [None, Some(15), Some(120), Some(200)] {
                            let input = ForecastInput {
                                planned_veg_weeks: None,
                                actuals: Actuals {
                                    germination_start: germ,
                                    seedling_start: seed,
                                    vegetative_start: veg,
                                    flower_start: flower,
                                    curing_start: cure,
                                    ..Actuals::default()
                                },
                                ..base_input()
                            };
                            let windows = forecast(&input);
                            phases_ordered_and_contiguous(&windows).unwrap_or_else(|error| {
                                panic!(
                                    "germ={germ:?} seed={seed:?} veg={veg:?} flower={flower:?} cure={cure:?}: {error}"
                                )
                            });
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn autoflower_forecasts_stay_ordered() {
        for total_min in [63_i64, 70, 77] {
            for total_max in [total_min, total_min + 14] {
                let input = ForecastInput {
                    photoperiod: crate::grows::PhotoperiodType::Auto,
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

    // --- Grow-sourced inputs (ticket 02: the grow carries the
    // photoperiod; no strain record is consulted) ---

    #[test]
    fn input_for_unset_photoperiod_is_a_photoperiod_grow() {
        // An unlabelled photoperiod grow with a 4-week plan: the whole
        // chain projects on the default windows alone.
        let input = input_for(
            crate::grows::PhotoperiodType::Photoperiod,
            Some(4),
            Actuals::default(),
        );
        assert_eq!(
            input.photoperiod,
            crate::grows::PhotoperiodType::Photoperiod
        );
        let windows = forecast(&input);
        assert_eq!(windows.len(), 6);
        // Defaults: germ 3–10 (likely 6), seedling 14–21 (likely 17),
        // so veg starts at day 23; 4 planned weeks project the flip at
        // 51 and the default 56–77 flower days follow.
        let flower = &windows[3];
        assert_eq!(flower.phase, Phase::Flower);
        assert_eq!(flower.likely_start, 51);
        assert_eq!(flower.earliest_end, 51 + 56);
        assert_eq!(flower.likely_end, 51 + 66);
        assert_eq!(flower.latest_end, 51 + 77);
    }

    #[test]
    fn input_for_autoflower_projects_the_default_total_window() {
        // An autoflower grow with no other data finishes 63–77 days
        // from seed (spec §4.5: ~9–11 weeks), anchored on the age flip.
        let input = input_for(
            crate::grows::PhotoperiodType::Auto,
            None,
            Actuals::default(),
        );
        let windows = forecast(&input);
        assert_eq!(windows.len(), 6);
        let flower = &windows[3];
        assert_eq!(flower.phase, Phase::Flower);
        assert!(flower.actual_start, "autos flip by age");
        assert_eq!(flower.likely_start, super::AUTO_FLIP_DAY);
        // The seed-to-harvest total lands exactly in the default
        // window: earliest 63, likely 70, latest 77.
        assert_eq!(flower.earliest_end, 63);
        assert_eq!(flower.likely_end, 70);
        assert_eq!(flower.latest_end, 77);
    }

    // --- Golden tests (spec §13) ---

    #[test]
    fn golden_photoperiod_grow() {
        // A photoperiod grow, 4 veg weeks planned. Defaults: germ 3–10
        // (likely 6), seedling 14–21 (likely 17).
        let input = input_for(
            crate::grows::PhotoperiodType::Photoperiod,
            Some(4),
            Actuals::default(),
        );
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
        // Flower: default 56–77 days, likely 66 → ends at 117.
        let flower = by_phase(Phase::Flower);
        assert_eq!(flower.likely_start, 51);
        assert_eq!(flower.earliest_end, 51 + 56);
        assert_eq!(flower.likely_end, 51 + 66);
        assert_eq!(flower.latest_end, 51 + 77);
        // Dry: 7–14 (likely 10) → 127; cure: 14–30 (likely 22) → 149.
        assert_eq!(by_phase(Phase::Drying).likely_start, 117);
        assert_eq!(by_phase(Phase::Drying).likely_end, 127);
        assert_eq!(by_phase(Phase::Curing).likely_start, 127);
        assert_eq!(by_phase(Phase::Curing).likely_end, 149);
    }

    #[test]
    fn golden_autoflower_grow() {
        // An autoflower grow on the default total window: flip at day
        // 28, harvest inside 63–77 days from seed.
        let input = input_for(
            crate::grows::PhotoperiodType::Auto,
            None,
            Actuals::default(),
        );
        let windows = forecast(&input);

        let flower = windows
            .iter()
            .find(|w| w.phase == Phase::Flower)
            .expect("flower window");
        // Autos flip by age (day 28), flower actual_start is true.
        assert!(flower.actual_start);
        // The total window lands within the seed-to-harvest default
        // (63–77 days from seed).
        let harvest = flower.likely_end;
        assert!(
            (63..=77).contains(&harvest),
            "autoflower harvest day {harvest} should be within 63–77"
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
