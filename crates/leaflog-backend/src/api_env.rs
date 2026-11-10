//! Environment, lamp advisor and phase coach API routes (M7).

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use leaflog_core::Phase;
use leaflog_core::env::{
    CoachTip, LampAdvice, LampInput, TempThresholds, advise_lamp, coach_tips, rh_window, vpd_band,
    vpd_kpa,
};
use leaflog_core::equipment::{EquipmentCategory, EquipmentItem};
use leaflog_core::strain::Tolerance;
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
    /// Temperature alert thresholds (strain-shifted).
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
        let strain = grow
            .strain_id
            .as_deref()
            .and_then(|sid| store::strain::get(conn, sid).ok());

        // Derive the current phase from the forecast.
        let flip = store::actions::first_event_day(
            conn,
            id,
            leaflog_core::actions::ActionType::Flip,
            u16::MAX,
        )?;
        let harvest = store::actions::first_event_day(
            conn,
            id,
            leaflog_core::actions::ActionType::Harvest,
            u16::MAX,
        )?;
        // Local calendar days in the canonical zone (spec §2);
        // UTC math is the fallback for unknown zones.
        let today =
            crate::app::day_number_in(&grow.started_on, &crate::db::now_rfc3339(), tz).unwrap_or(0);
        let phase = leaflog_core::actions::derive_phase(today, flip, harvest.is_some());

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

        // Strain-shifted thresholds.
        let heat = strain
            .as_ref()
            .map_or(Tolerance::Average, |s| s.heat_tolerance);
        let cold = strain
            .as_ref()
            .map_or(Tolerance::Average, |s| s.cold_tolerance);
        let thresholds = TempThresholds::default().shifted(heat, cold);

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
            let strain = grow
                .strain_id
                .as_deref()
                .and_then(|sid| store::strain::get(conn, sid).ok());

            // Phase from the forecast.
            let flip = store::actions::first_event_day(
                conn,
                &id,
                leaflog_core::actions::ActionType::Flip,
                u16::MAX,
            )?;
            let harvest = store::actions::first_event_day(
                conn,
                &id,
                leaflog_core::actions::ActionType::Harvest,
                u16::MAX,
            )?;
            // Local calendar days in the canonical zone (spec §2);
            // UTC math is the fallback for unknown zones.
            let today = crate::app::day_number_in(&grow.started_on, &crate::db::now_rfc3339(), &tz)
                .unwrap_or(0);
            let phase = leaflog_core::actions::derive_phase(today, flip, harvest.is_some());

            // Canopy temp from the bound temperature sensor.
            let bindings = store::sensors::list(conn, &id)?;
            let canopy_temp = bindings
                .get("temperature")
                .and_then(|e| live.entities.get(e))
                .and_then(|e| e.state.parse::<f64>().ok());

            // The most powerful light in the grow's space advises the
            // wattage class; without a space (or a known wattage) we
            // fall back to the historical sensible default
            // (docs/DECISIONS.md D021).
            let (light_watts, light_dimmable) = grow
                .grow_space_id
                .as_deref()
                .map(|space_id| store::equipment::list_for_space(conn, space_id))
                .transpose()?
                .map_or((330, true), |equipment| space_light(&equipment));

            // Stretch/stress from recent appearance tags.
            let recent = store::actions::list(conn, &id, None, 30)?;
            let stretch = recent
                .iter()
                .any(|entry| entry.appearance_tags.iter().any(|t| t == "stretchy"));
            let stress = recent.iter().any(|entry| {
                entry
                    .appearance_tags
                    .iter()
                    .any(|t| t == "bleaching" || t == "taco")
            });

            let input = LampInput {
                real_watts: light_watts,
                dimmable: light_dimmable,
                dim_pct: 100,
                canopy_temperature_c: canopy_temp,
                phase,
                light_tolerance: strain
                    .as_ref()
                    .map_or(Tolerance::Average, |s| s.light_tolerance),
                stretch_logged: stretch,
                stress_tags_logged: stress,
            };
            Ok(advise_lamp(&input))
        })
        .await?;
    Ok(Json(advice))
}

/// Picks the advising light from a grow space's equipment: the most
/// powerful light with a known real wattage, and whether any light is
/// dimmable. Returns the `(330, true)` historical default when the
/// space has no usable light.
fn space_light(equipment: &[EquipmentItem]) -> (u32, bool) {
    let lights: Vec<&EquipmentItem> = equipment
        .iter()
        .filter(|item| item.category == EquipmentCategory::Light)
        .collect();
    let watts = lights
        .iter()
        .filter_map(|light| light.real_watts)
        .fold(0.0_f64, f64::max);
    if watts <= 0.0 {
        return (330, true);
    }
    let dimmable = lights.iter().any(|light| light.dimmable);
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    // fixture wattage; the clamp keeps the cast exact (see docs/DECISIONS.md D021)
    let watts = watts.round().clamp(1.0, 9_999_999.0) as u32;
    (watts, dimmable)
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
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn grow_coach(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<CoachResponse>, ApiError> {
    let tz = crate::app::canonical_timezone(&state).await;
    let response = state
        .db
        .with(move |conn| -> Result<CoachResponse, ApiError> {
            let grow = store::grows::get(conn, &id)?;

            let flip = store::actions::first_event_day(
                conn,
                &id,
                leaflog_core::actions::ActionType::Flip,
                u16::MAX,
            )?;
            let harvest = store::actions::first_event_day(
                conn,
                &id,
                leaflog_core::actions::ActionType::Harvest,
                u16::MAX,
            )?;
            // Local calendar days in the canonical zone (spec §2);
            // UTC math is the fallback for unknown zones.
            let today = crate::app::day_number_in(&grow.started_on, &crate::db::now_rfc3339(), &tz)
                .unwrap_or(0);
            let phase = leaflog_core::actions::derive_phase(today, flip, harvest.is_some());

            let tips = coach_tips(phase, &grow.substrate, true);

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
