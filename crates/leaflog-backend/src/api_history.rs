//! Cross-grow history routes (spec §4.15): read-only analytics over
//! existing tables, the per-plant view, and one-tap "start next run".

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use leaflog_core::actions::ActionType;
use leaflog_core::grows::Grow;
use leaflog_core::grows::Photo;
use leaflog_core::grows::Plant;
use leaflog_core::grows::YieldSummary;
use leaflog_core::grows::{ActionEntry, GrowInput, GrowStatus};
use leaflog_core::history::BoundaryAccuracy;
use leaflog_core::history::YieldAggregate;
use leaflog_core::history::aggregate_yields;
use leaflog_core::history::boundary_accuracy;
use serde::Serialize;

use crate::api::ApiError;
use crate::app::AppState;
use crate::store::{self, StoreOrNotFound};

/// The history routes; merged into the API router.
pub fn router() -> axum::Router<AppState> {
    axum::Router::new()
        .route("/api/history", get(history))
        .route("/api/plants/{id}/view", get(plant_view))
        .route("/api/grows/{id}/next-run", post(next_run))
}

/// Predicted-vs-actual accuracy for one grow.
#[derive(Debug, Serialize)]
pub struct GrowAccuracy {
    /// Grow id.
    pub grow_id: String,
    /// Grow name.
    pub name: String,
    /// Strain name, when the grow has one.
    pub strain: Option<String>,
    /// The logged-boundary comparisons (empty until a flip or harvest
    /// is logged).
    pub boundaries: Vec<BoundaryAccuracy>,
}

/// The whole history section (spec §4.15): per-grow accuracy plus
/// yield aggregates by strain, substrate and feed template.
#[derive(Debug, Serialize)]
pub struct HistoryReport {
    /// One row per grow, oldest start first.
    pub accuracy: Vec<GrowAccuracy>,
    /// Dry/wet/trimmed totals grouped by strain name.
    pub yield_by_strain: Vec<YieldAggregate>,
    /// Totals grouped by substrate label.
    pub yield_by_substrate: Vec<YieldAggregate>,
    /// Totals grouped by feed-template name.
    pub yield_by_feed_template: Vec<YieldAggregate>,
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn history(State(state): State<AppState>) -> Result<Json<HistoryReport>, ApiError> {
    let report = state
        .db
        .with(|conn| -> Result<HistoryReport, StoreError> {
            let mut grows = store::grows::list(conn)?;
            grows.sort_by(|a, b| a.started_on.cmp(&b.started_on));

            let mut accuracy = Vec::new();
            let mut by_strain: Vec<(String, YieldSummary)> = Vec::new();
            let mut by_substrate: Vec<(String, YieldSummary)> = Vec::new();
            let mut by_template: Vec<(String, YieldSummary)> = Vec::new();
            for grow in &grows {
                let strain = grow
                    .strain_id
                    .as_deref()
                    .and_then(|id| store::strain::get(conn, id).ok().map(|strain| strain.name));
                let flip =
                    store::actions::first_event_day(conn, &grow.id, ActionType::Flip, u16::MAX)?;
                let harvest =
                    store::actions::first_event_day(conn, &grow.id, ActionType::Harvest, u16::MAX)?;
                let actuals = leaflog_core::forecast::Actuals {
                    flip_day: flip.map(i64::from),
                    harvest_day: harvest.map(i64::from),
                    trichome_check_day: None,
                    germinated: false,
                };
                let strain_row = grow
                    .strain_id
                    .as_deref()
                    .and_then(|id| store::strain::get(conn, id).ok());
                // Accuracy compares the *plan* — the forecast without
                // actuals — against the logged days; feeding the
                // actuals in would pin the boundaries and zero every
                // delta.
                let input = leaflog_core::forecast::input_for(
                    strain_row.as_ref(),
                    None,
                    leaflog_core::forecast::Actuals::default(),
                );
                let windows = leaflog_core::forecast::forecast(&input);
                accuracy.push(GrowAccuracy {
                    grow_id: grow.id.clone(),
                    name: grow.name.clone(),
                    strain,
                    boundaries: boundary_accuracy(&windows, &actuals),
                });

                let plants = store::grows::list_plants(conn, &grow.id)?;
                let summary = YieldSummary::of(&grow.id, plants);
                by_strain.push((
                    strain_row
                        .as_ref()
                        .map_or_else(|| "unknown".to_owned(), |s| s.name.clone()),
                    summary.clone(),
                ));
                by_substrate.push((grow.substrate.clone(), summary.clone()));
                let template = grow.feed_template_id.as_deref().and_then(|id| {
                    store::feeds::get_template(conn, id)
                        .ok()
                        .map(|template| template.name)
                });
                by_template.push((template.unwrap_or_else(|| "none".to_owned()), summary));
            }
            Ok(HistoryReport {
                accuracy,
                yield_by_strain: aggregate_yields(&by_strain),
                yield_by_substrate: aggregate_yields(&by_substrate),
                yield_by_feed_template: aggregate_yields(&by_template),
            })
        })
        .await?;
    Ok(Json(report))
}

/// The per-plant view (spec §4.15): one plant's timeline, log entries,
/// photos and final yield.
#[derive(Debug, Serialize)]
pub struct PlantView {
    /// The plant.
    pub plant: Plant,
    /// Log entries that name this plant, newest first.
    pub actions: Vec<ActionEntry>,
    /// Photos linked to this plant, newest first.
    pub photos: Vec<Photo>,
    /// The plant's harvest weights, when logged.
    pub yield_g: Option<YieldSummary>,
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn plant_view(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<PlantView>, ApiError> {
    let view = state
        .db
        .with(move |conn| -> Result<PlantView, StoreOrNotFound> {
            let plant = store::grows::get_plant(conn, &id)?;
            let actions = store::actions::list_for_plant(conn, &plant.id)?;
            let photos = store::photos::list_for_plant(conn, &plant.id)?;
            let yield_g =
                (plant.wet_g.is_some() || plant.dry_g.is_some() || plant.trimmed_g.is_some())
                    .then(|| YieldSummary::of(&plant.grow_id, vec![plant.clone()]));
            Ok(PlantView {
                plant,
                actions,
                photos,
                yield_g,
            })
        })
        .await?;
    Ok(Json(view))
}

/// `POST /api/grows/{id}/next-run` — one tap creates a new grow
/// copying strain, substrate, grow space and feed template from this
/// one, by reference: bundled entries are never duplicated (spec
/// §4.15).
// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn next_run(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<(StatusCode, Json<Grow>), ApiError> {
    let created = state
        .db
        .with(move |conn| -> Result<Grow, ApiError> {
            let previous = store::grows::get(conn, &id)?;
            let today = crate::db::now_rfc3339()
                .get(..10)
                .unwrap_or_default()
                .to_owned();
            let input = GrowInput {
                name: format!("{} (next run)", previous.name),
                strain_id: previous.strain_id.clone(),
                grow_space_id: previous.grow_space_id.clone(),
                seed_batch_id: None,
                feed_template_id: previous.feed_template_id.clone(),
                substrate: previous.substrate,
                started_on: today,
                status: GrowStatus::Active,
                notes: None,
            };
            Ok(store::grows::create(conn, &input)?)
        })
        .await?;
    Ok((StatusCode::CREATED, Json(created)))
}

/// A type alias so the store error converts uniformly.
type StoreError = crate::db::StoreError;
