//! The `ECharts` option-builder seam (Light Cycle spec, tickets 02–04):
//! pure functions from the grow history response to an `ECharts`
//! option JSON, unit-tested without a DOM. The wasm component passes
//! the result to `echarts.setOption` via `serde-wasm-bindgen`;
//! `ECharts` only draws — every axis, unit, colour, and band decision
//! lives here, and this seam serves every chart in the app.

use crate::api::{ChartSeriesData, GrowHistory};

/// The chart's colours, resolved from the app theme at runtime
/// (probe `text-primary` / `text-secondary` / `text-base-content` / a
/// 50 % faded variant) so both daisyUI themes render natively.
#[derive(Debug, Clone)]
pub struct ChartLook {
    /// The main series line, the band fill, and the pair input dots
    /// (`text-primary`).
    pub accent: String,
    /// The pair runoff dots' second colour (`text-secondary`), kept
    /// distinct from the input series so the two are tellable apart.
    pub compare: String,
    /// Axis labels and the axis name (`text-base-content`).
    pub text: String,
    /// Axis lines, ticks, and gridlines (a faded variant) — subtle,
    /// not competing with the data.
    pub muted: String,
}

/// How one role draws its series.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeriesKind {
    /// A plain line through the readings.
    Line,
    /// A staircase: each value holds until the next reading arrives
    /// (`step: "end"`). The binary light role.
    StepLine,
    /// Event dots: the input readings as dots plus the pair's runoff
    /// compare series as a second dot colour. The pH/EC pair roles.
    Scatter,
}

/// How one role draws: its value-axis unit label and series kind.
#[derive(Debug, Clone, Copy)]
pub struct ChartStyle {
    /// The value-axis name: the role's unit (`°C / °F`, `%`, `kPa`,
    /// `ppm`, `µS/cm`), the dimensionless scale (`pH`), or the state
    /// pair for binary roles.
    pub axis_name: &'static str,
    /// Which series shape the role draws with.
    pub kind: SeriesKind,
}

/// The drawing style for one history role. Unknown roles fall back to
/// a percent axis; every real role has its own arm.
#[must_use]
pub fn role_style(role: &str) -> ChartStyle {
    let axis_name = match role {
        "temperature" => "°C / °F",
        "vpd" => "kPa",
        "co2" => "ppm",
        "light" => "on / off",
        // The pH scale is dimensionless; EC is canonical in µS/cm
        // (spec §5.3; TDS stays derived and labelled elsewhere).
        "ph" => "pH",
        "ec" => "µS/cm",
        // humidity and soil_moisture (and anything new) read percent.
        _ => "%",
    };
    let kind = match role {
        "light" => SeriesKind::StepLine,
        "ph" | "ec" => SeriesKind::Scatter,
        _ => SeriesKind::Line,
    };
    ChartStyle { axis_name, kind }
}

/// The option for one history chart: a time axis spanning the whole
/// Chart window, a value axis labelled with the role's unit, gridlines,
/// the Target Band regions, and an axis-triggered hover tooltip. The
/// tick and tooltip *text* formatting is attached later, at the
/// component layer, as wasm closures over the card's role formatter.
#[must_use]
pub fn history_option(
    chart: &GrowHistory,
    look: &ChartLook,
    style: &ChartStyle,
) -> serde_json::Value {
    let (y_min, y_max) = padded_range(chart);
    // An empty window has no values to label: with the padded fallback
    // range the axis draws "2 1 0 0 −0"-style degenerate ticks. The
    // frame stays (the window toggles keep their meaning), but the
    // value axis goes quiet — the hint line above the chart explains
    // why.
    let bare_axis = window_is_empty(chart);
    // The x axis spans the whole Chart window (the old SVG geometry put
    // the window edges at the canvas edges), so sparse data cannot
    // zoom the axis in and fake density. An unparseable window bound
    // would be a backend contract break; the axis then auto-scales.
    let (x_min, x_max) = window_bounds(chart);
    let series = match style.kind {
        SeriesKind::Scatter => scatter_series(chart, look),
        SeriesKind::StepLine => vec![line_series(chart, look, true)],
        SeriesKind::Line => vec![line_series(chart, look, false)],
    };
    serde_json::json!({
        "animation": false,
        "grid": { "left": 8, "right": 16, "top": 28, "bottom": 8, "containLabel": true },
        "tooltip": { "trigger": "axis", "confine": true },
        "xAxis": {
            "type": "time",
            "min": x_min,
            "max": x_max,
            "axisLabel": { "hideOverlap": true, "color": look.text },
            "axisLine": { "lineStyle": { "color": look.muted } },
        },
        "yAxis": {
            "type": "value",
            "name": if bare_axis { "" } else { style.axis_name },
            "nameTextStyle": { "color": look.text, "align": "left" },
            "min": y_min,
            "max": y_max,
            "splitLine": { "show": true, "lineStyle": { "color": look.muted } },
            "axisLabel": { "show": !bare_axis, "hideOverlap": true, "color": look.text },
        },
        "series": series,
    })
}

/// Whether the loaded window carries nothing drawable: no points, no
/// pair series, no band segments. `history_option` quiets the value
/// axis for such a window, and the chart card shows a hint line so
/// the empty frame reads as data, not breakage.
#[must_use]
pub fn window_is_empty(chart: &GrowHistory) -> bool {
    chart.series.points.is_empty()
        && chart
            .compare
            .as_ref()
            .is_none_or(|compare| compare.points.is_empty())
        && chart.bands.is_empty()
}

/// The main series as a line: width-2 accent stroke, the Target Band
/// regions as `markArea`, and (stepped) the staircase semantics — each
/// value holds until the next reading's x, then jumps.
fn line_series(chart: &GrowHistory, look: &ChartLook, step: bool) -> serde_json::Value {
    let mut series = serde_json::json!({
        "type": "line",
        "showSymbol": false,
        "lineStyle": { "width": 2, "color": look.accent },
        "data": series_ms_data(chart),
        "markArea": band_mark_area(chart, &look.accent),
    });
    if step {
        // "end": from each reading the line holds that value until the
        // next reading's x, then steps — the old staircase semantics.
        series["step"] = serde_json::json!("end");
    }
    series
}

/// The pair role's two dot series: the input readings in the accent
/// colour and the runoff compare beside them in the distinct compare
/// colour (the old `fill-primary` / `fill-secondary` pair). Named, so
/// the hover tooltip tells the two apart.
fn scatter_series(chart: &GrowHistory, look: &ChartLook) -> Vec<serde_json::Value> {
    let compare_data = chart
        .compare
        .as_ref()
        .map_or_else(Vec::new, compare_ms_data);
    vec![
        serde_json::json!({
            "name": "Input",
            "type": "scatter",
            "symbolSize": 8,
            "itemStyle": { "color": look.accent },
            "data": series_ms_data(chart),
            "markArea": band_mark_area(chart, &look.accent),
        }),
        serde_json::json!({
            "name": "Runoff",
            "type": "scatter",
            "symbolSize": 8,
            "itemStyle": { "color": look.compare },
            "data": compare_data,
        }),
    ]
}

/// The series data as `[epoch ms, value]` pairs; points with
/// unparseable timestamps drop out (the same rule the merge applies).
fn series_ms_data(chart: &GrowHistory) -> Vec<(i64, f64)> {
    points_ms_data(&chart.series)
}

/// The compare series' data as `[epoch ms, value]` pairs.
fn compare_ms_data(series: &ChartSeriesData) -> Vec<(i64, f64)> {
    points_ms_data(series)
}

/// Maps one series' points to `[epoch ms, value]` pairs; points with
/// unparseable timestamps drop out.
fn points_ms_data(series: &ChartSeriesData) -> Vec<(i64, f64)> {
    series
        .points
        .iter()
        .filter_map(|point| {
            // Epoch ms (~1.7e12) travels through JSON.parse into an
            // f64 well below the 2^53 exact-integer bound.
            leaflog_core::chart::parse_epoch(&point.at).map(|at| (at * 1000, point.value))
        })
        .collect()
}

/// The axis bounds: the series (and pair compare) plus band range
/// padded 10 % — at least half a unit — so nothing touches the frame.
/// The old SVG geometry used the same padding, and the compare series
/// shares the scale (the runoff dots must never sit off-axis).
fn padded_range(chart: &GrowHistory) -> (f64, f64) {
    let mut values: Vec<f64> = chart
        .series
        .points
        .iter()
        .map(|point| point.value)
        .collect();
    if let Some(compare) = &chart.compare {
        values.extend(compare.points.iter().map(|point| point.value));
    }
    for band in &chart.bands {
        values.push(band.min);
        values.push(band.max);
    }
    let low = values.iter().copied().fold(f64::INFINITY, f64::min);
    let high = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let (low, high) = if low.is_finite() && high.is_finite() {
        (low, high)
    } else {
        (0.0, 1.0)
    };
    let padding = ((high - low) * 0.1).max(0.5);
    (low - padding, high + padding)
}

/// The Target Band regions as `ECharts` `markArea` pairs, clipped to the
/// window (the old SVG rects clipped the same way); segments fully
/// outside drop out. The tint rides on `opacity`, not an alpha colour,
/// so the runtime theme probe can supply any plain colour string.
fn band_mark_area(chart: &GrowHistory, accent: &str) -> serde_json::Value {
    // An unparseable window bound would be a backend contract break;
    // clip to nothing rather than drawing an unbounded region.
    let from = window_ms(&chart.series.from).unwrap_or(i64::MAX);
    let to = window_ms(&chart.series.to).unwrap_or(i64::MAX);
    let regions: Vec<[serde_json::Value; 2]> = chart
        .bands
        .iter()
        .filter_map(|band| {
            let start = window_ms(&band.start)?.clamp(from, to);
            let end = window_ms(&band.end)?.clamp(from, to);
            (start < end).then_some([
                serde_json::json!({ "xAxis": start, "yAxis": band.max }),
                serde_json::json!({ "xAxis": end, "yAxis": band.min }),
            ])
        })
        .collect();
    serde_json::json!({
        "silent": true,
        "itemStyle": { "opacity": 0.15, "color": accent },
        "data": regions,
    })
}

/// Parses a window/band timestamp to epoch ms; `None` on unparseable
/// input (the merge rule drops those, so the builder does too).
fn window_ms(at: &str) -> Option<i64> {
    leaflog_core::chart::parse_epoch(at).map(|seconds| seconds * 1000)
}

/// The x-axis bounds: the Chart window in epoch ms, so the axis always
/// spans the full window. `null` bounds let `ECharts` auto-scale when
/// the backend contract breaks (unparseable window edges).
fn window_bounds(chart: &GrowHistory) -> (serde_json::Value, serde_json::Value) {
    let bound =
        |at: &str| window_ms(at).map_or(serde_json::Value::Null, |ms| serde_json::json!(ms));
    (bound(&chart.series.from), bound(&chart.series.to))
}

#[cfg(test)]
mod tests {
    use super::{ChartLook, ChartStyle, SeriesKind, history_option, role_style, window_is_empty};
    use crate::api::{BandSegmentData, ChartPointData, ChartSeriesData, GrowHistory};

    /// Fixed test colours (the fallbacks the component probes for).
    fn look() -> ChartLook {
        ChartLook {
            accent: "#2e7d4f".to_owned(),
            compare: "#4c5f4e".to_owned(),
            text: "#1d2b1f".to_owned(),
            muted: "#1d2b1f80".to_owned(),
        }
    }

    /// The style one role renders with.
    fn style(role: &str) -> ChartStyle {
        role_style(role)
    }

    /// A loaded chart with two points, one band, and a 24-hour window.
    fn sample_history() -> GrowHistory {
        sample_for_role("temperature")
    }

    /// The same window with a different role echoed back.
    fn sample_for_role(role: &str) -> GrowHistory {
        GrowHistory {
            role: role.to_owned(),
            window: "day".to_owned(),
            series: ChartSeriesData {
                from: "2026-11-16T00:00:00+00:00".to_owned(),
                to: "2026-11-16T01:00:00+00:00".to_owned(),
                points: vec![
                    ChartPointData {
                        at: "2026-11-16T00:10:00+00:00".to_owned(),
                        value: 21.5,
                        source: "ha".to_owned(),
                    },
                    ChartPointData {
                        at: "2026-11-16T00:50:00+00:00".to_owned(),
                        value: 24.5,
                        source: "ha".to_owned(),
                    },
                ],
            },
            compare: None,
            bands: Vec::new(),
            degraded: false,
            bound: true,
        }
    }

    #[test]
    fn the_option_labels_axes_and_gridlines_and_arms_the_tooltip() {
        let option = history_option(&sample_history(), &look(), &style("temperature"));
        assert_eq!(option["xAxis"]["type"], "time");
        assert_eq!(option["xAxis"]["axisLabel"]["hideOverlap"], true);
        assert_eq!(option["yAxis"]["type"], "value");
        assert_eq!(option["yAxis"]["name"], "°C / °F");
        assert_eq!(option["yAxis"]["splitLine"]["show"], true);
        assert_eq!(option["tooltip"]["trigger"], "axis");
        assert_eq!(option["series"][0]["type"], "line");
    }

    #[test]
    fn the_x_axis_spans_the_whole_chart_window() {
        let option = history_option(&sample_history(), &look(), &style("temperature"));
        assert_eq!(
            option["xAxis"]["min"].as_i64(),
            Some(1_794_787_200_000),
            "window start anchors the time axis"
        );
        assert_eq!(
            option["xAxis"]["max"].as_i64(),
            Some(1_794_790_800_000),
            "window end anchors the time axis"
        );
    }

    #[test]
    fn an_unparseable_window_falls_back_to_auto_scaling() {
        let mut broken = sample_history();
        broken.series.from = "not-a-timestamp".to_owned();
        let option = history_option(&broken, &look(), &style("temperature"));
        assert!(
            option["xAxis"]["min"].is_null(),
            "broken window start auto-scales"
        );
        assert!(
            option["xAxis"]["max"].as_i64().is_some(),
            "the intact window end still pins the axis"
        );
    }

    #[test]
    fn the_value_range_keeps_headroom_above_the_peak() {
        let option = history_option(&sample_history(), &look(), &style("temperature"));
        let y_max = option["yAxis"]["max"].as_f64().expect("explicit max");
        assert!(y_max > 24.5, "axis must clear the peak reading: {y_max}");
        let y_min = option["yAxis"]["min"].as_f64().expect("explicit min");
        assert!(y_min < 21.5, "axis must sit under the low reading: {y_min}");
    }

    #[test]
    fn a_flat_series_still_gets_padded_bounds() {
        let mut flat = sample_history();
        for point in &mut flat.series.points {
            point.value = 21.0;
        }
        let option = history_option(&flat, &look(), &style("temperature"));
        let y_max = option["yAxis"]["max"].as_f64().expect("explicit max");
        let y_min = option["yAxis"]["min"].as_f64().expect("explicit min");
        assert!(
            y_max > 21.0 && y_min < 21.0,
            "flat line needs room: {y_min}..{y_max}"
        );
    }

    #[test]
    fn an_empty_window_quiets_the_value_axis_until_something_is_drawable() {
        let mut empty = sample_history();
        empty.series.points = Vec::new();
        assert!(window_is_empty(&empty));
        let option = history_option(&empty, &look(), &style("ec"));
        assert_eq!(
            option["yAxis"]["name"], "",
            "no unit label on an empty window"
        );
        assert_eq!(
            option["yAxis"]["axisLabel"]["show"], false,
            "the degenerate tick labels hide on an empty window"
        );
        // Any drawable content — even bands alone — restores the axis.
        empty.bands = vec![BandSegmentData {
            start: "2026-11-16T00:00:00+00:00".to_owned(),
            end: "2026-11-16T01:00:00+00:00".to_owned(),
            min: 20.0,
            max: 26.0,
            phase: "vegetative".to_owned(),
            light_on: None,
        }];
        assert!(!window_is_empty(&empty));
        let option = history_option(&empty, &look(), &style("ec"));
        assert_eq!(option["yAxis"]["name"], "µS/cm");
        assert_eq!(option["yAxis"]["axisLabel"]["show"], true);
    }

    #[test]
    fn band_segments_render_as_mark_area_regions() {
        let mut banded = sample_history();
        banded.bands = vec![BandSegmentData {
            start: "2026-11-16T00:00:00+00:00".to_owned(),
            end: "2026-11-16T01:00:00+00:00".to_owned(),
            min: 20.0,
            max: 26.0,
            phase: "vegetative".to_owned(),
            light_on: None,
        }];
        let option = history_option(&banded, &look(), &style("temperature"));
        let area = &option["series"][0]["markArea"];
        let region = &area["data"][0];
        assert_eq!(
            region[0]["xAxis"].as_i64(),
            Some(1_794_787_200_000),
            "band start, epoch ms"
        );
        assert_eq!(region[0]["yAxis"].as_f64(), Some(26.0), "band top");
        assert_eq!(
            region[1]["xAxis"].as_i64(),
            Some(1_794_790_800_000),
            "band end, epoch ms"
        );
        assert_eq!(region[1]["yAxis"].as_f64(), Some(20.0), "band bottom");
        assert_eq!(
            area["itemStyle"]["opacity"].as_f64(),
            Some(0.15),
            "band tint"
        );
    }

    #[test]
    fn band_segments_outside_the_window_drop_out() {
        let mut banded = sample_history();
        banded.bands = vec![
            // Fully before the window start.
            BandSegmentData {
                start: "2026-11-15T00:00:00+00:00".to_owned(),
                end: "2026-11-15T12:00:00+00:00".to_owned(),
                min: 20.0,
                max: 26.0,
                phase: "vegetative".to_owned(),
                light_on: None,
            },
            // Half in: must clip to the window edge.
            BandSegmentData {
                start: "2026-11-15T12:30:00+00:00".to_owned(),
                end: "2026-11-16T00:20:00+00:00".to_owned(),
                min: 20.0,
                max: 26.0,
                phase: "vegetative".to_owned(),
                light_on: None,
            },
        ];
        let option = history_option(&banded, &look(), &style("temperature"));
        let regions = option["series"][0]["markArea"]["data"]
            .as_array()
            .expect("mark area data");
        assert_eq!(regions.len(), 1, "out-of-window segment dropped");
        assert_eq!(
            regions[0][0]["xAxis"].as_i64(),
            Some(1_794_787_200_000),
            "clipped to the window start"
        );
    }

    #[test]
    fn an_empty_series_still_frames_the_bands() {
        let mut banded = sample_history();
        banded.series.points = Vec::new();
        banded.bands = vec![BandSegmentData {
            start: "2026-11-16T00:00:00+00:00".to_owned(),
            end: "2026-11-16T01:00:00+00:00".to_owned(),
            min: 20.0,
            max: 26.0,
            phase: "vegetative".to_owned(),
            light_on: None,
        }];
        let option = history_option(&banded, &look(), &style("temperature"));
        let y_max = option["yAxis"]["max"].as_f64().expect("explicit max");
        let y_min = option["yAxis"]["min"].as_f64().expect("explicit min");
        assert!(
            y_min < 20.0 && y_max > 26.0,
            "band frames the axis: {y_min}..{y_max}"
        );
        assert_eq!(
            option["series"][0]["data"].as_array().map(Vec::len),
            Some(0),
            "no phantom points"
        );
    }

    #[test]
    fn a_window_without_data_or_bands_still_gets_a_sane_axis() {
        let mut empty = sample_history();
        empty.series.points = Vec::new();
        let option = history_option(&empty, &look(), &style("temperature"));
        let y_max = option["yAxis"]["max"].as_f64().expect("explicit max");
        let y_min = option["yAxis"]["min"].as_f64().expect("explicit min");
        assert!(y_min < y_max, "axis stays sane: {y_min}..{y_max}");
        let regions = option["series"][0]["markArea"]["data"]
            .as_array()
            .expect("mark area data");
        assert!(regions.is_empty(), "no bands, no regions");
    }

    #[test]
    fn the_theme_look_colours_line_band_and_axis_chrome() {
        let mut banded = sample_history();
        banded.bands = vec![BandSegmentData {
            start: "2026-11-16T00:00:00+00:00".to_owned(),
            end: "2026-11-16T01:00:00+00:00".to_owned(),
            min: 20.0,
            max: 26.0,
            phase: "vegetative".to_owned(),
            light_on: None,
        }];
        let option = history_option(&banded, &look(), &style("temperature"));
        assert_eq!(option["series"][0]["lineStyle"]["color"], "#2e7d4f");
        assert_eq!(
            option["series"][0]["markArea"]["itemStyle"]["color"],
            "#2e7d4f"
        );
        assert_eq!(option["yAxis"]["axisLabel"]["color"], "#1d2b1f");
        assert_eq!(
            option["yAxis"]["splitLine"]["lineStyle"]["color"],
            "#1d2b1f80"
        );
        assert_eq!(option["xAxis"]["axisLabel"]["color"], "#1d2b1f");
    }

    /// A light-history sample: one off→on→off staircase across the
    /// window (binary role, values 0/1).
    fn light_history() -> GrowHistory {
        let mut chart = sample_for_role("light");
        chart.series.points = vec![
            ChartPointData {
                at: "2026-11-16T00:10:00+00:00".to_owned(),
                value: 0.0,
                source: "ha".to_owned(),
            },
            ChartPointData {
                at: "2026-11-16T00:30:00+00:00".to_owned(),
                value: 1.0,
                source: "ha".to_owned(),
            },
            ChartPointData {
                at: "2026-11-16T00:50:00+00:00".to_owned(),
                value: 0.0,
                source: "ha".to_owned(),
            },
        ];
        chart
    }

    #[test]
    fn each_role_labels_its_value_axis_with_its_unit() {
        assert_eq!(style("temperature").axis_name, "°C / °F");
        assert_eq!(style("humidity").axis_name, "%");
        assert_eq!(style("vpd").axis_name, "kPa");
        assert_eq!(style("co2").axis_name, "ppm");
        assert_eq!(style("soil_moisture").axis_name, "%");
        assert_eq!(style("light").axis_name, "on / off");
    }

    #[test]
    fn the_axis_name_reaches_the_option() {
        let option = history_option(&sample_for_role("vpd"), &look(), &style("vpd"));
        assert_eq!(option["yAxis"]["name"], "kPa");
    }

    #[test]
    fn each_role_maps_to_its_drawing_kind() {
        assert_eq!(style("temperature").kind, SeriesKind::Line);
        assert_eq!(style("humidity").kind, SeriesKind::Line);
        assert_eq!(style("vpd").kind, SeriesKind::Line);
        assert_eq!(style("co2").kind, SeriesKind::Line);
        assert_eq!(style("soil_moisture").kind, SeriesKind::Line);
        assert_eq!(style("light").kind, SeriesKind::StepLine);
        assert_eq!(style("ph").kind, SeriesKind::Scatter);
        assert_eq!(style("ec").kind, SeriesKind::Scatter);
    }

    /// A pH pair sample: one input reading and one runoff reading in
    /// the same window (the feed-entry detail fields).
    fn pair_history() -> GrowHistory {
        let mut chart = sample_for_role("ph");
        chart.series.points = vec![ChartPointData {
            at: "2026-11-16T00:10:00+00:00".to_owned(),
            value: 6.2,
            source: "ha".to_owned(),
        }];
        chart.compare = Some(ChartSeriesData {
            from: "2026-11-16T00:00:00+00:00".to_owned(),
            to: "2026-11-16T01:00:00+00:00".to_owned(),
            points: vec![ChartPointData {
                at: "2026-11-16T00:10:00+00:00".to_owned(),
                value: 6.8,
                source: "ha".to_owned(),
            }],
        });
        chart
    }

    #[test]
    fn the_pair_roles_draw_two_named_scatter_series_in_distinct_colours() {
        for role in ["ph", "ec"] {
            let option = history_option(&pair_history(), &look(), &style(role));
            let input = &option["series"][0];
            let runoff = &option["series"][1];
            assert_eq!(input["type"], "scatter", "{role} input series is dots");
            assert_eq!(input["name"], "Input");
            assert_eq!(runoff["type"], "scatter", "{role} runoff series is dots");
            assert_eq!(runoff["name"], "Runoff");
            assert_eq!(
                input["itemStyle"]["color"], "#2e7d4f",
                "{role} input dots carry the accent"
            );
            assert_eq!(
                runoff["itemStyle"]["color"], "#4c5f4e",
                "{role} runoff dots carry the distinct compare colour"
            );
        }
    }

    #[test]
    fn the_pair_scatter_data_are_epoch_ms_value_pairs() {
        let option = history_option(&pair_history(), &look(), &style("ph"));
        let input = &option["series"][0]["data"][0];
        assert_eq!(
            input[0].as_f64(),
            Some(1_794_787_800_000.0),
            "input reading, epoch ms"
        );
        assert_eq!(input[1].as_f64(), Some(6.2), "the input pH");
        let runoff = &option["series"][1]["data"][0];
        assert_eq!(runoff[0].as_f64(), Some(1_794_787_800_000.0));
        assert_eq!(runoff[1].as_f64(), Some(6.8), "the runoff pH");
    }

    #[test]
    fn runoff_values_join_the_value_axis_range() {
        let option = history_option(&pair_history(), &look(), &style("ph"));
        let y_max = option["yAxis"]["max"].as_f64().expect("explicit max");
        let y_min = option["yAxis"]["min"].as_f64().expect("explicit min");
        assert!(
            y_max > 6.8,
            "the axis must clear the runoff peak, not just the input: {y_max}"
        );
        assert!(
            y_min < 6.2,
            "the axis sits under the input reading: {y_min}"
        );
    }

    #[test]
    fn a_pair_without_runoff_still_draws_the_input_series() {
        let mut alone = pair_history();
        alone.compare = None;
        let option = history_option(&alone, &look(), &style("ph"));
        let runoff = option["series"][1]["data"].as_array().expect("runoff data");
        assert!(runoff.is_empty(), "no runoff, no runoff dots");
        let input = option["series"][0]["data"].as_array().expect("input data");
        assert_eq!(input.len(), 1, "the input dots survive alone");
    }

    #[test]
    fn each_pair_role_labels_its_axis_with_its_unit() {
        assert_eq!(style("ph").axis_name, "pH", "pH is dimensionless");
        assert_eq!(style("ec").axis_name, "µS/cm", "EC is canonical µS/cm");
        let option = history_option(&pair_history(), &look(), &style("ec"));
        assert_eq!(option["yAxis"]["name"], "µS/cm");
    }

    #[test]
    fn the_staircase_holds_each_value_until_the_next_reading() {
        let option = history_option(&light_history(), &look(), &style("light"));
        // `step: "end"`: from each reading the line runs horizontally
        // at its value until the next reading's x, then jumps — the
        // old `step_points` semantics, so an off at 00:10 draws on/1.0
        // only from 00:30.
        assert_eq!(option["series"][0]["step"], "end");
    }

    #[test]
    fn a_continuous_role_draws_a_plain_line_without_a_step_marker() {
        let option = history_option(&sample_history(), &look(), &style("humidity"));
        assert!(
            option["series"][0]["step"].is_null(),
            "no step key on plain lines"
        );
        assert_eq!(option["series"][0]["showSymbol"], false);
        assert_eq!(option["series"][0]["type"], "line");
    }

    #[test]
    fn the_series_data_are_epoch_ms_value_pairs() {
        let option = history_option(&light_history(), &look(), &style("light"));
        let data = option["series"][0]["data"].as_array().expect("series data");
        assert_eq!(data.len(), 3);
        let first = &data[0];
        assert_eq!(
            first[0].as_f64(),
            Some(1_794_787_800_000.0),
            "window epoch seconds to ms"
        );
        assert_eq!(first[1].as_f64(), Some(0.0), "the off reading");
    }

    #[test]
    fn a_co2_style_chart_without_bands_renders_a_clean_frame() {
        // CO₂ has no band source today: empty bands must leave no
        // markArea regions and still yield a sane axis.
        let mut co2 = sample_for_role("co2");
        co2.bands = Vec::new();
        let option = history_option(&co2, &look(), &style("co2"));
        assert_eq!(option["yAxis"]["name"], "ppm");
        let regions = option["series"][0]["markArea"]["data"]
            .as_array()
            .expect("mark area data");
        assert!(regions.is_empty(), "no bands, no regions");
        let y_max = option["yAxis"]["max"].as_f64().expect("explicit max");
        let y_min = option["yAxis"]["min"].as_f64().expect("explicit min");
        assert!(y_min < y_max, "axis stays sane: {y_min}..{y_max}");
    }

    #[test]
    fn the_soil_band_frames_its_corridor_on_a_percent_axis() {
        let mut soil = sample_for_role("soil_moisture");
        soil.bands = vec![BandSegmentData {
            start: "2026-11-16T00:00:00+00:00".to_owned(),
            end: "2026-11-16T01:00:00+00:00".to_owned(),
            min: 25.0,
            max: 45.0,
            phase: "vegetative".to_owned(),
            light_on: None,
        }];
        let option = history_option(&soil, &look(), &style("soil_moisture"));
        assert_eq!(option["yAxis"]["name"], "%");
        let region = &option["series"][0]["markArea"]["data"][0];
        assert_eq!(region[0]["yAxis"].as_f64(), Some(45.0), "band top");
        assert_eq!(region[1]["yAxis"].as_f64(), Some(25.0), "band bottom");
    }

    // --- Stepped day/night bands (Light Cycle spec, ticket 07) ---

    /// A day segment and a night segment with the germ-phase corridors
    /// the backend emits: different ranges, stepping at the boundary.
    fn stepped_bands() -> Vec<BandSegmentData> {
        vec![
            BandSegmentData {
                start: "2026-11-16T00:00:00+00:00".to_owned(),
                end: "2026-11-16T00:30:00+00:00".to_owned(),
                min: 20.0,
                max: 23.3,
                phase: "germination".to_owned(),
                light_on: Some(false),
            },
            BandSegmentData {
                start: "2026-11-16T00:30:00+00:00".to_owned(),
                end: "2026-11-16T01:00:00+00:00".to_owned(),
                min: 21.1,
                max: 26.7,
                phase: "germination".to_owned(),
                light_on: Some(true),
            },
        ]
    }

    #[test]
    fn stepped_bands_render_one_region_per_segment_with_its_own_range() {
        let mut stepped = sample_history();
        stepped.bands = stepped_bands();
        let option = history_option(&stepped, &look(), &style("temperature"));
        let regions = option["series"][0]["markArea"]["data"]
            .as_array()
            .expect("mark area data");
        assert_eq!(regions.len(), 2, "one region per stepped segment");
        // Night first: its ceiling sits below the day's.
        assert_eq!(regions[0][0]["yAxis"].as_f64(), Some(23.3));
        assert_eq!(regions[0][1]["yAxis"].as_f64(), Some(20.0));
        assert_eq!(
            regions[0][0]["xAxis"].as_i64(),
            Some(1_794_787_200_000),
            "night region starts at the window"
        );
        assert_eq!(regions[1][0]["yAxis"].as_f64(), Some(26.7));
        assert_eq!(regions[1][1]["yAxis"].as_f64(), Some(21.1));
        assert_eq!(
            regions[1][0]["xAxis"].as_i64(),
            Some(1_794_789_000_000),
            "day region starts at the step"
        );
        assert_eq!(
            regions[1][1]["xAxis"].as_i64(),
            Some(1_794_790_800_000),
            "day region ends at the window end"
        );
    }

    #[test]
    #[allow(clippy::float_cmp)] // literal band edges compared for inequality, on purpose
    fn many_varying_segments_all_reach_the_renderer() {
        // A month window makes the backend emit dozens of alternating
        // segments; none may collapse into another or drop out.
        let mut stepped = sample_for_role("temperature");
        stepped.series.from = "2026-11-16T00:00:00+00:00".to_owned();
        stepped.series.to = "2026-11-20T00:00:00+00:00".to_owned();
        // Four days of 12 h day/night alternation: 8 contiguous
        // segments of varying heights.
        stepped.bands = (0..8)
            .map(|step| BandSegmentData {
                start: format!(
                    "2026-11-{}T{:02}:00:00+00:00",
                    16 + step / 2,
                    (step % 2) * 12
                ),
                end: format!(
                    "2026-11-{}T{:02}:00:00+00:00",
                    16 + (step + 1) / 2,
                    ((step + 1) % 2) * 12
                ),
                min: if step % 2 == 0 { 20.0 } else { 21.1 },
                max: if step % 2 == 0 { 23.3 } else { 26.7 },
                phase: "germination".to_owned(),
                light_on: Some(step % 2 == 1),
            })
            .collect();
        let option = history_option(&stepped, &look(), &style("temperature"));
        let regions = option["series"][0]["markArea"]["data"]
            .as_array()
            .expect("mark area data");
        assert_eq!(regions.len(), 8, "every segment draws as its own region");
        let ceilings: Vec<_> = regions
            .iter()
            .map(|region| region[0]["yAxis"].as_f64().unwrap())
            .collect();
        assert!(
            ceilings.windows(2).any(|pair| pair[0] != pair[1]),
            "the region heights vary between day and night"
        );
    }

    #[test]
    fn the_co2_chart_frames_its_day_and_night_ambient_bands() {
        let mut co2 = sample_for_role("co2");
        co2.bands = stepped_bands()
            .into_iter()
            .map(|mut band| {
                band.min = if band.light_on == Some(true) {
                    800.0
                } else {
                    400.0
                };
                band.max = if band.light_on == Some(true) {
                    1500.0
                } else {
                    450.0
                };
                band
            })
            .collect();
        let option = history_option(&co2, &look(), &style("co2"));
        assert_eq!(option["yAxis"]["name"], "ppm");
        let regions = option["series"][0]["markArea"]["data"]
            .as_array()
            .expect("mark area data");
        assert_eq!(regions.len(), 2);
        assert_eq!(
            regions[0][0]["yAxis"].as_f64(),
            Some(450.0),
            "night ambient top"
        );
        assert_eq!(
            regions[0][1]["yAxis"].as_f64(),
            Some(400.0),
            "night ambient floor"
        );
        assert_eq!(
            regions[1][0]["yAxis"].as_f64(),
            Some(1500.0),
            "day enrichment top"
        );
        let y_max = option["yAxis"]["max"].as_f64().expect("explicit max");
        let y_min = option["yAxis"]["min"].as_f64().expect("explicit min");
        assert!(
            y_max > 1500.0 && y_min < 400.0,
            "axis frames both bands: {y_min}..{y_max}"
        );
    }
}
