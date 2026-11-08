//! Environment, lamp advisor and phase coach API routes (M7).

use axum::extract::{Path, State};
use axum::routing::get;
use axum::{Json, Router};
use leaflog_core::Phase;
use leaflog_core::env::{
    CoachTip, LampAdvice, LampInput, TempThresholds, advise_lamp, coach_tips, rh_window, vpd_band,
    vpd_kpa,
};
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

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn grow_env(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<EnvSnapshot>, ApiError> {
    let live = state.ha.live_now();
    let response = state
        .db
        .with(move |conn| -> Result<EnvSnapshot, ApiError> {
            let grow = store::grows::get(conn, &id)?;
            let strain = grow
                .strain_id
                .as_deref()
                .and_then(|sid| store::strain::get(conn, sid).ok());

            // Derive the current phase from the forecast.
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
            let today = {
                let start = time::Date::parse(
                    &grow.started_on,
                    &time::macros::format_description!("[year]-[month]-[day]"),
                )
                .map_err(|e| ApiError::BadRequest(e.to_string()))?;
                u16::try_from(
                    (time::OffsetDateTime::now_utc().date() - start)
                        .whole_days()
                        .max(0),
                )
                .unwrap_or(0)
            };
            let phase = leaflog_core::actions::derive_phase(today, flip, harvest.is_some());

            // Temperature and humidity from the bound sensors.
            let bindings = store::sensors::list(conn, &id)?;
            let temp_entity = bindings.get("temperature");
            let humidity_entity = bindings.get("humidity");
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

            let temp_ok =
                temperature_c.map(|t| t >= thresholds.floor_c && t <= thresholds.ceiling_c);
            let vpd_ok = vpd.map(|v| v >= band.0 && v <= band.1);

            Ok(EnvSnapshot {
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
        })
        .await?;
    Ok(Json(response))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn grow_lamp(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<LampAdvice>, ApiError> {
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
            let today = {
                let start = time::Date::parse(
                    &grow.started_on,
                    &time::macros::format_description!("[year]-[month]-[day]"),
                )
                .map_err(|e| ApiError::BadRequest(e.to_string()))?;
                u16::try_from(
                    (time::OffsetDateTime::now_utc().date() - start)
                        .whole_days()
                        .max(0),
                )
                .unwrap_or(0)
            };
            let phase = leaflog_core::actions::derive_phase(today, flip, harvest.is_some());

            // Canopy temp from the bound temperature sensor.
            let bindings = store::sensors::list(conn, &id)?;
            let canopy_temp = bindings
                .get("temperature")
                .and_then(|e| live.entities.get(e))
                .and_then(|e| e.state.parse::<f64>().ok());

            // The most powerful bound light's wattage (from the grow's
            // equipment assignments or any bound light entity).
            let light_watts: u32 = 330; // sensible default

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
                dimmable: true,
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
            let today = {
                let start = time::Date::parse(
                    &grow.started_on,
                    &time::macros::format_description!("[year]-[month]-[day]"),
                )
                .map_err(|e| ApiError::BadRequest(e.to_string()))?;
                u16::try_from(
                    (time::OffsetDateTime::now_utc().date() - start)
                        .whole_days()
                        .max(0),
                )
                .unwrap_or(0)
            };
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
