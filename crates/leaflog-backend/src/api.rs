//! JSON API routes for equipment, strains and settings (spec §4.1,
//! §4.4). All routes are under `/api`; the browser is the only client.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use leaflog_core::equipment::{EquipmentCategory, EquipmentInput, EquipmentItem};
use leaflog_core::strain::{Strain, StrainInput};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::app::AppState;
use crate::db::StoreError;
use crate::store::{self, StoreOrNotFound};

/// The API router; merged into the app router before `.with_state`.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/equipment", get(list_equipment).post(create_equipment))
        .route(
            "/api/equipment/{id}",
            get(get_equipment)
                .put(update_equipment)
                .delete(delete_equipment),
        )
        .route("/api/strains", get(list_strains).post(create_strain))
        .route(
            "/api/strains/{id}",
            get(get_strain).put(update_strain).delete(delete_strain),
        )
        .route("/api/strains/{id}/clone", post(clone_strain))
        .route("/api/settings", get(get_settings).put(put_settings))
        .merge(crate::api_grows::router())
        .merge(crate::api_ha::router())
        .merge(crate::api_feeds::router())
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
    /// Something unexpected broke; the message is safe to show.
    #[error("internal error: {0}")]
    Internal(String),
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };
        (status, Json(json!({ "error": self.to_string() }))).into_response()
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
async fn list_equipment(
    State(state): State<AppState>,
    Query(query): Query<EquipmentQuery>,
) -> Result<Json<Vec<EquipmentItem>>, ApiError> {
    let category = query
        .category
        .map(|label| parse_category(&label))
        .transpose()?;
    let items = state
        .db
        .with(move |conn| store::equipment::list(conn, category))
        .await?;
    Ok(Json(items))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn create_equipment(
    State(state): State<AppState>,
    Json(input): Json<EquipmentInput>,
) -> Result<(StatusCode, Json<EquipmentItem>), ApiError> {
    validate_equipment(&input)?;
    let created = state
        .db
        .with(move |conn| store::equipment::create(conn, &input))
        .await?;
    Ok((StatusCode::CREATED, Json(created)))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn get_equipment(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<EquipmentItem>, ApiError> {
    let item = state
        .db
        .with(move |conn| store::equipment::get(conn, &id))
        .await?;
    Ok(Json(item))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn update_equipment(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<EquipmentInput>,
) -> Result<Json<EquipmentItem>, ApiError> {
    validate_equipment(&input)?;
    let updated = state
        .db
        .with(move |conn| store::equipment::update(conn, &id, &input))
        .await?;
    Ok(Json(updated))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn delete_equipment(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state
        .db
        .with(move |conn| store::equipment::delete(conn, &id))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn list_strains(State(state): State<AppState>) -> Result<Json<Vec<Strain>>, ApiError> {
    let strains = state.db.with(store::strain::list).await?;
    Ok(Json(strains))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn create_strain(
    State(state): State<AppState>,
    Json(input): Json<StrainInput>,
) -> Result<(StatusCode, Json<Strain>), ApiError> {
    validate_strain(&input)?;
    let strain = state
        .db
        .with(move |conn| -> Result<Strain, ApiError> {
            let strain = store::strain::from_input(&input);
            store::strain::create(conn, &strain)?;
            Ok(strain)
        })
        .await?;
    Ok((StatusCode::CREATED, Json(strain)))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn get_strain(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Strain>, ApiError> {
    let strain = state
        .db
        .with(move |conn| store::strain::get(conn, &id))
        .await?;
    Ok(Json(strain))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn update_strain(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<StrainInput>,
) -> Result<Json<Strain>, ApiError> {
    validate_strain(&input)?;
    let updated = state
        .db
        .with(move |conn| store::strain::update(conn, &id, &input))
        .await?;
    Ok(Json(updated))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn delete_strain(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state
        .db
        .with(move |conn| store::strain::delete(conn, &id))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn clone_strain(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<Strain>), ApiError> {
    let clone = state
        .db
        .with(move |conn| -> Result<Strain, ApiError> {
            let parent = store::strain::get(conn, &id)?;
            let mut phenotype = parent.clone();
            phenotype.id = crate::db::new_id();
            phenotype.name = format!("{} (my phenotype)", parent.name);
            phenotype.is_phenotype_of = Some(parent.id);
            phenotype.source_url = None;
            phenotype.source_licence = None;
            phenotype.created_at = crate::db::now_rfc3339();
            phenotype.updated_at = phenotype.created_at.clone();
            store::strain::create(conn, &phenotype)?;
            Ok(phenotype)
        })
        .await?;
    Ok((StatusCode::CREATED, Json(clone)))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn get_settings(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let settings = state.db.with(store::settings::list).await?;
    Ok(Json(settings))
}

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

/// Query parameters of `GET /api/equipment`.
#[derive(Debug, Default, Deserialize)]
struct EquipmentQuery {
    /// Filter by category label.
    category: Option<String>,
}

/// Parses and validates an equipment category label.
fn parse_category(label: &str) -> Result<EquipmentCategory, ApiError> {
    EquipmentCategory::all()
        .into_iter()
        .find(|category| category.label() == label)
        .ok_or_else(|| ApiError::BadRequest(format!("unknown equipment category {label}")))
}

/// Validates equipment input shared by create and update.
fn validate_equipment(input: &EquipmentInput) -> Result<(), ApiError> {
    if input.name.trim().is_empty() {
        return Err(ApiError::BadRequest("name must not be empty".to_owned()));
    }
    if input.real_watts.is_some_and(|watts| watts < 0.0) {
        return Err(ApiError::BadRequest(
            "real_watts must not be negative".to_owned(),
        ));
    }
    Ok(())
}

/// Validates strain input shared by create and update.
fn validate_strain(input: &StrainInput) -> Result<(), ApiError> {
    if input.name.trim().is_empty() {
        return Err(ApiError::BadRequest("name must not be empty".to_owned()));
    }
    if input.indica_pct.is_some_and(|pct| pct > 100) {
        return Err(ApiError::BadRequest(
            "indica_pct must be between 0 and 100".to_owned(),
        ));
    }
    if matches!(
        (input.flower_days_min, input.flower_days_max),
        (Some(min), Some(max)) if min > max
    ) {
        return Err(ApiError::BadRequest(
            "flower_days_min must not exceed flower_days_max".to_owned(),
        ));
    }
    if matches!(
        (input.auto_total_days_min, input.auto_total_days_max),
        (Some(min), Some(max)) if min > max
    ) {
        return Err(ApiError::BadRequest(
            "auto_total_days_min must not exceed auto_total_days_max".to_owned(),
        ));
    }
    if input.user_rating.is_some_and(|rating| rating > 5) {
        return Err(ApiError::BadRequest(
            "user_rating must be between 0 and 5".to_owned(),
        ));
    }
    Ok(())
}
