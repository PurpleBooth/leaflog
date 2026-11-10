//! History page (spec §4.15): predicted-vs-actual accuracy across
//! grows, yield aggregates by strain / substrate / feed template, and
//! one-tap "start next run".

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_router::components::A;
use serde::Deserialize;

use crate::api;
use crate::equipment::ErrorIf;
use crate::route_href;
use leaflog_core::grows::Plant;

/// One logged-boundary comparison from the backend.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct BoundaryAccuracy {
    /// `"flip"` or `"harvest"`.
    pub boundary: String,
    /// The planned day (likely path).
    pub predicted_day: i64,
    /// The logged day.
    pub actual_day: i64,
    /// `actual − predicted` days.
    pub delta_days: i64,
}

/// Accuracy of one grow.
#[derive(Debug, Clone, Deserialize)]
pub struct GrowAccuracy {
    /// Grow id.
    pub grow_id: String,
    /// Grow name.
    pub name: String,
    /// Strain name.
    pub strain: Option<String>,
    /// Comparisons.
    pub boundaries: Vec<BoundaryAccuracy>,
}

/// One yield aggregate row.
#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct YieldAggregate {
    /// Grouping label.
    pub label: String,
    /// Dry grams.
    pub total_dry_g: f64,
    /// Wet grams.
    pub total_wet_g: f64,
    /// Trimmed grams.
    pub total_trimmed_g: f64,
    /// Contributing grows.
    pub grows: u32,
}

/// The whole history report.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct HistoryReport {
    /// One row per grow.
    pub accuracy: Vec<GrowAccuracy>,
    /// Yields by strain.
    pub yield_by_strain: Vec<YieldAggregate>,
    /// Yields by substrate.
    pub yield_by_substrate: Vec<YieldAggregate>,
    /// Yields by feed template.
    pub yield_by_feed_template: Vec<YieldAggregate>,
}

/// The history page.
#[component]
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
pub fn HistoryPage() -> impl IntoView {
    let report = RwSignal::<HistoryReport>::new(HistoryReport::default());
    let error = RwSignal::<Option<String>>::new(None);

    let refresh = move || {
        spawn_local(async move {
            match api::get::<HistoryReport>("api/history").await {
                Ok(fetched) => report.set(fetched),
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    };

    Effect::new(move |_| {
        refresh();
    });

    let next_run = move |grow_id: &str| {
        let grow_id = grow_id.to_owned();
        spawn_local(async move {
            match api::post_empty::<serde_json::Value>(&format!("api/grows/{grow_id}/next-run"))
                .await
            {
                Ok(_) => refresh(),
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    };

    view! {
        <section class="page">
            <div class="page-head">
                <h2>"History"</h2>
            </div>
            <ErrorIf error />

            <h3>"Predicted vs actual"</h3>
            <ul class="cards">
                {move || {
                    report
                        .get()
                        .accuracy
                        .into_iter()
                        .map(|row| {
                            let grow_id = row.grow_id.clone();
                            view! {
                                <li class="card">
                                    <div class="card-title">
                                        <strong>
                                            <A href=route_href(&format!("/grows/{}", row.grow_id))>
                                                {row.name.clone()}
                                            </A>
                                        </strong>
                                        {row.strain.clone().map(|strain| {
                                            view! { <span class="badge">{strain}</span> }
                                        })}
                                    </div>
                                    <dl class="facts">
                                        {row.boundaries.is_empty().then(|| {
                                            view! {
                                                <div>
                                                    <dt>"Plan accuracy"</dt>
                                                    <dd>"no flip or harvest logged yet"</dd>
                                                </div>
                                            }
                                        })}
                                        {row
                                            .boundaries
                                            .into_iter()
                                            .map(|boundary| {
                                                view! {
                                                    <div>
                                                        <dt>{boundary.boundary.clone()}</dt>
                                                        <dd>
                                                            {format!(
                                                                "day {} (planned {}, {:+})",
                                                                boundary.actual_day,
                                                                boundary.predicted_day,
                                                                boundary.delta_days,
                                                            )}
                                                        </dd>
                                                    </div>
                                                }
                                            })
                                            .collect::<Vec<_>>()}
                                    </dl>
                                    <div class="card-actions">
                                        <button on:click=move |_| next_run(&grow_id)>
                                            "Start next run"
                                        </button>
                                    </div>
                                </li>
                            }
                        })
                        .collect::<Vec<_>>()
                }}
            </ul>
            <Show when=move || report.get().accuracy.is_empty()>
                <p class="empty">"No grows yet — history builds itself as grows finish."</p>
            </Show>

            <Aggregate title="Yield by strain" rows=Signal::derive(move || report.get().yield_by_strain) />
            <Aggregate title="Yield by substrate" rows=Signal::derive(move || report.get().yield_by_substrate) />
            <Aggregate title="Yield by feed template" rows=Signal::derive(move || report.get().yield_by_feed_template) />

            <p><A href=route_href("/")>"Back to home"</A></p>
        </section>
    }
}

/// One yield-aggregate table.
#[component]
fn Aggregate(title: &'static str, rows: Signal<Vec<YieldAggregate>>) -> impl IntoView {
    view! {
        <h3>{title}</h3>
        <Show
            when=move || !rows.get().is_empty()
            fallback=move || view! { <p class="empty">"Nothing weighed yet."</p> }
        >
            <table>
                <thead>
                    <tr><th>" "</th><th>"Wet g"</th><th>"Dry g"</th><th>"Trimmed g"</th><th>"Grows"</th></tr>
                </thead>
                <tbody>
                    {move || {
                        rows.get()
                            .iter()
                            .map(|row| {
                                view! {
                                    <tr>
                                        <td>{row.label.clone()}</td>
                                        <td>{format!("{:.0}", row.total_wet_g)}</td>
                                        <td>{format!("{:.0}", row.total_dry_g)}</td>
                                        <td>{format!("{:.0}", row.total_trimmed_g)}</td>
                                        <td>{row.grows}</td>
                                    </tr>
                                }
                            })
                            .collect::<Vec<_>>()
                    }}
                </tbody>
            </table>
        </Show>
    }
}

/// The per-plant view (spec §4.15): one plant's timeline, log entries,
/// photos and final yield.
#[derive(Debug, Clone, Deserialize)]
struct PlantView {
    /// The plant.
    plant: Plant,
    /// Log entries naming this plant.
    actions: Vec<serde_json::Value>,
    /// Photos linked to this plant.
    photos: Vec<serde_json::Value>,
}

/// The per-plant page.
#[component]
pub fn PlantViewPage() -> impl IntoView {
    let params = leptos_router::hooks::use_params_map();
    let view = RwSignal::<Option<PlantView>>::new(None);
    let error = RwSignal::<Option<String>>::new(None);

    Effect::new(move |_| {
        let id = params.get().get("id").unwrap_or_default();
        spawn_local(async move {
            match api::get::<PlantView>(&format!("api/plants/{id}/view")).await {
                Ok(fetched) => view.set(Some(fetched)),
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    });

    view! {
        <section class="page">
            <ErrorIf error />
            {move || {
                view.get().map(|data| {
                    let grow_id = data.plant.grow_id.clone();
                    let actions = data.actions.clone();
                    let has_actions = !actions.is_empty();
                    let photos = data.photos.len();
                    view! {
                        <div class="card">
                            <div class="card-title">
                                <strong>{data.plant.name.clone()}</strong>
                                <A href=route_href(&format!("/grows/{grow_id}"))>"back to the grow"</A>
                            </div>
                            <dl class="facts">
                                {data.plant.wet_g.map(|grams| view! {
                                    <div><dt>"Wet"</dt><dd>{format!("{grams:.0} g")}</dd></div>
                                })}
                                {data.plant.dry_g.map(|grams| view! {
                                    <div><dt>"Dry"</dt><dd>{format!("{grams:.0} g")}</dd></div>
                                })}
                                {data.plant.trimmed_g.map(|grams| view! {
                                    <div><dt>"Trimmed"</dt><dd>{format!("{grams:.0} g")}</dd></div>
                                })}
                            </dl>
                            <h3>"Log"</h3>
                            <Show
                                when=move || has_actions
                                fallback=move || view! { <p class="empty">"No entries name this plant."</p> }
                            >
                                <ul>
                                    {actions
                                        .iter()
                                        .map(|entry| {
                                            view! {
                                                <li>
                                                    {format!(
                                                        "{} — {}",
                                                        entry["occurred_at"].as_str().unwrap_or(""),
                                                        entry["action_type"].as_str().unwrap_or(""),
                                                    )}
                                                </li>
                                            }
                                        })
                                        .collect::<Vec<_>>()}
                                </ul>
                            </Show>
                            <h3>"Photos"</h3>
                            <p class="empty">{format!("{photos} photo(s) linked")}</p>
                        </div>
                    }
                })
            }}
        </section>
    }
}
