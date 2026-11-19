//! The grow overview's feeding section: the fitted plan rendered
//! from plan days alone (the schedule itself lives in the Schedules
//! library; finished grows keep their frozen plan — ADR 0004).
//! Without a plan the section hides behind one quiet row — or, when
//! a schedule is assigned but its flowering window is not forecast
//! yet (a photoperiod grow pre-flip), one pending card (D130).

use leaflog_core::feed::{self, Dose, FeedPlan, FeedPlanDay, FeedSchedule, FittedPhase};
use leaflog_core::format::DecimalStyle;
use leaflog_core::grows::Grow;
use leptos::prelude::*;
use leptos::task::spawn_local;
use leptos_i18n::td_string;
use serde::Deserialize;

use crate::ErrorIf;
use crate::api;
use crate::i18n::{Locale, t_string, use_i18n};
use crate::labels;

/// The forecast slice the section reads for the today marker.
#[derive(Debug, Deserialize)]
struct TodaySlice {
    /// Days since grow start (1 = start day).
    pub today_day: i64,
}

/// The locale's decimal style: the only locale-dependent bit of the
/// dose rendering.
#[must_use]
pub fn decimal_style(locale: Locale) -> DecimalStyle {
    if locale == Locale::de {
        DecimalStyle::Comma
    } else {
        DecimalStyle::Dot
    }
}

/// One product row of the fitted chart.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PlanGridRow {
    /// Product name (verbatim user data).
    name: String,
    /// Dose unit suffix, e.g. `ml/L`.
    unit_display: String,
    /// Free-form product note, rendered under the name.
    note: Option<String>,
    /// Dose per column slot: propagation, veg, WK 1…N, water.
    cells: Vec<Option<Dose>>,
}

/// The fitted chart shape: the column plan plus the product rows.
#[derive(Debug, Clone, PartialEq, Eq)]
struct PlanGrid {
    /// The plan's flowering weeks (the WK 1…N column count).
    flower_weeks: u8,
    /// The product rows, in first-appearance order.
    rows: Vec<PlanGridRow>,
}

impl PlanGrid {
    /// The column slots per row: propagation, veg, the weeks, water.
    #[must_use]
    fn cell_count(&self) -> usize {
        3 + usize::from(self.flower_weeks)
    }

    /// The 0-based column slot of one plan day (the water tail
    /// occupies the last slot).
    #[must_use]
    fn slot_of(&self, phase: FittedPhase, week: Option<u8>) -> Option<usize> {
        match phase {
            FittedPhase::Propagation => Some(0),
            FittedPhase::Veg => Some(1),
            FittedPhase::Flower => week.and_then(|week| {
                let slot = 1 + usize::from(week);
                (slot < self.cell_count() - 1).then_some(slot)
            }),
            FittedPhase::Water => Some(self.cell_count() - 1),
        }
    }
}

/// The dose label of a cell: an em dash when unset, otherwise the
/// core renderer (`2` flat, `0,25–2` ranged, locale decimals).
#[must_use]
fn dose_label(dose: Option<Dose>, style: DecimalStyle) -> String {
    dose.map_or_else(|| "—".to_owned(), |dose| feed::format_dose(dose, style))
}

/// Builds the fitted chart shape from plan days: product rows in
/// first-appearance order, one dose cell per column (the seven days
/// of one fitted week share the dose, so the first wins).
#[must_use]
fn grid_from_plan(plan: &FeedPlan) -> PlanGrid {
    let flower_weeks = plan
        .days
        .iter()
        .filter(|day| day.phase == FittedPhase::Flower)
        .filter_map(|day| day.week)
        .max()
        .unwrap_or(plan.flower_weeks);
    let mut rows: Vec<PlanGridRow> = Vec::new();
    for day in &plan.days {
        for dose in &day.doses {
            if !rows.iter().any(|row| row.name == dose.name) {
                rows.push(PlanGridRow {
                    name: dose.name.clone(),
                    unit_display: dose.unit.display().to_owned(),
                    note: dose.note.clone(),
                    cells: vec![None; 3 + usize::from(flower_weeks)],
                });
            }
            let row = rows
                .iter_mut()
                .find(|row| row.name == dose.name)
                .expect("row just pushed");
            let slot = row_slot(day, row.cells.len()).filter(|slot| row.cells[*slot].is_none());
            if let Some(slot) = slot {
                row.cells[slot] = Some(dose.dose);
            }
        }
    }

    PlanGrid { flower_weeks, rows }
}

/// The column slot one plan day's doses land in; `None` for a day
/// that charts nowhere (a week number beyond the chart — never
/// produced by the fitter, but tolerated here).
#[must_use]
fn row_slot(day: &FeedPlanDay, cell_count: usize) -> Option<usize> {
    match day.phase {
        FittedPhase::Propagation => Some(0),
        FittedPhase::Veg => Some(1),
        FittedPhase::Flower => {
            let slot = 1 + usize::from(day.week?);
            (slot < cell_count - 1).then_some(slot)
        }
        FittedPhase::Water => Some(cell_count - 1),
    }
}

/// The feeding section of one grow overview.
#[component]
#[allow(clippy::needless_pass_by_value)] // Leptos component prop contract; see docs/DECISIONS.md
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
pub fn FeedSection(grow_id: String) -> impl IntoView {
    let i18n = use_i18n();
    let plan = RwSignal::<Option<FeedPlan>>::new(None);
    let pending = RwSignal::<Option<FeedSchedule>>::new(None);
    let today = RwSignal::<u16>::new(0);
    let error = RwSignal::<Option<String>>::new(None);

    Effect::new({
        let grow_id = grow_id.clone();
        move |_| {
            let grow_id = grow_id.clone();
            spawn_local(async move {
                let loaded =
                    api::get::<Option<FeedPlan>>(&format!("api/grows/{grow_id}/feed/plan")).await;
                match loaded {
                    Ok(fitted) => plan.set(fitted),
                    Err(failure) => error.set(Some(api::failure_text(
                        &failure,
                        i18n.get_locale_untracked(),
                    ))),
                }
                if plan.get_untracked().is_none() {
                    // No plan yet: either no schedule is assigned (the
                    // quiet row) or the grow's flowering window is not
                    // forecast yet — a photoperiod grow pre-flip — so
                    // the assignment itself gets a pending card (D130).
                    if let Ok(grow) = api::get::<Grow>(&format!("api/grows/{grow_id}")).await
                        && let Some(schedule_id) = grow.feed_schedule_id
                        && let Ok(schedule) =
                            api::get::<FeedSchedule>(&format!("api/schedules/{schedule_id}")).await
                    {
                        pending.set(Some(schedule));
                    }
                }
                if let Ok(slice) =
                    api::get::<TodaySlice>(&format!("api/grows/{grow_id}/forecast")).await
                {
                    // The forecast day count always fits u16: a grow
                    // spans at most a few years of days.
                    today.set(u16::try_from(slice.today_day).unwrap_or(u16::MAX));
                }
            });
        }
    });

    let edit_href = crate::route_href(&format!("/grows/{grow_id}/edit"));
    view! {
        <ErrorIf error />
        <Show
            when=move || plan.get().is_some()
            fallback=move || {
                let edit_href = edit_href.clone();
                view! {
                    <Show
                        when=move || pending.get().is_some()
                        fallback=move || {
                            view! {
                                <div class="my-4 flex flex-wrap items-center justify-between gap-3 rounded-box border border-dashed border-base-300 px-4 py-3 text-base-content/70">
                                    <p class="m-0">{t_string!(i18n, schedules.none_row)}</p>
                                    <a class="btn btn-outline btn-sm" href=edit_href.clone()>
                                        {t_string!(i18n, schedules.none_cta)}
                                    </a>
                                </div>
                            }
                        }
                    >
                        {move || {
                            let schedule = pending.get()?;
                            let locale = i18n.get_locale();
                            let open_href =
                                crate::route_href(&format!("/schedules/{}/edit", schedule.id));
                            let name = schedule.name;
                            Some(view! {
                                <div class="my-4 grid gap-4">
                                    <h3 class="m-0">{t_string!(i18n, grow.feed_schedule)}</h3>
                                    <div class="card bg-base-100 shadow-sm px-5 py-4">
                                        <p class="m-0">{td_string!(locale, schedules.pending_fit, name = name)}</p>
                                        <a class="btn btn-outline btn-sm mt-3" href=open_href>
                                            {t_string!(i18n, schedules.pending_open)}
                                        </a>
                                    </div>
                                </div>
                            })
                        }}
                    </Show>
                }
            }
        >
            {move || {
                let fitted = plan.get()?;
                let locale = i18n.get_locale();
                let style = decimal_style(locale);
                let grid = grid_from_plan(&fitted);
                let this_week = feed::this_week(&fitted.days, today.get());
                let marked = fitted
                    .days
                    .iter()
                    .find(|day| day.grow_day == today.get())
                    .and_then(|day| grid.slot_of(day.phase, day.week));
                let attribution = fitted
                    .fitted_at
                    .get(..10)
                    .unwrap_or(&fitted.fitted_at)
                    .to_owned();
                Some(view! {
                    <div class="my-4 grid gap-4">
                        <h3 class="m-0">{t_string!(i18n, grow.feed_schedule)}</h3>
                        {this_week.map(|week| {
                            let doses = week.doses.clone();
                            view! {
                                <div class="card bg-base-100 shadow-sm px-5 py-4">
                                    <div class="flex flex-wrap items-baseline gap-2">
                                        <h4 class="m-0">{t_string!(i18n, schedules.this_week)}</h4>
                                        <span class="badge badge-ghost">{week_chip(locale, &week)}</span>
                                    </div>
                                    <ul class="list-none m-0 mt-2 p-0 grid gap-1 sm:grid-cols-[repeat(auto-fill,minmax(13rem,1fr))]">
                                        {doses.iter().map(|dose| {
                                            let name = dose.name.clone();
                                            let note = dose.note.clone();
                                            let unit = dose.unit.display();
                                            let dose = dose.dose;
                                            view! {
                                                <li class="flex flex-wrap items-baseline gap-x-2">
                                                    <strong>{name}</strong>
                                                    <span class="ml-auto tabular-nums">
                                                        {dose_label(Some(dose), style)}
                                                        <span class="text-base-content/70">" "{unit}</span>
                                                    </span>
                                                    {note.map(|note| view! {
                                                        <span class="w-full text-base-content/70 text-sm">{note}</span>
                                                    })}
                                                </li>
                                            }
                                        }).collect::<Vec<_>>()}
                                    </ul>
                                    {flush_footer(locale, &week)}
                                </div>
                            }
                        })}
                        <div class="card bg-base-100 shadow-sm px-5 py-4">
                            <div class="flex flex-wrap items-baseline gap-2">
                                <h4 class="m-0">{t_string!(i18n, schedules.fitted_heading)}</h4>
                                <span class="badge badge-ghost">
                                    {td_string!(locale, schedules.fitted_days,
                                        count = fitted.days.len()).into_owned()}
                                </span>
                            </div>
                            <div class="mt-2 overflow-x-auto">
                                <table class="table table-sm"
                                    aria-label={t_string!(i18n, schedules.grid_aria).into_owned()}
                                >
                                    <thead>
                                        <tr>
                                            <th class="min-w-40">{t_string!(i18n, schedules.col_product)}</th>
                                            <th class="text-center">{t_string!(i18n, schedules.col_propagation)}</th>
                                            <th class="text-center">{t_string!(i18n, schedules.col_veg)}</th>
                                            {(1..=grid.flower_weeks).map(|week| {
                                                let header_slot = 1 + usize::from(week);
                                                view! {
                                                    <th class=move || {
                                                        if marked == Some(header_slot) {
                                                            "text-center bg-primary/10"
                                                        } else {
                                                            "text-center"
                                                        }
                                                    }>
                                                        {td_string!(locale, schedules.week_col,
                                                            week = week.to_string()).into_owned()}
                                                    </th>
                                                }
                                            }).collect::<Vec<_>>()}
                                            <th class="text-center">{t_string!(i18n, schedules.col_water)}</th>
                                        </tr>
                                    </thead>
                                    <tbody>
                                        {grid.rows.iter().map(|row| {
                                            let name = row.name.clone();
                                            let unit = row.unit_display.clone();
                                            let note = row.note.clone();
                                            let cells = row.cells.clone();
                                            view! {
                                                <tr>
                                                    <th class="min-w-40">
                                                        <div class="font-medium">{name}</div>
                                                        <div class="text-base-content/70 text-sm">{unit}</div>
                                                        {note.map(|note| view! {
                                                            <div class="text-base-content/70 text-sm">{note}</div>
                                                        })}
                                                    </th>
                                                    {cells.iter().enumerate().map(|(slot, cell)| {
                                                        let dose = *cell;
                                                        view! {
                                                            <td class=move || {
                                                                if marked == Some(slot) {
                                                                    "text-center tabular-nums bg-primary/10"
                                                                } else {
                                                                    "text-center tabular-nums"
                                                                }
                                                            }>
                                                                {dose_label(dose, style)}
                                                            </td>
                                                        }
                                                    }).collect::<Vec<_>>()}
                                                </tr>
                                            }
                                        }).collect::<Vec<_>>()}
                                    </tbody>
                                </table>
                            </div>
                            <p class="mt-2 mb-0 text-base-content/70 text-sm">
                                {td_string!(locale, schedules.fitted_from,
                                    name = fitted.schedule_name,
                                    date = attribution).into_owned()}
                                " "
                                {t_string!(i18n, schedules.refit_auto)}
                            </p>
                        </div>
                    </div>
                })
            }}
        </Show>
    }
}

/// The "This week" position chip: flowering weeks count themselves,
/// every other phase shows the day only.
#[must_use]
fn week_chip(locale: Locale, week: &feed::ThisWeek) -> String {
    let phase = labels::fitted_phase_label(locale, week.phase);
    match (week.week, week.total_weeks) {
        (Some(current), Some(total)) => td_string!(
            locale,
            schedules.chip_week,
            phase = phase,
            week = current.to_string(),
            total = total.to_string(),
        )
        .into_owned(),
        _ => td_string!(locale, schedules.chip_phase, phase = phase).into_owned(),
    }
}

/// The "This week" footer: the flush countdown while it still lies
/// ahead, a plain-water line once the tail starts.
fn flush_footer(locale: Locale, week: &feed::ThisWeek) -> Option<AnyView> {
    if week.phase == FittedPhase::Water {
        return Some(
            view! {
                <p class="mt-2 mb-0 border-t border-base-300 pt-2 text-base-content/70 text-sm">
                    {td_string!(locale, schedules.flush_now).into_owned()}
                </p>
            }
            .into_any(),
        );
    }
    week.days_until_flush.map(|days| {
        view! {
            <p class="mt-2 mb-0 border-t border-base-300 pt-2 text-base-content/70 text-sm">
                {td_string!(locale, schedules.flush_days, count = days).into_owned()}
            </p>
        }
        .into_any()
    })
}

#[cfg(test)]
mod tests {
    use super::{PlanGrid, PlanGridRow, dose_label, grid_from_plan, row_slot};
    use leaflog_core::feed::{Dose, DoseUnit, FeedPlan, FeedPlanDay, FittedDose, FittedPhase};
    use leaflog_core::format::DecimalStyle;

    fn day(grow_day: u16, phase: FittedPhase, week: Option<u8>) -> FeedPlanDay {
        FeedPlanDay {
            grow_day,
            phase,
            week,
            doses: Vec::new(),
        }
    }

    fn plan(days: Vec<FeedPlanDay>) -> FeedPlan {
        FeedPlan {
            schedule_id: "s1".to_owned(),
            schedule_name: "Test".to_owned(),
            flower_weeks: 2,
            fitted_at: "2026-11-19T00:00:00Z".to_owned(),
            days,
        }
    }

    fn dose(name: &str, dose: Dose) -> FittedDose {
        FittedDose {
            name: name.to_owned(),
            unit: DoseUnit::Milliliters,
            group: "Feeds".to_owned(),
            note: None,
            dose,
        }
    }

    /// The grid reads the column plan off the days: flower weeks
    /// from the widest week marker; every segment column exists.
    #[test]
    fn grid_counts_columns_from_days() {
        let mut day_one = day(1, FittedPhase::Propagation, None);
        day_one.doses = vec![dose(
            "Root",
            Dose {
                min: 1000,
                max: 4000,
            },
        )];
        let mut day_two = day(8, FittedPhase::Flower, Some(1));
        day_two.doses = vec![dose(
            "Bloom",
            Dose {
                min: 2000,
                max: 2000,
            },
        )];
        let mut day_three = day(70, FittedPhase::Water, None);
        day_three.doses = vec![dose("Bloom", Dose { min: 0, max: 0 })];
        let grid = grid_from_plan(&plan(vec![day_one, day_two, day_three]));
        assert_eq!(grid.flower_weeks, 1);
        assert_eq!(
            grid.rows
                .iter()
                .map(|row| row.name.as_str())
                .collect::<Vec<_>>(),
            ["Root", "Bloom"]
        );
        // Column slots: propagation, veg, WK 1, water.
        assert_eq!(grid.cell_count(), 4);
    }

    /// Repeated days of one fitted week collapse into the same
    /// column; the first fitted dose wins (repeats are identical).
    #[test]
    fn week_days_share_a_column() {
        let mut day_one = day(8, FittedPhase::Flower, Some(1));
        day_one.doses = vec![dose(
            "Bloom",
            Dose {
                min: 2000,
                max: 4000,
            },
        )];
        let mut day_two = day(9, FittedPhase::Flower, Some(1));
        day_two.doses = vec![dose(
            "Bloom",
            Dose {
                min: 2000,
                max: 4000,
            },
        )];
        let grid = grid_from_plan(&plan(vec![day_one, day_two]));
        assert_eq!(grid.rows.len(), 1);
        let cells = &grid.rows[0].cells;
        assert_eq!(cells.len(), 4); // propagation + veg slots stay empty
        assert_eq!(
            cells[2],
            Some(Dose {
                min: 2000,
                max: 4000
            })
        );
    }

    /// The plan's chart weeks fill in when no flower day carries a
    /// week marker (a degenerate plan that never flowers).
    #[test]
    fn weeks_fall_back_to_the_chart_count() {
        let mut veg_day = day(5, FittedPhase::Veg, None);
        veg_day.doses = vec![dose(
            "Grow",
            Dose {
                min: 1000,
                max: 2000,
            },
        )];
        let grid = grid_from_plan(&plan(vec![veg_day]));
        assert_eq!(grid.flower_weeks, 2);
    }

    /// Dose labels: dash for unset cells, one number for flat doses,
    /// a locale-aware range otherwise.
    #[test]
    fn dose_labels_render_ranges_in_locale() {
        let style = DecimalStyle::Comma;
        assert_eq!(dose_label(None, style), "—");
        assert_eq!(
            dose_label(
                Some(Dose {
                    min: 2000,
                    max: 2000
                }),
                style
            ),
            "2"
        );
        assert_eq!(
            dose_label(
                Some(Dose {
                    min: 250,
                    max: 4000
                }),
                style
            ),
            "0,25–4"
        );
    }

    /// Column slots: propagation, veg, then the 1-based weeks, then
    /// the water tail; a stray week slot beyond the chart drops.
    #[test]
    fn slots_order_propagation_veg_weeks_water() {
        let grid = PlanGrid {
            flower_weeks: 8,
            rows: Vec::new(),
        };
        assert_eq!(grid.slot_of(FittedPhase::Propagation, None), Some(0));
        assert_eq!(grid.slot_of(FittedPhase::Veg, None), Some(1));
        assert_eq!(grid.slot_of(FittedPhase::Flower, Some(3)), Some(4));
        assert_eq!(grid.slot_of(FittedPhase::Water, None), Some(10));
        assert_eq!(row_slot(&day(1, FittedPhase::Flower, Some(9)), 11), None);
    }

    /// Rows keep first-appearance order even when a product returns
    /// in a later segment.
    #[test]
    fn rows_keep_first_appearance_order() {
        let mut veg_day = day(5, FittedPhase::Veg, None);
        veg_day.doses = vec![dose(
            "Grow",
            Dose {
                min: 1000,
                max: 2000,
            },
        )];
        let mut flower_day = day(10, FittedPhase::Flower, Some(1));
        flower_day.doses = vec![
            dose(
                "Bloom",
                Dose {
                    min: 2000,
                    max: 2000,
                },
            ),
            dose(
                "Grow",
                Dose {
                    min: 1000,
                    max: 1000,
                },
            ),
        ];
        let grid = grid_from_plan(&plan(vec![veg_day, flower_day]));
        assert_eq!(
            grid.rows
                .iter()
                .map(|row| row.name.as_str())
                .collect::<Vec<_>>(),
            ["Grow", "Bloom"]
        );
        // The Grow row keeps its veg dose and gains the WK 1 cell.
        assert_eq!(
            grid.rows[0].cells[1],
            Some(Dose {
                min: 1000,
                max: 2000
            })
        );
        assert_eq!(
            grid.rows[0].cells[2],
            Some(Dose {
                min: 1000,
                max: 1000
            })
        );
    }

    /// The unit suffix renders from the core display spelling, and
    /// an unset note stays unset.
    #[test]
    fn rows_carry_unit_and_note() {
        let mut flower_day = day(10, FittedPhase::Flower, Some(1));
        flower_day.doses = vec![FittedDose {
            name: "Micro".to_owned(),
            unit: DoseUnit::Grams,
            group: "Biology".to_owned(),
            note: Some("once weekly".to_owned()),
            dose: Dose { min: 400, max: 400 },
        }];
        let grid = grid_from_plan(&plan(vec![flower_day]));
        assert_eq!(grid.rows[0].unit_display, "g/L");
        assert_eq!(grid.rows[0].note.as_deref(), Some("once weekly"));
        let empty = PlanGridRow {
            name: "X".to_owned(),
            unit_display: "ml/L".to_owned(),
            note: None,
            cells: Vec::new(),
        };
        assert_eq!(empty.note, None);
    }
}
