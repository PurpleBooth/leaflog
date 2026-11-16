//! The grow history chart endpoint (grow-overview ticket 05).
//!
//! One windowed series per sensor role, merging Home Assistant's
//! history API with Leaflog's own log-time sensor snapshots, plus the
//! phase-keyed temperature corridor for the chart's shaded band.
//!
//! Degradation follows the Dead binding rule: when Home Assistant is
//! unreachable the chart serves snapshot-only points with
//! `degraded: true` instead of an error wall; a missing binding is an
//! explicit `bound: false` no-data state.

use axum::extract::{Path, Query, State};
use axum::routing::get;
use axum::{Json, Router};
use leaflog_core::chart::{self, BandSegment, ChartSeries, ChartWindow, PhaseSpan, RawPoint};
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
    /// Sensor role to chart (`temperature`, ...); required.
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
    /// Whether the grow binds a sensor for this role at all. False is
    /// an explicit no-sensor state, never a data error.
    pub bound: bool,
}

/// Everything the chart needs from storage, fetched in one DB pass.
#[derive(Debug)]
struct LoadedGrow {
    /// Whether the grow binds this role.
    bound: bool,
    /// The bound entity id, when present (drives the HA call).
    entity_id: Option<String>,
    /// Log-time snapshots for the role, as raw series points.
    snapshots: Vec<RawPoint>,
    /// The grow's forecast phase windows (the corridor source).
    windows: Vec<leaflog_core::forecast::PhaseWindow>,
    /// The grow's start date (the corridor anchor).
    started_on: String,
}

/// Loads the storage half of the chart: the binding, the role's
/// snapshot points and the grow's forecast windows.
fn load_grow(conn: &rusqlite::Connection, id: &str, role: &str) -> Result<LoadedGrow, ApiError> {
    let grow = store::grows::get(conn, id)?;
    let bindings = store::sensors::list(conn, &grow.id)?;
    let entity_id = bindings.get(role).cloned();
    let snapshots = store::sensors::snapshots_for_role(conn, &grow.id, role)?
        .into_iter()
        .filter_map(|snapshot| {
            // Snapshot states are raw strings; anything a numeric
            // sensor would never report ("unavailable") drops out.
            let value = snapshot.state.parse::<f64>().ok()?;
            Some(RawPoint {
                at: snapshot.captured_at,
                value,
            })
        })
        .collect();
    // Same actuals as the forecast endpoint (D073 resolution), so the
    // corridor matches the Gantt: canonical events first, phase-change
    // pins for the rest, planned veg weeks when set.
    let strain = grow
        .strain_id
        .as_deref()
        .and_then(|sid| store::strain::get(conn, sid).ok());
    let actuals = crate::api_phase::forecast_actuals(conn, &grow.id)?;
    let windows = leaflog_core::forecast::forecast(&leaflog_core::forecast::input_for(
        strain.as_ref(),
        grow.planned_veg_weeks,
        actuals,
    ));
    Ok(LoadedGrow {
        bound: entity_id.is_some(),
        entity_id,
        snapshots,
        windows,
        started_on: grow.started_on,
    })
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
    if !store::sensors::ROLES.contains(&role.as_str()) {
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
    let role_for_db = role.clone();
    let loaded = state
        .db
        .with(move |conn| load_grow(conn, &id, &role_for_db))
        .await?;

    // HA history for the bound entity (never the token itself). A
    // failure degrades the chart to snapshot-only points instead of
    // surfacing an error wall (the Dead binding rule).
    let mut degraded = false;
    let mut ha_points = Vec::new();
    if let Some(entity_id) = &loaded.entity_id {
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        // Window hours are small positive constants (24 / 168 / 720).
        let hours = window.hours() as u32;
        match state.ha.history(entity_id, hours).await {
            Ok(value) => ha_points = ha_history_points(&value),
            Err(_) => degraded = true,
        }
    }

    let now = crate::db::now_rfc3339();
    let series = chart::merge_series(&now, window, &ha_points, &loaded.snapshots);
    let spans: Vec<PhaseSpan> = chart::phase_spans(&loaded.started_on, &tz, &loaded.windows);
    let bands = chart::build_bands(&series.from, &series.to, &spans, |phase| {
        // v1 ships the temperature corridor only; other roles chart
        // without a band until their presets land.
        (role == "temperature").then(|| {
            let corridor = leaflog_core::env::temperature_corridor(phase);
            (corridor.min_c, corridor.max_c)
        })
    });

    Ok(Json(GrowHistory {
        role,
        window: window.label().to_owned(),
        series,
        bands,
        degraded,
        bound: loaded.bound,
    }))
}

/// Extracts (timestamp, value) points from a Home Assistant
/// `history/period` response. The documented `minimal_response` shape
/// is one full state object first, then short `{s, lu}` objects;
/// non-numeric states (a sensor reporting `unavailable`) and objects
/// without a usable timestamp are skipped.
#[must_use]
fn ha_history_points(history: &Value) -> Vec<RawPoint> {
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
                .and_then(|text| text.parse::<f64>().ok());
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
    use super::ha_history_points;
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
        let points = ha_history_points(&history);
        assert_eq!(points.len(), 3);
        assert_eq!(points[0].value, 24.3);
        assert_eq!(points[0].at, "2026-11-08T12:00:00.000000+00:00");
        assert_eq!(points[1].value, 24.2);
        assert_eq!(points[2].value, 24.1);
    }

    #[test]
    fn history_points_fail_closed_on_garbage() {
        assert_eq!(ha_history_points(&json!("nope")), Vec::new());
        assert_eq!(ha_history_points(&json!({})), Vec::new());
        assert_eq!(ha_history_points(&json!([[]])), Vec::new());
    }
}
