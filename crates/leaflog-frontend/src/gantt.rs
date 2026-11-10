//! Gantt timeline for the phase forecast (M5, spec §4.5).

// Day counts are small (< 1000); the i64→f64 precision lint is
// pedantic here (see docs/DECISIONS.md).
#![allow(clippy::cast_precision_loss)]

use crate::api::{Forecast, PhaseWindowData};
use crate::equipment::ErrorIf;
use leptos::prelude::*;
use leptos::task::spawn_local;

/// Phase display colors (calm botanical palette).
fn phase_color(label: &str) -> &'static str {
    match label {
        "germination" => "#8bc34a",
        "seedling" => "#66bb6a",
        "vegetative" => "#4caf50",
        "flower" => "#ab47bc",
        "drying" => "#ff9800",
        "curing" => "#a1887f",
        _ => "#90a4ae",
    }
}

/// The Gantt timeline for one grow.
#[component]
#[allow(clippy::too_many_lines)]
// declarative view code, not logic; see docs/DECISIONS.md
// Component prop; Leptos takes owned props.
#[allow(clippy::needless_pass_by_value)] // Leptos component prop contract; see docs/DECISIONS.md
pub fn GanttTimeline(grow_id: String) -> impl IntoView {
    let forecast = RwSignal::<Option<Forecast>>::new(None);
    let error = RwSignal::<Option<String>>::new(None);

    Effect::new(move |_| {
        let grow_id = grow_id.clone();
        spawn_local(async move {
            match crate::api::get::<Forecast>(&format!("api/grows/{grow_id}/forecast")).await {
                Ok(data) => forecast.set(Some(data)),
                Err(failure) => error.set(Some(failure.message)),
            }
        });
    });

    view! {
        <div class="gantt">
            <ErrorIf error />
            {move || {
                let Some(data) = forecast.get() else {
                    return view! { <p class="text-base-content/70">"Loading forecast…"</p> }.into_any();
                };
                let total_days = data
                    .windows
                    .last()
                    .map_or(1, |w| w.latest_end.max(w.likely_end))
                    .max(1);
                let today_pct =
                    (data.today_day as f64 / total_days as f64 * 100.0).min(100.0);
                let rows: Vec<_> = data
                    .windows
                    .iter()
                    .map(|window| gantt_row(window, total_days))
                    .collect();
                view! {
                    <div class="gantt-chart" role="img"
                        aria-label="Phase forecast timeline">
                        <div class="gantt-rows">
                            {rows}
                        </div>
                        <div class="gantt-today" style=format!("left: {today_pct:.1}%")
                            title=format!("today (day {})", data.today_day)
                        >
                            <span class="gantt-today-line"></span>
                            <span class="gantt-today-label">"today"</span>
                        </div>
                    </div>
                    <p class="text-base-content/70 text-sm">
                        "Bars show the likely window; light extensions show the                          earliest/latest range. Day count: "
                        {data.today_day}
                        " / "
                        {total_days}
                    </p>
                }
                .into_any()
            }}
        </div>
    }
}

/// Renders one row of the Gantt chart.
fn gantt_row(window: &PhaseWindowData, total_days: i64) -> impl IntoView {
    let total = total_days.max(1) as f64;
    let to_pct = |day: i64| day as f64 / total * 100.0;

    let start = to_pct(window.earliest_start);
    let bar_start = to_pct(window.likely_start);
    let bar_end = to_pct(window.likely_end);
    let end = to_pct(window.latest_end);
    let color = phase_color(&window.phase);
    let label = format!(
        "{} (day {}–{})",
        window.phase.replace('_', " "),
        window.likely_start,
        window.likely_end,
    );

    view! {
        <div class="gantt-row">
            <span class="gantt-phase-label">{window.phase.replace('_', " ")}</span>
            <div class="gantt-track">
                <div class="gantt-range"
                    style=format!(
                        "left: {start:.1}%; width: {:.1}%; --color: {color}",
                        (end - start).max(0.5)
                    )
                >
                    <div class="gantt-bar"
                        style=format!(
                            "left: {:.1}%; width: {:.1}%",
                            bar_start - start,
                            (bar_end - bar_start).max(0.5),
                        )
                        title=label
                    >
                        {if window.actual_start { "✓ " } else { "" }}
                        {window.likely_end - window.likely_start}
                        "d"
                    </div>
                </div>
            </div>
        </div>
    }
}
