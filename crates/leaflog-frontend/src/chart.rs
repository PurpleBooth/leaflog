//! Hand-rolled SVG history chart (spec §3): a line over the shaded
//! recommended-band corridor, fed by the grow history endpoint, with
//! rolling Day / Week / Month windows (24 h / 7 d / 30 d). No chart
//! library — the SVG is composed from Rust views.

use crate::api;
use crate::api::GrowHistory;
use leaflog_core::chart::parse_epoch;
use leptos::prelude::*;
use leptos::task::spawn_local;

/// The SVG canvas width (the viewBox is fixed; the element scales).
const WIDTH: f64 = 600.0;
/// The SVG canvas height.
const HEIGHT: f64 = 220.0;

/// One windowed chart for one sensor role on the grow overview.
#[component]
pub fn HistoryChart(
    grow_id: String,
    #[prop(default = "temperature")] role: &'static str,
    #[prop(default = "Temperature")] title: &'static str,
) -> impl IntoView {
    let window = RwSignal::new("day".to_owned());
    let data = RwSignal::<Option<GrowHistory>>::new(None);
    let failure = RwSignal::<Option<String>>::new(None);

    Effect::new(move |_| {
        let id = grow_id.clone();
        let window_label = window.get();
        spawn_local(async move {
            match api::get::<GrowHistory>(&format!(
                "api/grows/{id}/history?role={role}&window={window_label}"
            ))
            .await
            {
                Ok(value) => {
                    data.set(Some(value));
                    failure.set(None);
                }
                Err(error) => failure.set(Some(error.message)),
            }
        });
    });

    view! {
        <div class="card bg-base-100 shadow-sm px-4 py-3 my-3">
            <div class="flex flex-wrap items-baseline gap-2">
                <h4 class="m-0">{title}</h4>
                <div class="flex gap-1" role="group" aria-label="Chart window">
                    {["day", "week", "month"]
                        .into_iter()
                        .map(|label| {
                            let caption = match label {
                                "day" => "Day",
                                "week" => "Week",
                                _ => "Month",
                            };
                            view! {
                                <button
                                    class=move || if window.get() == label {
                                        "btn btn-xs btn-primary"
                                    } else {
                                        "btn btn-xs"
                                    }
                                    on:click=move |_| window.set(label.to_owned())
                                >
                                    {caption}
                                </button>
                            }
                        })
                        .collect::<Vec<_>>()}
                </div>
                <Show
                    when=move || data.get().is_some_and(|chart| chart.degraded)
                    fallback=|| ()
                >
                    <span class="badge badge-warning badge-sm">"snapshot only"</span>
                </Show>
            </div>
            <Show
                when=move || data.get().is_some()
                fallback=move || {
                    view! {
                        <p class="text-base-content/70 text-sm">
                            {move || failure.get().unwrap_or_else(|| "Loading…".to_owned())}
                        </p>
                    }
                }
            >
                {move || {
                    let current = data.get()?;
                    if current.bound {
                        return Some(view! { <ChartSvg chart=current /> }.into_any());
                    }
                    Some(
                        view! {
                            <p class="text-base-content/70 text-sm">
                                "No sensor bound for this role."
                            </p>
                        }
                            .into_any(),
                    )
                }}
            </Show>
        </div>
    }
}

/// The line-and-corridor SVG for one loaded chart.
#[component]
fn ChartSvg(chart: GrowHistory) -> impl IntoView {
    let from = parse_epoch(&chart.series.from).unwrap_or(0);
    let to = parse_epoch(&chart.series.to).unwrap_or(from + 1);
    let points: Vec<(i64, f64)> = chart
        .series
        .points
        .iter()
        .filter_map(|point| parse_epoch(&point.at).map(|at| (at, point.value)))
        .collect();
    let bands: Vec<(i64, i64, f64, f64)> = chart
        .bands
        .iter()
        .filter_map(|band| {
            Some((
                parse_epoch(&band.start)?,
                parse_epoch(&band.end)?,
                band.min,
                band.max,
            ))
        })
        .collect();
    let geom = geometry(&points, &bands, from, to, WIDTH, HEIGHT);
    let line = geom
        .line_points
        .iter()
        .map(|(x, y)| format!("{x:.1},{y:.1}"))
        .collect::<Vec<_>>()
        .join(" ");
    let y_max_label = leaflog_core::units::temp_both(geom.y_max);
    let y_min_label = leaflog_core::units::temp_both(geom.y_min);
    let y_min_y = format!("{:.1}", HEIGHT - 6.0);

    view! {
        <svg viewBox=format!("0 0 {WIDTH} {HEIGHT}") class="w-full" role="img" aria-label="Temperature history">
            {geom
                .band_rects
                .iter()
                .map(|(x, w, y, h)| {
                    view! {
                        <rect
                            x=format!("{x:.1}")
                            y=format!("{y:.1}")
                            width=format!("{w:.1}")
                            height=format!("{h:.1}")
                            class="fill-primary/15"
                        />
                    }
                })
                .collect::<Vec<_>>()}
            <polyline points=line class="fill-none stroke-primary" stroke-width="2" />
            <text x="4" y="14" class="fill-base-content/60">{y_max_label}</text>
            <text x="4" y=y_min_y class="fill-base-content/60">{y_min_label}</text>
        </svg>
    }
}

/// The plotted geometry: screen-space points and band rects with the
/// padded value range (for the axis labels). Pure and unit-tested —
/// no DOM.
#[derive(Debug, Clone, PartialEq)]
pub struct ChartGeometry {
    /// Line points in canvas space.
    pub line_points: Vec<(f64, f64)>,
    /// Band rects as (x, width, y, height), clipped to the canvas.
    pub band_rects: Vec<(f64, f64, f64, f64)>,
    /// The padded lowest value (axis label).
    pub y_min: f64,
    /// The padded highest value (axis label).
    pub y_max: f64,
}

/// Maps the windowed series and corridor segments into canvas space.
///
/// Points at the window edges land on the canvas edges; the value
/// range is padded 10 % (at least half a unit) so the line never
/// touches the frame. Band rects shorter than a pixel drop out.
#[must_use]
pub fn geometry(
    points: &[(i64, f64)],
    bands: &[(i64, i64, f64, f64)],
    from: i64,
    to: i64,
    width: f64,
    height: f64,
) -> ChartGeometry {
    // Epoch seconds (~1.7e9) sit far below f64's 52-bit mantissa.
    #[allow(clippy::cast_precision_loss)]
    let span = (to - from).max(1) as f64;
    // Same bound as above; the subtraction stays within the window.
    #[allow(clippy::cast_precision_loss)]
    let x_of = |at: i64| width * (at - from) as f64 / span;
    let mut values: Vec<f64> = points.iter().map(|(_, value)| *value).collect();
    for &(_, _, min, max) in bands {
        values.push(min);
        values.push(max);
    }
    let low = values.iter().copied().fold(f64::INFINITY, f64::min);
    let high = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let (low, high) = if low.is_finite() && high.is_finite() {
        (low, high)
    } else {
        (0.0, 1.0)
    };
    let padding = ((high - low) * 0.1).max(0.5);
    let y_min = low - padding;
    let y_max = high + padding;
    let y_span = (y_max - y_min).max(0.001);
    let y_of = |value: f64| height * (1.0 - (value - y_min) / y_span);
    let line_points = points
        .iter()
        .map(|(at, value)| (x_of(*at), y_of(*value)))
        .collect();
    let band_rects = bands
        .iter()
        .map(|&(start, end, min, max)| {
            let x = x_of(start).clamp(0.0, width);
            let x_end = x_of(end).clamp(0.0, width);
            let y_top = y_of(max).clamp(0.0, height);
            let y_bottom = y_of(min).clamp(0.0, height);
            (x, x_end - x, y_top, (y_bottom - y_top).max(1.0))
        })
        .filter(|&(_, w, _, _)| w > 0.5)
        .collect();
    ChartGeometry {
        line_points,
        band_rects,
        y_min,
        y_max,
    }
}

#[cfg(test)]
mod tests {
    use super::geometry;

    #[test]
    fn points_map_to_the_canvas_edges() {
        let geom = geometry(&[(0, 10.0), (100, 20.0)], &[], 0, 100, 600.0, 220.0);
        let (first_x, _) = geom.line_points[0];
        let (last_x, _) = geom.line_points[1];
        assert!((first_x - 0.0).abs() < 0.001);
        assert!((last_x - 600.0).abs() < 0.001);
    }

    #[test]
    fn higher_values_plot_higher_on_screen() {
        let geom = geometry(&[(0, 10.0), (100, 20.0)], &[], 0, 100, 600.0, 220.0);
        let (_, first_y) = geom.line_points[0];
        let (_, last_y) = geom.line_points[1];
        assert!(last_y < first_y, "screen y grows downwards");
    }

    #[test]
    fn band_rects_clip_to_the_window() {
        // A corridor starting before the window clips at x = 0.
        let geom = geometry(
            &[(50, 20.0)],
            &[(-500, 500, 18.0, 22.0)],
            0,
            100,
            600.0,
            220.0,
        );
        assert_eq!(geom.band_rects.len(), 1);
        let (x, w, _, _) = geom.band_rects[0];
        assert!((x - 0.0).abs() < 0.001);
        assert!((w - 600.0).abs() < 0.001);
    }

    #[test]
    fn empty_series_still_yields_an_axis_range_from_the_bands() {
        let geom = geometry(&[], &[(0, 100, 18.0, 22.0)], 0, 100, 600.0, 220.0);
        assert!(geom.y_min < 18.0);
        assert!(geom.y_max > 22.0);
    }

    #[test]
    fn value_range_is_padded_so_the_line_never_touches_the_frame() {
        let geom = geometry(&[(0, 18.0), (100, 18.0)], &[], 0, 100, 600.0, 220.0);
        assert!(geom.y_max > geom.y_min);
        for (_, y) in &geom.line_points {
            assert!(*y > 0.0 && *y < 220.0);
        }
    }
}
