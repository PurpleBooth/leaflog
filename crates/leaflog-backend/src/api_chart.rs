//! The grow history chart endpoint (grow-overview ticket 05).
//!
//! One windowed series per sensor role, merging Home Assistant's
//! history API with Leaflog's own log-time sensor snapshots, plus the
//! phase-keyed corridor for the chart's shaded band.
//!
//! Degradation follows the Dead binding rule: when Home Assistant is
//! unreachable the chart serves snapshot-only points with
//! `degraded: true` instead of an error wall; a missing binding is an
//! explicit `bound: false` no-data state.

use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use leaflog_core::chart::{self, BandSegment, ChartSeries, ChartWindow, PhaseSpan, RawPoint};
use leaflog_core::grows::Grow;
use serde::Deserialize;
use serde_json::Value;

use crate::api::ApiError;
use crate::app::AppState;
use crate::store;

/// The chart routes; merged into the API router.
pub fn router() -> Router<AppState> {
    Router::new().route("/api/grows/{id}/history", get(grow_history))
}

/// Query of `GET /api/grows/{id}/history`.
#[derive(Debug, Default, Deserialize)]
struct HistoryQuery {
    /// Sensor role to chart (`temperature`, ...); required. The
    /// derived `vpd` role computes from the temperature and humidity
    /// series.
    role: Option<String>,
    /// Rolling window: `day`, `week` or `month`; required.
    window: Option<String>,
}

/// The merged chart payload for one role.
#[derive(Debug, serde::Serialize)]
pub struct GrowHistory {
    /// The requested role (echoed).
    pub role: String,
    /// The requested window label (echoed).
    pub window: String,
    /// The merged windowed series (SI units, ascending by time).
    pub series: ChartSeries,
    /// Phase corridor segments clipped to the window (empty for roles
    /// without a corridor).
    pub bands: Vec<BandSegment>,
    /// Home Assistant's history was unavailable, so the chart is
    /// snapshot-only (the Dead binding rule).
    pub degraded: bool,
    /// Whether the grow binds the sensor(s) this role needs. False is
    /// an explicit no-sensor state, never a data error.
    pub bound: bool,
}

/// The forecast windows and start date the corridor is built from,
/// plus each role's storage half.
#[derive(Debug)]
struct LoadedGrow {
    /// The grow row (its planned veg weeks drive the forecast).
    grow: Grow,
    /// The grow's forecast phase windows (the corridor source).
    windows: Vec<leaflog_core::forecast::PhaseWindow>,
    /// Per-role binding + snapshot points (VPD carries two).
    roles: Vec<(String, RolePoints)>,
    /// The user's soil-moisture band override, when stored (else the
    /// core default applies).
    soil_band: Option<(f64, f64)>,
}

/// One role's storage half: the binding and the role's snapshot
/// points.
#[derive(Debug)]
struct RolePoints {
    bound: bool,
    entity_id: Option<String>,
    snapshots: Vec<RawPoint>,
}

/// Parses one raw sensor state into a series value. The `light` role
/// reads binary states as 1/0; everything else parses numerically
/// (states a numeric sensor would never report drop out).
fn parse_value(role: &str, state: &str) -> Option<f64> {
    if role == "light" {
        return match state.trim().to_ascii_lowercase().as_str() {
            "on" => Some(1.0),
            "off" => Some(0.0),
            _ => state.parse::<f64>().ok(),
        };
    }
    state.parse::<f64>().ok()
}

/// Loads one role's storage half: the binding and the role's snapshot
/// points.
fn load_role(
    conn: &rusqlite::Connection,
    grow_id: &str,
    role: &str,
) -> Result<RolePoints, ApiError> {
    let bindings = store::sensors::list(conn, grow_id)?;
    let entity_id = bindings.get(role).cloned();
    let snapshots = store::sensors::snapshots_for_role(conn, grow_id, role)?
        .into_iter()
        .filter_map(|snapshot| {
            Some(RawPoint {
                at: snapshot.captured_at,
                value: parse_value(role, &snapshot.state)?,
            })
        })
        .collect();
    Ok(RolePoints {
        bound: entity_id.is_some(),
        entity_id,
        snapshots,
    })
}

/// Loads the forecast windows for a grow: the same actuals as the
/// forecast endpoint (D073 resolution), so the corridor matches the
/// Gantt.
fn load_windows(
    conn: &rusqlite::Connection,
    grow: &Grow,
) -> Result<Vec<leaflog_core::forecast::PhaseWindow>, ApiError> {
    let strain = grow
        .strain_id
        .as_deref()
        .and_then(|sid| store::strain::get(conn, sid).ok());
    let actuals = crate::api_phase::forecast_actuals(conn, &grow.id)?;
    Ok(leaflog_core::forecast::forecast(
        &leaflog_core::forecast::input_for(strain.as_ref(), grow.planned_veg_weeks, actuals),
    ))
}

/// Parses a stored soil-moisture band override: an object with
/// 0 ≤ min ≤ max ≤ 100. Anything else (including `null` — the reset
/// value) falls back to the core default.
fn parse_soil_band(value: &Value) -> Option<(f64, f64)> {
    let min = value.get("min")?.as_f64()?;
    let max = value.get("max")?.as_f64()?;
    if !(0.0..=100.0).contains(&min) || !(min..=100.0).contains(&max) {
        return None;
    }
    Some((min, max))
}

/// The DB half of the chart: the grow, its forecast windows and the
/// wanted roles' points, in one connection pass.
fn load_grow_db(
    conn: &rusqlite::Connection,
    grow_id: &str,
    wanted: &[String],
) -> Result<LoadedGrow, ApiError> {
    let grow = store::grows::get(conn, grow_id)?;
    let windows = load_windows(conn, &grow)?;
    let mut roles = Vec::new();
    for role_name in wanted {
        let points = load_role(conn, &grow.id, role_name)?;
        roles.push((role_name.clone(), points));
    }
    let soil_band = store::settings::get(conn, "soil_moisture_band")
        .ok()
        .and_then(|value| parse_soil_band(&value));
    Ok(LoadedGrow {
        grow,
        windows,
        roles,
        soil_band,
    })
}

/// Fetches HA history per role, merges each with its snapshots, and
/// reports whether HA degraded the chart and whether every needed
/// role is bound.
async fn fetch_role_series(
    state: &AppState,
    loaded: &LoadedGrow,
    hours: u32,
    window: ChartWindow,
    now: &str,
) -> (Vec<ChartSeries>, bool, bool) {
    let mut degraded = false;
    let mut series = Vec::new();
    let mut all_bound = true;
    for (role_name, points) in &loaded.roles {
        let mut ha_points = Vec::new();
        if let Some(entity_id) = &points.entity_id {
            match state.ha.history(entity_id, hours).await {
                Ok(value) => ha_points = ha_history_points(role_name, &value),
                Err(_) => degraded = true,
            }
        }
        all_bound &= points.bound;
        series.push(chart::merge_series(
            now,
            window,
            &ha_points,
            &points.snapshots,
        ));
    }
    (series, degraded, all_bound)
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn grow_history(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<GrowHistory>, ApiError> {
    let role = query
        .role
        .ok_or_else(|| ApiError::BadRequest("role query parameter is required".to_owned()))?;
    // `vpd` is derived, not bound; every other role must be a sensor
    // role.
    let is_vpd = role == "vpd";
    if !is_vpd && !store::sensors::ROLES.contains(&role.as_str()) {
        return Err(ApiError::BadRequest(format!("unknown role {role}")));
    }
    let window_label = query
        .window
        .ok_or_else(|| ApiError::BadRequest("window query parameter is required".to_owned()))?;
    let window = ChartWindow::parse(&window_label)
        .ok_or_else(|| ApiError::BadRequest("window must be day, week or month".to_owned()))?;
    let tz = crate::app::canonical_timezone(&state).await;

    // The DB pass needs the role by value; the response echoes the
    // original afterwards.
    let wanted: Vec<String> = if is_vpd {
        vec!["temperature".to_owned(), "humidity".to_owned()]
    } else {
        vec![role.clone()]
    };
    let id_for_db = id.clone();
    let loaded = state
        .db
        .with(move |conn| load_grow_db(conn, &id_for_db, &wanted))
        .await?;

    // HA history per bound entity (never the token itself). A failure
    // degrades the chart to snapshot-only points instead of surfacing
    // an error wall (the Dead binding rule).
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    // Window hours are small positive constants (24 / 168 / 720).
    let hours = window.hours() as u32;
    let now = crate::db::now_rfc3339();
    let (mut role_series, degraded, all_bound) =
        fetch_role_series(&state, &loaded, hours, window, &now).await;

    let series = if is_vpd {
        // Roles arrived as [temperature, humidity].
        let mut series_iter = role_series.into_iter();
        let temperature = series_iter
            .next()
            .unwrap_or_else(|| empty_series(&now, window));
        let humidity = series_iter
            .next()
            .unwrap_or_else(|| empty_series(&now, window));
        chart::derive_vpd_series(
            &temperature,
            &humidity,
            leaflog_core::env::DEFAULT_LEAF_OFFSET_C,
        )
    } else {
        role_series
            .pop()
            .unwrap_or_else(|| empty_series(&now, window))
    };

    let spans: Vec<PhaseSpan> = chart::phase_spans(&loaded.grow.started_on, &tz, &loaded.windows);
    let series_points: Vec<(i64, f64)> = series
        .points
        .iter()
        .filter_map(|point| Some((chart::parse_epoch(&point.at)?, point.value)))
        .collect();
    let bands = chart::build_bands(
        &series.from,
        &series.to,
        &series_points,
        &spans,
        |phase, mean| {
            match role.as_str() {
                "temperature" => {
                    let corridor = leaflog_core::env::temperature_corridor(phase);
                    Some((corridor.min_c, corridor.max_c))
                }
                // The RH window is the VPD band converted at the segment's
                // mean temperature; no points, no honest window.
                "humidity" => mean.map(|value| {
                    leaflog_core::env::rh_window(
                        value,
                        leaflog_core::env::DEFAULT_LEAF_OFFSET_C,
                        leaflog_core::env::vpd_band(phase),
                    )
                }),
                "vpd" => {
                    let (min, max) = leaflog_core::env::vpd_band(phase);
                    Some((min, max))
                }
                // The user's override, else the sourced default preset.
                "soil_moisture" => {
                    let default = leaflog_core::env::SOIL_MOISTURE_BAND;
                    Some(loaded.soil_band.unwrap_or((default.min, default.max)))
                }
                // The remaining roles land with their own tickets.
                _ => None,
            }
        },
    );

    Ok(Json(GrowHistory {
        role,
        window: window.label().to_owned(),
        series,
        bands,
        degraded,
        bound: all_bound,
    }))
}

/// An empty series covering the requested window (a role whose
/// storage or HA fetch produced nothing).
fn empty_series(now: &str, window: ChartWindow) -> ChartSeries {
    chart::merge_series(now, window, &[], &[])
}

/// Extracts (timestamp, value) points from a Home Assistant
/// `history/period` response. The documented `minimal_response` shape
/// is one full state object first, then short `{s, lu}` objects;
/// unparseable states (a sensor reporting `unavailable`) and objects
/// without a usable timestamp are skipped.
#[must_use]
fn ha_history_points(role: &str, history: &Value) -> Vec<RawPoint> {
    let Some(series_list) = history.as_array() else {
        return Vec::new();
    };
    let mut points = Vec::new();
    for series in series_list {
        let Some(states) = series.as_array() else {
            continue;
        };
        for state in states {
            let value = state
                .get("state")
                .or_else(|| state.get("s"))
                .and_then(Value::as_str)
                .and_then(|text| parse_value(role, text));
            let at = ["lu", "last_changed", "last_updated"]
                .iter()
                .find_map(|key| state.get(*key).and_then(Value::as_str));
            if let (Some(at), Some(value)) = (at, value) {
                points.push(RawPoint {
                    at: at.to_owned(),
                    value,
                });
            }
        }
    }
    points
}

#[cfg(test)]
mod tests {
    use super::{ha_history_points, parse_value};
    use serde_json::json;

    #[test]
    fn history_points_parse_full_and_minimal_objects() {
        // The documented `minimal_response` shape: one full state
        // object first, then short `{s, lu}` objects.
        let history = json!([
            [
                {
                    "entity_id": "sensor.grow_temperature",
                    "state": "24.3",
                    "attributes": {},
                    "last_changed": "2026-11-08T12:00:00.000000+00:00",
                    "last_updated": "2026-11-08T12:00:00.000000+00:00"
                },
                {"s": "24.2", "lu": "2026-11-08T11:00:00+00:00"},
                {"s": "24.1", "lu": "2026-11-08T10:00:00+00:00"},
                {"s": "unavailable", "lu": "2026-11-08T09:00:00+00:00"},
                {"s": "24.0"}
            ],
            "not-an-array"
        ]);
        let points = ha_history_points("temperature", &history);
        assert_eq!(points.len(), 3);
        assert_eq!(points[0].value, 24.3);
        assert_eq!(points[0].at, "2026-11-08T12:00:00.000000+00:00");
        assert_eq!(points[1].value, 24.2);
        assert_eq!(points[2].value, 24.1);
    }

    #[test]
    fn history_points_fail_closed_on_garbage() {
        assert_eq!(ha_history_points("temperature", &json!("nope")), Vec::new());
        assert_eq!(ha_history_points("temperature", &json!({})), Vec::new());
        assert_eq!(ha_history_points("temperature", &json!([[]])), Vec::new());
    }

    #[test]
    fn light_states_parse_as_one_and_zero() {
        assert_eq!(parse_value("light", "on"), Some(1.0));
        assert_eq!(parse_value("light", "OFF"), Some(0.0));
        assert_eq!(parse_value("light", "unavailable"), None);
        // Numeric light states (a dimmer's percentage) still parse.
        assert_eq!(parse_value("light", "42"), Some(42.0));
        // Other roles stay strictly numeric.
        assert_eq!(parse_value("temperature", "on"), None);
    }

    #[test]
    fn soil_band_overrides_validate_and_reset_to_default() {
        use super::parse_soil_band;
        assert_eq!(
            parse_soil_band(&json!({"min": 25.0, "max": 45.0})),
            Some((25.0, 45.0))
        );
        // Out of range, inverted, null (the reset value) and garbage
        // all fall back to the sourced default.
        assert_eq!(parse_soil_band(&json!({"min": 45.0, "max": 25.0})), None);
        assert_eq!(parse_soil_band(&json!({"min": -1.0, "max": 45.0})), None);
        assert_eq!(parse_soil_band(&json!(null)), None);
        assert_eq!(parse_soil_band(&json!("nope")), None);
    }
}
