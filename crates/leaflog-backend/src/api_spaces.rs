//! JSON API routes for grow spaces (spec §4.1): a space combines
//! equipment catalogue entries and contains grows.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use leaflog_core::equipment::EquipmentItem;
use leaflog_core::grows::Grow;
use leaflog_core::spaces::{GrowSpace, GrowSpaceInput};
use rusqlite::Connection;
use serde::Serialize;

use crate::api::ApiError;
use crate::app::AppState;
use crate::db::StoreError;
use crate::store;

/// The grow-space routes; merged into the API router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/spaces", get(list_spaces).post(create_space))
        .route(
            "/api/spaces/{id}",
            get(get_space).put(update_space).delete(delete_space),
        )
        .route(
            "/api/spaces/{id}/equipment/{equipment_id}",
            post(add_equipment).delete(remove_equipment),
        )
}

/// A grow space with its equipment and grows resolved — the response
/// shape of both the list and the detail endpoint.
///
/// The database is small (a handful of spaces per install); no summary
/// shape is worth the extra type.
#[derive(Debug, Serialize)]
pub struct GrowSpaceDetail {
    /// The space itself.
    pub space: GrowSpace,
    /// Catalogue entries referenced by this space, ordered by name.
    pub equipment: Vec<EquipmentItem>,
    /// Grows inside this space, active first, newest start first.
    pub grows: Vec<Grow>,
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn list_spaces(
    State(state): State<AppState>,
) -> Result<Json<Vec<GrowSpaceDetail>>, ApiError> {
    let details = state
        .db
        .with(move |conn| -> Result<Vec<GrowSpaceDetail>, StoreError> {
            store::spaces::list(conn)?
                .into_iter()
                .map(|space| detail(conn, space))
                .collect()
        })
        .await?;
    Ok(Json(details))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn create_space(
    State(state): State<AppState>,
    Json(input): Json<GrowSpaceInput>,
) -> Result<(StatusCode, Json<GrowSpace>), ApiError> {
    validate_space(&input)?;
    let space = state
        .db
        .with(move |conn| store::spaces::create(conn, &input))
        .await?;
    Ok((StatusCode::CREATED, Json(space)))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn get_space(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<GrowSpaceDetail>, ApiError> {
    let detail = state
        .db
        .with(move |conn| -> Result<GrowSpaceDetail, ApiError> {
            let space = store::spaces::get(conn, &id)?;
            Ok(detail(conn, space)?)
        })
        .await?;
    Ok(Json(detail))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn update_space(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<GrowSpaceInput>,
) -> Result<Json<GrowSpace>, ApiError> {
    validate_space(&input)?;
    let space = state
        .db
        .with(move |conn| store::spaces::update(conn, &id, &input))
        .await?;
    Ok(Json(space))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn delete_space(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state
        .db
        .with(move |conn| store::spaces::delete(conn, &id))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn add_equipment(
    State(state): State<AppState>,
    Path((id, equipment_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    state
        .db
        .with(move |conn| store::spaces::add_equipment(conn, &id, &equipment_id))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn remove_equipment(
    State(state): State<AppState>,
    Path((id, equipment_id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    state
        .db
        .with(move |conn| store::spaces::remove_equipment(conn, &id, &equipment_id))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Resolves one space's equipment and grows.
///
/// # Errors
///
/// Returns a [`StoreError`] on SQLite failures.
fn detail(conn: &Connection, space: GrowSpace) -> Result<GrowSpaceDetail, StoreError> {
    Ok(GrowSpaceDetail {
        equipment: store::equipment::list_for_space(conn, &space.id)?,
        grows: store::grows::list_for_space(conn, &space.id)?,
        space,
    })
}

/// Validates grow-space input shared by create and update.
fn validate_space(input: &GrowSpaceInput) -> Result<(), ApiError> {
    if input.name.trim().is_empty() {
        return Err(ApiError::BadRequest("name must not be empty".to_owned()));
    }
    let dims = [
        ("width_cm", input.width_cm),
        ("depth_cm", input.depth_cm),
        ("height_cm", input.height_cm),
    ];
    for (field, value) in dims {
        if value.is_some_and(|dim| dim <= 0.0) {
            return Err(ApiError::BadRequest(format!("{field} must be positive")));
        }
    }
    Ok(())
}
