//! JSON API routes for equipment and settings (spec §4.1). All routes
//! are under `/api`; the browser is the only client. The strain
//! catalogue left with the feature strip (D075) — the grow carries its
//! strain label and photoperiod itself.

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use leaflog_core::equipment::{EquipmentCategory, EquipmentInput, EquipmentItem};
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
        .route("/api/equipment/{id}/clone", post(clone_equipment))
        .route("/api/settings", get(get_settings).put(put_settings))
        .route("/api/onboarding", get(onboarding_state))
        .merge(crate::api_grows::router())
        .merge(crate::api_ha::router())
        .merge(crate::api_feeds::router())
        .merge(crate::api_env::router())
        .merge(crate::api_spaces::router())
        .merge(crate::api_chart::router())
        .merge(crate::api_history::router())
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

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = match &self {
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::Conflict(_) => StatusCode::CONFLICT,
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
        .with(move |conn| -> Result<EquipmentItem, ApiError> {
            reject_immutable_equipment(conn, &id)?;
            Ok(store::equipment::update(conn, &id, &input)?)
        })
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
        .with(move |conn| -> Result<(), ApiError> {
            reject_immutable_equipment(conn, &id)?;
            store::equipment::delete(conn, &id)?;
            Ok(())
        })
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn clone_equipment(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<EquipmentItem>), ApiError> {
    let clone = state
        .db
        .with(move |conn| -> Result<EquipmentItem, ApiError> {
            let parent = store::equipment::get(conn, &id)?;
            let mut copy = EquipmentInput::from_item(&parent);
            copy.name = format!("{} (copy)", parent.name);
            Ok(store::equipment::create(conn, &copy)?)
        })
        .await?;
    Ok((StatusCode::CREATED, Json(clone)))
}

/// Fails with [`ApiError::Conflict`] when the addressed equipment entry
/// is bundled: bundled records are read-only, and modifying one is a
/// clone away (spec §4.1).
fn reject_immutable_equipment(
    conn: &rusqlite::Connection,
    id: &str,
) -> Result<EquipmentItem, ApiError> {
    let existing = store::equipment::get(conn, id)?;
    if existing.is_builtin {
        return Err(ApiError::Conflict(
            "bundled equipment is read-only; clone it into your own entry instead".to_owned(),
        ));
    }
    Ok(existing)
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

/// `GET /api/onboarding` — the first-run wizard state (spec §4.12):
/// the wizard shows while there are no grow spaces, no grows and no
/// prior dismissal. Dismissal and restart are the
/// `onboarding_dismissed` settings flag (`PUT /api/settings`).
// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn onboarding_state(State(state): State<AppState>) -> Result<Json<Value>, ApiError> {
    let report = state
        .db
        .with(|conn| -> Result<Value, StoreError> {
            let spaces = store::spaces::list(conn)?.len();
            let grows = store::grows::list(conn)?.len();
            let equipment = store::equipment::list(conn, None)?.len();
            let dismissed = matches!(
                store::settings::get(conn, "onboarding_dismissed"),
                Ok(Value::Bool(true))
            );
            Ok(json!({
                "show_wizard": spaces == 0 && grows == 0 && !dismissed,
                "dismissed": dismissed,
                "has_spaces": spaces > 0,
                "has_grows": grows > 0,
                "has_equipment": equipment > 0,
            }))
        })
        .await?;
    Ok(Json(report))
}

/// Query parameters of `GET /api/equipment`.
#[derive(Debug, Default, Deserialize)]
struct EquipmentQuery {
    /// Filter by category label.
    category: Option<String>,
}

/// Parses and validates an equipment category label.
fn parse_category(label: &str) -> Result<EquipmentCategory, ApiError> {
    EquipmentCategory::catalogue()
        .into_iter()
        .find(|category| category.label() == label)
        .ok_or_else(|| ApiError::BadRequest(format!("unknown equipment category {label}")))
}

/// Validates equipment input shared by create and update.
fn validate_equipment(input: &EquipmentInput) -> Result<(), ApiError> {
    if input.name.trim().is_empty() {
        return Err(ApiError::BadRequest("name must not be empty".to_owned()));
    }
    if !EquipmentCategory::catalogue().contains(&input.category) {
        return Err(ApiError::BadRequest(format!(
            "equipment category must be one of the catalogue categories \
             (light, tent_room, fan, sensor); '{}' is not accepted",
            input.category.label()
        )));
    }
    validate_ha_binding(input)?;

    if input.real_watts.is_some_and(|watts| watts < 0.0) {
        return Err(ApiError::BadRequest(
            "real_watts must not be negative".to_owned(),
        ));
    }
    Ok(())
}

/// The sensor device classes v1 supports (spec §4.1).
const SENSOR_DEVICE_CLASSES: [&str; 2] = ["temperature", "humidity"];

/// Validates the HA entity binding rules of spec §4.1: a sensor item
/// exists only as a picked HA entity (`sensor.*` plus one of the
/// supported device classes — sensor items are never hand-entered or
/// bundled); every other category may bind an entity of any domain but
/// carries no device class.
fn validate_ha_binding(input: &EquipmentInput) -> Result<(), ApiError> {
    let entity = input.ha_entity_id.as_deref().map(str::trim);
    if input.category == EquipmentCategory::Sensor {
        let entity = entity
            .filter(|id| !id.is_empty())
            .ok_or_else(|| {
                ApiError::BadRequest(
                    "sensor items are created from a Home Assistant entity: \
                     ha_entity_id is required (pick one in the entity picker)"
                        .to_owned(),
                )
            })?
            .to_owned();
        if !valid_entity_id(&entity) || !entity.starts_with("sensor.") {
            return Err(ApiError::BadRequest(format!(
                "ha_entity_id must be a sensor entity id like sensor.grow_temperature, \
                 got '{entity}'"
            )));
        }
        let class = input.ha_device_class.as_deref().ok_or_else(|| {
            ApiError::BadRequest(
                "sensor items need ha_device_class (temperature or humidity)".to_owned(),
            )
        })?;
        if !SENSOR_DEVICE_CLASSES.contains(&class) {
            return Err(ApiError::BadRequest(format!(
                "unsupported sensor device class '{class}': v1 supports temperature and \
                 humidity"
            )));
        }
    } else if input.ha_device_class.is_some() {
        return Err(ApiError::BadRequest(format!(
            "ha_device_class only applies to sensor items, not {}",
            input.category.label()
        )));
    } else if let Some(entity) = entity
        .filter(|id| !id.is_empty())
        .filter(|id| !valid_entity_id(id))
    {
        return Err(ApiError::BadRequest(format!(
            "ha_entity_id must look like domain.entity_id, got '{entity}'"
        )));
    }
    Ok(())
}

/// An HA entity id is `domain.object_id`, both non-empty; domains are
/// lowercase and `object_id`s use word characters.
fn valid_entity_id(entity: &str) -> bool {
    let Some((domain, object)) = entity.split_once('.') else {
        return false;
    };
    !domain.is_empty()
        && !object.is_empty()
        && domain
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        && object
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}
