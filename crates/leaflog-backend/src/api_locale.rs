//! The locale endpoint (D112): reports the language the locale chain
//! should start at — the settings override (SPEC §5.3) when set, else
//! the Home Assistant user language.
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

/// `GET /api/locale`: the settings `language` override (SPEC §5.3,
/// wired by D145) when set, else the user language from the latest
/// documented config fetch, `null` when neither is known (the
/// frontend falls through the chain to the browser locale, then the
/// English default).
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn locale(State(state): State<AppState>) -> Json<Value> {
    let override_language = state
        .db
        .with(|conn| {
            Ok::<_, crate::db::StoreError>(
                crate::store::settings::get(conn, "language")
                    .ok()
                    .and_then(|value| value.as_str().map(str::to_owned)),
            )
        })
        .await
        .ok()
        .flatten();
    Json(json!({
        "language": override_language.or_else(|| state.ha.cached_language()),
        "unit_system": state.ha.cached_units()
    }))
}
