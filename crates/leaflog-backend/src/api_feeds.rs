//! Feeding-schedule, plan and substrate-profile API routes (M6).

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use leaflog_core::feed::{FeedPlan, FeedSchedule, SubstrateProfile};

use crate::api::ApiError;
use crate::app::AppState;
use crate::store;

/// The schedule, plan and substrate routes.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/schedules", get(list_schedules).post(create_schedule))
        .route(
            "/api/schedules/{id}",
            get(get_schedule)
                .put(update_schedule)
                .delete(delete_schedule),
        )
        .route("/api/schedules/{id}/duplicate", post(duplicate_schedule))
        .route("/api/schedules/{id}/grows", get(list_grow_ids))
        .route("/api/grows/{id}/feed/plan", get(get_plan))
        .route("/api/grows/{id}/feed/refit", post(refit_feed))
        .route("/api/substrates", get(list_profiles))
}

/// Maps a schedule validation failure to a client-facing message.
fn validation_message(error: leaflog_core::feed::ScheduleValidationError) -> String {
    use leaflog_core::feed::ScheduleValidationError as Error;
    match error {
        Error::Name => "the schedule name must not be empty".to_owned(),
        Error::FlowerWeeks => "the flowering-week count is out of range".to_owned(),
        Error::TooManyProducts => "too many product rows for one schedule".to_owned(),
        Error::ProductName => "every product row needs a name".to_owned(),
    }
}

async fn list_schedules(
    State(state): State<AppState>,
) -> Result<Json<Vec<store::feeds::ScheduleSummary>>, ApiError> {
    let summaries = state.db.with(store::feeds::list_schedules).await?;
    Ok(Json(summaries))
}

#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn create_schedule(
    State(state): State<AppState>,
    Json(schedule): Json<FeedSchedule>,
) -> Result<(StatusCode, Json<FeedSchedule>), ApiError> {
    schedule
        .validate()
        .map_err(|error| ApiError::BadRequest(validation_message(error)))?;
    let created = state
        .db
        .with(move |conn| -> Result<FeedSchedule, crate::db::StoreError> {
            let mut schedule = schedule;
            schedule.id = crate::db::new_id();
            schedule.created_at = crate::db::now_rfc3339();
            schedule.updated_at = schedule.created_at.clone();
            for product in &mut schedule.products {
                if product.id.is_empty() {
                    product.id = crate::db::new_id();
                }
            }
            store::feeds::put_schedule(conn, &schedule)?;
            Ok(schedule)
        })
        .await?;
    Ok((StatusCode::CREATED, Json(created)))
}

#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
/// The ids of the grows currently referencing a schedule: the edit
/// page's refit loop walks this list after a schedule save so the
/// frozen plans re-fit onto the new chart.
async fn list_grow_ids(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<String>>, ApiError> {
    let ids = state
        .db
        .with(move |conn| store::feeds::schedule_grow_ids(conn, &id))
        .await?;
    Ok(Json(ids))
}

async fn get_schedule(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<FeedSchedule>, ApiError> {
    let schedule = state
        .db
        .with(move |conn| store::feeds::get_schedule(conn, &id))
        .await?;
    Ok(Json(schedule))
}

#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn update_schedule(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(schedule): Json<FeedSchedule>,
) -> Result<Json<FeedSchedule>, ApiError> {
    schedule
        .validate()
        .map_err(|error| ApiError::BadRequest(validation_message(error)))?;
    let updated = state
        .db
        .with(move |conn| -> Result<FeedSchedule, crate::api::ApiError> {
            let mut schedule = schedule;
            let existing = store::feeds::get_schedule(conn, &id)?;
            schedule.id = id;
            schedule.created_at = existing.created_at;
            schedule.updated_at = crate::db::now_rfc3339();
            for product in &mut schedule.products {
                if product.id.is_empty() {
                    product.id = crate::db::new_id();
                }
            }
            store::feeds::put_schedule(conn, &schedule)?;
            Ok(schedule)
        })
        .await?;
    Ok(Json(updated))
}

#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn delete_schedule(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state
        .db
        .with(move |conn| store::feeds::delete_schedule(conn, &id))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn duplicate_schedule(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<FeedSchedule>), ApiError> {
    let copy = state
        .db
        .with(move |conn| store::feeds::duplicate_schedule(conn, &id))
        .await?;
    Ok((StatusCode::CREATED, Json(copy)))
}

#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn get_plan(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Option<FeedPlan>>, ApiError> {
    let plan = state
        .db
        .with(move |conn| store::feeds::plan(conn, &id))
        .await?;
    Ok(Json(plan))
}

#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
/// Fits the grow's assigned schedule to its timeline and stores the
/// plan. `Ok(None)` when the grow has no schedule or its forecast is
/// incomplete (the stored plan stays untouched); `Err` on store
/// failures. The shared engine behind the assign-time fit and the
/// explicit refit endpoint.
pub(crate) fn fit_grow(
    conn: &rusqlite::Connection,
    grow_id: &str,
) -> Result<Option<leaflog_core::feed::FeedPlan>, crate::api::ApiError> {
    let grow = store::grows::get(conn, grow_id)?;
    let Some(schedule_id) = grow.feed_schedule_id.clone() else {
        return Ok(None);
    };
    let actuals = crate::api_phase::forecast_actuals(conn, grow_id)?;
    let input =
        leaflog_core::forecast::input_for(grow.photoperiod, grow.planned_veg_weeks, actuals);
    let windows = leaflog_core::forecast::forecast(&input);
    let Some(actual) = store::feeds::actual_from_forecast(&windows) else {
        return Ok(None);
    };
    let schedule = store::feeds::get_schedule(conn, &schedule_id)?;
    Ok(Some(store::feeds::fit_and_store(
        conn, grow_id, &schedule, actual,
    )?))
}

#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn refit_feed(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<FeedPlan>, ApiError> {
    let plan = state
        .db
        .with(move |conn| -> Result<FeedPlan, crate::api::ApiError> {
            let plan = fit_grow(conn, &id)?
                .ok_or_else(|| ApiError::BadRequest("the grow has no schedule".to_owned()))?;
            Ok(plan)
        })
        .await?;
    Ok(Json(plan))
}

#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn list_profiles(
    State(state): State<AppState>,
) -> Result<Json<Vec<SubstrateProfile>>, ApiError> {
    let profiles = state.db.with(store::feeds::list_profiles).await?;
    Ok(Json(profiles))
}
