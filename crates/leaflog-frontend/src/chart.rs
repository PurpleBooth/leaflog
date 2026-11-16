//! Hand-rolled SVG history chart (spec §3): a line over the shaded
//! recommended-band corridor, fed by the grow history endpoint, with
//! rolling Day / Week / Month windows (24 h / 7 d / 30 d). No chart
//! library — the SVG is composed from Rust views.

use crate::api;
use crate::api::{GrowHistory, SoilBand};
use leaflog_core::chart::parse_epoch;
use leptos::prelude::*;
use leptos::task::spawn_local;

/// Axis formatter: relative humidity percent.
#[must_use]
pub fn format_pct(value: f64) -> String {
    format!("{value:.0} %")
}

/// The soil-moisture band editor: two percent inputs with save and
/// reset-to-default, stored under the `soil_moisture_band` settings
/// key. The chart card reads the same override for its corridor.
#[component]
pub fn BandEditor() -> impl IntoView {
    let min = RwSignal::<String>::new(String::new());
    let max = RwSignal::<String>::new(String::new());
    let stored = RwSignal::<bool>::new(false);
    let status = RwSignal::<Option<String>>::new(None);

    Effect::new(move |_| {
        spawn_local(async move {
            match api::get::<serde_json::Value>("api/settings").await {
                Ok(settings) => {
                    let parsed = settings
                        .get("soil_moisture_band")
                        .and_then(|value| serde_json::from_value::<SoilBand>(value.clone()).ok());
                    stored.set(parsed.is_some());
                    if let Some(band) = parsed {
                        min.set(format!("{}", band.min));
                        max.set(format!("{}", band.max));
                    }
                }
                Err(failure) => status.set(Some(failure.message)),
            }
        });
    });

    let save = move |reset: bool| {
        let payload = if reset {
            serde_json::json!({ "soil_moisture_band": null })
        } else {
            match (min.get().parse::<f64>().ok(), max.get().parse::<f64>().ok()) {
                (Some(min_value), Some(max_value))
                    if (0.0..=100.0).contains(&min_value)
                        && min_value <= max_value
                        && max_value <= 100.0 =>
                {
                    serde_json::json!({
                        "soil_moisture_band": { "min": min_value, "max": max_value }
                    })
                }
                _ => {
                    status.set(Some("Enter 0–100 with min ≤ max.".to_owned()));
                    return;
                }
            }
        };
        spawn_local(async move {
            match api::put::<serde_json::Value, serde_json::Value>("api/settings", &payload).await {
                Ok(_) => {
                    stored.set(!reset);
                    status.set(Some(if reset {
                        "Reset to default.".to_owned()
                    } else {
                        "Saved.".to_owned()
                    }));
                }
                Err(failure) => status.set(Some(failure.message)),
            }
        });
    };

    view! {
        <div class="flex flex-wrap items-end gap-2 my-1">
            <label class="fieldset">
                <span class="fieldset-label">"Band min %"</span>
                <input class="input input-sm w-24" type="number" min="0" max="100"
                    prop:value=move || min.get()
                    on:input=move |ev| min.set(event_target_value(&ev))
                />
            </label>
            <label class="fieldset">
                <span class="fieldset-label">"Band max %"</span>
                <input class="input input-sm w-24" type="number" min="0" max="100"
                    prop:value=move || max.get()
                    on:input=move |ev| max.set(event_target_value(&ev))
                />
            </label>
            <button class="btn btn-sm self-end" on:click=move |_| save(false)>
                "Apply band"
            </button>
            <Show when=move || stored.get() fallback=|| ()>
                <button class="btn btn-sm btn-ghost self-end" on:click=move |_| save(true)>
                    "Reset to default"
                </button>
            </Show>
            {move || {
                status.get().map(|message| {
                    view! { <span class="text-base-content/60 text-sm">{message}</span> }
                })
            }}
        </div>
    }
}

/// Axis formatter: VPD kilopascals.
#[must_use]
pub fn format_kpa(value: f64) -> String {
    format!("{value:.2} kPa")
}

/// Axis formatter: CO₂ parts per million.
#[must_use]
pub fn format_ppm(value: f64) -> String {
    format!("{value:.0} ppm")
}

/// Axis formatter: a binary entity's on/off state.
#[must_use]
pub fn format_on_off(value: f64) -> String {
    if value >= 0.5 {
        "on".to_owned()
    } else {
        "off".to_owned()
    }
}

/// Axis formatter: pH values (dimensionless).
#[must_use]
pub fn format_ph(value: f64) -> String {
    format!("{value:.1}")
}

/// Axis formatter: EC in µS/cm (canonical, spec §5.3).
#[must_use]
pub fn format_ec_us(value: f64) -> String {
    format!("{value:.0} µS/cm")
}

/// The SVG canvas width (the viewBox is fixed; the element scales).
const WIDTH: f64 = 600.0;
/// The SVG canvas height.
const HEIGHT: f64 = 220.0;

/// One windowed chart for one sensor role on the grow overview.
/// `format_value` renders axis labels (per the HA unit system);
/// `step` draws the series as a staircase (binary entities); `scatter`
/// plots points instead of a line (event series such as pH/EC).
#[component]
pub fn HistoryChart(
    grow_id: String,
    #[prop(default = "temperature")] role: &'static str,
    #[prop(default = "Temperature")] title: &'static str,
    format_value: fn(f64) -> String,
    #[prop(default = false)] step: bool,
    #[prop(default = false)] scatter: bool,
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
                        return Some(
                            view! { <ChartSvg chart=current format_value step scatter /> }
                                .into_any(),
                        );
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

/// The line-and-corridor SVG for one loaded chart. `step` renders the
/// series as a staircase (binary entities such as a lamp's on/off);
/// `scatter` plots the points as dots instead (event series such as
/// pH/EC), overlaying the pair's compare series in a second colour.
#[component]
fn ChartSvg(
    chart: GrowHistory,
    format_value: fn(f64) -> String,
    step: bool,
    scatter: bool,
) -> impl IntoView {
    let from = parse_epoch(&chart.series.from).unwrap_or(0);
    let to = parse_epoch(&chart.series.to).unwrap_or(from + 1);
    let points: Vec<(i64, f64)> = chart
        .series
        .points
        .iter()
        .filter_map(|point| parse_epoch(&point.at).map(|at| (at, point.value)))
        .collect();
    let compare: Vec<(i64, f64)> = chart
        .compare
        .as_ref()
        .map(|series| {
            series
                .points
                .iter()
                .filter_map(|point| parse_epoch(&point.at).map(|at| (at, point.value)))
                .collect()
        })
        .unwrap_or_default();
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
    // The compare series shares the scale, so its values join the
    // range the axis is computed from.
    let mut range_points = points.clone();
    range_points.extend(compare.iter().copied());
    let geom = geometry(&range_points, &bands, from, to, WIDTH, HEIGHT);
    let line_points = if step {
        step_points(&geom.line_points)
    } else {
        geom.line_points.clone()
    };
    let line = line_points
        .iter()
        .map(|(x, y)| format!("{x:.1},{y:.1}"))
        .collect::<Vec<_>>()
        .join(" ");
    let y_max_label = format_value(geom.y_max);
    let y_min_label = format_value(geom.y_min);
    let label = format!("{} history", chart.role);
    let y_min_y = format!("{:.1}", HEIGHT - 6.0);

    // Scatter dots map their epochs through the same scale the line
    // uses. Epoch seconds stay far below f64's 52-bit mantissa.
    #[allow(clippy::cast_precision_loss)]
    let x_of = |at: i64| WIDTH * (at - from) as f64 / (to - from).max(1) as f64;
    #[allow(clippy::cast_precision_loss)]
    let y_of =
        |value: f64| HEIGHT * (1.0 - (value - geom.y_min) / (geom.y_max - geom.y_min).max(0.001));
    let dot = |(at, value): (i64, f64), class: &'static str| {
        view! {
            <circle
                cx=format!("{:.1}", x_of(at))
                cy=format!("{:.1}", y_of(value))
                r="3.5"
                class=class
            />
        }
    };
    let main_dots = if scatter {
        points
            .iter()
            .map(|&point| dot(point, "fill-primary"))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let compare_dots = if scatter {
        compare
            .iter()
            .map(|&point| dot(point, "fill-secondary"))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    view! {
        <svg viewBox=format!("0 0 {WIDTH} {HEIGHT}") class="w-full" role="img" aria-label=label>
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
            {(!scatter).then(|| {
                view! { <polyline points=line class="fill-none stroke-primary" stroke-width="2" /> }
            })}
            {main_dots}
            {compare_dots}
            <text x="4" y="14" class="fill-base-content/60">{y_max_label}</text>
            <text x="4" y=y_min_y class="fill-base-content/60">{y_min_label}</text>
        </svg>
    }
}

/// Turns a line into a staircase: every horizontal run holds until the
/// next point's x, then steps vertically. Empty input stays empty.
#[must_use]
pub fn step_points(points: &[(f64, f64)]) -> Vec<(f64, f64)> {
    let mut stepped = Vec::with_capacity(points.len().saturating_mul(2));
    for &(x, y) in points {
        // Exact comparison on purpose: `last_y` is the same copied
        // value, not a computed float.
        #[allow(clippy::float_cmp)]
        match stepped.last().copied() {
            Some((_, last_y)) if last_y != y => {
                stepped.push((x, last_y));
                stepped.push((x, y));
            }
            Some(_) => {}
            None => stepped.push((x, y)),
        }
    }
    stepped
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

    use super::step_points;

    #[test]
    fn step_points_hold_each_value_until_the_next() {
        let stepped = step_points(&[(0.0, 1.0), (10.0, 0.0), (20.0, 1.0)]);
        assert_eq!(
            stepped,
            [
                (0.0, 1.0),
                (10.0, 1.0), // hold on, then step down at x = 10
                (10.0, 0.0),
                (20.0, 0.0), // hold off, then step up at x = 20
                (20.0, 1.0),
            ]
        );
    }

    #[test]
    fn step_points_pass_through_empty_and_single() {
        assert_eq!(step_points(&[]), Vec::new());
        let single = step_points(&[(5.0, 1.0)]);
        assert_eq!(single, vec![(5.0, 1.0)]);
    }
}
