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
