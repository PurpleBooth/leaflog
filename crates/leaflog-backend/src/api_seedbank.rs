//! Seed-bank API routes (spec §4.13): owned seed batches with their
//! §5.4 viability window computed per storage method.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use leaflog_core::seedbank::{SeedBatchInput, SeedBatchView};

use crate::api::ApiError;
use crate::app::AppState;
use crate::store;

/// The seed-bank routes; merged into the API router.
pub fn router() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/api/seed-batches", get(list_batches).post(create_batch))
        .route(
            "/api/seed-batches/{id}",
            get(get_batch).put(update_batch).delete(delete_batch),
        )
}

/// Today as `YYYY-MM-DD` (UTC): viability is assessed against now.
fn today_ymd() -> String {
    use time::OffsetDateTime;
    let now = OffsetDateTime::now_utc().date();
    format!(
        "{:04}-{:02}-{:02}",
        now.year(),
        u8::from(now.month()),
        now.day()
    )
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn list_batches(State(state): State<AppState>) -> Result<Json<Vec<SeedBatchView>>, ApiError> {
    let today = today_ymd();
    let views = state
        .db
        .with(move |conn| -> Result<Vec<SeedBatchView>, ApiError> {
            let batches = store::seedbank::list(conn)?;
            Ok(batches
                .into_iter()
                .map(|batch| SeedBatchView::new(batch, &today))
                .collect())
        })
        .await?;
    Ok(Json(views))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn create_batch(
    State(state): State<AppState>,
    Json(input): Json<SeedBatchInput>,
) -> Result<(StatusCode, Json<SeedBatchView>), ApiError> {
    validate_batch(&input)?;
    let today = today_ymd();
    let view = state
        .db
        .with(move |conn| -> Result<SeedBatchView, ApiError> {
            if let Some(strain_id) = &input.strain_id {
                store::strain::get(conn, strain_id)?;
            }
            let batch = store::seedbank::create(conn, &input)?;
            Ok(SeedBatchView::new(batch, &today))
        })
        .await?;
    Ok((StatusCode::CREATED, Json(view)))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn get_batch(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<SeedBatchView>, ApiError> {
    let today = today_ymd();
    let view = state
        .db
        .with(move |conn| -> Result<SeedBatchView, ApiError> {
            let batch = store::seedbank::get(conn, &id)?;
            Ok(SeedBatchView::new(batch, &today))
        })
        .await?;
    Ok(Json(view))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn update_batch(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<SeedBatchInput>,
) -> Result<Json<SeedBatchView>, ApiError> {
    validate_batch(&input)?;
    let today = today_ymd();
    let view = state
        .db
        .with(move |conn| -> Result<SeedBatchView, ApiError> {
            if let Some(strain_id) = &input.strain_id {
                store::strain::get(conn, strain_id)?;
            }
            let batch = store::seedbank::update(conn, &id, &input)?;
            Ok(SeedBatchView::new(batch, &today))
        })
        .await?;
    Ok(Json(view))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn delete_batch(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state
        .db
        .with(move |conn| -> Result<(), ApiError> {
            store::seedbank::delete(conn, &id)?;
            Ok(())
        })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Validates batch input shared by create and update.
fn validate_batch(input: &SeedBatchInput) -> Result<(), ApiError> {
    if !leaflog_core::seedbank::valid_date(&input.acquisition_on) {
        return Err(ApiError::BadRequest(
            "acquisition_on must be a YYYY-MM-DD date".to_owned(),
        ));
    }
    Ok(())
}
