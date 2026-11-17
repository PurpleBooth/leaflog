//! Minimal JSON API client. All paths are *relative* (`api/...`) so
//! they resolve through the runtime-injected `<base href>` under
//! ingress (see the M1 spike).

// WASM is single-threaded; gloo-net response types are not `Send`, and
// `spawn_local` does not require it (docs/DECISIONS.md).
#![allow(clippy::future_not_send)]

use serde::Serialize;
use serde::de::DeserializeOwned;

use gloo_net::http::{Request, RequestBuilder, Response};

/// The HTTP status returned with an error body.
#[derive(Debug, Clone)]
pub struct ApiFailure {
    /// HTTP status code (or 0 when the request never completed).
    pub status: u16,
    /// The server's error message, or a transport error string.
    pub message: String,
}

/// `GET` a JSON endpoint.
///
/// # Errors
///
/// Returns an [`ApiFailure`] on transport errors or non-success
/// statuses.
pub async fn get<T: DeserializeOwned>(path: &str) -> Result<T, ApiFailure> {
    let response = check(Request::get(path).send().await).await?;
    parse(response).await
}

/// `POST` a JSON body and parse the JSON response.
///
/// # Errors
///
/// Returns an [`ApiFailure`] on transport errors or non-success
/// statuses.
pub async fn post<T: Serialize, R: DeserializeOwned>(
    path: &str,
    body: &T,
) -> Result<R, ApiFailure> {
    let response = check(json_body(Request::post(path), body).send().await).await?;
    parse(response).await
}

/// `POST` with no body and parse the JSON response (used for clone).
///
/// # Errors
///
/// Returns an [`ApiFailure`] on transport errors or non-success
/// statuses.
pub async fn post_empty<R: DeserializeOwned>(path: &str) -> Result<R, ApiFailure> {
    let response = check(Request::post(path).send().await).await?;
    parse(response).await
}

/// `POST` with no body, expecting an empty response (used for
/// equipment membership).
///
/// # Errors
///
/// Returns an [`ApiFailure`] on transport errors or non-success
/// statuses.
pub async fn post_action(path: &str) -> Result<(), ApiFailure> {
    check(Request::post(path).send().await).await?;
    Ok(())
}

/// `PUT` a JSON body and parse the JSON response.
///
/// # Errors
///
/// Returns an [`ApiFailure`] on transport errors or non-success
/// statuses.
pub async fn put<T: Serialize, R: DeserializeOwned>(path: &str, body: &T) -> Result<R, ApiFailure> {
    let response = check(json_body(Request::put(path), body).send().await).await?;
    parse(response).await
}

/// `POST` a `FormData` (photo uploads) and expect an empty response.
///
/// # Errors
///
/// Returns an [`ApiFailure`] on transport errors or non-success
/// statuses.
///
/// # Panics
///
/// Cannot panic: a `FormData` is always accepted as a body.
pub async fn post_form(path: &str, form: &leptos::web_sys::FormData) -> Result<(), ApiFailure> {
    let request = Request::post(path)
        .body(form.clone())
        .expect("form body is always valid");
    check(request.send().await).await?;
    Ok(())
}

/// `DELETE` a resource (expects 204).
///
/// # Errors
///
/// Returns an [`ApiFailure`] on transport errors or non-success
/// statuses.
pub async fn delete(path: &str) -> Result<(), ApiFailure> {
    check(Request::delete(path).send().await).await?;
    Ok(())
}

/// Adds the JSON content type and body to a request builder.
fn json_body<T: Serialize>(builder: RequestBuilder, body: &T) -> Request {
    builder
        .header("Content-Type", "application/json")
        .body(serde_json::to_string(body).expect("request serializes"))
        .expect("string body is always valid")
}

/// Turns a transport result into a response or an [`ApiFailure`],
/// decoding the JSON `error` field when the status is not a success.
async fn check(result: Result<Response, gloo_net::Error>) -> Result<Response, ApiFailure> {
    let response = result.map_err(|error| ApiFailure {
        status: 0,
        message: error.to_string(),
    })?;
    if response.ok() {
        return Ok(response);
    }
    let status = response.status();
    let message = response
        .json::<serde_json::Value>()
        .await
        .ok()
        .and_then(|body| {
            body.get("error")
                .and_then(serde_json::Value::as_str)
                .map(str::to_owned)
        })
        .unwrap_or_else(|| format!("request failed with status {status}"));
    Err(ApiFailure { status, message })
}

/// Parses a response body as JSON.
async fn parse<T: DeserializeOwned>(response: Response) -> Result<T, ApiFailure> {
    response.json::<T>().await.map_err(|error| ApiFailure {
        status: 0,
        message: error.to_string(),
    })
}

/// The live-data snapshot shape.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct LiveData {
    /// Whether the Supervisor WebSocket is connected.
    pub connected: bool,
    /// Entity states.
    pub entities: std::collections::BTreeMap<String, LiveEntity>,
}

/// One live entity.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct LiveEntity {
    /// Entity id.
    pub entity_id: String,
    /// Raw state.
    pub state: String,
    /// Notable attributes.
    pub attributes: std::collections::BTreeMap<String, serde_json::Value>,
}

/// The forecast response (grow day timeline).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct Forecast {
    /// Days since the grow start (today marker).
    pub today_day: i64,
    /// Phase windows in chronological order.
    pub windows: Vec<PhaseWindowData>,
}

/// The soil-moisture band override (settings key
/// `soil_moisture_band`), volumetric percent.
#[derive(Debug, Clone, Copy, serde::Deserialize, serde::Serialize)]
pub struct SoilBand {
    /// Lower edge, percent.
    pub min: f64,
    /// Upper edge, percent.
    pub max: f64,
}

/// The history chart payload (`GET /api/grows/{id}/history`).
#[derive(Debug, Clone, serde::Deserialize)]
pub struct GrowHistory {
    /// The requested role (echoed).
    pub role: String,
    /// The requested window label (echoed).
    pub window: String,
    /// The merged windowed series (SI units, ascending by time).
    pub series: ChartSeriesData,
    /// The pair role's second series (pH runoff next to input), when
    /// the role has one.
    pub compare: Option<ChartSeriesData>,
    /// Phase corridor segments clipped to the window.
    pub bands: Vec<BandSegmentData>,
    /// Home Assistant's history was unavailable, so the chart is
    /// snapshot-only (the Dead binding rule).
    pub degraded: bool,
    /// The grow binds a sensor for this role. False is an explicit
    /// no-sensor state, never a data error.
    pub bound: bool,
}

/// The merged series for one chart: window bounds plus ascending
/// points.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ChartSeriesData {
    /// Window start, RFC 3339 UTC, inclusive.
    pub from: String,
    /// Window end, RFC 3339 UTC, inclusive.
    pub to: String,
    /// Points ascending by time.
    pub points: Vec<ChartPointData>,
}

/// One merged chart point.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct ChartPointData {
    /// When the value was recorded, RFC 3339 UTC.
    pub at: String,
    /// The value in SI units.
    pub value: f64,
    /// Which capture path produced it (`ha` or `snapshot`).
    pub source: String,
}

/// One corridor segment: the phase's min/max range between two
/// instants, already clipped to the window.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct BandSegmentData {
    /// Segment start, RFC 3339 UTC, inclusive.
    pub start: String,
    /// Segment end, RFC 3339 UTC.
    pub end: String,
    /// Lower band edge in SI units.
    pub min: f64,
    /// Upper band edge in SI units.
    pub max: f64,
    /// The phase the segment belongs to.
    pub phase: String,
    /// Whether the grow lights were on during the segment (`None`
    /// when no light state applies). The day/night steps ride on it.
    pub light_on: Option<bool>,
}

/// One phase window as the API serializes it.
#[derive(Debug, Clone, serde::Deserialize)]
pub struct PhaseWindowData {
    /// Which phase.
    pub phase: String,
    /// Earliest start.
    pub earliest_start: i64,
    /// Likely start.
    pub likely_start: i64,
    /// Latest start.
    pub latest_start: i64,
    /// Earliest end.
    pub earliest_end: i64,
    /// Likely end.
    pub likely_end: i64,
    /// Latest end.
    pub latest_end: i64,
    /// True when pinned by a logged event.
    pub actual_start: bool,
}
