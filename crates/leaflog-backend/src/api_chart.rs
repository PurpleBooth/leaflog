//! The grow history chart endpoint (grow-overview ticket 05).
//!
//! One windowed series per sensor role, merging Home Assistant's
//! history API with Leaflog's own log-time sensor snapshots, plus the
//! Target Band segments for the chart's shaded range. The bands are
//! tolerance-derived and flat for temperature, light-stepped
//! day/night ranges for VPD and humidity (env-section ticket 06).
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
use leaflog_core::grows::PhotoperiodType;
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
    /// The pair role's second series (pH runoff next to input), when
    /// the role has one.
    pub compare: Option<ChartSeries>,
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
    /// The light role's binding + snapshot points, for the bound-light
    /// staircase (only loaded for the stepped-band roles).
    light: Option<RolePoints>,
    /// The grow's photoperiod type, for the Light Cycle
    /// defaults (an unset grow matches the forecast's).
    photoperiod: PhotoperiodType,
    /// The user's soil-moisture band override, when stored (else the
    /// core default applies).
    soil_band: Option<(f64, f64)>,
}

/// One role's storage half: the binding and the role's snapshot
/// points.
#[derive(Debug)]
pub(crate) struct RolePoints {
    pub(crate) bound: bool,
    pub(crate) entity_id: Option<String>,
    pub(crate) snapshots: Vec<RawPoint>,
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

/// The action-details field a pH/EC chart role reads, when the role
/// is one of them.
fn detail_field(role: &str) -> Option<&'static str> {
    match role {
        "input_ph" => Some("input_ph"),
        "runoff_ph" => Some("runoff_ph"),
        "input_ec" => Some("input_ec_us"),
        "runoff_ec" => Some("runoff_ec_us"),
        _ => None,
    }
}

/// Loads a pH/EC role's points from the grow's feed and water
/// entries' structured details. Bound means at least one point exists
/// (there is no sensor behind this role).
fn load_detail_role(
    conn: &rusqlite::Connection,
    grow_id: &str,
    field: &str,
) -> Result<RolePoints, ApiError> {
    let snapshots: Vec<RawPoint> = store::actions::detail_rows(conn, grow_id)?
        .into_iter()
        .filter_map(|(at, details)| {
            let value = serde_json::from_str::<Value>(&details)
                .ok()?
                .get(field)?
                .as_f64()?;
            Some(RawPoint { at, value })
        })
        .collect();
    let bound = !snapshots.is_empty();
    Ok(RolePoints {
        bound,
        entity_id: None,
        snapshots,
    })
}

/// Loads one role's storage half: the binding and the role's snapshot
/// points (pH/EC roles read their points from log-entry details).
pub(crate) fn load_role(
    conn: &rusqlite::Connection,
    grow_id: &str,
    role: &str,
) -> Result<RolePoints, ApiError> {
    if let Some(field) = detail_field(role) {
        return load_detail_role(conn, grow_id, field);
    }
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
/// Gantt. Also the phase-span source for the coach's Light Cycle
/// point query (D096).
pub(crate) fn load_windows(
    conn: &rusqlite::Connection,
    grow: &Grow,
) -> Result<Vec<leaflog_core::forecast::PhaseWindow>, ApiError> {
    let actuals = crate::api_phase::forecast_actuals(conn, &grow.id)?;
    Ok(leaflog_core::forecast::forecast(
        &leaflog_core::forecast::input_for(grow.photoperiod, grow.planned_veg_weeks, actuals),
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
/// wanted roles' points, in one connection pass. The stepped-band
/// roles also load the light role's binding (the bound-light
/// staircase) and the grow's photoperiod (the Light Cycle
/// defaults).
fn load_grow_db(
    conn: &rusqlite::Connection,
    grow_id: &str,
    wanted: &[String],
    load_light: bool,
) -> Result<LoadedGrow, ApiError> {
    let grow = store::grows::get(conn, grow_id)?;
    let windows = load_windows(conn, &grow)?;
    let mut roles = Vec::new();
    for role_name in wanted {
        let points = load_role(conn, &grow.id, role_name)?;
        roles.push((role_name.clone(), points));
    }
    let light = load_light
        .then(|| load_role(conn, &grow.id, "light"))
        .transpose()?;
    let photoperiod = grow.photoperiod;
    let soil_band = store::settings::get(conn, "soil_moisture_band")
        .ok()
        .and_then(|value| parse_soil_band(&value));
    Ok(LoadedGrow {
        grow,
        windows,
        roles,
        light,
        photoperiod,
        soil_band,
    })
}

/// One role's fetched half: the merged windowed series plus whether
/// HA degraded that role's fetch and whether it is bound.
struct RoleFetch {
    series: ChartSeries,
    degraded: bool,
    bound: bool,
}

/// Fetches HA history per role and merges each with its snapshots.
async fn fetch_role_series(
    state: &AppState,
    loaded: &LoadedGrow,
    hours: u32,
    window: ChartWindow,
    now: &str,
) -> Vec<RoleFetch> {
    let mut fetched = Vec::new();
    for (role_name, points) in &loaded.roles {
        let mut ha_points = Vec::new();
        let mut degraded = false;
        if let Some(entity_id) = &points.entity_id {
            match state.ha.history(entity_id, hours).await {
                Ok(value) => ha_points = ha_history_points(role_name, &value),
                Err(_) => degraded = true,
            }
        }
        fetched.push(RoleFetch {
            series: chart::merge_series(now, window, &ha_points, &points.snapshots),
            degraded,
            bound: points.bound,
        });
    }
    fetched
}

/// The light role's staircase as `(epoch, value)` pairs, for the
/// sensor-wins light timeline. A failed HA fetch contributes nothing —
/// the Light Cycle schedule covers the window instead, and the chart
/// is not marked degraded (the charted role's own fetch decides that).
async fn fetch_light_staircase(
    state: &AppState,
    loaded: &LoadedGrow,
    hours: u32,
    window: ChartWindow,
    now: &str,
) -> Vec<(i64, f64)> {
    let Some(points) = &loaded.light else {
        return Vec::new();
    };
    let ha_points = if let Some(entity_id) = &points.entity_id
        && let Ok(value) = state.ha.history(entity_id, hours).await
    {
        ha_history_points("light", &value)
    } else {
        Vec::new()
    };
    let merged = chart::merge_series(now, window, &ha_points, &points.snapshots);
    series_points(&merged)
}

/// The epochs of a series' points — the band builder's mean input.
pub(crate) fn series_points(series: &ChartSeries) -> Vec<(i64, f64)> {
    series
        .points
        .iter()
        .filter_map(|point| Some((chart::parse_epoch(&point.at)?, point.value)))
        .collect()
}

/// Reduces the fetched per-role series to the response pair: the
/// derived VPD from its two inputs, or the single role's series.
/// (The humidity and pH/EC pairs are handled by the caller — their
/// degraded/bound state rides on one role only.)
fn reduce_series(
    role: &str,
    is_vpd: bool,
    mut role_fetch: Vec<RoleFetch>,
    now: &str,
    window: ChartWindow,
) -> (ChartSeries, Option<ChartSeries>) {
    if is_vpd {
        let mut series_iter = role_fetch.into_iter();
        let temperature = series_iter
            .next()
            .map_or_else(|| empty_series(now, window), |fetch| fetch.series);
        let humidity = series_iter
            .next()
            .map_or_else(|| empty_series(now, window), |fetch| fetch.series);
        let derived = chart::derive_vpd_series(
            &temperature,
            &humidity,
            leaflog_core::env::DEFAULT_LEAF_OFFSET_C,
        );
        return (derived, None);
    }
    let _ = role;
    (
        role_fetch
            .pop()
            .map_or_else(|| empty_series(now, window), |fetch| fetch.series),
        None,
    )
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
    // `vpd` is derived and `ph`/`ec` read log-entry details; every
    // other role must be a sensor role.
    let is_vpd = role == "vpd";
    let is_pair = role == "ph" || role == "ec";
    if !is_vpd && !is_pair && !store::sensors::ROLES.contains(&role.as_str()) {
        return Err(ApiError::BadRequest(format!("unknown role {role}")));
    }
    let is_stepped = matches!(role.as_str(), "humidity" | "vpd");
    let window_label = query
        .window
        .ok_or_else(|| ApiError::BadRequest("window query parameter is required".to_owned()))?;
    let window = ChartWindow::parse(&window_label)
        .ok_or_else(|| ApiError::BadRequest("window must be day, week or month".to_owned()))?;
    let tz = crate::app::canonical_timezone(&state).await;

    // The DB pass needs the role by value; the response echoes the
    // original afterwards. pH/EC pairs read [input, runoff]; the
    // humidity chart's band input is the temperature series (the
    // humidity-as-temperature bug fixed at the root), so it fetches
    // both.
    let wanted: Vec<String> = if is_vpd {
        vec!["temperature".to_owned(), "humidity".to_owned()]
    } else if role == "humidity" {
        vec!["humidity".to_owned(), "temperature".to_owned()]
    } else if role == "ph" {
        vec!["input_ph".to_owned(), "runoff_ph".to_owned()]
    } else if role == "ec" {
        vec!["input_ec".to_owned(), "runoff_ec".to_owned()]
    } else {
        vec![role.clone()]
    };
    let id_for_db = id.clone();
    let loaded = state
        .db
        .with(move |conn| load_grow_db(conn, &id_for_db, &wanted, is_stepped))
        .await?;

    // HA history per bound entity (never the token itself). A failure
    // degrades the chart to snapshot-only points instead of surfacing
    // an error wall (the Dead binding rule).
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    // Window hours are small positive constants (24 / 168 / 720).
    let hours = window.hours() as u32;
    let now = crate::db::now_rfc3339();
    let mut role_fetch = fetch_role_series(&state, &loaded, hours, window, &now).await;
    // The humidity band solves at the temperature series' points —
    // pulled off before the reduce consumes the fetched series.
    let humidity_band_points = role_fetch
        .get(1)
        .map_or_else(Vec::new, |fetch| series_points(&fetch.series));

    // Pair roles carry [input, runoff]: the input side decides the
    // no-data state. The humidity chart's degraded/bound state rides
    // on its own role; the temperature series only feeds the band.
    let (series, compare, degraded, all_bound) = reduce_response(
        &role,
        is_pair,
        is_vpd,
        &loaded,
        &mut role_fetch,
        &now,
        window,
    );

    let spans: Vec<PhaseSpan> = chart::phase_spans(&loaded.grow.started_on, &tz, &loaded.windows);

    // The light-state timeline: the bound light entity's real history
    // wins wherever it has readings; the Light Cycle schedule fills
    // the gaps (and the whole window when unbound).
    let light_periods = if is_stepped {
        let staircase = fetch_light_staircase(&state, &loaded, hours, window, &now).await;
        resolve_light_timeline(&staircase, &loaded, &tz, &spans, &series.from, &series.to)
    } else {
        None
    };

    // The humidity band solves at the temperature series' points;
    // every other role's band input is its own series (the VPD
    // corridor ignores the mean, and the temperature closure below
    // ignores it too — the points still anchor the segments).
    let band_points = if role == "humidity" {
        humidity_band_points
    } else {
        series_points(&series)
    };

    let bands = build_role_bands(
        role.as_str(),
        &series,
        &band_points,
        &spans,
        light_periods.as_deref(),
        &loaded,
    );

    Ok(Json(GrowHistory {
        role,
        window: window.label().to_owned(),
        series,
        compare,
        bands,
        degraded,
        bound: all_bound,
    }))
}

/// Builds the history response's Target Band segments for one role:
/// humidity and VPD step with the light state, temperature draws the
/// grow's flat tolerance band, soil keeps its single user-tunable
/// corridor, and the remaining roles (CO₂, pH, EC, light) stay bare
/// (env-section ticket 06).
fn build_role_bands(
    role: &str,
    series: &ChartSeries,
    band_points: &[(i64, f64)],
    spans: &[PhaseSpan],
    light_periods: Option<&[chart::LightInterval]>,
    loaded: &LoadedGrow,
) -> Vec<BandSegment> {
    match role {
        "humidity" | "vpd" => chart::build_light_bands(
            &series.from,
            &series.to,
            band_points,
            spans,
            light_periods,
            |phase, light_on, mean| stepped_corridor(role, phase, light_on, mean),
        ),
        // The tolerance-derived flat Target Band — the same
        // thresholds the env snapshot's temp_ok checks; one segment
        // per phase span, no light state.
        "temperature" => {
            let band = leaflog_core::env::temperature_target_band(
                loaded.grow.heat_tolerance,
                loaded.grow.cold_tolerance,
            );
            chart::build_bands(
                &series.from,
                &series.to,
                band_points,
                spans,
                |_phase, _mean| Some(band),
            )
        }
        // The soil band stays a single user-tunable corridor.
        "soil_moisture" => chart::build_bands(
            &series.from,
            &series.to,
            band_points,
            spans,
            |_phase, _mean| {
                let default = leaflog_core::env::SOIL_MOISTURE_BAND;
                Some(loaded.soil_band.unwrap_or((default.min, default.max)))
            },
        ),
        _ => Vec::new(),
    }
}

/// Combines the fetched per-role series into the response series plus
/// compare, degraded and bound flags. Pair roles ride on their input
/// side; humidity's own fetch decides its chart's state (the
/// temperature series only feeds the band).
#[allow(clippy::type_complexity)] // the four-tuple mirrors the GrowHistory fields
fn reduce_response(
    role: &str,
    is_pair: bool,
    is_vpd: bool,
    loaded: &LoadedGrow,
    role_fetch: &mut [RoleFetch],
    now: &str,
    window: ChartWindow,
) -> (ChartSeries, Option<ChartSeries>, bool, bool) {
    let blank = || RoleFetch {
        series: empty_series(now, window),
        degraded: false,
        bound: false,
    };
    if is_pair {
        let mut role_series = role_fetch
            .iter_mut()
            .map(|fetch| std::mem::replace(&mut fetch.series, empty_series(now, window)))
            .collect::<Vec<_>>();
        let compare = role_series.pop();
        let series = role_series
            .pop()
            .unwrap_or_else(|| empty_series(now, window));
        let bound = loaded.roles.first().is_some_and(|(_, points)| points.bound);
        return (series, compare, false, bound);
    }
    if role == "humidity" {
        let humidity = role_fetch.first();
        let series =
            humidity.map_or_else(|| empty_series(now, window), |fetch| fetch.series.clone());
        let degraded = humidity.is_some_and(|fetch| fetch.degraded);
        let bound = humidity.is_some_and(|fetch| fetch.bound);
        return (series, None, degraded, bound);
    }
    let mut degraded = false;
    let mut all_bound = true;
    for fetch in role_fetch.iter() {
        degraded |= fetch.degraded;
        all_bound &= fetch.bound;
    }
    let owned = role_fetch
        .iter_mut()
        .map(|fetch| std::mem::replace(fetch, blank()))
        .collect();
    let (series, compare) = reduce_series(role, is_vpd, owned, now, window);
    (series, compare, degraded, all_bound)
}

/// Resolves the window's light-state timeline: the bound light
/// entity's real staircase wins wherever it has readings; the Light
/// Cycle schedule fills the gaps (and the whole window when unbound).
fn resolve_light_timeline(
    staircase: &[(i64, f64)],
    loaded: &LoadedGrow,
    tz: &str,
    spans: &[PhaseSpan],
    from: &str,
    to: &str,
) -> Option<Vec<chart::LightInterval>> {
    let schedule = chart::schedule_light_periods(
        from,
        to,
        tz,
        spans,
        &loaded.grow.light_cycle.unwrap_or_default(),
        loaded.photoperiod,
    );
    let sensor = chart::sensor_light_periods(from, to, staircase);
    chart::resolve_light_periods(from, &schedule, &sensor)
}

/// The stepped corridor for one light-stepped role and light state:
/// the day presets while lit (and when the state is unknown — today's
/// behaviour), the sourced night presets in the dark. The humidity
/// arm solves the RH window at the segment's actual mean temperature
/// with the matching leaf offset; no points, no honest window.
fn stepped_corridor(
    role: &str,
    phase: leaflog_core::Phase,
    light_on: Option<bool>,
    mean: Option<f64>,
) -> Option<(f64, f64)> {
    let dark = light_on == Some(false);
    match role {
        "humidity" => {
            let band = if dark {
                leaflog_core::env::night_vpd_band(phase).map_or_else(
                    || leaflog_core::env::vpd_band(phase),
                    |night| (night.min_kpa, night.max_kpa),
                )
            } else {
                leaflog_core::env::vpd_band(phase)
            };
            let offset = if dark {
                leaflog_core::env::NIGHT_LEAF_OFFSET_C
            } else {
                leaflog_core::env::DEFAULT_LEAF_OFFSET_C
            };
            mean.map(|mean| leaflog_core::env::rh_window(mean, offset, band))
        }
        "vpd" => {
            let day = leaflog_core::env::vpd_band(phase);
            let night = dark
                .then(|| leaflog_core::env::night_vpd_band(phase))
                .flatten();
            Some(night.map_or(day, |band| (band.min_kpa, band.max_kpa)))
        }
        _ => None,
    }
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
pub(crate) fn ha_history_points(role: &str, history: &Value) -> Vec<RawPoint> {
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
