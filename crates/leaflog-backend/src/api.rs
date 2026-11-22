//! JSON API routes for settings and the onboarding state (spec §4.12).
//! All routes are under `/api`; the browser is the only client.
//!
//! The grow's own routes live in `api_grows` and friends, merged below.
//! The strain catalogue (D075) and the equipment catalogue plus grow
//! spaces (D078) left with the feature strip: their routes are gone
//! and answer 404; see docs/DECISIONS.md for the decision records.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{Value, json};

use crate::app::AppState;
use crate::catalog::{self, Message};
use crate::db::StoreError;
use crate::store::{self, StoreOrNotFound};

/// The API router; merged into the app router before `.with_state`.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/settings", get(get_settings).put(put_settings))
        .route("/api/onboarding", get(onboarding_state))
        .merge(crate::api_grows::router())
        .merge(crate::api_ha::router())
        .merge(crate::api_feeds::router())
        .merge(crate::api_env::router())
        .merge(crate::api_chart::router())
        .merge(crate::api_story::router())
        .merge(crate::api_history::router())
        .merge(crate::api_locale::router())
}

/// An API failure mapped to an HTTP status and a JSON error body.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// The addressed resource does not exist.
    #[error("not found")]
    NotFound,
    /// The request body or query is invalid.
    #[error("bad request: {0}")]
    BadRequest(String),
    /// The request conflicts with the resource's state — used for
    /// mutating a bundled (read-only) record.
    #[error("conflict: {0}")]
    Conflict(String),
    /// Something unexpected broke; the message is safe to show.
    #[error("internal error: {0}")]
    Internal(String),
}

impl ApiError {
    /// The catalog message for this error: the wrapper phrase is
    /// keyed, the detail rides along in English (D112).
    fn message(&self) -> Message<'_> {
        match self {
            Self::NotFound => Message::NotFound,
            Self::BadRequest(detail) => Message::BadRequest { detail },
            Self::Conflict(detail) => Message::Conflict { detail },
            Self::Internal(detail) => Message::Internal { detail },
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        // The wrapper phrase follows the HA language (D112); the
        // detail payload stays English.
        let error = catalog::render(self.message(), catalog::current_locale());
        (status, Json(json!({ "error": error }))).into_response()
    }
}

impl From<StoreOrNotFound> for ApiError {
    fn from(error: StoreOrNotFound) -> Self {
        match error {
            StoreOrNotFound::NotFound(_) => Self::NotFound,
            StoreOrNotFound::Store(store_error) => Self::Internal(store_error.to_string()),
        }
    }
}

impl From<StoreError> for ApiError {
    fn from(error: StoreError) -> Self {
        Self::Internal(error.to_string())
    }
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn get_settings(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let settings = state.db.with(store::settings::list).await?;
    Ok(Json(settings))
}

/// The settings keys this app reads (see `docs/SPEC.md` §4 and the
/// audit D145 allow-list): every other key is rejected with a 400 so
/// junk values cannot accumulate or change behaviour later.
const KNOWN_SETTINGS: &[&str] = &[
    "auto_log_ha_events",
    "display_timezone",
    "env_alert_notifications",
    "language",
    "onboarding_dismissed",
    "photo_warn_bytes",
    "quiet_hours",
    "snapshot_retention_months",
    "soil_moisture_band",
];

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn put_settings(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, ApiError> {
    let Value::Object(map) = body else {
        return Err(ApiError::BadRequest(
            "settings body must be a JSON object".to_owned(),
        ));
    };
    if let Some(unknown) = map
        .keys()
        .find(|key| !KNOWN_SETTINGS.contains(&key.as_str()))
    {
        return Err(ApiError::BadRequest(format!(
            "unknown setting {unknown:?}; known keys: {}",
            KNOWN_SETTINGS.join(", ")
        )));
    }
    let settings = state
        .db
        .with(move |conn| {
            for (key, value) in &map {
                store::settings::put(conn, key, value)?;
            }
            store::settings::list(conn)
        })
        .await?;
    Ok(Json(settings))
}

/// `GET /api/onboarding` — the first-run wizard state (spec §4.12):
/// the wizard shows while there are no grows and no prior dismissal.
/// Dismissal and restart are the `onboarding_dismissed` settings flag
/// (`PUT /api/settings`).
// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn onboarding_state(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let report = state
        .db
        .with(|conn| -> Result<Value, StoreError> {
            let grows = store::grows::list(conn)?.len();
            let dismissed = matches!(
                store::settings::get(conn, "onboarding_dismissed"),
                Ok(Value::Bool(true))
            );
            Ok(json!({
                "show_wizard": grows == 0 && !dismissed,
                "dismissed": dismissed,
                "has_grows": grows > 0,
            }))
        })
        .await?;
    Ok(Json(report))
}
