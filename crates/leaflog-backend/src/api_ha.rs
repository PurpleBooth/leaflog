//! HA-related API routes (M4): entity list, history passthrough, the
//! live SSE stream, per-grow sensor bindings, and auto-capture of
//! bound sensors into log entries.

use std::collections::BTreeMap;

use axum::extract::{Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use futures_util::Stream;
use serde::Deserialize;
use serde_json::{Value, json};

use crate::api::ApiError;
use crate::app::AppState;
use crate::store;

/// The HA routes; merged into the API router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/ha/entities", get(ha_entities))
        .route("/api/ha/history", get(ha_history))
        .route("/api/live", get(live_stream))
        .route(
            "/api/grows/{id}/bindings",
            get(get_bindings).put(put_bindings),
        )
}

/// Query of `GET /api/ha/entities`.
#[derive(Debug, Default, Deserialize)]
struct EntitiesQuery {
    /// Keep only entities whose `device_class` attribute matches (the
    /// sensor-item picker of spec §4.1 filters temperature/humidity).
    device_class: Option<String>,
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn ha_entities(
    State(state): State<AppState>,
    Query(query): Query<EntitiesQuery>,
) -> Result<Json<Value>, ApiError> {
    let source = if let Ok(entities) = state.ha.list_entities().await {
        let filtered = match &query.device_class {
            Some(class) => entities
                .into_iter()
                .filter(|entity| {
                    entity["attributes"]["device_class"].as_str() == Some(class.as_str())
                })
                .collect::<Vec<_>>(),
            None => entities,
        };
        json!({ "source": "supervisor", "entities": filtered })
    } else {
        // Supervisor unreachable: fall back to the live map so pickers
        // still work offline.
        let live = state.ha.live_now();
        let filtered: Vec<_> = live
            .entities
            .values()
            .filter(|entity| {
                query.device_class.as_ref().is_none_or(|class| {
                    entity
                        .attributes
                        .get("device_class")
                        .and_then(Value::as_str)
                        == Some(class.as_str())
                })
            })
            .collect();
        json!({ "source": "live", "entities": filtered })
    };
    Ok(Json(source))
}

/// Query of `GET /api/ha/history`.
#[derive(Debug, Default, Deserialize)]
struct HistoryQuery {
    /// Entity id to fetch.
    entity_id: Option<String>,
    /// Hours of history (default 24).
    hours: Option<u32>,
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn ha_history(
    State(state): State<AppState>,
    Query(query): Query<HistoryQuery>,
) -> Result<Json<Value>, ApiError> {
    let entity = query
        .entity_id
        .ok_or_else(|| ApiError::BadRequest("entity_id query parameter is required".to_owned()))?;
    let hours = query.hours.unwrap_or(24).clamp(1, 24 * 30);
    let history = state
        .ha
        .history(&entity, hours)
        .await
        .map_err(|error| match error {
            crate::ha::HaError::Unreachable(message) => {
                ApiError::BadRequest(format!("supervisor unreachable: {message}"))
            }
            crate::ha::HaError::Status { status, message } => {
                ApiError::Internal(format!("supervisor error {status}: {message}"))
            }
        })?;
    Ok(Json(history))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn live_stream(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, std::convert::Infallible>>> {
    let rx = state.ha.live();
    // Emit the current snapshot, then one event per change (awaiting
    // `changed()` — without it the unfold polls hot and floods).
    let stream = futures_util::stream::unfold(rx, |mut rx| async move {
        let data = rx.borrow_and_update().clone();
        let payload = serde_json::to_string(&data).unwrap_or_else(|_| "{}".to_owned());
        let item = Ok(Event::default().data(payload));
        if rx.changed().await.is_err() {
            return None;
        }
        Some((item, rx))
    });
    Sse::new(stream).keep_alive(KeepAlive::default())
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn get_bindings(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<BTreeMap<String, String>>, ApiError> {
    let bindings = state
        .db
        .with(move |conn| store::sensors::list(conn, &id))
        .await?;
    Ok(Json(bindings))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn put_bindings(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<store::sensors::BindingsInput>,
) -> Result<Json<BTreeMap<String, String>>, ApiError> {
    if input
        .bindings
        .values()
        .any(|entity| entity.trim().is_empty())
    {
        return Err(ApiError::BadRequest(
            "entity ids must not be empty".to_owned(),
        ));
    }
    let bridge = state.ha.clone();
    let bindings = state
        .db
        .with(move |conn| -> Result<_, ApiError> {
            store::grows::get(conn, &id)?;
            store::sensors::replace(conn, &id, &input)?;
            let bindings = store::sensors::list(conn, &id)?;
            // The union across all drives the WS subscription.
            let entities = store::sensors::all_bound_entities(conn)?;
            bridge.track_entities(entities);
            Ok(bindings)
        })
        .await?;
    Ok(Json(bindings))
}

/// Captures the bound entities' live states into a log entry's
/// snapshots (auto-capture, spec §4.2). Called by the action create
/// path; failures never block the log entry.
pub async fn capture_into(state: &AppState, grow_id: &str, action_log_id: &str) {
    let live = state.ha.live_now();
    let db = state.db.clone();
    let grow = grow_id.to_owned();
    let action = action_log_id.to_owned();
    let result = db
        .with(move |conn| -> Result<(), crate::db::StoreError> {
            let bindings = store::sensors::list(conn, &grow)?;
            for (role, entity_id) in &bindings {
                if let Some(entity) = live.entities.get(entity_id) {
                    store::sensors::snapshot(conn, &action, role, entity_id, &entity.state)?;
                }
            }
            Ok(())
        })
        .await;
    if let Err(error) = result {
        eprintln!("auto-capture skipped: {error}");
    }
}

/// A raw JSON body with the `application/json` content type.
#[derive(Debug)]
pub struct JsonBytes(pub Vec<u8>);

impl IntoResponse for JsonBytes {
    fn into_response(self) -> Response {
        Response::builder()
            .status(StatusCode::OK)
            .header(header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(self.0))
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
    }
}
