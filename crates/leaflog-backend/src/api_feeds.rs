//! Feed template, fit, plan-day and substrate-profile API routes (M6).

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use leaflog_core::feed::{FitConfig, SubstrateProfile};
use serde::Deserialize;
use serde_json::Value;

use crate::api::ApiError;
use crate::app::AppState;
use crate::store;

/// The feed routes.
pub fn router() -> Router<AppState> {
    Router::new()
        .route(
            "/api/feeds/templates",
            get(list_templates).post(create_template),
        )
        .route(
            "/api/feeds/templates/{id}",
            get(get_template).delete(delete_template),
        )
        .route("/api/substrates", get(list_profiles))
        .route("/api/grows/{id}/feed/fit", get(get_plan).post(fit_feed))
}

/// Query for template listing.
#[derive(Debug, Default, Deserialize)]
struct TemplateQuery {
    /// Filter by substrate label.
    substrate: Option<String>,
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn list_templates(
    State(state): State<AppState>,
    Query(query): Query<TemplateQuery>,
) -> Result<Json<Vec<store::feeds::FeedTemplate>>, ApiError> {
    let templates = state
        .db
        .with(move |conn| store::feeds::list_templates(conn, query.substrate.as_deref()))
        .await?;
    Ok(Json(templates))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn create_template(
    State(state): State<AppState>,
    Json(template): Json<store::feeds::FeedTemplate>,
) -> Result<(StatusCode, Json<store::feeds::FeedTemplate>), ApiError> {
    if template.name.trim().is_empty() {
        return Err(ApiError::BadRequest("name must not be empty".to_owned()));
    }
    let created = state
        .db
        .with(move |conn| -> Result<_, crate::db::StoreError> {
            let id = crate::db::new_id();
            conn.execute(
                "INSERT INTO feed_template (id, name, substrate, source, is_builtin, created_at, updated_at)
                 VALUES (?1, ?2, ?3, ?4, 0, ?5, ?6)",
                rusqlite::params![
                    id,
                    template.name,
                    template.substrate,
                    template.source,
                    crate::db::now_rfc3339(),
                    crate::db::now_rfc3339(),
                ],
            )?;
            for row in &template.rows {
                conn.execute(
                    "INSERT INTO feed_template_row (id, template_id, phase, day_offset, ec_target_us, ph_min, ph_max)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                    rusqlite::params![
                        crate::db::new_id(),
                        id,
                        row.phase.label(),
                        i64::from(row.day_offset),
                        i64::from(row.ec_target_us),
                        row.ph_min,
                        row.ph_max,
                    ],
                )?;
            }
            Ok(template)
        })
        .await?;
    Ok((StatusCode::CREATED, Json(created)))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn get_template(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<store::feeds::FeedTemplate>, ApiError> {
    let template = state
        .db
        .with(move |conn| store::feeds::get_template(conn, &id))
        .await?;
    Ok(Json(template))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn delete_template(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state
        .db
        .with(move |conn| -> Result<(), ApiError> {
            let affected = conn
                .execute(
                    "DELETE FROM feed_template WHERE id = ?1",
                    rusqlite::params![id],
                )
                .map_err(crate::db::StoreError::from)?;
            if affected == 0 {
                return Err(ApiError::NotFound);
            }
            Ok(())
        })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn list_profiles(
    State(state): State<AppState>,
) -> Result<Json<Vec<SubstrateProfile>>, ApiError> {
    let profiles = state.db.with(store::feeds::list_profiles).await?;
    Ok(Json(profiles))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn get_plan(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<leaflog_core::feed::FeedPlanDay>>, ApiError> {
    let days = state
        .db
        .with(move |conn| store::feeds::plan_days(conn, &id))
        .await?;
    Ok(Json(days))
}

/// The fit request: template id + optional config.
#[derive(Debug, Default, Deserialize)]
struct FitRequest {
    /// The template to fit onto the grow.
    pub template_id: String,
    /// EC step clamp override (µS/cm; default 200).
    pub max_ec_step_us: Option<u32>,
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn fit_feed(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(request): Json<FitRequest>,
) -> Result<Json<Value>, ApiError> {
    let result = state
        .db
        .with(move |conn| -> Result<_, ApiError> {
            let grow = store::grows::get(conn, &id)?;
            let strain = grow
                .strain_id
                .as_deref()
                .and_then(|sid| store::strain::get(conn, sid).ok());
            let flip = store::actions::first_event_day(
                conn,
                &id,
                leaflog_core::actions::ActionType::Flip,
                u16::MAX,
            )?;
            let harvest = store::actions::first_event_day(
                conn,
                &id,
                leaflog_core::actions::ActionType::Harvest,
                u16::MAX,
            )?;
            let actuals = leaflog_core::forecast::Actuals {
                flip_day: flip.map(i64::from),
                harvest_day: harvest.map(i64::from),
                trichome_check_day: None,
                germinated: false,
            };
            let input = leaflog_core::forecast::input_for(strain.as_ref(), None, actuals);
            let windows = leaflog_core::forecast::forecast(&input);
            let actual = store::feeds::actual_from_forecast(&windows)
                .ok_or_else(|| ApiError::BadRequest("forecast incomplete".to_owned()))?;

            let template = store::feeds::get_template(conn, &request.template_id)?;
            let config = FitConfig {
                max_ec_step_us: request.max_ec_step_us.unwrap_or(200),
            };
            let result = store::feeds::fit_and_store(conn, &id, &template, actual, config)?;

            // Return both the fit result and the template rows for the
            // before/after diff view.
            let before: Vec<Value> = template
                .rows
                .iter()
                .map(|row| {
                    serde_json::json!({
                        "phase": row.phase.label(),
                        "day_offset": row.day_offset,
                        "ec_target_us": row.ec_target_us,
                        "ph_min": row.ph_min,
                        "ph_max": row.ph_max,
                    })
                })
                .collect();
            Ok(serde_json::json!({
                "before": before,
                "after": result,
            }))
        })
        .await?;
    Ok(Json(result))
}
