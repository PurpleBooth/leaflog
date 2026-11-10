//! JSON API routes for grows, plants, the action log and photos
//! (spec §4.2, §4.3).

use axum::extract::{Multipart, Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use leaflog_core::actions::ActionType;
use leaflog_core::grows::ActionInput;
use leaflog_core::grows::{GrowInput, Plant, PlantInput};
use serde::Deserialize;
use time::format_description::well_known::Rfc3339;
use time::macros::format_description;

use crate::api::ApiError;
use crate::app::AppState;
use crate::app::canonical_timezone;
use crate::app::day_number_in;
use crate::db::StoreError;
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
        .route(
            "/api/plants/{id}",
            axum::routing::put(update_plant).delete(delete_plant),
        )
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
        .route("/api/grows/{id}/forecast", get(get_forecast))
        .route("/api/grows/{id}/yield", get(get_yield))
        .route("/api/grows/{id}/writeback", post(writeback_grow))
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
    if let Some(grow_space_id) = &input.grow_space_id {
        state
            .db
            .with({
                let grow_space_id = grow_space_id.clone();
                move |conn| store::spaces::get(conn, &grow_space_id)
            })
            .await?;
    }
    if let Some(seed_batch_id) = &input.seed_batch_id {
        state
            .db
            .with({
                let seed_batch_id = seed_batch_id.clone();
                move |conn| store::seedbank::get(conn, &seed_batch_id)
            })
            .await?;
    }
    if let Some(feed_template_id) = &input.feed_template_id {
        state
            .db
            .with({
                let feed_template_id = feed_template_id.clone();
                move |conn| store::feeds::get_template(conn, &feed_template_id)
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
    if let Some(grow_space_id) = &input.grow_space_id {
        state
            .db
            .with({
                let grow_space_id = grow_space_id.clone();
                move |conn| store::spaces::get(conn, &grow_space_id)
            })
            .await?;
    }
    if let Some(seed_batch_id) = &input.seed_batch_id {
        state
            .db
            .with({
                let seed_batch_id = seed_batch_id.clone();
                move |conn| store::seedbank::get(conn, &seed_batch_id)
            })
            .await?;
    }
    if let Some(feed_template_id) = &input.feed_template_id {
        state
            .db
            .with({
                let feed_template_id = feed_template_id.clone();
                move |conn| store::feeds::get_template(conn, &feed_template_id)
            })
            .await?;
    }
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
            let plant = store::grows::create_plant(conn, &id, &input)?;
            // Starting a plant from a linked batch decrements the count
            // (spec §4.13); manual edits always win, and the decrement
            // clamps at zero.
            if let Ok(Some(batch_id)) = store::seedbank::batch_of_grow(conn, &id) {
                store::seedbank::decrement(conn, &batch_id, 1)?;
            }
            Ok(plant)
        })
        .await?;
    Ok((StatusCode::CREATED, Json(plant)))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn update_plant(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Json(input): Json<PlantInput>,
) -> Result<Json<Plant>, ApiError> {
    if input.name.trim().is_empty() {
        return Err(ApiError::BadRequest("name must not be empty".to_owned()));
    }
    for weight in [input.wet_g, input.dry_g, input.trimmed_g] {
        if weight.is_some_and(|grams| grams < 0.0) {
            return Err(ApiError::BadRequest(
                "harvest weights must not be negative".to_owned(),
            ));
        }
    }
    let plant = state
        .db
        .with(move |conn| store::grows::update_plant(conn, &id, &input))
        .await?;
    Ok(Json(plant))
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
    Json(mut input): Json<ActionInput>,
) -> Result<(StatusCode, Json<leaflog_core::grows::ActionEntry>), ApiError> {
    validate_action(&mut input)?;
    let tz = canonical_timezone(&state).await;
    let grow_id_for_capture = id.clone();
    let entry = state
        .db
        .with(move |conn| -> Result<_, ApiError> {
            let grow = store::grows::get(conn, &id)?;
            let day =
                day_number_in(&grow.started_on, &input.occurred_at, &tz).ok_or_else(|| {
                    ApiError::BadRequest("invalid occurred_at or started_on date".to_owned())
                })?;
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
    Json(mut input): Json<ActionInput>,
) -> Result<Json<leaflog_core::grows::ActionEntry>, ApiError> {
    validate_action(&mut input)?;
    let tz = canonical_timezone(&state).await;
    let entry = state
        .db
        .with(move |conn| -> Result<_, ApiError> {
            let existing = store::actions::get(conn, &id)?;
            let grow = store::grows::get(conn, &existing.grow_id)?;
            let day =
                day_number_in(&grow.started_on, &input.occurred_at, &tz).ok_or_else(|| {
                    ApiError::BadRequest("invalid occurred_at or started_on date".to_owned())
                })?;
            // The entry being written counts as happening on its own
            // day, as in `create_action`: `first_event_day` still sees
            // the row's old stored type/day, so an incoming flip or
            // harvest must be special-cased or it cannot affect the
            // re-derived phase.
            let flip = if input.action_type == ActionType::Flip {
                Some(day)
            } else {
                store::actions::first_event_day(conn, &grow.id, ActionType::Flip, day)?
            };
            let harvested = input.action_type == ActionType::Harvest
                || store::actions::first_event_day(conn, &grow.id, ActionType::Harvest, day)?
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

/// The forecast response: windows + the grow's day count.
#[derive(Debug, serde::Serialize)]
pub struct ForecastResponse {
    /// Days since the grow start (today marker for the UI).
    pub today_day: u16,
    /// Phase windows in chronological order.
    pub windows: Vec<leaflog_core::forecast::PhaseWindow>,
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn get_forecast(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<ForecastResponse>, ApiError> {
    let tz = canonical_timezone(&state).await;
    let response = state
        .db
        .with(move |conn| -> Result<ForecastResponse, ApiError> {
            let grow = store::grows::get(conn, &id)?;
            let strain = grow
                .strain_id
                .as_deref()
                .and_then(|strain_id| store::strain::get(conn, strain_id).ok());
            let flip = store::actions::first_event_day(conn, &id, ActionType::Flip, u16::MAX)?;
            let harvest =
                store::actions::first_event_day(conn, &id, ActionType::Harvest, u16::MAX)?;
            let trichome =
                store::actions::first_event_day(conn, &id, ActionType::TrichomeCheck, u16::MAX)?;
            let actuals = leaflog_core::forecast::Actuals {
                flip_day: flip.map(i64::from),
                harvest_day: harvest.map(i64::from),
                trichome_check_day: trichome.map(i64::from),
                germinated: false,
            };
            let input = leaflog_core::forecast::input_for(strain.as_ref(), None, actuals);
            let windows = leaflog_core::forecast::forecast(&input);
            let today_day = day_number_in(&grow.started_on, &crate::db::now_rfc3339(), &tz)
                .ok_or_else(|| ApiError::BadRequest("invalid started_on date".to_owned()))?;
            Ok(ForecastResponse { today_day, windows })
        })
        .await?;
    Ok(Json(response))
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
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn get_yield(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<leaflog_core::grows::YieldSummary>, ApiError> {
    let summary = state
        .db
        .with(
            move |conn| -> Result<leaflog_core::grows::YieldSummary, StoreOrNotFound> {
                let grow = store::grows::get(conn, &id)?;
                let plants = store::grows::list_plants(conn, &grow.id)?;
                Ok(leaflog_core::grows::YieldSummary::of(&grow.id, plants))
            },
        )
        .await?;
    Ok(Json(summary))
}

/// `POST /api/grows/{id}/writeback` — copies the grow's achieved yield
/// into its strain's yield notes (spec §4.4, §4.14). User-owned strains
/// only: a bundled strain is read-only and answers 409.
// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn writeback_grow(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<leaflog_core::strain::Strain>, ApiError> {
    let strain = state
        .db
        .with(
            move |conn| -> Result<leaflog_core::strain::Strain, ApiError> {
                let grow = store::grows::get(conn, &id)?;
                let plants = store::grows::list_plants(conn, &grow.id)?;
                let summary = leaflog_core::grows::YieldSummary::of(&grow.id, plants);
                let strain_id = grow.strain_id.clone().ok_or_else(|| {
                    ApiError::BadRequest("the grow has no strain to write back to".to_owned())
                })?;
                let mut strain = store::strain::get(conn, &strain_id)?;
                if strain.is_builtin {
                    return Err(ApiError::Conflict(
                        "bundled strain is read-only; clone it as your phenotype and write back \
                     there"
                            .to_owned(),
                    ));
                }
                // One line per grow, replaced on re-write so the write-back
                // is idempotent.
                let line = format!(
                    "achieved: {} g dry, {} g wet, {} g trimmed over {} plant(s) [grow:{}]",
                    summary
                        .total_dry_g
                        .map_or_else(|| "-".to_owned(), |grams| format!("{grams:.0}")),
                    summary
                        .total_wet_g
                        .map_or_else(|| "-".to_owned(), |grams| format!("{grams:.0}")),
                    summary
                        .total_trimmed_g
                        .map_or_else(|| "-".to_owned(), |grams| format!("{grams:.0}")),
                    summary.plants.len(),
                    grow.id
                );
                let marker = format!("[grow:{}]", grow.id);
                let notes = strain.yield_notes.clone().unwrap_or_default();
                let updated_lines: Vec<String> = notes
                    .lines()
                    .filter(|existing| !existing.contains(&marker))
                    .map(str::to_owned)
                    .chain([line])
                    .collect();
                strain.yield_notes = Some(updated_lines.join("\n"));
                // Rebuild the update payload from the strain and keep the
                // evidence map the grower curated.
                let mut input = leaflog_core::strain::StrainInput::from_strain(&strain);
                input.yield_notes.clone_from(&strain.yield_notes);
                Ok(store::strain::update(conn, &strain.id, &input)?)
            },
        )
        .await?;
    Ok(Json(strain))
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::too_many_lines)] // multipart field walk, not logic; see docs/DECISIONS.md
async fn upload_photo(
    State(state): State<AppState>,
    Path(id): Path<String>,
    mut multipart: Multipart,
) -> Result<(StatusCode, Json<leaflog_core::grows::Photo>), ApiError> {
    let tz = canonical_timezone(&state).await;
    let mut files: Vec<Vec<u8>> = Vec::new();
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
                if bytes.len() > MAX_PHOTO_BYTES {
                    return Err(ApiError::BadRequest(format!(
                        "photo exceeds {} MiB",
                        MAX_PHOTO_BYTES / (1024 * 1024)
                    )));
                }
                files.push(bytes.to_vec());
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
    if files.is_empty() {
        return Err(ApiError::BadRequest(
            "multipart field 'file' is required".to_owned(),
        ));
    }

    let photos = state.photos.clone();
    let photo = state
        .db
        .with(move |conn| -> Result<_, ApiError> {
            let grow = store::grows::get(conn, &id)?;
            // One row per uploaded part: the frontend appends every
            // selected photo under the same "file" field name, so each
            // part gets its own store() + row. The last created row is
            // returned, matching the previous single-file response.
            //
            // The whole request is all-or-nothing: the file pipeline
            // runs for every part *before* any row exists, all rows go
            // in through one transaction, and any failure removes the
            // files of every part written so far. A failed request
            // therefore leaves nothing behind — a client retry (there
            // is no idempotency key) must never duplicate photos.
            let mut stored: Vec<_> = Vec::with_capacity(files.len());
            let outcome = (|| -> Result<leaflog_core::grows::Photo, ApiError> {
                for bytes in &files {
                    // Blocking image work inside the DB task keeps one
                    // flow; store() is pure CPU + file I/O.
                    stored.push(
                        photos
                            .store(&id, bytes)
                            .map_err(|error| ApiError::BadRequest(error.to_string()))?,
                    );
                }
                // One transaction for every row (D012 single-connection
                // design, hence `unchecked_transaction` on the shared
                // `&Connection`): a failed INSERT rolls back the earlier
                // parts' rows too.
                let tx = conn
                    .unchecked_transaction()
                    .map_err(StoreError::from)
                    .map_err(ApiError::from)?;
                let mut last = None;
                for part in &stored {
                    let captured_at = part.captured_at.clone().unwrap_or_else(now_rfc3339);
                    let day = day_number_in(&grow.started_on, &captured_at, &tz)
                        .ok_or_else(|| ApiError::BadRequest("invalid capture date".to_owned()))?;
                    let flip = store::actions::first_event_day(conn, &id, ActionType::Flip, day)?;
                    let harvested =
                        store::actions::first_event_day(conn, &id, ActionType::Harvest, day)?
                            .is_some();
                    let phase = leaflog_core::actions::derive_phase(day, flip, harvested);
                    let new = store::photos::NewPhoto {
                        grow_id: id.clone(),
                        plant_id: plant_id.clone(),
                        action_log_id: action_log_id.clone(),
                        captured_at,
                        grow_day: day,
                        phase: Some(phase),
                        tags: tags.clone(),
                        original_name: part.original_name.clone(),
                        thumbnail_name: Some(part.thumbnail_name.clone()),
                        bytes_stored: part.bytes_stored,
                    };
                    last = Some(store::photos::create(&tx, &new).map_err(ApiError::from)?);
                }
                tx.commit()
                    .map_err(StoreError::from)
                    .map_err(ApiError::from)?;
                Ok(last.expect("files is non-empty (checked above)"))
            })();
            if let Err(error) = outcome {
                // Rows are already gone (single transaction); remove the
                // files of every part that reached the disk.
                for part in &stored {
                    photos.remove(&id, &part.original_name, Some(&part.thumbnail_name));
                }
                return Err(error);
            }
            outcome
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

/// Rewrites an offset-less `datetime-local` timestamp (what
/// `<input type="datetime-local">` yields: `YYYY-MM-DDTHH:MM[:SS]`,
/// no UTC offset) to RFC 3339 UTC; values already in RFC 3339 pass
/// through unchanged.
fn normalize_occurred_at(occurred_at: &str) -> Result<String, ApiError> {
    if time::OffsetDateTime::parse(occurred_at, &Rfc3339).is_ok() {
        return Ok(occurred_at.to_owned());
    }
    let with_seconds = format_description!("[year]-[month]-[day]T[hour]:[minute]:[second]");
    let without_seconds = format_description!("[year]-[month]-[day]T[hour]:[minute]");
    let moment = time::PrimitiveDateTime::parse(occurred_at, &with_seconds)
        .or_else(|_| time::PrimitiveDateTime::parse(occurred_at, &without_seconds))
        .map_err(|_| ApiError::BadRequest("occurred_at must be RFC 3339".to_owned()))?
        .assume_utc();
    Ok(moment
        .format(&Rfc3339)
        .expect("RFC 3339 formatting of a UTC instant cannot fail"))
}

/// Shared action validation; first rewrites an offset-less
/// `datetime-local` timestamp in place to RFC 3339 UTC (see
/// [`normalize_occurred_at`]) so the stored value round-trips.
fn validate_action(input: &mut ActionInput) -> Result<(), ApiError> {
    input.occurred_at = normalize_occurred_at(&input.occurred_at)?;
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
