//! Feed schedule view (M6): template picker, fit button, and the
//! before/after diff chart.

use crate::api;
use crate::equipment::ErrorIf;
use leptos::prelude::*;
use leptos::task::spawn_local;
use serde::Deserialize;

/// One template row as the API returns it (for the "before" list).
#[derive(Debug, Clone, Deserialize)]
pub struct TemplateRow {
    /// Template phase.
    pub phase: String,
    /// Template day offset (kept for the diff table; not charted yet).
    #[allow(dead_code)] // deserialized from the API; will be used by the diff table
    pub day_offset: u16,
    /// EC target, µS/cm.
    pub ec_target_us: u32,
}

/// The fit response.
#[derive(Debug, Clone, Deserialize)]
pub struct FitResponse {
    /// Template rows (before).
    pub before: Vec<TemplateRow>,
    /// The fitted days (after).
    pub after: FitResultData,
}

/// The fit result data.
#[derive(Debug, Clone, Deserialize)]
pub struct FitResultData {
    /// Fitted days.
    pub days: Vec<PlanDayData>,
    /// Template total days.
    pub template_total_days: u16,
    /// Actual total days (kept for the summary; not yet displayed).
    #[allow(dead_code)] // deserialized from the API; will be used by the summary
    pub actual_total_days: u16,
}

/// One fitted day.
#[derive(Debug, Clone, Deserialize)]
pub struct PlanDayData {
    /// Grow day.
    pub grow_day: u16,
    /// Feed phase.
    pub phase: String,
    /// EC target, µS/cm.
    pub ec_target_us: u32,
    /// Protected (ripen/flush).
    pub protected: bool,
}

/// A feed template (for the picker).
#[derive(Debug, Clone, Deserialize)]
pub struct FeedTemplateInfo {
    /// ULID.
    pub id: String,
    /// Name.
    pub name: String,
    /// Substrate.
    pub substrate: String,
}

/// The feed schedule panel for one grow.
#[component]
#[allow(clippy::too_many_lines)] // declarative view code, not logic; see docs/DECISIONS.md
pub fn FeedPanel(grow_id: String) -> impl IntoView {
    let templates = RwSignal::<Vec<FeedTemplateInfo>>::new(Vec::new());
    let selected = RwSignal::<String>::new(String::new());
    let fit = RwSignal::<Option<FitResponse>>::new(None);
    let error = RwSignal::<Option<String>>::new(None);

    Effect::new({
        let grow_id = grow_id.clone();
        move |_| {
            let grow_id = grow_id.clone();
            spawn_local(async move {
                match api::get::<Vec<FeedTemplateInfo>>("api/feeds/templates").await {
                    Ok(list) => templates.set(list),
                    Err(failure) => error.set(Some(failure.message)),
                }
                // Load the existing plan if there is one.
                match api::get::<Vec<PlanDayData>>(&format!("api/grows/{grow_id}/feed/fit")).await {
                    Ok(days) if !days.is_empty() => {
                        fit.set(Some(FitResponse {
                            before: Vec::new(),
                            after: FitResultData {
                                days,
                                template_total_days: 0,
                                actual_total_days: 0,
                            },
                        }));
                    }
                    _ => {}
                }
            });
        }
    });

    let do_fit = move |_ev: leptos::ev::MouseEvent| {
        let grow_id = grow_id.clone();
        let template_id = selected.get();
        if template_id.is_empty() {
            return;
        }
        spawn_local(async move {
            let body = serde_json::json!({ "template_id": template_id });
            match api::post::<serde_json::Value, FitResponse>(
                &format!("api/grows/{grow_id}/feed/fit"),
                &body,
            )
            .await
            {
                Ok(result) => fit.set(Some(result)),
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    };

    view! {
        <div class="feed-panel">
            <ErrorIf error />
            <div class="feed-toolbar">
                <label class="field">
                    <span>"Feed template"</span>
                    <select
                        prop:value=move || selected.get()
                        on:change=move |ev| selected.set(event_target_value(&ev))
                    >
                        <option value="">"— pick a template —"</option>
                        {move || {
                            templates
                                .get()
                                .into_iter()
                                .map(|template| {
                                    view! {
                                        <option value=template.id>
                                            {format!("{} ({})", template.name, template.substrate)}
                                        </option>
                                    }
                                })
                                .collect::<Vec<_>>()
                        }}
                    </select>
                </label>
                <button class="primary" on:click=do_fit>"Fit to timeline"</button>
            </div>

            {move || {
                let Some(result) = fit.get() else {
                    return view! {
                        <p class="muted">"Pick a template and fit it to see the feeding schedule."</p>
                    }.into_any();
                };
                let max_ec = result
                    .after
                    .days
                    .iter()
                    .map(|d| d.ec_target_us)
                    .max()
                    .unwrap_or(1)
                    .max(1);
                let total_days = result.after.days.len().max(1);
                view! {
                    <div class="feed-diff">
                        {result.before.first().map(|_| {
                            view! {
                                <div class="feed-before">
                                    <h4>"Template (before)"</h4>
                                    <ul class="feed-list">
                                        {result
                                            .before
                                            .iter()
                                            .map(|row| {
                                                view! {
                                                    <li>
                                                        <span class="muted">{row.phase.replace('_', " ")}</span>
                                                        <span>{format!("{} µS/cm", row.ec_target_us)}</span>
                                                    </li>
                                                }
                                            })
                                            .collect::<Vec<_>>()}
                                    </ul>
                                </div>
                            }
                        })}
                        <div class="feed-after">
                            <h4>"Fitted schedule (after)"</h4>
                            <div class="feed-chart" role="img" aria-label="Feed schedule chart">
                                {result
                                    .after
                                    .days
                                    .iter()
                                    .map(|day| {
                                        let height = f64::from(day.ec_target_us) / f64::from(max_ec) * 100.0;
                                        let color = if day.protected {
                                            "#ff9800"
                                        } else {
                                            "#4caf50"
                                        };
                                        view! {
                                            <div class="feed-bar-container"
                                                title=format!("day {}: {} µS/cm ({})", day.grow_day, day.ec_target_us, day.phase)
                                            >
                                                <div class="feed-bar"
                                                    style=format!("height: {height:.0}%; background: {color};")
                                                ></div>
                                            </div>
                                        }
                                    })
                                    .collect::<Vec<_>>()}
                            </div>
                            <p class="muted small">
                                {format!(
                                    "{} days (template: {})",
                                    total_days,
                                    if result.after.template_total_days > 0 {
                                        format!("{} days", result.after.template_total_days)
                                    } else {
                                        "—".to_owned()
                                    }
                                )}
                                " · orange bars = ripen/flush (protected)"
                            </p>
                        </div>
                    </div>
                }
                    .into_any()
            }}
        </div>
    }
}
