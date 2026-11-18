//! Environment, lamp advisor and phase coach API routes (M7). All
//! engine inputs are grow attributes (D075) — no strain record or
//! space equipment is consulted.

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use leaflog_core::Phase;
use leaflog_core::env::{
    CoachTip, LampAdvice, LampInput, TempThresholds, advise_lamp, advisor_watts, coach_tips,
    rh_window, vpd_band, vpd_kpa,
};
use serde::Serialize;

use crate::api::ApiError;
use crate::app::AppState;
use crate::store;

/// The env/lamp/coach routes.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/grows/{id}/env", get(grow_env))
        .route("/api/grows/{id}/lamp", get(grow_lamp))
        .route("/api/grows/{id}/coach", get(grow_coach))
}

/// The environment snapshot for a grow.
#[derive(Debug, Clone, Serialize)]
pub struct EnvSnapshot {
    /// Per-binding drift status (spec §4.1): a binding whose entity is
    /// unavailable is reported here so the UI shows a chip and the
    /// rebind affordance instead of silently missing data.
    pub bindings: Vec<BindingStatus>,
    /// Air temperature, °C (from bound sensor; None when offline).
    pub temperature_c: Option<f64>,
    /// Relative humidity, %.
    pub humidity_pct: Option<f64>,
    /// Computed VPD, kPa.
    pub vpd_kpa: Option<f64>,
    /// The VPD band for the current phase, kPa.
    pub vpd_band: (f64, f64),
    /// The RH window that keeps VPD in band.
    pub rh_window: (f64, f64),
    /// Temperature alert thresholds (shifted by the grow's heat and
    /// cold tolerance, D075).
    pub thresholds: TempThresholds,
    /// Current phase label.
    pub phase: String,
    /// Whether the temperature is in range.
    pub temp_ok: Option<bool>,
    /// Whether the VPD is in range.
    pub vpd_ok: Option<bool>,
}

/// One bound entity's drift status.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BindingStatus {
    /// Binding role (`temperature`, `humidity`, ...).
    pub role: String,
    /// The bound HA entity id.
    pub entity_id: String,
    /// Whether the entity currently reports a usable value. A dead or
    /// renamed entity (HA sends `unavailable`/`unknown`) is `false`,
    /// and everything downstream degrades to "no data" — never "in
    /// range".
    pub available: bool,
}

/// An entity counts as available when it is tracked and its state is
/// neither HA's `unavailable` nor `unknown`.
fn entity_available(entity: Option<&crate::ha::LiveEntity>) -> bool {
    entity.is_some_and(|live| live.state != "unavailable" && live.state != "unknown")
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn grow_env(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<EnvSnapshot>, ApiError> {
    let tz = crate::app::canonical_timezone(&state).await;
    let live = state.ha.live_now();
    let response = state
        .db
        .with(move |conn| snapshot(conn, &id, &live, &tz))
        .await?;
    Ok(Json(response))
}

/// Computes a grow's environment snapshot (shared by the env route and
/// the §4.9 alert evaluator).
pub(crate) fn snapshot(
    conn: &rusqlite::Connection,
    id: &str,
    live: &crate::ha::LiveData,
    tz: &str,
) -> Result<EnvSnapshot, ApiError> {
    {
        let grow = store::grows::get(conn, id)?;

        // The phase the grow is in today, from the re-anchored
        // forecast (D073). Local calendar days in the canonical zone
        // (spec §2); UTC math is the fallback for unknown zones.
        let today =
            crate::app::day_number_in(&grow.started_on, &crate::db::now_rfc3339(), tz).unwrap_or(0);
        let phase = crate::api_phase::current_phase(conn, &grow, today)?;

        // Temperature and humidity from the bound sensors, with
        // each binding's drift status beside them (spec §4.1).
        let bindings = store::sensors::list(conn, id)?;
        let temp_entity = bindings.get("temperature");
        let humidity_entity = bindings.get("humidity");
        let drift = bindings
            .iter()
            .map(|(role, entity_id)| BindingStatus {
                role: role.clone(),
                entity_id: entity_id.clone(),
                available: entity_available(live.entities.get(entity_id)),
            })
            .collect::<Vec<_>>();
        let temperature_c = temp_entity
            .and_then(|e| live.entities.get(e))
            .and_then(|e| e.state.parse::<f64>().ok());
        let humidity_pct = humidity_entity
            .and_then(|e| live.entities.get(e))
            .and_then(|e| e.state.parse::<f64>().ok());

        // VPD (needs both temp and RH).
        let vpd = match (temperature_c, humidity_pct) {
            (Some(t), Some(h)) => Some(vpd_kpa(t, h, leaflog_core::env::DEFAULT_LEAF_OFFSET_C)),
            _ => None,
        };

        let band = vpd_band(phase);
        let rh_w = rh_window(
            temperature_c.unwrap_or(25.0),
            leaflog_core::env::DEFAULT_LEAF_OFFSET_C,
            band,
        );

        // Grow-shifted thresholds (D075): unset tolerances are
        // Average, so a fresh grow needs no tuning.
        let thresholds =
            TempThresholds::default().shifted(grow.heat_tolerance, grow.cold_tolerance);

        let temp_ok = temperature_c.map(|t| t >= thresholds.floor_c && t <= thresholds.ceiling_c);
        let vpd_ok = vpd.map(|v| v >= band.0 && v <= band.1);

        Ok(EnvSnapshot {
            bindings: drift,
            temperature_c,
            humidity_pct,
            vpd_kpa: vpd,
            vpd_band: band,
            rh_window: rh_w,
            thresholds,
            phase: phase.label().to_owned(),
            temp_ok,
            vpd_ok,
        })
    }
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn grow_lamp(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<LampAdvice>, ApiError> {
    let tz = crate::app::canonical_timezone(&state).await;
    let live = state.ha.live_now();
    let advice = state
        .db
        .with(move |conn| -> Result<LampAdvice, ApiError> {
            let grow = store::grows::get(conn, &id)?;

            // Phase from the re-anchored forecast (D073).
            // Local calendar days in the canonical zone (spec §2);
            // UTC math is the fallback for unknown zones.
            let today = crate::app::day_number_in(&grow.started_on, &crate::db::now_rfc3339(), &tz)
                .unwrap_or(0);
            let phase = crate::api_phase::current_phase(conn, &grow, today)?;

            // Canopy temp from the bound temperature sensor.
            let bindings = store::sensors::list(conn, &id)?;
            let canopy_temp = bindings
                .get("temperature")
                .and_then(|e| live.entities.get(e))
                .and_then(|e| e.state.parse::<f64>().ok());

            // The grow's own lamp facts (D075) advise the wattage
            // class; unset watts fall back to the historical sensible
            // default (docs/DECISIONS.md D044).

            // Stretch/stress from recent appearance tags. The matchers
            // live in core (leaflog_core::appearance) so the merged
            // vocabulary and the old spellings in stored entries are
            // treated alike.
            let recent = store::actions::list(conn, &id, None, 30)?;
            let stretch = recent.iter().any(|entry| {
                entry
                    .appearance_tags
                    .iter()
                    .any(|t| leaflog_core::appearance::is_stretch_tag(t))
            });
            let stress = recent.iter().any(|entry| {
                entry
                    .appearance_tags
                    .iter()
                    .any(|t| leaflog_core::appearance::is_light_stress_tag(t))
            });

            let input = LampInput {
                real_watts: advisor_watts(grow.real_watts),
                dimmable: grow.dimmable,
                dim_pct: 100,
                canopy_temperature_c: canopy_temp,
                phase,
                light_tolerance: grow.light_tolerance,
                stretch_logged: stretch,
                stress_tags_logged: stress,
            };
            Ok(advise_lamp(&input))
        })
        .await?;
    Ok(Json(advice))
}

/// The coach response.
#[derive(Debug, Clone, Serialize)]
pub struct CoachResponse {
    /// The current phase label.
    pub phase: String,
    /// Tips for this phase.
    pub tips: Vec<CoachTip>,
    /// Recommended equipment categories.
    pub equipment_suggestions: Vec<String>,
    /// The light state the tips were computed for, so the frontend can
    /// localise the VPD tip's lights-on/lights-off wording.
    pub light_on: bool,
}

/// Reads one raw light state the way the chart staircase does:
/// `"on"` (any case) is lit, `"off"` is dark, and a numeric state
/// counts as lit by the core rule — any positive value, D093.
/// Anything else is unknown.
fn light_state_is_on(state: &str) -> Option<bool> {
    match state.trim().to_ascii_lowercase().as_str() {
        "on" => Some(true),
        "off" => Some(false),
        _ => state
            .trim()
            .parse::<f64>()
            .ok()
            .map(leaflog_core::chart::light_value_is_on),
    }
}

/// Resolves the coach's light state at call time (spec §4.9, D096):
///
/// 1. **A bound light entity's latest known state wins.** The live
///    entity map first (the WebSocket subscription's current
///    snapshot); when that has nothing — HA unreachable, or the
///    entity not yet subscribed — the newest auto-captured snapshot
///    row of the `light` role.
/// 2. **Without a known sensor state, the grow's Light Cycle
///    schedule is evaluated for "now"** (also the whole answer when
///    nothing is bound).
/// 3. **No source at all keeps the lights-on assumption** — the same
///    convention the stepped corridors use for an unknown state
///    (D094), and the only reading available in the unlit phases.
///
/// A failed DB read of the schedule inputs surfaces as an API error;
/// a merely unknown state does not.
fn resolve_coach_light_on(
    conn: &rusqlite::Connection,
    ha: &crate::ha::HaBridge,
    grow: &leaflog_core::grows::Grow,
    tz: &str,
) -> Result<bool, ApiError> {
    let bindings = store::sensors::list(conn, &grow.id)?;
    if let Some(entity_id) = bindings.get("light") {
        let live = ha.live_now();
        if let Some(entity) = live.entities.get(entity_id)
            && let Some(on) = light_state_is_on(&entity.state)
        {
            return Ok(on);
        }
        if let Some(snapshot) = store::sensors::snapshots_for_role(conn, &grow.id, "light")?
            .last()
            .and_then(|snapshot| light_state_is_on(&snapshot.state))
        {
            return Ok(snapshot);
        }
    }
    let windows = crate::api_chart::load_windows(conn, grow)?;
    let spans = leaflog_core::chart::phase_spans(&grow.started_on, tz, &windows);
    // The grow's own photoperiod behaviour (D075) replaces the
    // removed strain lookup.
    let photoperiod = grow.photoperiod;
    let now = crate::db::now_rfc3339();
    Ok(leaflog_core::chart::schedule_light_on_at(
        &now,
        tz,
        &spans,
        &grow.light_cycle.unwrap_or_default(),
        photoperiod,
    )
    .unwrap_or(true))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn grow_coach(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<CoachResponse>, ApiError> {
    let tz = crate::app::canonical_timezone(&state).await;
    let ha = state.ha.clone();
    let response = state
        .db
        .with(move |conn| -> Result<CoachResponse, ApiError> {
            let grow = store::grows::get(conn, &id)?;

            // Phase from the re-anchored forecast (D073).
            // Local calendar days in the canonical zone (spec §2);
            // UTC math is the fallback for unknown zones.
            let today = crate::app::day_number_in(&grow.started_on, &crate::db::now_rfc3339(), &tz)
                .unwrap_or(0);
            let phase = crate::api_phase::current_phase(conn, &grow, today)?;

            // The real light state at call time (spec §4.9, D096) —
            // not the hardcoded lights-on assumption.
            let light_on = resolve_coach_light_on(conn, &ha, &grow, &tz)?;

            let tips = coach_tips(phase, &grow.substrate, light_on);

            // Equipment suggestions matched to the phase.
            let equipment = match phase {
                Phase::Seedling => vec![
                    "humidifier".to_owned(),
                    "light".to_owned(),
                    "meter".to_owned(),
                ],
                Phase::Vegetative => vec!["light".to_owned(), "fan".to_owned(), "meter".to_owned()],
                Phase::Flower => vec![
                    "dehumidifier".to_owned(),
                    "fan".to_owned(),
                    "carbon_filter".to_owned(),
                    "meter".to_owned(),
                ],
                Phase::Drying => vec!["dehumidifier".to_owned(), "sensor".to_owned()],
                Phase::Curing | Phase::Germination => vec!["sensor".to_owned()],
            };

            Ok(CoachResponse {
                phase: phase.label().to_owned(),
                tips,
                equipment_suggestions: equipment,
                light_on,
            })
        })
        .await?;
    Ok(Json(response))
}

#[cfg(test)]
mod tests {
    use super::entity_available;
    use crate::ha::LiveEntity;
    use std::collections::BTreeMap;

    fn live(state: &str) -> LiveEntity {
        LiveEntity {
            entity_id: "sensor.grow_temperature".to_owned(),
            state: state.to_owned(),
            attributes: BTreeMap::new(),
        }
    }

    #[test]
    fn ha_dead_states_are_unavailable() {
        let reporting = live("24.3");
        assert!(entity_available(Some(&reporting)));
        assert!(!entity_available(Some(&live("unavailable"))));
        assert!(!entity_available(Some(&live("unknown"))));
        assert!(!entity_available(None), "an untracked entity is no data");
    }
}
