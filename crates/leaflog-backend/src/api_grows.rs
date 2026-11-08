//! JSON API routes for grows, plants, the action log and photos
//! (spec §4.2, §4.3).

use axum::extract::{Multipart, Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use leaflog_core::actions::ActionType;
use leaflog_core::grows::ActionInput;
use leaflog_core::grows::{GrowInput, PlantInput};
use serde::Deserialize;
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;

use crate::api::ApiError;
use crate::app::AppState;
use crate::db::now_rfc3339;
use crate::store::{self, StoreOrNotFound};

/// Maximum accepted upload size per photo (the body limit is 32 MiB).
const MAX_PHOTO_BYTES: usize = 25 * 1024 * 1024;

/// Default storage warning threshold (2 GiB), overridable via the
/// `photo_warn_bytes` setting.
const DEFAULT_PHOTO_WARN_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// The grow-related routes; merged into the API router.
pub fn router() -> Router<AppState> {
    Router::new()
        .route("/api/grows", get(list_grows).post(create_grow))
        .route(
            "/api/grows/{id}",
            get(get_grow).put(update_grow).delete(delete_grow),
        )
        .route(
            "/api/grows/{id}/plants",
            get(list_plants).post(create_plant),
        )
        .route("/api/plants/{id}", axum::routing::delete(delete_plant))
        .route(
            "/api/grows/{id}/actions",
            get(list_actions).post(create_action),
        )
        .route(
            "/api/actions/{id}",
            get(get_action).put(update_action).delete(delete_action),
        )
        .route(
            "/api/grows/{id}/photos",
            get(list_photos).post(upload_photo),
        )
        .route("/api/photos/{id}", axum::routing::delete(delete_photo))
        .route("/api/photos/{id}/original", get(photo_original))
        .route("/api/photos/{id}/thumbnail", get(photo_thumbnail))
        .route("/api/photos/usage", get(photo_usage))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn list_grows(
    State(state): State<AppState>,
) -> Result<Json<Vec<leaflog_core::grows::Grow>>, ApiError> {
    Ok(Json(state.db.with(store::grows::list).await?))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn create_grow(
    State(state): State<AppState>,
    Json(input): Json<GrowInput>,
) -> Result<(StatusCode, Json<leaflog_core::grows::Grow>), ApiError> {
    validate_grow(&input)?;
    if let Some(strain_id) = &input.strain_id {
        state
            .db
            .with({
                let strain_id = strain_id.clone();
                move |conn| store::strain::get(conn, &strain_id)
            })
            .await?;
    }
    let grow = state
        .db
        .with(move |conn| store::grows::create(conn, &input))
        .await?;
    Ok((StatusCode::CREATED, Json(grow)))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn get_grow(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<leaflog_core::grows::Grow>, ApiError> {
    Ok(Json(
        state
            .db
            .with(move |conn| store::grows::get(conn, &id))
            .await?,
    ))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn update_grow(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<GrowInput>,
) -> Result<Json<leaflog_core::grows::Grow>, ApiError> {
    validate_grow(&input)?;
    let grow = state
        .db
        .with(move |conn| store::grows::update(conn, &id, &input))
        .await?;
    Ok(Json(grow))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn delete_grow(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let photos = state.photos.clone();
    state
        .db
        .with(move |conn| -> Result<(), ApiError> {
            store::grows::delete(conn, &id)?;
            photos.remove_grow(&id);
            Ok(())
        })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn list_plants(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<leaflog_core::grows::Plant>>, ApiError> {
    Ok(Json(
        state
            .db
            .with(move |conn| store::grows::list_plants(conn, &id))
            .await?,
    ))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn create_plant(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<PlantInput>,
) -> Result<(StatusCode, Json<leaflog_core::grows::Plant>), ApiError> {
    if input.name.trim().is_empty() {
        return Err(ApiError::BadRequest("name must not be empty".to_owned()));
    }
    let plant = state
        .db
        .with(move |conn| -> Result<_, StoreOrNotFound> {
            store::grows::get(conn, &id)?;
            store::grows::create_plant(conn, &id, &input).map_err(StoreOrNotFound::from)
        })
        .await?;
    Ok((StatusCode::CREATED, Json(plant)))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn delete_plant(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state
        .db
        .with(move |conn| store::grows::delete_plant(conn, &id))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Query parameters of `GET /api/grows/{id}/actions`.
#[derive(Debug, Default, Deserialize)]
struct ActionQuery {
    /// Filter by action type label.
    kind: Option<String>,
    /// Maximum entries (default 200).
    limit: Option<u32>,
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn list_actions(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<ActionQuery>,
) -> Result<Json<Vec<leaflog_core::grows::ActionEntry>>, ApiError> {
    let kind = query
        .kind
        .map(|label| {
            ActionType::all()
                .into_iter()
                .find(|kind| kind.label() == label)
                .ok_or_else(|| ApiError::BadRequest(format!("unknown action type {label}")))
        })
        .transpose()?;
    let limit = query.limit.unwrap_or(200).clamp(1, 1000);
    Ok(Json(
        state
            .db
            .with(move |conn| store::actions::list(conn, &id, kind, limit))
            .await?,
    ))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn create_action(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<ActionInput>,
) -> Result<(StatusCode, Json<leaflog_core::grows::ActionEntry>), ApiError> {
    validate_action(&input)?;
    let grow_id_for_capture = id.clone();
    let entry = state
        .db
        .with(move |conn| -> Result<_, ApiError> {
            let grow = store::grows::get(conn, &id)?;
            let day = day_number(&grow.started_on, &input.occurred_at)?;
            // The entry being created counts as happening on its own
            // day: a flip entry is already "flower".
            let flip = if input.action_type == ActionType::Flip {
                Some(day)
            } else {
                store::actions::first_event_day(conn, &id, ActionType::Flip, day)?
            };
            let harvested = input.action_type == ActionType::Harvest
                || store::actions::first_event_day(conn, &id, ActionType::Harvest, day)?.is_some();
            let phase = leaflog_core::actions::derive_phase(day, flip, harvested);
            let entry = store::actions::from_input(&id, &input, phase, day);
            store::actions::create(conn, &entry)?;
            Ok(entry)
        })
        .await?;
    // Auto-capture bound sensors (spec 4.2); never blocks the entry.
    crate::api_ha::capture_into(&state, &grow_id_for_capture, &entry.id).await;
    Ok((StatusCode::CREATED, Json(entry)))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn get_action(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<leaflog_core::grows::ActionEntry>, ApiError> {
    Ok(Json(
        state
            .db
            .with(move |conn| store::actions::get(conn, &id))
            .await?,
    ))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn update_action(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<ActionInput>,
) -> Result<Json<leaflog_core::grows::ActionEntry>, ApiError> {
    validate_action(&input)?;
    let entry = state
        .db
        .with(move |conn| -> Result<_, ApiError> {
            let existing = store::actions::get(conn, &id)?;
            let grow = store::grows::get(conn, &existing.grow_id)?;
            let day = day_number(&grow.started_on, &input.occurred_at)?;
            let flip = store::actions::first_event_day(conn, &grow.id, ActionType::Flip, day)?;
            let harvested =
                store::actions::first_event_day(conn, &grow.id, ActionType::Harvest, day)?
                    .is_some();
            let phase = leaflog_core::actions::derive_phase(day, flip, harvested);
            store::actions::update(conn, &id, &input, phase, day).map_err(ApiError::from)
        })
        .await?;
    Ok(Json(entry))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn delete_action(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    state
        .db
        .with(move |conn| store::actions::delete(conn, &id))
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn list_photos(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<Vec<leaflog_core::grows::Photo>>, ApiError> {
    Ok(Json(
        state
            .db
            .with(move |conn| store::photos::list(conn, &id))
            .await?,
    ))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::too_many_lines)] // multipart field walk, not logic; see docs/DECISIONS.md
async fn upload_photo(
    State(state): State<AppState>,
    Path(id): Path<String>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<leaflog_core::grows::Photo>), ApiError> {
    let mut file: Option<Vec<u8>> = None;
    let mut plant_id: Option<String> = None;
    let mut action_log_id: Option<String> = None;
    let mut tags: Vec<String> = Vec::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| ApiError::BadRequest(error.to_string()))?
    {
        match field.name() {
            Some("file") => {
                let bytes = field
                    .bytes()
                    .await
                    .map_err(|error| ApiError::BadRequest(error.to_string()))?;
                file = Some(bytes.to_vec());
            }
            Some("plant_id") => {
                plant_id = non_empty_field(field).await;
            }
            Some("action_log_id") => {
                action_log_id = non_empty_field(field).await;
            }
            Some("tags") => {
                if let Ok(text) = field.text().await {
                    tags = text
                        .split(',')
                        .map(str::trim)
                        .filter(|tag| !tag.is_empty())
                        .map(str::to_owned)
                        .collect();
                }
            }
            _ => {}
        }
    }
    let bytes =
        file.ok_or_else(|| ApiError::BadRequest("multipart field 'file' is required".to_owned()))?;
    if bytes.len() > MAX_PHOTO_BYTES {
        return Err(ApiError::BadRequest(format!(
            "photo exceeds {} MiB",
            MAX_PHOTO_BYTES / (1024 * 1024)
        )));
    }

    let photos = state.photos.clone();
    let photo = state
        .db
        .with(move |conn| -> Result<_, ApiError> {
            let grow = store::grows::get(conn, &id)?;
            // Blocking image work inside the DB task keeps one
            // transaction-ish flow; the connection lock is not held
            // across the pipeline (store() is pure CPU + file I/O).
            let stored = photos
                .store(&id, &bytes)
                .map_err(|error| ApiError::BadRequest(error.to_string()))?;
            let captured_at = stored.captured_at.clone().unwrap_or_else(now_rfc3339);
            let day = day_number(&grow.started_on, &captured_at)?;
            let flip = store::actions::first_event_day(conn, &id, ActionType::Flip, day)?;
            let harvested =
                store::actions::first_event_day(conn, &id, ActionType::Harvest, day)?.is_some();
            let phase = leaflog_core::actions::derive_phase(day, flip, harvested);
            let new = store::photos::NewPhoto {
                grow_id: id.clone(),
                plant_id,
                action_log_id,
                captured_at,
                grow_day: day,
                phase: Some(phase),
                tags,
                original_name: stored.original_name,
                thumbnail_name: Some(stored.thumbnail_name),
                bytes_stored: stored.bytes_stored,
            };
            store::photos::create(conn, &new).map_err(ApiError::from)
        })
        .await?;
    Ok((StatusCode::CREATED, Json(photo)))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn delete_photo(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let photos = state.photos.clone();
    state
        .db
        .with(move |conn| -> Result<(), ApiError> {
            let photo = store::photos::get(conn, &id)?;
            store::photos::delete(conn, &id)?;
            photos.remove(
                &photo.grow_id,
                &photo.original_name,
                photo.thumbnail_name.as_deref(),
            );
            Ok(())
        })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn photo_original(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let photos = state.photos.clone();
    let bytes = state
        .db
        .with(move |conn| -> Result<Vec<u8>, ApiError> {
            let photo = store::photos::get(conn, &id)?;
            photos
                .read_original(&photo.grow_id, &photo.original_name)
                .map_err(|error| ApiError::Internal(error.to_string()))
        })
        .await?;
    Ok(([(header::CONTENT_TYPE, "image/jpeg")], bytes).into_response())
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn photo_thumbnail(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let photos = state.photos.clone();
    let bytes = state
        .db
        .with(move |conn| -> Result<Vec<u8>, ApiError> {
            let photo = store::photos::get(conn, &id)?;
            let Some(name) = photo.thumbnail_name.clone() else {
                return Err(ApiError::NotFound);
            };
            let bytes = photos
                .read_thumbnail(&photo.grow_id, &name)
                .map_err(|error| ApiError::Internal(error.to_string()))?;
            // A lazy regeneration writes the file but the row keeps its
            // name; nothing to update unless it changed.
            if photo.thumbnail_name.as_deref() != Some(&name) {
                store::photos::set_thumbnail(conn, &id, Some(&name))?;
            }
            Ok(bytes)
        })
        .await?;
    Ok(([(header::CONTENT_TYPE, "image/webp")], bytes).into_response())
}

/// Usage summary for the storage-size warning (spec §4.3).
#[derive(Debug, Clone, Copy, serde::Serialize)]
pub struct PhotoUsage {
    /// Total stored original bytes.
    pub total_bytes: u64,
    /// Configured warning threshold in bytes.
    pub warn_bytes: u64,
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn photo_usage(State(state): State<AppState>) -> Result<Json<PhotoUsage>, ApiError> {
    let usage = state
        .db
        .with(|conn| -> Result<PhotoUsage, StoreOrNotFound> {
            let total = store::photos::total_bytes(conn)?;
            let warn = store::settings::get(conn, "photo_warn_bytes")
                .ok()
                .and_then(|value| value.as_u64())
                .unwrap_or(DEFAULT_PHOTO_WARN_BYTES);
            Ok(PhotoUsage {
                total_bytes: total,
                warn_bytes: warn,
            })
        })
        .await?;
    Ok(Json(usage))
}

/// Reads an optional text field.
async fn non_empty_field(field: axum::extract::multipart::Field<'_>) -> Option<String> {
    let text = field.text().await.ok()?;
    let trimmed = text.trim().to_owned();
    (!trimmed.is_empty()).then_some(trimmed)
}

/// Days between a grow start (`YYYY-MM-DD`) and an RFC 3339 instant.
fn day_number(started_on: &str, occurred_at: &str) -> Result<u16, ApiError> {
    let date_format = format_description!("[year]-[month]-[day]");
    let start = time::Date::parse(started_on, &date_format)
        .map_err(|error| ApiError::BadRequest(format!("invalid grow start date: {error}")))?;
    let moment = time::OffsetDateTime::parse(occurred_at, &Rfc3339)
        .map_err(|error| ApiError::BadRequest(format!("invalid timestamp: {error}")))?;
    let days = (moment.date() - start).whole_days();
    u16::try_from(days.max(0)).map_err(|_| ApiError::BadRequest("grow day out of range".to_owned()))
}

/// Shared grow validation.
fn validate_grow(input: &GrowInput) -> Result<(), ApiError> {
    if input.name.trim().is_empty() {
        return Err(ApiError::BadRequest("name must not be empty".to_owned()));
    }
    let date_format = format_description!("[year]-[month]-[day]");
    if time::Date::parse(&input.started_on, &date_format).is_err() {
        return Err(ApiError::BadRequest(
            "started_on must be YYYY-MM-DD".to_owned(),
        ));
    }
    if input.substrate.trim().is_empty() {
        return Err(ApiError::BadRequest(
            "substrate must not be empty".to_owned(),
        ));
    }
    Ok(())
}

/// Shared action validation.
fn validate_action(input: &ActionInput) -> Result<(), ApiError> {
    if time::OffsetDateTime::parse(&input.occurred_at, &Rfc3339).is_err() {
        return Err(ApiError::BadRequest(
            "occurred_at must be RFC 3339".to_owned(),
        ));
    }
    if input
        .details
        .input_ph
        .is_some_and(|ph| !(0.0..=14.0).contains(&ph))
    {
        return Err(ApiError::BadRequest("input_ph out of range".to_owned()));
    }
    if input
        .details
        .runoff_ph
        .is_some_and(|ph| !(0.0..=14.0).contains(&ph))
    {
        return Err(ApiError::BadRequest("runoff_ph out of range".to_owned()));
    }
    Ok(())
}
