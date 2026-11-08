//! VPD and environment engines (spec §4.9, §4.10, §4.4, M7).
//!
//! VPD (vapour-pressure deficit) from air temperature + RH with a
//! configurable leaf-temperature offset; the RH-window solver finds
//! the humidity range that keeps VPD in band at the current
//! temperature. The lamp-height advisor applies the spec §5.1
//! wattage-class table with dimming, stretch and floor rules, plus a
//! strain light-tolerance caution. Strain heat/cold tolerance shifts
//! alert thresholds (tolerant +2 °F ceiling, sensitive −3 °F).

use serde::{Deserialize, Serialize};

use crate::Phase;
use crate::strain::Tolerance;

// ---------------------------------------------------------------------------
// VPD engine (spec §4.9, §5.2)
// ---------------------------------------------------------------------------

/// Default leaf-temperature offset: −2 °C / −3.6 °F (spec §4.9).
pub const DEFAULT_LEAF_OFFSET_C: f64 = -2.0;

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

// ---------------------------------------------------------------------------
// Strain-tolerance threshold shifts (spec §4.4)
// ---------------------------------------------------------------------------

/// Default temperature alert thresholds in °C.
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
    /// Shifts thresholds for the strain's heat tolerance (spec §4.4:
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
    /// The strain's light tolerance.
    pub light_tolerance: Tolerance,
    /// Whether stretch symptoms are logged (raise the lamp).
    pub stretch_logged: bool,
    /// Whether bleaching/taco/crispy tags are logged (dim or raise).
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
    /// Confidence: "high" with a PPFD map, "low" from the wattage
    /// table.
    pub confidence: &'static str,
    /// Plain-English explanation.
    pub why: String,
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

    // Dimming rule: ~50% dim lets the lamp hang ~30% closer.
    if input.dimmable && input.dim_pct <= 50 {
        height.min_in *= 0.7;
        height.max_in *= 0.7;
        why.push_str("Dimmed to 50%: the lamp can hang about 30% closer. ");
    }

    // Temperature rule: canopy above ~29.4 °C (85 °F) → raise ~2 in or dim to ~80%.
    if let Some(temp) = input.canopy_temperature_c {
        let hot_c = f64::from(85_u8) * 5.0 / 9.0 - 17.78;
        if temp > hot_c {
            if input.dimmable {
                dim_pct = Some(80);
                why.push_str("Canopy is hot: dim to ~80% or raise ~2 in. ");
            } else {
                height.min_in += 2.0;
                height.max_in += 2.0;
                why.push_str("Canopy is hot: raise the lamp ~2 in. ");
            }
        }
    }

    // Stretch rule: during stretch, raise 2–4 in as plants grow.
    if input.stretch_logged {
        height.min_in += 2.0;
        height.max_in += 4.0;
        why.push_str("Stretch logged: raise 2–4 in as plants grow. ");
    }

    // Stress tags (bleaching/taco/crispy): +2–4 in or dim to 70–80%.
    if input.stress_tags_logged {
        if input.dimmable {
            dim_pct = Some(70);
            why.push_str("Bleaching/taco/crispy: dim to ~70–80% and re-check after a week. ");
        } else {
            height.min_in += 2.0;
            height.max_in += 4.0;
            why.push_str("Bleaching/taco/crispy: raise 2–4 in and re-check after a week. ");
        }
    }

    // Floor rule: never below the fixture's minimum distance.
    height.min_in = height.min_in.max(HEIGHT_FLOOR_IN);
    height.max_in = height.max_in.max(HEIGHT_FLOOR_IN);

    // Light-tolerance caution.
    if input.light_tolerance == Tolerance::Sensitive {
        why.push_str("This strain is light-sensitive: err toward the higher end. ");
    } else if input.light_tolerance == Tolerance::Tolerant {
        why.push_str("This strain is light-tolerant: the lower end may work. ");
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
        why = "Set the distance by stage, then fine-tune with the dimmer. These are starting points — a PAR meter calibrates them.".into();
    } else {
        why.push_str(
            "These are starting points — a PAR meter or a PPFD-meter log entry calibrates them.",
        );
    }

    LampAdvice {
        height,
        dim_pct,
        target_ppfd,
        dli,
        confidence: "low",
        why,
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
    /// Why it matters (with a source note).
    pub why: String,
}

/// Generates phase-coach tips for the current state (spec §4.10).
#[must_use]
pub fn coach_tips(phase: Phase, substrate_label: &str, light_on: bool) -> Vec<CoachTip> {
    let mut tips = Vec::new();

    let (vpd_min, vpd_max) = vpd_band(phase);
    tips.push(CoachTip {
        title: format!("Keep VPD {vpd_min:.1}–{vpd_max:.1} kPa{}", if light_on { " (lights on)" } else { " (lights off: run higher)" }),
        why: "VPD drives transpiration and nutrient uptake; the band shifts by phase and lights state (spec §5.2).".to_owned(),
    });

    match substrate_label {
        "coco" => {
            tips.push(CoachTip {
                title: "Fertigate at least once daily with 10–20% runoff".to_owned(),
                why: "In coco, keep the media wet and run 10–20% runoff to stop salt build-up (spec §4.8).".to_owned(),
            });
            tips.push(CoachTip {
                title: "Check runoff EC every feed".to_owned(),
                why: "Compare input vs runoff EC: far above input (≥1000 ppm) suggests a flush (spec §5.3).".to_owned(),
            });
        }
        "hydro" => {
            tips.push(CoachTip {
                title: "Log reservoir pH and EC trends".to_owned(),
                why: "In hydro, reservoir management replaces soil wet–dry cycles (spec §5.3)."
                    .to_owned(),
            });
        }
        _ => {
            tips.push(CoachTip {
                title: "Water when the top 1–2 in of soil is dry".to_owned(),
                why: "Soil prefers a wet–dry cycle; overwatering suffocates roots (spec §5.3)."
                    .to_owned(),
            });
        }
    }

    match phase {
        Phase::Flower => {
            tips.push(CoachTip {
                title: "Lower RH in late flower to protect from mould".to_owned(),
                why: "Late flower RH above ~62% at week 7+ is a mould risk (spec §4.9).".to_owned(),
            });
            tips.push(CoachTip {
                title: "Raise the lamp 2–4 in during stretch".to_owned(),
                why:
                    "Plants can double in height during the first 1–3 weeks of flower (spec §4.7)."
                        .to_owned(),
            });
        }
        Phase::Seedling => {
            tips.push(CoachTip {
                title: "Keep RH 60–80% for seedlings".to_owned(),
                why: "Young roots can't drink enough; high RH keeps them hydrated (spec §5.2)."
                    .to_owned(),
            });
        }
        Phase::Drying => {
            tips.push(CoachTip {
                title: "Dry at 60–68 °F, RH 50% then ~55% until stems snap".to_owned(),
                why: "Slow, dark drying preserves terpenes (spec §5.2).".to_owned(),
            });
        }
        Phase::Curing => {
            tips.push(CoachTip {
                title: "Jar at 58–62% RH; burp daily week 1, every 2–3 days week 2".to_owned(),
                why: "Curing at the right RH smooths the smoke and develops aroma (spec §5.2)."
                    .to_owned(),
            });
        }
        _ => {}
    }

    tips
}

#[cfg(test)]
mod tests {
    use super::{
        LampInput, Phase, TempThresholds, Tolerance, advise_lamp, coach_tips, rh_window,
        saturation_vp_kpa, vpd_band, vpd_kpa, wattage_classes,
    };
    use crate::strain::PhotoperiodType;

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
    fn strain_tolerance_shifts_alert_thresholds() {
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

    // --- Lamp advisor (spec §13) ---

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

    // Compile check that PhotoperiodType is reachable.
    #[test]
    fn photoperiod_type_available() {
        let _ = PhotoperiodType::Photoperiod;
    }
}
