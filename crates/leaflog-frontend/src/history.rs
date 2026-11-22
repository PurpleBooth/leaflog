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
use crate::ui::{CardTitle, Chip, EmptyState, Fact, FactGrid};
use leaflog_core::grows::{Plant, PlantInput};
use leaflog_core::units::Measure;

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
    pub yield_by_feed_schedule: Vec<YieldAggregate>,
}

/// The figures behind the summary band: run count, mean absolute
/// boundary deviation, and summed harvest weights. Pure over the
/// report so the band view stays declarative and the arithmetic is
/// unit-testable.
#[derive(Debug, Clone, Copy, PartialEq)]
struct SummaryStats {
    /// Grows covered by the accuracy report.
    runs: usize,
    /// Mean of the absolute boundary deltas, `None` until one flip
    /// or harvest is logged.
    mean_abs_delta: Option<f64>,
    /// Summed dry grams across all harvests.
    dry_g: f64,
    /// Summed wet grams.
    wet_g: f64,
    /// Summed trimmed grams.
    trimmed_g: f64,
    /// Whether any harvest was weighed at all — the band shows a
    /// dash rather than a row of zeros until then.
    weighed: bool,
}

/// Reduces a report to the band's figures. Day counts and row
/// tallies fit a diary's scales by far below float precision, so the
/// two numeric casts are exact in practice.
#[must_use]
#[allow(clippy::cast_precision_loss)]
fn summary(report: &HistoryReport) -> SummaryStats {
    let deltas: Vec<f64> = report
        .accuracy
        .iter()
        .flat_map(|grow| {
            grow.boundaries
                .iter()
                .map(|boundary| boundary.delta_days.abs() as f64)
        })
        .collect::<Vec<_>>();
    let mean_abs_delta =
        (!deltas.is_empty()).then(|| deltas.iter().sum::<f64>() / deltas.len() as f64);
    // Each yield grouping is a complete partition of the harvests
    // (unlabelled grows group together), so the first non-empty list
    // already carries the grand total; the fallthrough only matters
    // for a report with no harvests at all.
    let found = [
        &report.yield_by_label,
        &report.yield_by_substrate,
        &report.yield_by_feed_schedule,
    ]
    .into_iter()
    .find(|rows| !rows.is_empty());
    let (dry_g, wet_g, trimmed_g) = found.map_or((0.0, 0.0, 0.0), |rows| {
        rows.iter().fold((0.0, 0.0, 0.0), |(dry, wet, trim), row| {
            (
                dry + row.total_dry_g,
                wet + row.total_wet_g,
                trim + row.total_trimmed_g,
            )
        })
    });
    SummaryStats {
        runs: report.accuracy.len(),
        mean_abs_delta,
        dry_g,
        wet_g,
        trimmed_g,
        weighed: found.is_some(),
    }
}

/// The history page.
#[component]
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
pub fn HistoryPage() -> impl IntoView {
    let units = crate::presentation_units();
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
        <section>
            <ui::PageHeader>
                <h2>{t!(i18n, page.history_heading)}</h2>
            </ui::PageHeader>
            <ErrorIf error />

            // The dashboard band: runs, mean plan deviation, summed
            // harvest — the facts register scaled to page figures.
            {move || {
                let stats = summary(&report.get());
                let locale = i18n.get_locale();
                let style = crate::decimal_style(locale);
                let deviation = stats.mean_abs_delta.map_or_else(
                    || "—".to_owned(),
                    |days| {
                        td_string!(
                            locale,
                            history.stat_deviation_days,
                            value = leaflog_core::format::format_decimal(days, 1, style)
                        )
                        .into_owned()
                    },
                );
                let system = units.get();
                let dry = Measure::Weight.format(stats.dry_g, system, style);
                let wet = Measure::Weight.format(stats.wet_g, system, style);
                let trim = Measure::Weight.format(stats.trimmed_g, system, style);
                view! {
                    <div class={format!("{} my-4", ui::card())} data-testid="history-summary">
                        <dl class="statband">
                            <Fact
                                label=Signal::derive(move || t_string!(i18n, history.stat_runs).into_owned())
                                testid="history-stat-runs"
                            >
                                {stats.runs}
                            </Fact>
                            <Fact
                                label=Signal::derive(move || {
                                    t_string!(i18n, history.stat_deviation).into_owned()
                                })
                                testid="history-stat-deviation"
                            >
                                {deviation}
                            </Fact>
                            <Fact
                                label=Signal::derive(move || {
                                    t_string!(i18n, history.stat_yield_dry).into_owned()
                                })
                                testid="history-stat-yield"
                            >
                                {if stats.weighed { dry } else { "—".to_owned() }}
                                <small>{td_string!(
                                    locale,
                                    history.stat_yield_sub,
                                    wet = wet,
                                    trim = trim,
                                )}</small>
                            </Fact>
                        </dl>
                    </div>
                }
            }}

            <h3>{t!(i18n, history.predicted_heading)}</h3>
            // One column: the accuracy cards are wide rows of labelled
            // figures, and a half-width card under the full-width stat
            // band left the band edges ragged.
            <ul class="grid grid-cols-1 gap-4">
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
                                        {row.strain.clone().map(|strain| view! { <Chip>{strain}</Chip> })}
                                    </div>
                                    // One fact per card until a flip or
                                    // harvest lands: the auto-fill grid
                                    // would cram it into a 10rem column.
                                    <FactGrid single=row.boundaries.is_empty()>
                                        {row.boundaries.is_empty().then(|| {
                                            view! {
                                                <Fact label=Signal::derive(move || {
                                                    t_string!(i18n, history.plan_accuracy).into_owned()
                                                })>{t!(i18n, history.no_boundaries)}</Fact>
                                            }
                                        })}
                                        {row
                                            .boundaries
                                            .into_iter()
                                            .map(|boundary| {
                                                view! {
                                                    <Fact label=Signal::derive(move || {
                                                        action_label_for_kind(
                                                            i18n.get_locale(),
                                                            &boundary.boundary,
                                                        )
                                                    })>
                                                        {t_string!(
                                                            i18n,
                                                            history.day_line,
                                                            actual = boundary.actual_day.to_string(),
                                                            planned = boundary.predicted_day.to_string(),
                                                            delta = format!("{:+}", boundary.delta_days),
                                                        )}
                                                    </Fact>
                                                }
                                            })
                                            .collect::<Vec<_>>()}
                                    </FactGrid>
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

            // mt-4 keeps the 1rem band rhythm; flush cards put two
            // rounded corners against each other, which reads as a
            // dark notch between the bands.
            <div class="grid gap-4 lg:grid-cols-3 mt-4">
                <Aggregate
                    title=Signal::derive(move || t_string!(i18n, history.yield_by_label).into_owned())
                    rows=Signal::derive(move || report.get().yield_by_label)
                />
                <Aggregate
                    title=Signal::derive(move || t_string!(i18n, history.yield_by_substrate).into_owned())
                    rows=Signal::derive(move || report.get().yield_by_substrate)
                />
                <Aggregate
                    title=Signal::derive(move || {
                        t_string!(i18n, history.yield_by_feed_schedule).into_owned()
                    })
                    rows=Signal::derive(move || report.get().yield_by_feed_schedule)
                />
            </div>
        </section>
    }
}

/// One yield-aggregate table, carded so the three groupings sit as
/// one band. The title arrives as a signal so a locale swap
/// re-translates it alongside the rows.
#[component]
fn Aggregate(title: Signal<String>, rows: Signal<Vec<YieldAggregate>>) -> impl IntoView {
    let units = crate::presentation_units();
    let i18n = use_i18n();
    view! {
        <div class=ui::card()>
            <CardTitle title=title />
            <Show
                when=move || !rows.get().is_empty()
                fallback=move || view! {
                    <p class="text-base-content/60 text-sm m-0">{t!(i18n, history.nothing_weighed)}</p>
                }
            >
                <div class="overflow-x-auto">
                    <table class="table table-sm">
                        <thead>
                            <tr>
                                <th>" "</th>
                                <th>{t!(i18n, history.wet_g, unit = move || Measure::Weight.symbol(units.get()))}</th>
                                <th>{t!(i18n, history.dry_g, unit = move || Measure::Weight.symbol(units.get()))}</th>
                                <th>{t!(i18n, history.trimmed_g, unit = move || Measure::Weight.symbol(units.get()))}</th>
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
                                                <td>{Measure::Weight.format(row.total_wet_g, units.get(), crate::decimal_style(i18n.get_locale()))}</td>
                                                <td>{Measure::Weight.format(row.total_dry_g, units.get(), crate::decimal_style(i18n.get_locale()))}</td>
                                                <td>{Measure::Weight.format(row.total_trimmed_g, units.get(), crate::decimal_style(i18n.get_locale()))}</td>
                                                <td>{row.grows}</td>
                                            </tr>
                                        }
                                    })
                                    .collect::<Vec<_>>()
                            }}
                        </tbody>
                    </table>
                </div>
            </Show>
        </div>
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
    let units = crate::presentation_units();
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
        <section>
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
                        <div class={format!("{} mb-4", ui::card())}>
                            <div class="flex flex-wrap items-baseline gap-2">
                                <strong>{data.plant.name.clone()}</strong>
                                <A href=route_href(&format!("/grows/{grow_id}"))>{t!(i18n, plant.back_to_grow)}</A>
                                <button class="btn btn-sm" on:click=move |_| start_edit(&for_edit)>
                                    {t!(i18n, plant.edit)}
                                </button>
                            </div>
                            <FactGrid>
                                {data.plant.wet_g.map(|grams| view! {
                                    <Fact label=Signal::derive(move || {
                                        t_string!(i18n, plant.wet).into_owned()
                                    })>{move || Measure::Weight.format(grams, units.get(), crate::decimal_style(i18n.get_locale()))}</Fact>
                                })}
                                {data.plant.dry_g.map(|grams| view! {
                                    <Fact label=Signal::derive(move || {
                                        t_string!(i18n, plant.dry).into_owned()
                                    })>{move || Measure::Weight.format(grams, units.get(), crate::decimal_style(i18n.get_locale()))}</Fact>
                                })}
                                {data.plant.trimmed_g.map(|grams| view! {
                                    <Fact label=Signal::derive(move || {
                                        t_string!(i18n, plant.trimmed).into_owned()
                                    })>{move || Measure::Weight.format(grams, units.get(), crate::decimal_style(i18n.get_locale()))}</Fact>
                                })}
                            </FactGrid>
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
                    <CardTitle title=Signal::derive(move || t_string!(i18n, plant.edit).into_owned()) />
                    <div class="grid gap-3 grid-cols-[repeat(auto-fill,minmax(14rem,1fr))]">
                        <ui::Field label=Signal::derive(move || t_string!(i18n, form.name).into_owned())>
                            <input class="input" type="text" required
                                prop:value=move || draft.get().name
                                on:input=move |ev| draft.update(|d| d.name = event_target_value(&ev))
                            />
                        </ui::Field>
                        {weight_field(
                            Signal::derive(move || td_string!(i18n.get_locale(), form.wet_weight_g, unit = Measure::Weight.symbol(units.get())).into_owned()),
                            draft,
                            |d| d.wet_g,
                            |d, v| d.wet_g = v,
                        )}
                        {weight_field(
                            Signal::derive(move || td_string!(i18n.get_locale(), form.dry_weight_g, unit = Measure::Weight.symbol(units.get())).into_owned()),
                            draft,
                            |d| d.dry_g,
                            |d, v| d.dry_g = v,
                        )}
                        {weight_field(
                            Signal::derive(move || td_string!(i18n.get_locale(), form.trimmed_weight_g, unit = Measure::Weight.symbol(units.get())).into_owned()),
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
    label: Signal<String>,
    draft: RwSignal<PlantInput>,
    read: impl Fn(&PlantInput) -> Option<f64> + Clone + Send + 'static,
    write: impl Fn(&mut PlantInput, Option<f64>) + Clone + Send + 'static,
) -> impl IntoView {
    view! {
        <ui::NumberField
            label=label
            step="any"
            draft=draft
            read=read
            write=write
            minimum="0"
            measure=Measure::Weight
        />
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

#[cfg(test)]
mod tests {
    use super::{BoundaryAccuracy, GrowAccuracy, HistoryReport, YieldAggregate, summary};

    fn boundary(delta: i64) -> BoundaryAccuracy {
        BoundaryAccuracy {
            boundary: "flip".to_owned(),
            predicted_day: 20,
            actual_day: 20 + delta,
            delta_days: delta,
        }
    }

    fn aggregate(label: &str, dry: f64, wet: f64, trim: f64) -> YieldAggregate {
        YieldAggregate {
            label: label.to_owned(),
            total_dry_g: dry,
            total_wet_g: wet,
            total_trimmed_g: trim,
            grows: 1,
        }
    }

    /// An empty report shows zero runs, no deviation and an unweighed
    /// dash — the band never fabricates a figure (D130's spirit).
    #[test]
    fn empty_report_reduces_to_dashes() {
        let stats = summary(&HistoryReport::default());
        assert_eq!(stats.runs, 0);
        assert_eq!(stats.mean_abs_delta, None);
        assert!(!stats.weighed);
        assert_eq!((stats.dry_g, stats.wet_g, stats.trimmed_g), (0.0, 0.0, 0.0));
    }

    /// Deviation is the mean of absolute deltas; sign cancels.
    #[test]
    fn deviation_averages_absolute_deltas() {
        let report = HistoryReport {
            accuracy: vec![GrowAccuracy {
                grow_id: "g1".to_owned(),
                name: "One".to_owned(),
                strain: None,
                boundaries: vec![boundary(-4), boundary(6)],
            }],
            ..HistoryReport::default()
        };
        let stats = summary(&report);
        assert_eq!(stats.runs, 1);
        assert_eq!(stats.mean_abs_delta, Some(5.0));
    }

    /// The first non-empty yield grouping is a complete partition, so
    /// it alone carries the grand total. Literals compared exactly:
    /// the sums re-add the same constants bit-identically.
    #[test]
    #[allow(clippy::float_cmp)]
    fn yield_totals_come_from_the_first_nonempty_grouping() {
        let report = HistoryReport {
            yield_by_substrate: vec![aggregate("Erde", 40.0, 10.0, 2.5)],
            ..HistoryReport::default()
        };
        let stats = summary(&report);
        assert!(stats.weighed);
        assert_eq!(stats.dry_g, 40.0);
        assert_eq!(stats.wet_g, 10.0);
        assert_eq!(stats.trimmed_g, 2.5);

        let report = HistoryReport {
            yield_by_label: vec![
                aggregate("Tropical Tangie", 12.0, 3.0, 1.0),
                aggregate("", 8.0, 2.0, 0.5),
            ],
            ..HistoryReport::default()
        };
        let stats = summary(&report);
        assert_eq!(stats.dry_g, 20.0);
        assert_eq!(stats.wet_g, 5.0);
    }
}
