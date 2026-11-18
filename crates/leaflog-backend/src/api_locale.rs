//! The locale endpoint (D112): reports the Home Assistant user
//! language to the frontend so the locale chain can start at the
//! user's configured language.
//!
//! Read-only plumbing — the backend never translates, it only
//! forwards the observed config field.

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use serde_json::{Value, json};

use crate::app::AppState;

/// The locale route; merged into the API router.
pub fn router() -> Router<AppState> {
    Router::new().route("/api/locale", get(locale))
}

/// `GET /api/locale`: the user language from the latest documented
/// config fetch, `null` while Home Assistant is unreachable or before
/// the first fetch (the frontend falls through the chain to the
/// browser locale, then the English default).
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn locale(State(state): State<AppState>) -> Json<Value> {
    Json(json!({ "language": state.ha.cached_language() }))
}
