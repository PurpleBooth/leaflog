//! VPD and environment engines (spec §4.9, §4.10, §4.4, M7).
//!
//! VPD (vapour-pressure deficit) from air temperature + RH with a
//! configurable leaf-temperature offset; the RH-window solver finds
//! the humidity range that keeps VPD in band at the current
//! temperature. The lamp-height advisor applies the spec §5.1
//! wattage-class table with dimming, stretch and floor rules, plus a
//! light-tolerance caution. Grow heat/cold tolerance shifts alert
//! thresholds (tolerant +2 °F ceiling, sensitive −3 °F). All sources
//! are grow attributes (D075).

use serde::{Deserialize, Serialize};

use crate::Phase;
use crate::grows::Tolerance;

// ---------------------------------------------------------------------------
// VPD engine (spec §4.9, §5.2)
// ---------------------------------------------------------------------------

/// Default leaf-temperature offset: −2 °C / −3.6 °F (spec §4.9).
pub const DEFAULT_LEAF_OFFSET_C: f64 = -2.0;

/// Lights-off leaf-temperature offset: −1 °C.
///
/// [`DEFAULT_LEAF_OFFSET_C`] is a lights-on assumption: with the lamps
/// running, the leaf runs measurably below the air sensor's reading.
/// In the dark the stomata close, transpiration cooling stops, and
/// the leaf sits close to air temperature — Black Dog LED's guidance
/// spans 0 to −1 °C for lights-off. −1 °C takes the conservative
/// edge. Any night VPD or RH computation must pass this offset in
/// place of the default, or the numbers carry the lights-on cooling
/// assumption into a dark room.
pub const NIGHT_LEAF_OFFSET_C: f64 = -1.0;

/// Saturation vapour pressure over water, kPa.
///
/// Magnus/Tetens approximation, ASHRAE-fundamentals accuracy ±0.1 %.
#[must_use]
pub fn saturation_vp_kpa(temperature_c: f64) -> f64 {
    0.61078 * (17.27 * temperature_c / (temperature_c + 237.3)).exp()
}

/// VPD in kPa from air temperature (°C), RH (%), and a leaf offset
/// (°C).
///
/// The leaf temperature is `air + offset`; the VPD uses the
/// leaf's saturation pressure minus the air's actual vapour
/// pressure (the standard grow-room formulation).
#[must_use]
pub fn vpd_kpa(air_temperature_c: f64, relative_humidity_pct: f64, leaf_offset_c: f64) -> f64 {
    let leaf_temperature = air_temperature_c + leaf_offset_c;
    let saturation = saturation_vp_kpa(leaf_temperature);
    let actual = saturation_vp_kpa(air_temperature_c) * relative_humidity_pct / 100.0;
    (saturation - actual).max(0.0)
}

/// The VPD target band per phase (spec §5.2, kPa).
#[must_use]
pub const fn vpd_band(phase: Phase) -> (f64, f64) {
    match phase {
        Phase::Germination | Phase::Seedling => (0.4, 0.8),
        Phase::Vegetative => (0.8, 1.2),
        Phase::Flower => (1.0, 1.6),
        Phase::Drying | Phase::Curing => (0.5, 1.0),
    }
}

/// Whether the VPD is in the target band for the phase.
#[must_use]
pub fn vpd_in_band(vpd: f64, phase: Phase) -> bool {
    let (min, max) = vpd_band(phase);
    (min..=max).contains(&vpd)
}

/// A lights-off VPD target band in kPa, with the sources behind its
/// numbers (seed-data discipline: a label on every row).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NightVpdBand {
    /// Lower edge, kPa.
    pub min_kpa: f64,
    /// Upper edge, kPa.
    pub max_kpa: f64,
    /// Where the numbers come from.
    pub sources: &'static str,
}

/// The per-phase lights-off VPD band (kPa).
///
/// No peer-reviewed source publishes cannabis night-VPD numbers; the
/// bands are vendor/community consensus (one guide keeps a dedicated
/// "VPD at Night" section), each consistent with the peer-reviewed
/// night-transpiration work those guides cite. The row labels name
/// them. The shared rule across sources: keep night VPD close to the
/// day band — transpiration falls ~10× in the dark and the room cools
/// with it.
///
/// Drying and curing rooms run with no lights, so there is no
/// lights-off variant: the single day band applies 24/7 and this
/// returns [`None`].
#[must_use]
pub const fn night_vpd_band(phase: Phase) -> Option<NightVpdBand> {
    match phase {
        // Dome and clone numbers disagree widely across sources; the
        // label keeps that disagreement visible.
        Phase::Germination | Phase::Seedling => Some(NightVpdBand {
            min_kpa: 0.5,
            max_kpa: 0.9,
            sources: "Pulse Grow night 0.6–1.0 kPa; THCFarmer 0.4–0.8; Black Dog LED \
                clones 0.3–0.6 — sources vary (0.3–1.0 kPa)",
        }),
        Phase::Vegetative => Some(NightVpdBand {
            min_kpa: 0.8,
            max_kpa: 1.2,
            sources: "Pulse Grow night 0.8–1.2 kPa; Black Dog LED 0.8–1.1; \
                THCFarmer 0.8–1.1",
        }),
        Phase::Flower => Some(NightVpdBand {
            min_kpa: 1.0,
            max_kpa: 1.4,
            sources: "Pulse Grow night 1.0–1.5 kPa; THCFarmer 0.9–1.5 across flower; \
                Black Dog LED 1.0–1.6 by stage",
        }),
        // No lights, no lights-off period: the day band applies 24/7.
        Phase::Drying | Phase::Curing => None,
    }
}

/// One phase's temperature corridor: the lights-on range (spec §5.2).
///
/// Not drawn by any chart today — the temperature Target Band is the
/// grow-tolerance band (see [`temperature_target_band`]); these
/// sourced rows are kept as seed data.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
pub struct TemperatureCorridor {
    /// Lower edge of the lights-on range, °C.
    pub min_c: f64,
    /// Upper edge of the lights-on range, °C.
    pub max_c: f64,
    /// Where the range comes from (preset discipline: every row is
    /// labelled).
    pub source: &'static str,
}

/// The per-phase lights-on temperature corridor (spec §5.2 table, °F
/// converted to °C).
///
/// Germination has no row of its own in §5.2, so it reuses the
/// seedling/clone range. Curing is "room temp" in §5.2; the 18–25 °C
/// fill-in is the common room-temperature convention and marked
/// unverified.
#[must_use]
pub const fn temperature_corridor(phase: Phase) -> TemperatureCorridor {
    match phase {
        Phase::Germination | Phase::Seedling => TemperatureCorridor {
            min_c: 21.1,
            max_c: 26.7,
            source: "spec §5.2 seedling/clone 70–80 °F (germination reuses the row)",
        },
        Phase::Vegetative => TemperatureCorridor {
            min_c: 21.1,
            max_c: 29.4,
            source: "spec §5.2 veg 70–85 °F",
        },
        Phase::Flower => TemperatureCorridor {
            min_c: 18.3,
            max_c: 26.7,
            source: "spec §5.2 flower 65–80 °F",
        },
        Phase::Drying => TemperatureCorridor {
            min_c: 15.6,
            max_c: 20.0,
            source: "spec §5.2 dry 60–68 °F",
        },
        Phase::Curing => TemperatureCorridor {
            min_c: 18.0,
            max_c: 25.0,
            source: "spec §5.2 room temp; 18–25 °C convention, unverified",
        },
    }
}

/// The per-phase lights-off temperature corridor (°C).
///
/// From the 2026-11-16 lights-off research pass: a grower guide's
/// stage table is the only source with an explicit lights-off column
/// (named in the row labels), and the peer-reviewed anchor (Agronomy
/// 13(10):2636) found 21 °C nights optimal and 27 °C nights harmful in
/// the reproductive stage. Drying and curing rooms run with no lights,
/// so they have no lights-off variant — the single preset in
/// [`temperature_corridor`] applies 24/7 and this returns [`None`].
#[must_use]
pub const fn night_temperature_corridor(phase: Phase) -> Option<TemperatureCorridor> {
    match phase {
        Phase::Germination | Phase::Seedling => Some(TemperatureCorridor {
            min_c: 20.0,
            max_c: 23.3,
            source: "THCFarmer lights-off 68–74 °F (germination reuses the seedling row); \
                Agronomy 13(10):2636 rooting ran a constant 24 °C day/night",
        }),
        Phase::Vegetative => Some(TemperatureCorridor {
            min_c: 18.3,
            max_c: 23.9,
            source: "THCFarmer lights-off 65–75 °F; Growers Network ~68 °F lights-off; \
                Agronomy 13(10):2636 night 21 °C optimal",
        }),
        Phase::Flower => Some(TemperatureCorridor {
            min_c: 15.6,
            max_c: 23.3,
            source: "THCFarmer lights-off 60–74 °F across flower (early → top, late → \
                bottom); Agronomy 13(10):2636 night 21 °C optimal, ≥27 °C harmful",
        }),
        // No lights, no lights-off period: the day preset applies 24/7.
        Phase::Drying | Phase::Curing => None,
    }
}

/// The RH window that keeps VPD in band at the given temperature.
///
/// Returns `(rh_min, rh_max)` in %; the range may be empty (no RH can
/// fix it at this temperature — the leaf offset is too large).
#[must_use]
pub fn rh_window(air_temperature_c: f64, leaf_offset_c: f64, band: (f64, f64)) -> (f64, f64) {
    let leaf_saturation = saturation_vp_kpa(air_temperature_c + leaf_offset_c);
    let air_saturation = saturation_vp_kpa(air_temperature_c);
    // VPD(h) = leaf_sat - air_sat × h/100 → h = 100 × (leaf_sat - VPD) / air_sat
    let h_for_vpd = |vpd: f64| 100.0 * (leaf_saturation - vpd) / air_saturation;
    let (band_min, band_max) = band;
    // Higher VPD = lower RH; the RH window is [h(max_vpd), h(min_vpd)].
    let rh_min = h_for_vpd(band_max).clamp(0.0, 100.0);
    let rh_max = h_for_vpd(band_min).clamp(0.0, 100.0);
    (rh_min, rh_max)
}

/// Whether an [`rh_window`] is empty: no RH keeps VPD in band at that
/// temperature (a cold room, or a band that is entirely out of reach).
/// Callers show a dash instead of "0–0 %".
#[must_use]
pub const fn rh_window_is_empty(window: (f64, f64)) -> bool {
    window.0 >= window.1
}

/// A recommended value band in volumetric percent, with the sources
/// behind its numbers (≥2 sources per the seed-data discipline).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BandPct {
    /// Lower edge, percent.
    pub min: f64,
    /// Upper edge, percent.
    pub max: f64,
    /// Where the numbers come from.
    pub sources: &'static str,
}

/// The soil-moisture recommended band default.
///
/// Container substrates irrigate around 40 % VWC and hold 50–60 % at
/// container capacity. Capacitive meters are calibration-dependent —
/// this is a default, user-editable with reset-to-default, not
/// gospel.
pub const SOIL_MOISTURE_BAND: BandPct = BandPct {
    min: 40.0,
    max: 60.0,
    sources: "HortScience 47(1):98 — peat substrates hold 50–60 % VWC \
        at container capacity; AcuRite soil-moisture guide — 40 %+ \
        for heavy-feeding plantings",
};

/// A recommended CO2 band in ppm, with the sources behind its numbers
/// (seed-data discipline: a label on every row).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BandPpm {
    /// Lower edge, ppm.
    pub min_ppm: f64,
    /// Upper edge, ppm.
    pub max_ppm: f64,
    /// Where the numbers come from.
    pub sources: &'static str,
}

/// The lights-on CO2 band (ppm): the enrichment target.
///
/// Extracted from the two independent enrichment articles in the
/// research pass (both named in the label): one gives 800–1,200 ppm as
/// the beneficial range (900–1,200 ppm research-backed; above
/// 1,500 ppm not economic), the other maintains 1,200–1,500 ppm. The
/// union is the encoded range and the ceiling disagreement is kept
/// visible in the label.
pub const CO2_LIGHTS_ON_PPM: BandPpm = BandPpm {
    min_ppm: 800.0,
    max_ppm: 1500.0,
    sources: "Cannabis Business Times 800–1,200 ppm (>1,500 uneconomic); \
        GrowWeedEasy 1,200–1,500 ppm — sources vary on the ceiling",
};

/// The lights-off CO2 band (ppm): ambient air, enrichment off.
///
/// Enrichment is useless in darkness (stomata close, photosynthesis
/// stops) and commercial practice injects from lights-on only, so the
/// lights-off target is plain ambient air. Ambient baseline reads
/// ~400–420 ppm across the sources; the room drifts up overnight from
/// respiration, so the band tops out at 450 ppm.
pub const CO2_LIGHTS_OFF_PPM: BandPpm = BandPpm {
    min_ppm: 400.0,
    max_ppm: 450.0,
    sources: "Oklahoma State Extension and Cannabis Business Times \
        (~400–410 ppm ambient); GrowWeedEasy ~400 ppm; CO2Meter — \
        enrichment off in darkness",
};

// ---------------------------------------------------------------------------
// Strain-tolerance threshold shifts (spec §4.4)
// ---------------------------------------------------------------------------
///
/// Spec §5.2: cold stress below ~15.6 °C / 60 °F; above ~29.4 °C /
/// 85 °F tolerable under strong light with low RH; above ~26.7 °C /
/// 80 °F in flower can mean airy buds.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TempThresholds {
    /// Lower alert threshold, °C.
    pub floor_c: f64,
    /// Upper alert threshold, °C.
    pub ceiling_c: f64,
}

impl Default for TempThresholds {
    fn default() -> Self {
        Self {
            floor_c: (60.0 - 32.0) * 5.0 / 9.0,   // 60 °F ≈ 15.6 °C
            ceiling_c: (80.0 - 32.0) * 5.0 / 9.0, // 80 °F ≈ 26.7 °C
        }
    }
}

impl TempThresholds {
    /// Shifts thresholds for the grow's heat tolerance (spec §4.4:
    /// tolerant +2 °F ceiling, sensitive −3 °F ceiling; the floor
    /// mirrors for cold tolerance).
    ///
    /// [`Must_use`]: pure function.
    #[must_use]
    pub fn shifted(self, heat_tolerance: Tolerance, cold_tolerance: Tolerance) -> Self {
        // Temperature *differences* convert without the 32 offset.
        let delta_f_to_c = |f: f64| f * 5.0 / 9.0;
        let mut result = self;
        match heat_tolerance {
            Tolerance::Tolerant => result.ceiling_c += delta_f_to_c(2.0),
            Tolerance::Sensitive => result.ceiling_c += delta_f_to_c(-3.0),
            Tolerance::Average => {}
        }
        match cold_tolerance {
            Tolerance::Tolerant => result.floor_c -= delta_f_to_c(2.0),
            Tolerance::Sensitive => result.floor_c += delta_f_to_c(3.0),
            Tolerance::Average => {}
        }
        result
    }
}

/// The temperature chart's Target Band, `(floor °C, ceiling °C)`.
///
/// The tolerance-shifted alert thresholds — spec §4.4 shifts on the
/// §5.2 default 60–80 °F thresholds. The same floor and ceiling the
/// environment snapshot's `temp_ok` checks, so the shaded band always
/// agrees with the sensor tile's verdict (env-section ticket 06).
#[must_use]
pub fn temperature_target_band(heat_tolerance: Tolerance, cold_tolerance: Tolerance) -> (f64, f64) {
    let thresholds = TempThresholds::default().shifted(heat_tolerance, cold_tolerance);
    (thresholds.floor_c, thresholds.ceiling_c)
}

// ---------------------------------------------------------------------------
// Lamp-height advisor (spec §4.7, §5.1)
// ---------------------------------------------------------------------------

/// The wattage-class height table (spec §5.1, inches from canopy).
/// Ranges per stage for a generic LED at full power.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct HeightRange {
    /// Minimum height, inches.
    pub min_in: f64,
    /// Maximum height, inches.
    pub max_in: f64,
}

/// The per-stage ranges for one wattage class.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WattageClass {
    /// Nominal real watts (the class label).
    pub watts: u32,
    /// Seedling heights.
    pub seedling: HeightRange,
    /// Veg heights.
    pub veg: HeightRange,
    /// Early-flower heights.
    pub early_flower: HeightRange,
    /// Late-flower heights.
    pub late_flower: HeightRange,
}

/// The spec §5.1 wattage-class table.
#[must_use]
pub fn wattage_classes() -> Vec<WattageClass> {
    vec![
        WattageClass {
            watts: 220,
            seedling: HeightRange {
                min_in: 24.0,
                max_in: 24.0,
            },
            veg: HeightRange {
                min_in: 18.0,
                max_in: 20.0,
            },
            early_flower: HeightRange {
                min_in: 14.0,
                max_in: 16.0,
            },
            late_flower: HeightRange {
                min_in: 12.0,
                max_in: 14.0,
            },
        },
        WattageClass {
            watts: 330,
            seedling: HeightRange {
                min_in: 28.0,
                max_in: 30.0,
            },
            veg: HeightRange {
                min_in: 20.0,
                max_in: 24.0,
            },
            early_flower: HeightRange {
                min_in: 16.0,
                max_in: 18.0,
            },
            late_flower: HeightRange {
                min_in: 14.0,
                max_in: 16.0,
            },
        },
        WattageClass {
            watts: 420,
            seedling: HeightRange {
                min_in: 32.0,
                max_in: 36.0,
            },
            veg: HeightRange {
                min_in: 24.0,
                max_in: 28.0,
            },
            early_flower: HeightRange {
                min_in: 18.0,
                max_in: 22.0,
            },
            late_flower: HeightRange {
                min_in: 16.0,
                max_in: 20.0,
            },
        },
        WattageClass {
            watts: 750,
            seedling: HeightRange {
                min_in: 30.0,
                max_in: 36.0,
            },
            veg: HeightRange {
                min_in: 24.0,
                max_in: 30.0,
            },
            early_flower: HeightRange {
                min_in: 20.0,
                max_in: 26.0,
            },
            late_flower: HeightRange {
                min_in: 18.0,
                max_in: 24.0,
            },
        },
    ]
}

/// The advisor's default fixture wattage (D044): the historical
/// sensible 330 W light, used when the grow records no real wattage.
pub const DEFAULT_FIXTURE_WATTS: u32 = 330;

/// Converts a grow's optional real wall watts (D076: `f64` in, `REAL`
/// in SQLite) into the advisor's whole-watt class input.
///
/// `None` falls back to the D044 default fixture; a recorded value
/// rounds to the nearest watt and never lands below 1.
#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
// the guard routes zero, negative and NaN inputs to the 1 W floor
// (NaN fails every comparison), and f64-to-u32 casts saturate,
// never wrap (see docs/DECISIONS.md D044)
pub fn advisor_watts(real_watts: Option<f64>) -> u32 {
    let Some(watts) = real_watts else {
        return DEFAULT_FIXTURE_WATTS;
    };
    let rounded = watts.round();
    if rounded.is_nan() || rounded < 1.0 {
        return 1;
    }
    rounded as u32
}

/// The advisor's inputs.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct LampInput {
    /// Real wall wattage of the fixture.
    pub real_watts: u32,
    /// Whether the fixture is dimmable.
    pub dimmable: bool,
    /// Current dim percentage (100 = full).
    pub dim_pct: u8,
    /// Current canopy temperature, °C (from HA or a manual reading).
    pub canopy_temperature_c: Option<f64>,
    /// The plant's current phase.
    pub phase: Phase,
    /// The grow's light tolerance.
    pub light_tolerance: Tolerance,
    /// Whether stretch symptoms are logged (raise the lamp).
    pub stretch_logged: bool,
    /// Whether light-stress tags (bleaching, canoeing) are logged
    /// (dim or raise).
    pub stress_tags_logged: bool,
}

/// The advisor's output.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LampAdvice {
    /// Recommended height range, inches.
    pub height: HeightRange,
    /// Recommended dim percentage (None = not dimmable or no change).
    pub dim_pct: Option<u8>,
    /// Target PPFD (approximate, from the wattage class).
    pub target_ppfd: Option<(u32, u32)>,
    /// Estimated DLI (mol/m²/day).
    pub dli: Option<f64>,
    /// Confidence of the advice: always "low" — the wattage-class
    /// table is the only source (no fixture map exists, D075).
    pub confidence: &'static str,
    /// Plain-English explanation.
    pub why: String,
    /// Stable codes for each sentence in `why`, in order. The frontend
    /// localises advice from these codes (`env.lamp_*` catalog keys);
    /// `why` stays as the English fallback for unknown codes.
    pub why_codes: Vec<&'static str>,
}

/// The minimum safe height floor (spec §4.7: default 10–12 in for
/// dimmable LEDs).
const HEIGHT_FLOOR_IN: f64 = 10.0;

/// Advises on lamp height and dimming (spec §4.7).
#[must_use]
pub fn advise_lamp(input: &LampInput) -> LampAdvice {
    // Find the nearest wattage class.
    let classes = wattage_classes();
    let class = classes
        .iter()
        .min_by_key(|candidate| candidate.watts.abs_diff(input.real_watts))
        .copied()
        .unwrap_or(classes[0]);

    let range = match input.phase {
        Phase::Germination | Phase::Seedling => class.seedling,
        Phase::Vegetative => class.veg,
        Phase::Flower => class.early_flower,
        Phase::Drying | Phase::Curing => class.late_flower,
    };

    let mut height = range;
    let mut dim_pct: Option<u8> = None;
    let mut why = String::new();
    let mut why_codes: Vec<&'static str> = Vec::new();

    // Dimming rule: ~50% dim lets the lamp hang ~30% closer.
    if input.dimmable && input.dim_pct <= 50 {
        height.min_in *= 0.7;
        height.max_in *= 0.7;
        why.push_str("Dimmed to 50%: the lamp can hang about 30% closer. ");
        why_codes.push("dimmed_closer");
    }

    // Temperature rule: canopy above ~29.4 °C (85 °F) → raise ~2 in or dim to ~80%.
    if let Some(temp) = input.canopy_temperature_c {
        let hot_c = f64::from(85_u8) * 5.0 / 9.0 - 17.78;
        if temp > hot_c {
            if input.dimmable {
                dim_pct = Some(80);
                why.push_str("Canopy is hot: dim to ~80% or raise ~2 in. ");
                why_codes.push("canopy_hot_dim");
            } else {
                height.min_in += 2.0;
                height.max_in += 2.0;
                why.push_str("Canopy is hot: raise the lamp ~2 in. ");
                why_codes.push("canopy_hot_raise");
            }
        }
    }

    // Stretch rule: during stretch, raise 2–4 in as plants grow.
    if input.stretch_logged {
        height.min_in += 2.0;
        height.max_in += 4.0;
        why.push_str("Stretch logged: raise 2–4 in as plants grow. ");
        why_codes.push("stretch_raise");
    }

    // Light-stress tags (bleaching/canoeing): +2–4 in or dim to 70–80%.
    if input.stress_tags_logged {
        if input.dimmable {
            dim_pct = Some(70);
            why.push_str("Bleaching or canoeing: dim to ~70–80% and re-check after a week. ");
            why_codes.push("stress_dim");
        } else {
            height.min_in += 2.0;
            height.max_in += 4.0;
            why.push_str("Bleaching or canoeing: raise 2–4 in and re-check after a week. ");
            why_codes.push("stress_raise");
        }
    }

    // Floor rule: never below the fixture's minimum distance.
    height.min_in = height.min_in.max(HEIGHT_FLOOR_IN);
    height.max_in = height.max_in.max(HEIGHT_FLOOR_IN);

    // Light-tolerance caution.
    if input.light_tolerance == Tolerance::Sensitive {
        why.push_str("This plant is light-sensitive: err toward the higher end. ");
        why_codes.push("light_sensitive");
    } else if input.light_tolerance == Tolerance::Tolerant {
        why.push_str("This plant is light-tolerant: the lower end may work. ");
        why_codes.push("light_tolerant");
    }

    // Target PPFD and DLI (approximate from the class).
    let target_ppfd = match input.phase {
        Phase::Germination | Phase::Seedling => Some((200_u32, 300)),
        Phase::Vegetative => Some((400, 600)),
        Phase::Flower => Some((600, 1000)),
        Phase::Drying | Phase::Curing => None,
    };
    let dli = target_ppfd.map(|(ppfd_min, _)| {
        // DLI = PPFD × hours × 0.0036; assume 18 h veg / 12 h flower.
        let hours = if input.phase == Phase::Flower {
            12.0
        } else {
            18.0
        };
        f64::from(ppfd_min) * hours * 0.0036
    });

    if why.is_empty() {
        why = "Set the distance by stage, then fine-tune with the dimmer.".into();
    }

    LampAdvice {
        height,
        dim_pct,
        target_ppfd,
        dli,
        confidence: "low",
        why,
        why_codes,
    }
}

// ---------------------------------------------------------------------------
// Phase coach (spec §4.10)
// ---------------------------------------------------------------------------

/// One coach tip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoachTip {
    /// What to check or do.
    pub title: String,
    /// Why it matters.
    pub why: String,
    /// Stable code naming the tip (frontend localisation key suffix,
    /// e.g. `vpd_band`, `coco_fertigate`, `curing_jars`). Unknown codes
    /// fall back to the English `title`/`why`.
    pub code: &'static str,
}

/// Generates phase-coach tips for the current state (spec §4.10).
#[must_use]
pub fn coach_tips(phase: Phase, substrate_label: &str, light_on: bool) -> Vec<CoachTip> {
    let mut tips = Vec::new();

    let (vpd_min, vpd_max) = vpd_band(phase);
    // Night guidance per the lights-off research (caveat C2): measured
    // VPD falls on its own as the room cools, so the fix is not "run
    // higher" — hold near the day band and watch the RH spike.
    tips.push(CoachTip {
        title: format!(
            "Keep VPD {vpd_min:.1}–{vpd_max:.1} kPa{}",
            if light_on {
                " (lights on)"
            } else {
                " (lights off: keep close to daytime; watch the RH spike)"
            }
        ),
        // The why sentence is gone (D137): the title carries the whole
        // message, so the fallback why is empty too.
        why: String::new(),
        code: "vpd_band",
    });

    match substrate_label {
        "coco" => {
            tips.push(CoachTip {
                title: "Fertigate at least once daily with 10–20% runoff".to_owned(),
                why: "In coco, keep the media wet and run 10–20% runoff to stop salt build-up."
                    .to_owned(),
                code: "coco_fertigate",
            });
            tips.push(CoachTip {
                title: "Check runoff EC every feed".to_owned(),
                why: "Compare input vs runoff EC: far above input (≥1000 ppm) suggests a flush."
                    .to_owned(),
                code: "coco_runoff_ec",
            });
        }
        "hydro" => {
            tips.push(CoachTip {
                title: "Log reservoir pH and EC trends".to_owned(),
                why: "In hydro, reservoir management replaces soil wet–dry cycles.".to_owned(),
                code: "hydro_reservoir",
            });
        }
        _ => {
            tips.push(CoachTip {
                title: "Water when the top 1–2 in of soil is dry".to_owned(),
                // The why sentence is gone (D137); the title stands alone.
                why: String::new(),
                code: "soil_wet_dry",
            });
        }
    }

    match phase {
        Phase::Flower => {
            tips.push(CoachTip {
                title: "Lower RH in late flower to protect from mould".to_owned(),
                why: "Late flower RH above ~62% at week 7+ is a mould risk.".to_owned(),
                code: "flower_late_rh",
            });
            tips.push(CoachTip {
                title: "Raise the lamp 2–4 in during stretch".to_owned(),
                why: "Plants can double in height during the first 1–3 weeks of flower.".to_owned(),
                code: "flower_stretch",
            });
        }
        Phase::Seedling => {
            tips.push(CoachTip {
                title: "Keep RH 60–80% for seedlings".to_owned(),
                why: "Young roots can't drink enough; high RH keeps them hydrated.".to_owned(),
                code: "seedling_rh",
            });
        }
        Phase::Drying => {
            tips.push(CoachTip {
                title: "Dry at 60–68 °F, RH 50% then ~55% until stems snap".to_owned(),
                why: "Slow, dark drying preserves terpenes.".to_owned(),
                code: "drying_conditions",
            });
        }
        Phase::Curing => {
            tips.push(CoachTip {
                title: "Jar at 58–62% RH; burp daily week 1, every 2–3 days week 2".to_owned(),
                why: "Curing at the right RH smooths the smoke and develops aroma.".to_owned(),
                code: "curing_jars",
            });
        }
        _ => {}
    }

    tips
}

#[cfg(test)]
mod tests {
    use super::{
        CO2_LIGHTS_OFF_PPM, CO2_LIGHTS_ON_PPM, DEFAULT_LEAF_OFFSET_C, LampInput,
        NIGHT_LEAF_OFFSET_C, NightVpdBand, Phase, TempThresholds, Tolerance, advise_lamp,
        advisor_watts, coach_tips, night_temperature_corridor, night_vpd_band, rh_window,
        rh_window_is_empty, saturation_vp_kpa, temperature_corridor, temperature_target_band,
        vpd_band, vpd_kpa, wattage_classes,
    };

    // --- VPD golden values (spec §13) ---

    /// Reference: well-known `VPD` table (e.g. Pulse/Alchimia guides).
    /// At 25 °C, 50% RH, leaf offset −2 °C → leaf at 23 °C:
    /// `VP_saturation`(23 °C) ≈ 2.81 kPa, `VP_actual`(25 °C, 50%) ≈ 1.57 kPa
    /// → VPD ≈ 1.24 kPa.
    #[test]
    fn vpd_golden_values() {
        let cases = [
            // (air °C, RH %, leaf offset, expected VPD kPa ± tolerance)
            (20.0, 50.0, -2.0, 0.88), // cool, moderate
            (25.0, 50.0, -2.0, 1.24), // the canonical mid-flower
            (25.0, 60.0, -2.0, 0.93), // higher RH = lower VPD
            (28.0, 40.0, -2.0, 1.91), // hot + dry = high VPD
            (30.0, 70.0, -2.0, 0.83), // hot + humid = moderate
            (22.0, 80.0, -2.0, 0.35), // cool + very humid = low
            // Saturated air with a negative leaf offset makes the raw
            // difference negative; the derivation clamps at zero (D087).
            (25.0, 100.0, -2.0, 0.0), // raw ≈ −0.36 kPa
            (20.0, 100.0, -2.0, 0.0), // raw ≈ −0.27 kPa
            (22.0, 95.0, -2.0, 0.0),  // raw ≈ −0.17 kPa
            // The lights-off offset is part of the same contract.
            (25.0, 50.0, NIGHT_LEAF_OFFSET_C, 1.40),
        ];
        for (temp, rh, offset, expected) in cases {
            let vpd = vpd_kpa(temp, rh, offset);
            assert!(
                (vpd - expected).abs() < 0.15,
                "vpd({temp}°C, {rh}%, {offset} offset) = {vpd:.3}, expected ≈ {expected}"
            );
        }
    }

    #[test]
    fn vpd_kpa_is_never_negative() {
        // The clamp is a contract for every consumer (charts, bands,
        // alerts): no air temperature, humidity, or leaf offset in the
        // realistic ranges may yield a negative VPD, however deep the
        // negative raw difference runs.
        let offsets = [0.0, NIGHT_LEAF_OFFSET_C, DEFAULT_LEAF_OFFSET_C, -3.0];
        for temp in [0.0, 5.0, 10.0, 15.0, 20.0, 25.0, 30.0, 35.0, 40.0] {
            for rh in [0.0, 25.0, 50.0, 75.0, 90.0, 99.0, 100.0] {
                for offset in offsets {
                    let vpd = vpd_kpa(temp, rh, offset);
                    assert!(
                        vpd >= 0.0,
                        "vpd({temp}°C, {rh}%, {offset}) = {vpd} must clamp at zero"
                    );
                }
            }
        }
    }

    #[test]
    fn saturation_vp_at_known_temperatures() {
        // 0 °C ≈ 0.611 kPa, 20 °C ≈ 2.34 kPa, 25 °C ≈ 3.17 kPa, 30 °C ≈ 4.24 kPa
        assert!((saturation_vp_kpa(0.0) - 0.611).abs() < 0.01);
        assert!((saturation_vp_kpa(20.0) - 2.34).abs() < 0.02);
        assert!((saturation_vp_kpa(25.0) - 3.17).abs() < 0.02);
        assert!((saturation_vp_kpa(30.0) - 4.24).abs() < 0.03);
    }

    #[test]
    fn leaf_offset_respected() {
        // Without the offset, VPD is lower (leaf is warmer = higher
        // saturation pressure at the leaf).
        let with_offset = vpd_kpa(25.0, 50.0, -2.0);
        let without = vpd_kpa(25.0, 50.0, 0.0);
        assert!(with_offset < without, "negative leaf offset must lower VPD");
        // The difference is the saturation-pressure delta.
        assert!((without - with_offset).abs() > 0.1);
    }

    #[test]
    fn rh_window_is_empty_when_the_room_is_too_cold() {
        // A freezing room: the saturation pressures are so small that
        // no RH reaches the seedling band, so the solver clamps to
        // (0, 0). (Seen live with a mock sensor reading −36 °C.)
        let cold = rh_window(-20.0, -2.0, (0.4, 0.8));
        assert_eq!(cold, (0.0, 0.0));
        assert!(rh_window_is_empty(cold));
        assert!(!rh_window_is_empty(rh_window(25.0, -2.0, (0.4, 0.8))));
    }

    #[test]
    fn rh_window_solver() {
        // At 25 °C, leaf offset −2 °C, seedling band 0.4–0.8 kPa,
        // the RH window should be roughly 65–80%.
        let (min, max) = rh_window(25.0, -2.0, (0.4, 0.8));
        assert!(min > 50.0 && min < 75.0, "RH min {min} should be ~65–70");
        assert!(max > 75.0 && max <= 100.0, "RH max {max} should be ~80–85");
        assert!(min < max, "RH window must be non-empty");

        // The window boundary VPDs should be near the band edges
        // (clamping to 0–100% can push them slightly outside).
        let vpd_at_min = vpd_kpa(25.0, min, -2.0);
        let vpd_at_max = vpd_kpa(25.0, max, -2.0);
        let in_band = vpd_kpa(25.0, f64::midpoint(min, max), -2.0);
        assert!(
            (0.4..=0.8).contains(&in_band),
            "midpoint VPD {in_band} must be in band"
        );
        assert!(
            (0.3..=0.9).contains(&vpd_at_min),
            "boundary min {vpd_at_min} near band"
        );
        assert!(
            (0.3..=0.9).contains(&vpd_at_max),
            "boundary max {vpd_at_max} near band"
        );
    }

    #[test]
    fn vpd_band_covers_all_phases() {
        for phase in Phase::all() {
            let (min, max) = vpd_band(phase);
            assert!(min > 0.0 && max > min, "band for {phase:?} invalid");
        }
    }

    // --- Strain tolerance shifts (spec §13) ---

    #[test]
    fn grow_tolerance_shifts_alert_thresholds() {
        let base = TempThresholds::default();

        let tolerant = base.shifted(Tolerance::Tolerant, Tolerance::Average);
        let sensitive = base.shifted(Tolerance::Sensitive, Tolerance::Average);
        let average = base.shifted(Tolerance::Average, Tolerance::Average);

        // Tolerant: +2 °F ceiling (≈ +1.1 °C).
        assert!(tolerant.ceiling_c > base.ceiling_c);
        assert!((tolerant.ceiling_c - base.ceiling_c - 1.11).abs() < 0.1);

        // Sensitive: −3 °F ceiling (≈ −1.7 °C).
        assert!(sensitive.ceiling_c < base.ceiling_c);
        assert!((base.ceiling_c - sensitive.ceiling_c - 1.67).abs() < 0.1);

        // Average: no shift.
        assert_eq!(average.ceiling_c, base.ceiling_c);

        // Cold tolerance mirrors.
        let cold_tolerant = base.shifted(Tolerance::Average, Tolerance::Tolerant);
        let cold_sensitive = base.shifted(Tolerance::Average, Tolerance::Sensitive);
        assert!(cold_tolerant.floor_c < base.floor_c);
        assert!(cold_sensitive.floor_c > base.floor_c);
    }

    // --- Temperature Target Band (env-section ticket 06) ---

    #[test]
    fn temperature_target_band_matches_the_alert_thresholds() {
        // Average tolerances must reproduce the plain thresholds: the
        // chart's band and the tile's temp_ok verdict use one source.
        let (min, max) = temperature_target_band(Tolerance::Average, Tolerance::Average);
        let base = TempThresholds::default();
        assert_eq!(min, base.floor_c);
        assert_eq!(max, base.ceiling_c);
    }

    #[test]
    fn temperature_target_band_shifts_with_both_tolerances() {
        let (base_min, base_max) = temperature_target_band(Tolerance::Average, Tolerance::Average);

        // Heat tolerance moves the ceiling (±): +2 °F ≈ +1.11 °C,
        // −3 °F ≈ −1.67 °C.
        let (_, heat_tolerant_max) =
            temperature_target_band(Tolerance::Tolerant, Tolerance::Average);
        let (_, heat_sensitive_max) =
            temperature_target_band(Tolerance::Sensitive, Tolerance::Average);
        assert!((heat_tolerant_max - base_max - 1.11).abs() < 0.01);
        assert!((heat_sensitive_max - base_max + 1.67).abs() < 0.01);

        // Cold tolerance mirrors on the floor.
        let (cold_tolerant_min, _) =
            temperature_target_band(Tolerance::Average, Tolerance::Tolerant);
        let (cold_sensitive_min, _) =
            temperature_target_band(Tolerance::Average, Tolerance::Sensitive);
        assert!((cold_tolerant_min - base_min + 1.11).abs() < 0.01);
        assert!((cold_sensitive_min - base_min - 1.67).abs() < 0.01);

        // Both shifts at once: doubly sensitive tightens both edges.
        let (min, max) = temperature_target_band(Tolerance::Sensitive, Tolerance::Sensitive);
        assert!(min > base_min && max < base_max);
    }

    #[test]
    fn temperature_target_band_stays_ordered_for_every_tolerance_pair() {
        let all = [
            Tolerance::Tolerant,
            Tolerance::Average,
            Tolerance::Sensitive,
        ];
        for heat in all {
            for cold in all {
                let (min, max) = temperature_target_band(heat, cold);
                assert!(min < max, "band edges inverted for {heat:?}/{cold:?}");
            }
        }
    }

    // --- Lamp advisor (spec §13) ---

    #[test]
    fn advisor_watts_rounds_and_falls_back_to_the_default_fixture() {
        // Unset: the D044 default fixture.
        assert_eq!(advisor_watts(None), 330);
        // A recorded value rounds to the nearest whole watt.
        assert_eq!(advisor_watts(Some(419.6)), 420);
        assert_eq!(advisor_watts(Some(240.0)), 240);
        // Degenerate stored values never produce a zero-watt lamp;
        // NaN cannot compare, so it must be routed by the guard too.
        assert_eq!(advisor_watts(Some(0.4)), 1);
        assert_eq!(advisor_watts(Some(-5.0)), 1);
        assert_eq!(advisor_watts(Some(f64::NAN)), 1);
    }

    #[test]
    fn lamp_advisor_wattage_table() {
        let classes = wattage_classes();
        assert!(classes.len() >= 4);

        let input = LampInput {
            real_watts: 330,
            dimmable: true,
            dim_pct: 100,
            canopy_temperature_c: None,
            phase: Phase::Vegetative,
            light_tolerance: Tolerance::Average,
            stretch_logged: false,
            stress_tags_logged: false,
        };
        let advice = advise_lamp(&input);
        assert!(advice.height.min_in >= 18.0, "330 W veg should be ≥18 in");
        assert!(advice.height.max_in <= 26.0, "330 W veg should be ≤26 in");
        assert_eq!(advice.confidence, "low");
    }

    #[test]
    fn lamp_advisor_dimming_lets_closer() {
        let base = LampInput {
            real_watts: 330,
            dimmable: true,
            dim_pct: 100,
            canopy_temperature_c: None,
            phase: Phase::Vegetative,
            light_tolerance: Tolerance::Average,
            stretch_logged: false,
            stress_tags_logged: false,
        };
        let dimmed = LampInput {
            dim_pct: 50,
            ..base
        };
        let base_advice = advise_lamp(&base);
        let dimmed_advice = advise_lamp(&dimmed);
        assert!(
            dimmed_advice.height.max_in < base_advice.height.max_in,
            "dimmed lamp should hang closer"
        );
    }

    #[test]
    fn lamp_advisor_hot_canopy() {
        let base = LampInput {
            real_watts: 330,
            dimmable: true,
            dim_pct: 100,
            canopy_temperature_c: None,
            phase: Phase::Flower,
            light_tolerance: Tolerance::Average,
            stretch_logged: false,
            stress_tags_logged: false,
        };
        let hot = LampInput {
            canopy_temperature_c: Some(31.0),
            ..base
        };
        let hot_advice = advise_lamp(&hot);
        assert!(
            hot_advice.dim_pct == Some(80)
                || hot_advice.height.min_in > advise_lamp(&base).height.min_in,
            "hot canopy should dim or raise"
        );
        assert!(
            hot_advice.why.contains("hot"),
            "why should mention the heat"
        );
    }

    #[test]
    fn lamp_advisor_stretch_raises() {
        let base = LampInput {
            real_watts: 330,
            dimmable: false,
            dim_pct: 100,
            canopy_temperature_c: None,
            phase: Phase::Flower,
            light_tolerance: Tolerance::Average,
            stretch_logged: false,
            stress_tags_logged: false,
        };
        let stretched = LampInput {
            stretch_logged: true,
            ..base
        };
        assert!(advise_lamp(&stretched).height.min_in > advise_lamp(&base).height.min_in);
    }

    #[test]
    fn lamp_advisor_floor_rule() {
        let input = LampInput {
            real_watts: 220,
            dimmable: true,
            dim_pct: 30,
            canopy_temperature_c: None,
            phase: Phase::Seedling,
            light_tolerance: Tolerance::Average,
            stretch_logged: false,
            stress_tags_logged: false,
        };
        let advice = advise_lamp(&input);
        assert!(
            advice.height.min_in >= 10.0,
            "never below the floor (10 in)"
        );
    }

    #[test]
    fn lamp_advisor_stress_tags() {
        let base = LampInput {
            real_watts: 330,
            dimmable: true,
            dim_pct: 100,
            canopy_temperature_c: None,
            phase: Phase::Flower,
            light_tolerance: Tolerance::Average,
            stretch_logged: false,
            stress_tags_logged: false,
        };
        let stressed = LampInput {
            stress_tags_logged: true,
            ..base
        };
        let advice = advise_lamp(&stressed);
        assert!(advice.why.contains("Bleaching") || advice.why.contains("dim"));
    }

    /// Localisation contract (loc ticket 02): every advice sentence
    /// carries a stable code, the codes line up one-to-one with the
    /// sentences, and no UI string embeds a spec citation (removed by
    /// the owner during keying).
    #[test]
    fn lamp_advice_codes_track_the_sentences() {
        let input = LampInput {
            real_watts: 330,
            dimmable: true,
            dim_pct: 40,
            canopy_temperature_c: Some(31.0),
            phase: Phase::Flower,
            light_tolerance: Tolerance::Sensitive,
            stretch_logged: true,
            stress_tags_logged: true,
        };
        let advice = advise_lamp(&input);
        assert_eq!(advice.why_codes.len(), 5);
        assert_eq!(
            advice.why_codes,
            vec![
                "dimmed_closer",
                "canopy_hot_dim",
                "stretch_raise",
                "stress_dim",
                "light_sensitive",
            ]
        );

        let bare = advise_lamp(&LampInput {
            real_watts: 330,
            dimmable: true,
            dim_pct: 100,
            canopy_temperature_c: None,
            phase: Phase::Vegetative,
            light_tolerance: Tolerance::Average,
            stretch_logged: false,
            stress_tags_logged: false,
        });
        // The starting-points why code is gone (D137): no sentence, no
        // marker code.
        assert_eq!(bare.why_codes, Vec::<&str>::new());

        assert!(!advice.why.contains("spec §"), "no citations in UI strings");
    }

    // --- Phase coach (spec §4.10) ---

    #[test]
    fn coach_gives_substrate_aware_tips() {
        let soil = coach_tips(Phase::Vegetative, "soil", true);
        let coco = coach_tips(Phase::Vegetative, "coco", true);
        assert!(soil.iter().any(|t| t.title.contains("dry")));
        assert!(coco.iter().any(|t| t.title.contains("runoff")));
        assert!(coco.len() != soil.len() || coco.iter().any(|t| !soil.contains(t)));
    }

    #[test]
    fn coach_flower_mentions_mould_and_stretch() {
        let tips = coach_tips(Phase::Flower, "soil", true);
        assert!(tips.iter().any(|t| t.title.contains("mould")));
        assert!(tips.iter().any(|t| t.title.contains("stretch")));
    }

    /// Localisation contract (loc ticket 02): every tip names its
    /// catalog key via a stable code, the VPD tip's code is stable
    /// across light states, and no why carries a spec citation.
    #[test]
    fn coach_tips_carry_codes_without_citations() {
        for (phase, substrate, light_on) in [
            (Phase::Germination, "soil", true),
            (Phase::Seedling, "coco", false),
            (Phase::Vegetative, "hydro", true),
            (Phase::Flower, "soil", true),
            (Phase::Drying, "soil", false),
            (Phase::Curing, "coco", true),
        ] {
            let tips = coach_tips(phase, substrate, light_on);
            assert!(
                !tips.is_empty(),
                "every phase/substrate combination yields tips"
            );
            for tip in &tips {
                assert!(!tip.code.is_empty(), "every tip carries a code");
                assert!(!tip.why.contains("spec §"), "no citations in UI strings");
            }
        }
        let day = coach_tips(Phase::Vegetative, "soil", true)
            .into_iter()
            .find(|tip| tip.code == "vpd_band")
            .expect("VPD tip always present");
        let night = coach_tips(Phase::Vegetative, "soil", false)
            .into_iter()
            .find(|tip| tip.code == "vpd_band")
            .expect("VPD tip always present");
        assert_eq!(day.code, night.code);
        assert!(day.title.contains("(lights on)"));
        assert!(night.title.contains("(lights off:"));
    }

    /// The tip selection by light state (ticket 08): lights-on keeps
    /// the plain band wording; lights-off carries the researched night
    /// guidance — close to daytime, watch the RH spike — and never
    /// tells the grower to "run higher".
    #[test]
    fn coach_vpd_tip_matches_the_light_state() {
        let day = coach_tips(Phase::Vegetative, "soil", true);
        let vpd_day = day
            .iter()
            .find(|tip| tip.title.contains("Keep VPD"))
            .unwrap_or_else(|| panic!("veg tips must carry the VPD tip"));
        assert!(vpd_day.title.contains("(lights on)"));
        assert!(!vpd_day.title.contains("run higher"));

        let night = coach_tips(Phase::Vegetative, "soil", false);
        let vpd_night = night
            .iter()
            .find(|tip| tip.title.contains("Keep VPD"))
            .unwrap_or_else(|| panic!("veg tips must carry the VPD tip"));
        assert!(
            vpd_night
                .title
                .contains("(lights off: keep close to daytime; watch the RH spike)")
        );
        assert!(
            !vpd_night.title.contains("run higher"),
            "the research contradicts 'run higher' at night (caveat C2)"
        );
        // The band numbers themselves are the daytime band in both
        // states — night guidance is "hold near the day band".
        let band_part =
            |tip: &super::CoachTip| tip.title.split(" (").next().unwrap_or_default().to_owned();
        assert_eq!(band_part(vpd_day), band_part(vpd_night));
    }

    // --- Temperature corridor (spec §5.2, seed data) ---

    #[test]
    fn temperature_corridor_covers_every_phase_with_sourced_ranges() {
        for phase in Phase::all() {
            let band = temperature_corridor(phase);
            assert!(
                band.min_c < band.max_c,
                "{} range must be non-empty",
                phase.label()
            );
            assert!(
                !band.source.is_empty(),
                "{} must carry a source label",
                phase.label()
            );
        }
    }

    #[test]
    fn temperature_corridor_matches_the_spec_section_5_2_table() {
        // Worked values from the §5.2 lights-on ranges (°F → °C).
        let seedling = temperature_corridor(Phase::Seedling);
        assert!((seedling.min_c - 21.1).abs() < 0.05);
        assert!((seedling.max_c - 26.7).abs() < 0.05);
        assert!((temperature_corridor(Phase::Vegetative).max_c - 29.4).abs() < 0.05);
        let flower = temperature_corridor(Phase::Flower);
        assert!((flower.min_c - 18.3).abs() < 0.05);
        assert!((flower.max_c - 26.7).abs() < 0.05);
        let drying = temperature_corridor(Phase::Drying);
        assert!((drying.min_c - 15.6).abs() < 0.05);
        assert!((drying.max_c - 20.0).abs() < 0.05);
        // Germination has no §5.2 row: it reuses the seedling/clone
        // range, and the source label says so.
        assert_eq!(temperature_corridor(Phase::Germination), seedling);
        assert!(seedling.source.contains("germination reuses"));
    }

    // --- Lights-off (night) presets: temperature (spec §4.9) ---

    #[test]
    fn night_temperature_corridor_covers_lit_phases_with_sourced_ranges() {
        for phase in [
            Phase::Germination,
            Phase::Seedling,
            Phase::Vegetative,
            Phase::Flower,
        ] {
            let band = night_temperature_corridor(phase)
                .unwrap_or_else(|| panic!("{} must have a lights-off range", phase.label()));
            assert!(
                band.min_c < band.max_c,
                "{} lights-off range must be non-empty",
                phase.label()
            );
            assert!(
                !band.source.is_empty(),
                "{} lights-off range must carry a source label",
                phase.label()
            );
        }
        // Drying and curing rooms have no lights, so there is no
        // lights-off variant: the single preset applies 24/7 and the
        // accessor says so with None.
        for phase in [Phase::Drying, Phase::Curing] {
            assert!(
                night_temperature_corridor(phase).is_none(),
                "{} must have no lights-off variant",
                phase.label()
            );
        }
    }

    #[test]
    fn night_temperature_corridor_matches_the_research_table() {
        // Worked values from the lights-off research (°F → °C).
        let seedling = night_temperature_corridor(Phase::Seedling).expect("seedling");
        assert!((seedling.min_c - 20.0).abs() < 0.05);
        assert!((seedling.max_c - 23.3).abs() < 0.05);
        // Germination has no research row of its own: it reuses the
        // seedling row, and the source label says so.
        assert_eq!(
            night_temperature_corridor(Phase::Germination),
            Some(seedling)
        );
        assert!(seedling.source.contains("germination reuses"));
        let vegetative = night_temperature_corridor(Phase::Vegetative).expect("veg");
        assert!((vegetative.min_c - 18.3).abs() < 0.05);
        assert!((vegetative.max_c - 23.9).abs() < 0.05);
        let flower = night_temperature_corridor(Phase::Flower).expect("flower");
        assert!((flower.min_c - 15.6).abs() < 0.05);
        assert!((flower.max_c - 23.3).abs() < 0.05);
    }

    #[test]
    fn night_temperature_sits_below_the_lights_on_corridor() {
        // Nights run cooler than the lights-on corridor for every lit
        // phase; the dark-room phases have no night range at all.
        for phase in [
            Phase::Germination,
            Phase::Seedling,
            Phase::Vegetative,
            Phase::Flower,
        ] {
            let day = temperature_corridor(phase);
            let night = night_temperature_corridor(phase)
                .unwrap_or_else(|| panic!("{} must have a lights-off range", phase.label()));
            assert!(
                night.max_c <= day.max_c,
                "{}: night ceiling {:.1} must not exceed the lights-on ceiling {:.1}",
                phase.label(),
                night.max_c,
                day.max_c
            );
        }
    }

    // --- Lights-off (night) presets: VPD (spec §4.9) ---

    #[test]
    fn night_vpd_band_covers_lit_phases_with_sourced_ranges() {
        for phase in [
            Phase::Germination,
            Phase::Seedling,
            Phase::Vegetative,
            Phase::Flower,
        ] {
            let band = night_vpd_band(phase)
                .unwrap_or_else(|| panic!("{} must have a lights-off VPD band", phase.label()));
            assert!(
                band.min_kpa > 0.0 && band.max_kpa > band.min_kpa,
                "{} lights-off VPD band must be a positive non-empty range",
                phase.label()
            );
            assert!(
                !band.sources.is_empty(),
                "{} lights-off VPD band must carry a source label",
                phase.label()
            );
        }
        // No lights, no lights-off period: the single day band applies
        // 24/7 in drying and curing.
        for phase in [Phase::Drying, Phase::Curing] {
            assert!(
                night_vpd_band(phase).is_none(),
                "{} must have no lights-off VPD variant",
                phase.label()
            );
        }
    }

    #[test]
    fn night_vpd_band_matches_the_research_table() {
        let seedling = night_vpd_band(Phase::Seedling).expect("seedling");
        assert!((seedling.min_kpa - 0.5).abs() < 0.01);
        assert!((seedling.max_kpa - 0.9).abs() < 0.01);
        // Germination shares the seedling/clones row.
        assert_eq!(night_vpd_band(Phase::Germination), Some(seedling));
        // The dome numbers disagree across sources; the label must say
        // so (seed-data discipline).
        assert!(seedling.sources.contains("sources vary"));

        let vegetative = night_vpd_band(Phase::Vegetative).expect("veg");
        assert!((vegetative.min_kpa - 0.8).abs() < 0.01);
        assert!((vegetative.max_kpa - 1.2).abs() < 0.01);

        let flower = night_vpd_band(Phase::Flower).expect("flower");
        assert!((flower.min_kpa - 1.0).abs() < 0.01);
        assert!((flower.max_kpa - 1.4).abs() < 0.01);
    }

    #[test]
    fn night_vpd_band_overlaps_the_day_band() {
        // The sources' shared rule is "keep night VPD close to the day
        // band" (Pulse Grow, Black Dog LED), so every lights-off band
        // must overlap its day counterpart.
        for phase in [
            Phase::Germination,
            Phase::Seedling,
            Phase::Vegetative,
            Phase::Flower,
        ] {
            let night = night_vpd_band(phase)
                .unwrap_or_else(|| panic!("{} must have a lights-off VPD band", phase.label()));
            let NightVpdBand {
                min_kpa: night_min,
                max_kpa: night_max,
                ..
            } = night;
            let (day_min, day_max) = vpd_band(phase);
            assert!(
                night_min <= day_max && day_min <= night_max,
                "{}: night {:.1}–{:.1} must overlap day {:.1}–{:.1}",
                phase.label(),
                night_min,
                night_max,
                day_min,
                day_max
            );
        }
    }

    // --- Lights-on / lights-off CO2 bands (spec §4.9) ---

    #[test]
    fn co2_bands_carry_sourced_ranges() {
        // Lights-off: ambient only, enrichment off in darkness.
        assert!((CO2_LIGHTS_OFF_PPM.min_ppm - 400.0).abs() < 0.5);
        assert!((CO2_LIGHTS_OFF_PPM.max_ppm - 450.0).abs() < 0.5);
        assert!(
            CO2_LIGHTS_OFF_PPM.sources.contains("enrichment off"),
            "the night label must state the enrichment-off convention"
        );
        // Lights-on: the enrichment range, extracted from two
        // independent articles.
        assert!((CO2_LIGHTS_ON_PPM.min_ppm - 800.0).abs() < 0.5);
        assert!((CO2_LIGHTS_ON_PPM.max_ppm - 1500.0).abs() < 0.5);
        assert!(
            CO2_LIGHTS_ON_PPM.sources.contains("sources vary"),
            "the two articles disagree on the ceiling; the label must say so"
        );
        // Ambient air never reaches the enrichment floor, so the
        // lights-off band sits entirely below the lights-on band; the
        // gap must be a real margin, not a shared edge.
        let gap = CO2_LIGHTS_ON_PPM.min_ppm - CO2_LIGHTS_OFF_PPM.max_ppm;
        assert!(
            gap > 0.0,
            "night ambient band must sit below the day enrichment band"
        );
    }

    // --- Lights-off leaf offset (spec §4.9) ---

    #[test]
    fn night_leaf_offset_is_the_documented_constant() {
        assert!((NIGHT_LEAF_OFFSET_C - -1.0).abs() < 1e-9);
    }

    #[test]
    fn night_leaf_offset_swap_is_visible_in_the_maths() {
        // The lights-off offset assumes less transpiration cooling, so
        // the leaf sits closer to air temperature and the computed VPD
        // rises relative to the lights-on default. The difference must
        // exceed rounding noise (research: ~0.2–0.3 kPa at typical
        // setpoints).
        let with_default = vpd_kpa(25.0, 50.0, DEFAULT_LEAF_OFFSET_C);
        let with_night = vpd_kpa(25.0, 50.0, NIGHT_LEAF_OFFSET_C);
        assert!(
            with_night > with_default,
            "less cooling at night must raise the computed VPD"
        );
        assert!(
            with_night - with_default > 0.1,
            "the offset swap must move VPD by more than {:.3}",
            with_night - with_default
        );
    }

    #[test]
    fn night_rh_window_solver_round_trip() {
        // At each lit phase's night setpoint (corridor midpoint) with
        // the night leaf offset, the lights-off VPD band converts to a
        // non-empty RH window whose midpoint lands back in band — the
        // solver and the band agree on the night numbers.
        let cases = [
            (Phase::Seedling, 20.0, 23.3),
            (Phase::Vegetative, 18.3, 23.9),
            (Phase::Flower, 15.6, 23.3),
        ];
        for (phase, min_c, max_c) in cases {
            let night = night_vpd_band(phase)
                .unwrap_or_else(|| panic!("{} must have a lights-off VPD band", phase.label()));
            let setpoint = f64::midpoint(min_c, max_c);
            let (lo, hi) = rh_window(
                setpoint,
                NIGHT_LEAF_OFFSET_C,
                (night.min_kpa, night.max_kpa),
            );
            assert!(
                lo < hi,
                "{}: lights-off RH window must be non-empty at {:.1} °C",
                phase.label(),
                setpoint
            );
            let midpoint_rh = f64::midpoint(lo, hi);
            let back = vpd_kpa(setpoint, midpoint_rh, NIGHT_LEAF_OFFSET_C);
            assert!(
                (night.min_kpa..=night.max_kpa).contains(&back),
                "{}: midpoint RH {:.1} % round-trips to VPD {:.2}, outside {:.1}–{:.1}",
                phase.label(),
                midpoint_rh,
                back,
                night.min_kpa,
                night.max_kpa
            );
        }
    }
}
