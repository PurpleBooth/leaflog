//! The Schedules library: the global list of reusable feeding
//! schedules (ADR 0004) and the product–dose grid editor. Schedules
//! live independently of grows — finished grows keep their fitted
//! plan, so one schedule serves run after run.

use leaflog_core::feed::{
    Dose, DoseUnit, FeedSchedule, ProductDoses, ScheduleProduct, format_dose, parse_dose_text,
};
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_i18n::td_string;
use leptos_router::hooks::use_params_map;
use serde::Deserialize;

use crate::ErrorIf;
use crate::api;
use crate::feed::decimal_style;
use crate::i18n::{Locale, t_string, use_i18n};
use crate::route_href;
use crate::ui;
use crate::ui::Chip;

/// One library row: the schedule plus the counts the backend
/// aggregates in one pass.
#[derive(Debug, Clone, Deserialize)]
pub struct ScheduleSummary {
    /// The schedule itself, products included.
    pub schedule: FeedSchedule,
    /// How many product rows the grid has.
    pub product_count: i64,
    /// Distinct group labels, first-seen order.
    pub groups: Vec<String>,
    /// Distinct dose units, first-seen order.
    pub units: Vec<String>,
    /// How many grows reference the schedule.
    pub used_by: i64,
}

/// One product row of the editor draft.
#[derive(Debug, Clone)]
struct RowDraft {
    /// The wire id; empty while the row is unsaved (the server
    /// generates it).
    id: String,
    /// Product name; trimmed on save.
    name: String,
    /// The unit every cell in this row doses in.
    unit: DoseUnit,
    /// The free-text group; blank reads as the default group.
    group: String,
    /// The note under the name; blank reads as none.
    note: String,
    /// The raw cell texts: PROPAGATION, VEG, WK 1…N, WATER.
    cells: Vec<String>,
}

/// The cell count of one row: PROPAGATION, VEG, the weeks, WATER.
#[must_use]
fn cell_count(weeks: u8) -> usize {
    3 + usize::from(weeks)
}

/// The draft text of one dose cell: blank when unset, the
/// locale-styled dose otherwise.
fn cell_text(dose: Option<Dose>, locale: Locale) -> String {
    dose.map_or_else(String::new, |dose| format_dose(dose, decimal_style(locale)))
}

/// The editor rows of a stored schedule, one per product, in chart
/// order.
fn rows_from(schedule: &FeedSchedule, locale: Locale) -> Vec<RowDraft> {
    let weeks = usize::from(schedule.flower_weeks);
    schedule
        .products
        .iter()
        .map(|product| {
            let mut cells = vec![
                cell_text(product.doses.propagation, locale),
                cell_text(product.doses.veg, locale),
            ];
            cells.extend(
                product
                    .doses
                    .flower
                    .iter()
                    .take(weeks)
                    .map(|dose| cell_text(*dose, locale)),
            );
            cells.push(cell_text(product.doses.water, locale));
            RowDraft {
                id: product.id.clone(),
                name: product.name.clone(),
                unit: product.unit,
                group: product.group.clone(),
                note: product.note.clone().unwrap_or_default(),
                cells,
            }
        })
        .collect()
}

/// The localized chart-column label for cell `index` (the error
/// copy names the offending column).
fn column_label(locale: Locale, index: usize, weeks: u8) -> String {
    if index == 0 {
        td_string!(locale, schedules.col_propagation).into_owned()
    } else if index == 1 {
        td_string!(locale, schedules.col_veg).into_owned()
    } else if index + 1 < cell_count(weeks) {
        td_string!(
            locale,
            schedules.week_col,
            week = u32::try_from(index - 1).unwrap_or(0)
        )
        .into_owned()
    } else {
        td_string!(locale, schedules.col_water).into_owned()
    }
}

/// Trims free text; blank reads as `None` (the note convention).
#[must_use]
fn non_empty(text: &str) -> Option<String> {
    let trimmed = text.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_owned())
}

/// The dose-unit display suffix for a stored unit label; the two
/// known labels map to the catalog strings the editor's unit select
/// uses, an unknown label echoes through (tolerant rendering, like
/// the entity map).
#[must_use]
fn dose_unit_display(locale: Locale, unit: &str) -> String {
    match unit {
        "ml_per_l" => td_string!(locale, schedules.unit_ml).into_owned(),
        "g_per_l" => td_string!(locale, schedules.unit_gl).into_owned(),
        other => other.to_owned(),
    }
}

/// The dose behind one raw cell: `None` for blank and dash cells,
/// the parsed dose otherwise.
#[must_use]
fn parse_cell(cells: &[String], index: usize, unit: DoseUnit) -> Option<Dose> {
    cells.get(index).and_then(|text| {
        let trimmed = text.trim();
        if trimmed.is_empty() || trimmed == "—" {
            return None;
        }
        parse_dose_text(trimmed, unit)
    })
}

/// The Schedules library page (`/schedules`).
#[component]
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
pub fn SchedulesPage() -> impl IntoView {
    let i18n = use_i18n();
    let rows = RwSignal::<Vec<ScheduleSummary>>::new(Vec::new());
    let loading = RwSignal::new(true);
    let error = RwSignal::<Option<String>>::new(None);
    // The id whose Delete click armed the confirm button; the list
    // reload disarms it again.
    let armed = RwSignal::<Option<String>>::new(None);

    let load = move || {
        let locale = i18n.get_locale_untracked();
        spawn_local(async move {
            match api::get::<Vec<ScheduleSummary>>("api/schedules").await {
                Ok(list) => {
                    armed.set(None);
                    rows.set(list);
                    loading.set(false);
                }
                Err(failure) => {
                    error.set(Some(api::failure_text(&failure, locale)));
                    loading.set(false);
                }
            }
        });
    };
    Effect::new(move |_| load());

    let duplicate = move |id: String| {
        let locale = i18n.get_locale_untracked();
        spawn_local(async move {
            match api::post_empty::<FeedSchedule>(&format!("api/schedules/{id}/duplicate")).await {
                Ok(_) => load(),
                Err(failure) => {
                    error.set(Some(api::failure_text(&failure, locale)));
                }
            }
        });
    };

    let remove = move |id: String| {
        let locale = i18n.get_locale_untracked();
        spawn_local(async move {
            match api::delete(&format!("api/schedules/{id}")).await {
                Ok(()) => load(),
                Err(failure) => {
                    error.set(Some(api::failure_text(&failure, locale)));
                }
            }
        });
    };

    view! {
        <ErrorIf error />
        {move || {
            if loading.get() {
                view! {
                    <p class="py-6 text-center text-base-content/60">
                        {t_string!(i18n, page.loading)}
                    </p>
                }
                    .into_any()
            } else if rows.get().is_empty() {
                view! {
                    <div class="card bg-base-100 shadow-sm px-5 py-8 text-center mb-4">
                        <h2 class="m-0">{t_string!(i18n, schedules.page_title)}</h2>
                        <h3 class="mt-3 mb-0 text-lg">
                            {t_string!(i18n, schedules.library_empty_title)}
                        </h3>
                        <p class="text-base-content/70 max-w-prose mx-auto">
                            {t_string!(i18n, schedules.library_empty_body)}
                        </p>
                        <a class="btn btn-primary btn-sm mt-4" href=route_href("/schedules/new")>
                            {t_string!(i18n, schedules.new_schedule)}
                        </a>
                    </div>
                }
                    .into_any()
            } else {
                let locale = i18n.get_locale();
                view! {
                    <div class="mb-4 grid gap-3">
                        <ui::PageHeader>
                            <h2>{t_string!(i18n, schedules.page_title)}</h2>
                            <a class="btn btn-primary btn-sm" href=route_href("/schedules/new")>
                                {t_string!(i18n, schedules.new_schedule)}
                            </a>
                        </ui::PageHeader>
                        {rows
                            .get()
                            .into_iter()
                            .map(|row| {
                                let id = row.schedule.id.clone();
                                let name = row.schedule.name.clone();
                                let source = row.schedule.source.clone();
                                let weeks = row.schedule.flower_weeks;
                                let products = row.product_count;
                                let groups = row.groups.len();
                                let units = row.units
                                    .iter()
                                    .map(|unit| dose_unit_display(locale, unit))
                                    .collect::<Vec<_>>()
                                    .join(", ");
                                let used_by = row.used_by;
                                let edit_url = format!("/schedules/{id}/edit");
                                let delete_id = id.clone();
                                let duplicate_id = id;
                                let badge_class = if used_by > 0 {
                                    "badge badge-primary badge-outline"
                                } else {
                                    "badge badge-ghost"
                                };
                                view! {
                                    <div class="card bg-base-100 shadow-sm px-5 py-4">
                                        <div class="flex flex-wrap items-baseline gap-x-3 gap-y-1">
                                            <h3 class="m-0">{name}</h3>
                                            {source.map(|source| {
                                                view! { <Chip>{source}</Chip> }
                                            })}
                                            <span class=badge_class>
                                                {if used_by > 0 {
                                                    td_string!(
                                                        locale,
                                                        schedules.used_by,
                                                        count = used_by,
                                                    )
                                                    .into_owned()
                                                } else {
                                                    td_string!(locale, schedules.badge_unused)
                                                        .into_owned()
                                                }}
                                            </span>
                                        </div>
                                        <p class="mt-1 mb-0 text-base-content/70 text-sm">
                                            {format!(
                                                "{} · {} · {}{}",
                                                td_string!(
                                                    locale,
                                                    schedules.products,
                                                    count = products,
                                                ),
                                                td_string!(locale, schedules.groups, count = groups),
                                                td_string!(locale, schedules.weeks, count = weeks),
                                                if units.is_empty() {
                                                    String::new()
                                                } else {
                                                    format!(" · {units}")
                                                },
                                            )}
                                        </p>
                                        <div class="mt-3 flex flex-wrap items-center gap-2">
                                            <a class="btn btn-outline btn-sm" href=route_href(&edit_url)>
                                                {t_string!(i18n, schedules.edit)}
                                            </a>
                                            <button
                                                class="btn btn-ghost btn-sm"
                                                on:click=move |_| duplicate(duplicate_id.clone())
                                            >
                                                {t_string!(i18n, schedules.duplicate)}
                                            </button>
                                            {
                                                let id_for_armed = delete_id.clone();
                                                let id_for_arm = delete_id.clone();
                                                let id_for_confirm = delete_id;
                                                view! {
                                                    <ui::ArmConfirmButton
                                                        armed=Signal::derive(move || {
                                                            armed.get().as_deref() == Some(id_for_armed.as_str())
                                                        })
                                                        label=Signal::derive(move || t_string!(i18n, schedules.delete).into_owned())
                                                        confirm_label=Signal::derive(move || {
                                                            t_string!(i18n, schedules.confirm_delete).into_owned()
                                                        })
                                                        on_arm=move |_| {
                                                            armed.set(Some(id_for_arm.clone()));
                                                        }
                                                        on_confirm=move |_| remove(id_for_confirm.clone())
                                                    />
                                                }
                                            }
                                        </div>
                                    </div>
                                }
                            })
                            .collect::<Vec<_>>()}
                    </div>
                }
                    .into_any()
            }
        }}
    }
}

/// The schedule editor route for a stored schedule
/// (`/schedules/:id/edit`).
#[component]
pub fn ScheduleEditRoute() -> impl IntoView {
    let params = use_params_map();
    let existing = params.get().get("id");
    view! { <ScheduleEditorPage existing /> }
}

/// The schedule editor route for a new schedule (`/schedules/new`).
#[component]
pub fn ScheduleNewRoute() -> impl IntoView {
    view! { <ScheduleEditorPage existing=None /> }
}

/// The schedule editor: name, source, the flowering-week count, and
/// the product–dose grid. Saving a stored schedule re-fits every
/// grow that uses it (the refit note).
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos component prop contract; see docs/DECISIONS.md
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
pub fn ScheduleEditorPage(
    /// `Some(id)` when editing a stored schedule.
    existing: Option<String>,
) -> impl IntoView {
    let i18n = use_i18n();
    let navigate = StoredValue::new_local(leptos_router::hooks::use_navigate());
    let id = StoredValue::new(existing);
    let error = RwSignal::<Option<String>>::new(None);
    let name = RwSignal::<String>::new(String::new());
    let source = RwSignal::<String>::new(String::new());
    let weeks = RwSignal::<u8>::new(8);
    let rows = RwSignal::<Vec<RowDraft>>::new(Vec::new());
    let loading = RwSignal::new(id.get_value().is_some());
    let saving = RwSignal::new(false);
    let saved = RwSignal::new(false);
    let refitting = RwSignal::new(false);
    let armed = RwSignal::new(false);

    // A new schedule starts with one empty row to edit (seeded
    // eagerly: the load effect only runs while a stored schedule is
    // still loading, so an else-branch there would never fire).
    if id.get_value().is_none() {
        rows.set(vec![empty_row(cell_count(weeks.get_untracked()))]);
    }

    Effect::new(move |_| {
        if !loading.get() {
            return;
        }
        if let Some(id) = id.get_value() {
            let locale = i18n.get_locale_untracked();
            spawn_local(async move {
                match api::get::<FeedSchedule>(&format!("api/schedules/{id}")).await {
                    Ok(schedule) => {
                        name.set(schedule.name.clone());
                        source.set(schedule.source.clone().unwrap_or_default());
                        weeks.set(schedule.flower_weeks);
                        rows.set(rows_from(&schedule, locale));
                        loading.set(false);
                    }
                    Err(failure) => {
                        error.set(Some(api::failure_text(&failure, locale)));
                        loading.set(false);
                    }
                }
            });
        } else {
            loading.set(false);
        }
    });

    let set_weeks = move |count: u8| {
        let count = count.clamp(1, FeedSchedule::MAX_FLOWER_WEEKS);
        weeks.set(count);
        let cells = cell_count(count);
        rows.update(|rows| {
            for row in rows {
                row.cells.resize(cells, String::new());
            }
        });
    };
    let add_week = move |_| set_weeks(weeks.get().saturating_add(1));
    let remove_week = move |_| set_weeks(weeks.get().saturating_sub(1));

    let add_row = move |_| {
        rows.update(|rows| {
            rows.push(empty_row(cell_count(weeks.get_untracked())));
        });
    };

    // Builds the wire schedule from the draft. `Err` carries a
    // localized validation message; the server re-validates as the
    // backstop.
    let build = move |locale: Locale| -> Result<FeedSchedule, String> {
        let trimmed_name = name.get_untracked().trim().to_owned();
        if trimmed_name.is_empty() {
            return Err(td_string!(locale, schedules.err_name_required).into_owned());
        }
        let count = weeks.get_untracked();
        let default_group = td_string!(locale, schedules.group_default).into_owned();
        let mut products = Vec::new();
        for row in rows.get_untracked() {
            let trimmed = row.name.trim().to_owned();
            if trimmed.is_empty() {
                continue;
            }
            let mut doses = ProductDoses::empty();
            doses.propagation = parse_cell(&row.cells, 0, row.unit);
            doses.veg = parse_cell(&row.cells, 1, row.unit);
            let week_cells = cell_count(count) - 3;
            doses.flower = (0..week_cells)
                .map(|week| parse_cell(&row.cells, 2 + week, row.unit))
                .collect::<Vec<_>>();
            doses.water = parse_cell(&row.cells, 2 + week_cells, row.unit);
            // Unparsable, non-blank text aborts the save with the
            // row and column named.
            for (index, text) in row.cells.iter().enumerate() {
                let trimmed_cell = text.trim();
                if trimmed_cell.is_empty() || trimmed_cell == "—" {
                    continue;
                }
                if parse_dose_text(trimmed_cell, row.unit).is_none() {
                    return Err(td_string!(
                        locale,
                        schedules.err_dose_invalid,
                        text = trimmed_cell.to_owned(),
                        product = trimmed,
                        column = column_label(locale, index, count),
                    )
                    .into_owned());
                }
            }
            let group = row.group.trim();
            products.push(ScheduleProduct {
                id: row.id,
                name: trimmed,
                unit: row.unit,
                group: if group.is_empty() {
                    default_group.clone()
                } else {
                    group.to_owned()
                },
                note: non_empty(&row.note),
                doses,
            });
        }
        if products.len() > FeedSchedule::MAX_PRODUCTS {
            return Err(td_string!(
                locale,
                schedules.err_products_limit,
                count = FeedSchedule::MAX_PRODUCTS,
            )
            .into_owned());
        }
        Ok(FeedSchedule {
            id: id.get_value().unwrap_or_default(),
            name: trimmed_name,
            source: non_empty(&source.get_untracked()),
            flower_weeks: count,
            products,
            created_at: String::new(),
            updated_at: String::new(),
        })
    };

    let save = move |_: leptos::ev::MouseEvent| {
        let locale = i18n.get_locale_untracked();
        let schedule = match build(locale) {
            Ok(schedule) => schedule,
            Err(message) => {
                error.set(Some(message));
                return;
            }
        };
        saving.set(true);
        saved.set(false);
        error.set(None);
        let stored = id.get_value();
        spawn_local(async move {
            let result = match &stored {
                Some(id) => {
                    let updated = api::put::<FeedSchedule, FeedSchedule>(
                        &format!("api/schedules/{id}"),
                        &schedule,
                    )
                    .await;
                    // Every grow fitted with this schedule re-fits to
                    // the new chart; single failures surface in the
                    // grow's feed section, not here.
                    if updated.is_ok() {
                        refitting.set(true);
                        let grow_ids =
                            api::get::<Vec<String>>(&format!("api/schedules/{id}/grows"))
                                .await
                                .unwrap_or_default();
                        for grow_id in grow_ids {
                            let _ = api::post_empty::<leaflog_core::feed::FeedPlan>(&format!(
                                "api/grows/{grow_id}/feed/refit"
                            ))
                            .await;
                        }
                        refitting.set(false);
                    }
                    updated
                }
                None => api::post::<FeedSchedule, FeedSchedule>("api/schedules", &schedule).await,
            };
            saving.set(false);
            match result {
                Ok(_) => {
                    navigate.with_value(|navigate| {
                        navigate(
                            &route_href("/schedules"),
                            leptos_router::NavigateOptions::default(),
                        );
                    });
                }
                Err(failure) => {
                    error.set(Some(api::failure_text(&failure, locale)));
                }
            }
        });
    };

    let cancel = move |_: leptos::ev::MouseEvent| {
        navigate.with_value(|navigate| {
            navigate(
                &route_href("/schedules"),
                leptos_router::NavigateOptions::default(),
            );
        });
    };

    let delete_schedule = move |_: leptos::ev::MouseEvent| {
        if let Some(id) = id.get_value() {
            let locale = i18n.get_locale_untracked();
            spawn_local(async move {
                match api::delete(&format!("api/schedules/{id}")).await {
                    Ok(()) => {
                        navigate.with_value(|navigate| {
                            navigate(
                                &route_href("/schedules"),
                                leptos_router::NavigateOptions::default(),
                            );
                        });
                    }
                    Err(failure) => {
                        error.set(Some(api::failure_text(&failure, locale)));
                    }
                }
            });
        }
    };

    view! {
        <ErrorIf error />
        <h2 class="mb-3">
            {move || if id.get_value().is_some() {
                t_string!(i18n, schedules.editor_edit_title).into_owned()
            } else {
                t_string!(i18n, schedules.editor_new_title).into_owned()
            }}
        </h2>
        {move || {
            if loading.get() {
                view! {
                    <p class="py-6 text-center text-base-content/60">
                        {t_string!(i18n, page.loading)}
                    </p>
                }
                    .into_any()
            } else {
                view! {
                    <div class="grid gap-3 sm:grid-cols-[repeat(auto-fill,minmax(16rem,1fr))]">
                        <ui::Field label=Signal::derive(move || t_string!(i18n, schedules.name_label).into_owned())>
                            <input
                                class="input"
                                type="text"
                                placeholder={t_string!(i18n, schedules.name_placeholder)}
                                prop:value=move || name.get()
                                on:input=move |ev| name.set(event_target_value(&ev))
                            />
                        </ui::Field>
                        <ui::Field label=Signal::derive(move || t_string!(i18n, schedules.source_label).into_owned())>
                            <input
                                class="input"
                                type="text"
                                placeholder={t_string!(i18n, schedules.source_placeholder)}
                                prop:value=move || source.get()
                                on:input=move |ev| source.set(event_target_value(&ev))
                            />
                        </ui::Field>
                        <ui::Field label=Signal::derive(move || t_string!(i18n, schedules.weeks_label).into_owned())>
                            <div class="flex items-center gap-2">
                                <button class="btn btn-outline btn-sm" on:click=remove_week>
                                    "−"
                                </button>
                                <span class="tabular-nums font-medium">{move || weeks.get().to_string()}</span>
                                <button class="btn btn-outline btn-sm" on:click=add_week>
                                    +
                                </button>
                            </div>
                        </ui::Field>
                    </div>
                    <p class="text-base-content/60 text-sm mt-1 mb-3">
                        {t_string!(i18n, schedules.range_hint)}
                    </p>
                    <div class="overflow-x-auto">
                        <EditorGrid rows weeks />
                    </div>
                    <div class="mt-3 flex flex-wrap items-center gap-2">
                        <button class="btn btn-outline btn-sm" on:click=add_row>
                            {t_string!(i18n, schedules.add_product)}
                        </button>
                        <span class="flex-1" />
                        <Show when=move || id.get_value().is_some()>
                            <ui::ArmConfirmButton
                                armed=Signal::derive(move || armed.get())
                                label=Signal::derive(move || t_string!(i18n, schedules.delete_schedule).into_owned())
                                confirm_label=Signal::derive(move || {
                                    t_string!(i18n, schedules.confirm_delete).into_owned()
                                })
                                on_arm=move |_| armed.set(true)
                                on_confirm=delete_schedule
                            />
                        </Show>
                        <button class="btn btn-primary" disabled=move || saving.get() on:click=save>
                            {t_string!(i18n, schedules.save)}
                        </button>
                        <button class="btn btn-ghost" on:click=cancel>
                            {t_string!(i18n, schedules.cancel)}
                        </button>
                    </div>
                    <div class="mt-2 flex flex-wrap items-center gap-3 text-sm">
                        <Show when=move || refitting.get()>
                            <span class="text-base-content/60">
                                {t_string!(i18n, schedules.refitting)}
                            </span>
                        </Show>
                        <Show when=move || saved.get()>
                            <span class="text-success">{t_string!(i18n, schedules.saved)}</span>
                        </Show>
                        {move || if id.get_value().is_some() {
                            view! {
                                <span class="text-base-content/60 text-xs">
                                    <strong class="font-medium">
                                        {t_string!(i18n, schedules.refit_heading)}
                                    </strong>
                                    {t_string!(i18n, schedules.refit_note)}
                                </span>
                            }
                                .into_any()
                        } else {
                            view! { <span /> }.into_any()
                        }}
                    </div>
                }
                    .into_any()
            }
        }}
    }
}

/// An empty draft row with `cells` blank cells.
#[must_use]
fn empty_row(cells: usize) -> RowDraft {
    RowDraft {
        id: String::new(),
        name: String::new(),
        unit: DoseUnit::Milliliters,
        group: String::new(),
        note: String::new(),
        cells: vec![String::new(); cells],
    }
}

/// The editable grid: one product row per row, dose inputs per
/// chart column. The group input lives in the product cell (the
/// grid keeps the chart's product order).
#[component]
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
fn EditorGrid(rows: RwSignal<Vec<RowDraft>>, weeks: RwSignal<u8>) -> impl IntoView {
    let i18n = use_i18n();

    let set_text = move |index: usize, field: &'static str| {
        move |ev| {
            let text = event_target_value(&ev);
            rows.update(|rows| {
                if let Some(row) = rows.get_mut(index) {
                    match field {
                        "name" => row.name = text,
                        "group" => row.group = text,
                        _ => row.note = text,
                    }
                }
            });
        }
    };

    let set_cell = move |(index, cell): (usize, usize)| {
        move |ev| {
            let text = event_target_value(&ev);
            rows.update(|rows| {
                if let Some(cell) = rows.get_mut(index).and_then(|row| row.cells.get_mut(cell)) {
                    *cell = text;
                }
            });
        }
    };

    let remove_row = move |index: usize| {
        rows.update(|rows| {
            rows.remove(index);
        });
    };

    let set_unit = move |index: usize| {
        move |ev| {
            let value = event_target_value(&ev);
            rows.update(|rows| {
                if let Some(row) = rows.get_mut(index) {
                    row.unit = DoseUnit::from_label(&value);
                }
            });
        }
    };

    view! {
        <table class="table table-sm min-w-full">
            <thead>
                <tr>
                    <th class="min-w-44">{t_string!(i18n, schedules.col_product)}</th>
                    <th class="text-center min-w-[4.5rem]">
                        {t_string!(i18n, schedules.col_propagation)}
                    </th>
                    <th class="text-center min-w-[4.5rem]">{t_string!(i18n, schedules.col_veg)}</th>
                    {move || {
                        let locale = i18n.get_locale();
                        (2..cell_count(weeks.get()) - 1)
                            .map(|index| {
                                view! {
                                    <th class="text-center min-w-[4.5rem]">
                                        {td_string!(
                                            locale,
                                            schedules.week_col,
                                            week = (index - 1).to_string(),
                                        )
                                        .into_owned()}
                                    </th>
                                }
                            })
                            .collect::<Vec<_>>()
                    }}
                    <th class="text-center min-w-[4.5rem]">{t_string!(i18n, schedules.col_water)}</th>
                    <th class="min-w-8" />
                </tr>
            </thead>
            <tbody>
                {move || {
                    let locale = i18n.get_locale();
                    rows.get()
                        .into_iter()
                        .enumerate()
                        .map(|(index, row)| {
                            let name_value = row.name.clone();
                            let group_value = row.group.clone();
                            let note_value = row.note.clone();
                            let unit = row.unit;
                            let cells = row.cells;
                            let name_set = set_text(index, "name");
                            let group_set = set_text(index, "group");
                            let note_set = set_text(index, "note");
                            let unit_set = set_unit(index);
                            let remove = move |_: leptos::ev::MouseEvent| remove_row(index);
                            view! {
                                <tr>
                                    <td class="align-top">
                                        <input
                                            class="input input-sm w-full bg-transparent"
                                            type="text"
                                            placeholder={t_string!(i18n, schedules.product_name_placeholder)}
                                            prop:value=move || name_value.clone()
                                            on:input=name_set
                                        />
                                        <div class="flex items-center gap-1 mt-1">
                                            <select
                                                class="select select-xs"
                                                prop:value=move || unit.label().to_owned()
                                                on:change=unit_set
                                            >
                                                <option value=DoseUnit::Milliliters.label()>
                                                    {td_string!(locale, schedules.unit_ml).into_owned()}
                                                </option>
                                                <option value=DoseUnit::Grams.label()>
                                                    {td_string!(locale, schedules.unit_gl).into_owned()}
                                                </option>
                                            </select>
                                            <input
                                                class="input input-xs w-24 bg-transparent"
                                                type="text"
                                                placeholder={t_string!(i18n, schedules.group_placeholder)}
                                                prop:value=move || group_value.clone()
                                                on:input=group_set
                                            />
                                        </div>
                                        <input
                                            class="input input-xs w-full bg-transparent mt-1"
                                            type="text"
                                            placeholder={t_string!(i18n, schedules.note_placeholder)}
                                            prop:value=move || note_value.clone()
                                            on:input=note_set
                                        />
                                    </td>
                                    {cells
                                        .into_iter()
                                        .enumerate()
                                        .map(|(cell_index, text)| {
                                            let cell_set = set_cell((index, cell_index));
                                            view! {
                                                <td class="align-top">
                                                    <input
                                                        class="input input-sm w-full text-center bg-transparent"
                                                        type="text"
                                                        inputmode="decimal"
                                                        placeholder="—"
                                                        prop:value=move || text.clone()
                                                        on:input=cell_set
                                                    />
                                                </td>
                                            }
                                        })
                                        .collect::<Vec<_>>()}
                                    <td class="align-top">
                                        <button
                                            class="btn btn-ghost btn-xs text-error"
                                            title={t_string!(i18n, schedules.delete)}
                                            on:click=remove
                                        >
                                            "✕"
                                        </button>
                                    </td>
                                </tr>
                            }
                        })
                        .collect::<Vec<_>>()
                }}
            </tbody>
        </table>
    }
}

#[cfg(test)]
mod tests {
    use super::dose_unit_display;
    use crate::i18n::Locale;

    /// The library meta line renders the same unit strings the
    /// editor's select offers; unknown stored labels echo through.
    #[test]
    fn dose_unit_display_maps_and_echoes() {
        assert_eq!(dose_unit_display(Locale::en, "ml_per_l"), "ml/L");
        assert_eq!(dose_unit_display(Locale::de, "g_per_l"), "g/L");
        assert_eq!(dose_unit_display(Locale::en, "tsp_per_gal"), "tsp_per_gal");
    }
}
