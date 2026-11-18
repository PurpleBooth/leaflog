//! Gantt timeline for the phase forecast (M5, spec §4.5).

// Day counts are small (< 1000); the i64→f64 precision lint is
// pedantic here (see docs/DECISIONS.md).
#![allow(clippy::cast_precision_loss)]

use crate::ErrorIf;
use crate::api::{Forecast, PhaseWindowData};
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

/// True when the forecast stops without a projected total: the
/// open-ended photoperiod veg (no flip pin, no plan) projects no
/// flower window, and drying and curing always follow the flower.
fn is_open_ended(windows: &[PhaseWindowData]) -> bool {
    !windows.iter().any(|window| window.phase == "flower")
}

/// The chart horizon in grow days: the percentage base for the bars
/// and the today marker. With a projected total it is the last
/// window's latest end; an open-ended forecast extends through today
/// so the current phase's bar still reaches the marker.
fn forecast_horizon(forecast: &Forecast) -> i64 {
    // The horizon must cover every window's latest band, not just the
    // last one's: a compressed boundary leaves earlier phases with a
    // wider uncertainty range that still has to fit on the track.
    let last_end = forecast
        .windows
        .iter()
        .map(|window| window.latest_end.max(window.likely_end))
        .max()
        .unwrap_or(1)
        .max(1);
    if is_open_ended(&forecast.windows) {
        last_end.max(forecast.today_day)
    } else {
        last_end
    }
}

/// Track percentages for one row: where the light earliest/latest
/// range starts, where the likely bar starts and ends, and where the
/// range ends. An open-ended row (a phase with no projected end)
/// stretches the bar and its range to the full track width so the
/// today marker sits inside it.
fn track_percentages(
    window: &PhaseWindowData,
    total_days: i64,
    open: bool,
) -> (f64, f64, f64, f64) {
    let total = total_days.max(1) as f64;
    let to_pct = |day: i64| day as f64 / total * 100.0;
    let bar_end = if open {
        100.0
    } else {
        to_pct(window.likely_end)
    };
    let end = if open {
        100.0
    } else {
        to_pct(window.latest_end)
    };
    (
        to_pct(window.earliest_start),
        to_pct(window.likely_start),
        bar_end,
        end,
    )
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
                // An open-ended photoperiod veg (no flip pin, no plan)
                // projects no flower, drying or curing window: there is
                // no total to count towards, so the chart horizon runs
                // through today and the caption shows only the count.
                let open = is_open_ended(&data.windows);
                let total_days = forecast_horizon(&data);
                let today_pct =
                    (data.today_day as f64 / total_days as f64 * 100.0).min(100.0);
                let rows: Vec<_> = data
                    .windows
                    .iter()
                    .enumerate()
                    .map(|(index, window)| {
                        gantt_row(window, total_days, open && index + 1 == data.windows.len())
                    })
                    .collect();
                let count_suffix = if open {
                    String::new()
                } else {
                    format!(" / {total_days}")
                };
                view! {
                    <div class="gantt-chart" role="img"
                        aria-label="Phase forecast timeline">
                        <div class="gantt-rows">
                            {rows}
                        </div>
                        // The overlay spans exactly the track columns, so
                        // the marker's inline percentage below resolves
                        // against the track width, past the label gutter
                        // (see `.gantt-today-area` in tailwind.css).
                        <div class="gantt-today-area">
                            <div class="gantt-today" style=format!("left: {today_pct:.1}%")
                                title=format!("today (day {})", data.today_day)
                            >
                                <span class="gantt-today-line"></span>
                                <span class="gantt-today-label">"today"</span>
                            </div>
                        </div>
                    </div>
                    <p class="text-base-content/70 text-sm">
                        "Bars show the likely window; light extensions show the earliest/latest range. Day count: "
                        {data.today_day}
                        {count_suffix}
                    </p>
                }
                .into_any()
            }}
        </div>
    }
}

/// Renders one row of the Gantt chart. The last row of an open-ended
/// forecast (the ongoing veg) has no projected end, so `open` is true
/// and its bar stretches to the track end.
fn gantt_row(window: &PhaseWindowData, total_days: i64, open: bool) -> impl IntoView {
    let (start, bar_start, bar_end, end) = track_percentages(window, total_days, open);
    let color = phase_color(&window.phase);
    let name = crate::phase_pick::phase_from_label(&window.phase).map_or_else(
        || window.phase.replace('_', " "),
        |phase| crate::phase_pick::phase_display(phase).to_owned(),
    );
    let label = if open {
        format!("{name} (day {}–ongoing)", window.likely_start)
    } else {
        format!("{name} (day {}–{})", window.likely_start, window.likely_end)
    };
    let duration = if open {
        "ongoing".to_owned()
    } else {
        format!("{}d", window.likely_end - window.likely_start)
    };

    view! {
        <div class="gantt-row">
            <span class="gantt-phase-label">{name}</span>
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
                        {duration}
                    </div>
                </div>
            </div>
        </div>
    }
}

#[cfg(test)]
mod open_ended_forecast_tests {
    use super::{forecast_horizon, is_open_ended, track_percentages};
    use crate::api::{Forecast, PhaseWindowData};

    fn window(phase: &str, start: i64, end: i64, latest_end: i64) -> PhaseWindowData {
        PhaseWindowData {
            phase: phase.to_owned(),
            earliest_start: start,
            likely_start: start,
            latest_start: start,
            earliest_end: start,
            likely_end: end,
            latest_end,
            actual_start: false,
        }
    }

    // The open-ended shape the backend now sends for an un-flipped
    // photoperiod: the chain stops at a point window for the veg.
    fn open_windows() -> Vec<PhaseWindowData> {
        vec![
            window("germination", 0, 6, 10),
            window("seedling", 6, 23, 23),
            window("vegetative", 23, 23, 23),
        ]
    }

    fn full_windows() -> Vec<PhaseWindowData> {
        vec![
            window("germination", 0, 6, 10),
            window("seedling", 6, 23, 23),
            window("vegetative", 23, 37, 40),
            window("flower", 37, 103, 114),
            window("drying", 103, 110, 117),
            window("curing", 110, 132, 142),
        ]
    }

    // Percentages go through f64; compare with slack, not equality.
    fn close(actual: f64, expected: f64) -> bool {
        (actual - expected).abs() < 1e-9
    }

    #[test]
    fn no_flower_window_means_no_projected_total() {
        assert!(is_open_ended(&open_windows()));
        assert!(!is_open_ended(&full_windows()));
        assert!(is_open_ended(&[]));
    }

    #[test]
    fn projected_total_comes_from_the_last_window() {
        let forecast = Forecast {
            today_day: 10,
            phase: "vegetative".to_owned(),
            phase_day: 10,
            windows: full_windows(),
        };
        assert_eq!(forecast_horizon(&forecast), 142);
    }

    #[test]
    fn open_ended_horizon_extends_through_today() {
        let late = Forecast {
            today_day: 40,
            phase: "vegetative".to_owned(),
            phase_day: 40,
            windows: open_windows(),
        };
        assert_eq!(forecast_horizon(&late), 40);
        let early = Forecast {
            today_day: 5,
            phase: "germination".to_owned(),
            phase_day: 5,
            windows: open_windows(),
        };
        assert_eq!(forecast_horizon(&early), 23);
    }

    #[test]
    fn open_row_stretches_to_the_track_end() {
        let (start, bar_start, bar_end, end) = track_percentages(&open_windows()[2], 40, true);
        assert!(close(start, 57.5) && close(bar_start, 57.5));
        assert!(close(bar_end, 100.0) && close(end, 100.0));
    }

    #[test]
    fn closed_row_keeps_the_window_bands() {
        let germ = window("germination", 0, 6, 10);
        let (start, bar_start, bar_end, end) = track_percentages(&germ, 100, false);
        assert!(close(start, 0.0) && close(bar_start, 0.0));
        assert!(close(bar_end, 6.0) && close(end, 10.0));
    }
}
