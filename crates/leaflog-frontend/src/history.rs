//! History page (spec §4.15): predicted-vs-actual accuracy across
//! grows, yield aggregates by strain label / substrate / feed
//! template, and one-tap "start next run".

use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_i18n::td_string;
use leptos_router::components::A;
use serde::Deserialize;

use crate::ErrorIf;
use crate::api;
use crate::i18n::{t, t_string, use_i18n};
use crate::labels::action_label_for_kind;
use crate::route_href;
use crate::ui;
use crate::ui::EmptyState;
use leaflog_core::grows::{Plant, PlantInput};

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
    /// The grow's strain label, when it has one (display only, D075).
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
    /// Yields by the grow's strain label (unlabelled grows group
    /// together).
    pub yield_by_label: Vec<YieldAggregate>,
    /// Yields by substrate.
    pub yield_by_substrate: Vec<YieldAggregate>,
    /// Yields by feed template.
    pub yield_by_feed_template: Vec<YieldAggregate>,
}

/// The history page.
#[component]
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
pub fn HistoryPage() -> impl IntoView {
    let i18n = use_i18n();
    let report = RwSignal::<HistoryReport>::new(HistoryReport::default());
    let error = RwSignal::<Option<String>>::new(None);

    let refresh = move || {
        spawn_local(async move {
            match api::get::<HistoryReport>("api/history").await {
                Ok(fetched) => report.set(fetched),
                Err(failure) => error.set(Some(api::failure_text(
                    &failure,
                    i18n.get_locale_untracked(),
                ))),
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
                Err(failure) => error.set(Some(api::failure_text(
                    &failure,
                    i18n.get_locale_untracked(),
                ))),
            }
        });
    };

    view! {
        <section class="pb-12">
            <div class="flex flex-wrap items-center justify-between gap-4">
                <h2>{t!(i18n, page.history_heading)}</h2>
            </div>
            <ErrorIf error />

            <h3>{t!(i18n, history.predicted_heading)}</h3>
            <ul class="grid gap-4 lg:grid-cols-2">
                {move || {
                    report
                        .get()
                        .accuracy
                        .into_iter()
                        .map(|row| {
                            let grow_id = row.grow_id.clone();
                            view! {
                                <li class=ui::card()>
                                    <div class="flex flex-wrap items-baseline gap-2">
                                        <strong>
                                            <A href=route_href(&format!("/grows/{}", row.grow_id))>
                                                {row.name.clone()}
                                            </A>
                                        </strong>
                                        {row.strain.clone().map(|strain| {
                                            view! { <span class="badge badge-ghost">{strain}</span> }
                                        })}
                                    </div>
                                    <dl class="facts">
                                        {row.boundaries.is_empty().then(|| {
                                            view! {
                                                <div>
                                                    <dt>{t!(i18n, history.plan_accuracy)}</dt>
                                                    <dd>{t!(i18n, history.no_boundaries)}</dd>
                                                </div>
                                            }
                                        })}
                                        {row
                                            .boundaries
                                            .into_iter()
                                            .map(|boundary| {
                                                view! {
                                                    <div>
                                                        <dt>{action_label_for_kind(i18n.get_locale(), &boundary.boundary)}</dt>
                                                        <dd>
                                                            {t_string!(
                                                                i18n,
                                                                history.day_line,
                                                                actual = boundary.actual_day.to_string(),
                                                                planned = boundary.predicted_day.to_string(),
                                                                delta = format!("{:+}", boundary.delta_days),
                                                            )}
                                                        </dd>
                                                    </div>
                                                }
                                            })
                                            .collect::<Vec<_>>()}
                                    </dl>
                                    <div class="flex flex-wrap gap-2 mt-4">
                                        <button class="btn btn-sm" on:click=move |_| next_run(&grow_id)>
                                            {t!(i18n, history.start_next_run)}
                                        </button>
                                    </div>
                                </li>
                            }
                        })
                        .collect::<Vec<_>>()
                }}
            </ul>
            <Show when=move || report.get().accuracy.is_empty()>
                <EmptyState>{t!(i18n, history.empty)}</EmptyState>
            </Show>

            <Aggregate
                title=Signal::derive(move || t_string!(i18n, history.yield_by_label).into_owned())
                rows=Signal::derive(move || report.get().yield_by_label)
            />
            <Aggregate
                title=Signal::derive(move || t_string!(i18n, history.yield_by_substrate).into_owned())
                rows=Signal::derive(move || report.get().yield_by_substrate)
            />
            <Aggregate
                title=Signal::derive(move || t_string!(i18n, history.yield_by_feed_template).into_owned())
                rows=Signal::derive(move || report.get().yield_by_feed_template)
            />

            <p><A href=route_href("/")>{t!(i18n, grows.back_home)}</A></p>
        </section>
    }
}

/// One yield-aggregate table. The title arrives as a signal so a
/// locale swap re-translates it alongside the rows.
#[component]
fn Aggregate(title: Signal<String>, rows: Signal<Vec<YieldAggregate>>) -> impl IntoView {
    let i18n = use_i18n();
    view! {
        <h3>{move || title.get()}</h3>
        <Show
            when=move || !rows.get().is_empty()
            fallback=move || view! {
                <EmptyState>{t!(i18n, history.nothing_weighed)}</EmptyState>
            }
        >
            <table class="table table-sm">
                <thead>
                    <tr>
                        <th>" "</th>
                        <th>{t!(i18n, history.wet_g)}</th>
                        <th>{t!(i18n, history.dry_g)}</th>
                        <th>{t!(i18n, history.trimmed_g)}</th>
                        <th>{t!(i18n, history.grows_col)}</th>
                    </tr>
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
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
pub fn PlantViewPage() -> impl IntoView {
    let i18n = use_i18n();
    let locale = i18n.get_locale();
    let params = leptos_router::hooks::use_params_map();
    let view = RwSignal::<Option<PlantView>>::new(None);
    let error = RwSignal::<Option<String>>::new(None);
    let edit_open = RwSignal::<bool>::new(false);
    let draft = RwSignal::<PlantInput>::new(empty_plant());

    let refresh = move || {
        let id = params.get().get("id").unwrap_or_default();
        spawn_local(async move {
            match api::get::<PlantView>(&format!("api/plants/{id}/view")).await {
                Ok(fetched) => view.set(Some(fetched)),
                Err(failure) => error.set(Some(api::failure_text(
                    &failure,
                    i18n.get_locale_untracked(),
                ))),
            }
        });
    };
    Effect::new(move |_| {
        params.track();
        refresh();
    });

    // Opens the edit form pre-filled from the payload the card cloned
    // out of the current plant.
    let start_edit = move |payload: &PlantInput| {
        draft.set(payload.clone());
        edit_open.set(true);
    };

    // prevent_default first — see docs/DECISIONS.md D061.
    let save = move |ev: leptos::ev::SubmitEvent| {
        ev.prevent_default();
        let id = params.get().get("id").unwrap_or_default();
        spawn_local(async move {
            let input = draft.get();
            match api::put::<PlantInput, Plant>(&format!("api/plants/{id}"), &input).await {
                Ok(_) => {
                    edit_open.set(false);
                    error.set(None);
                    refresh();
                }
                Err(failure) => error.set(Some(api::failure_text(
                    &failure,
                    i18n.get_locale_untracked(),
                ))),
            }
        });
    };

    view! {
        <section class="pb-12">
            <ErrorIf error />
            {move || {
                view.get().map(|data| {
                    let grow_id = data.plant.grow_id.clone();
                    let actions = data.actions.clone();
                    let has_actions = !actions.is_empty();
                    let photos = data.photos.len();
                    let for_edit = PlantInput {
                        name: data.plant.name.clone(),
                        wet_g: data.plant.wet_g,
                        dry_g: data.plant.dry_g,
                        trimmed_g: data.plant.trimmed_g,
                    };
                    view! {
                        <div class={format!("{} my-4", ui::card())}>
                            <div class="flex flex-wrap items-baseline gap-2">
                                <strong>{data.plant.name.clone()}</strong>
                                <A href=route_href(&format!("/grows/{grow_id}"))>{t!(i18n, plant.back_to_grow)}</A>
                                <button class="btn btn-sm" on:click=move |_| start_edit(&for_edit)>
                                    {t!(i18n, plant.edit)}
                                </button>
                            </div>
                            <dl class="facts">
                                {data.plant.wet_g.map(|grams| view! {
                                    <div><dt>{t!(i18n, plant.wet)}</dt><dd>{format!("{grams:.0} g")}</dd></div>
                                })}
                                {data.plant.dry_g.map(|grams| view! {
                                    <div><dt>{t!(i18n, plant.dry)}</dt><dd>{format!("{grams:.0} g")}</dd></div>
                                })}
                                {data.plant.trimmed_g.map(|grams| view! {
                                    <div><dt>{t!(i18n, plant.trimmed)}</dt><dd>{format!("{grams:.0} g")}</dd></div>
                                })}
                            </dl>
                            <h3>{t!(i18n, plant.log_heading)}</h3>
                            <Show
                                when=move || has_actions
                                fallback=move || view! { <EmptyState>{t!(i18n, plant.log_empty)}</EmptyState> }
                            >
                                <ul class="list-disc pl-5">
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
                            <h3>{t!(i18n, grow.photos)}</h3>
                            <EmptyState>
                                {td_string!(locale, plant.photos_linked, count = photos)}
                            </EmptyState>
                        </div>
                    }
                })
            }}

            <Show when=move || edit_open.get()>
                <form class={format!("{} my-4", ui::card())} on:submit=save>
                    <h3>{t!(i18n, plant.edit)}</h3>
                    <div class="grid gap-3 grid-cols-[repeat(auto-fill,minmax(14rem,1fr))]">
                        <label class="fieldset">
                            <span class="fieldset-label">{t!(i18n, form.name)}</span>
                            <input class="input" type="text" required
                                prop:value=move || draft.get().name
                                on:input=move |ev| draft.update(|d| d.name = event_target_value(&ev))
                            />
                        </label>
                        {weight_field(
                            td_string!(locale, form.wet_weight_g).into_owned(),
                            draft,
                            |d| d.wet_g,
                            |d, v| d.wet_g = v,
                        )}
                        {weight_field(
                            td_string!(locale, form.dry_weight_g).into_owned(),
                            draft,
                            |d| d.dry_g,
                            |d, v| d.dry_g = v,
                        )}
                        {weight_field(
                            td_string!(locale, form.trimmed_weight_g).into_owned(),
                            draft,
                            |d| d.trimmed_g,
                            |d, v| d.trimmed_g = v,
                        )}
                    </div>
                    <div class="flex flex-wrap gap-2 mt-4">
                        <button class="btn btn-primary btn-sm" type="submit">{t!(i18n, grows.save)}</button>
                        <button class="btn btn-sm" type="button" on:click=move |_| edit_open.set(false)>
                            {t!(i18n, grows.cancel)}
                        </button>
                    </div>
                </form>
            </Show>
        </section>
    }
}

/// One harvest-weight input bound to a spot in the plant draft.
fn weight_field(
    label: String,
    draft: RwSignal<PlantInput>,
    read: impl Fn(&PlantInput) -> Option<f64> + Clone + Send + 'static,
    write: impl Fn(&mut PlantInput, Option<f64>) + Clone + Send + 'static,
) -> impl IntoView {
    view! {
        <label class="fieldset">
            <span class="fieldset-label">{label}</span>
            <input class="input" type="number" min="0" step="1"
                prop:value=move || {
                    read(&draft.get()).map(|value| format!("{value:.0}")).unwrap_or_default()
                }
                on:input=move |ev| {
                    let parsed = event_target_value(&ev).parse::<f64>().ok();
                    draft.update(|current| write(current, parsed));
                }
            />
        </label>
    }
}

/// A blank plant draft.
const fn empty_plant() -> PlantInput {
    PlantInput {
        name: String::new(),
        wet_g: None,
        dry_g: None,
        trimmed_g: None,
    }
}
