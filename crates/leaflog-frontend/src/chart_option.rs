//! The `ECharts` option-builder seam (Light Cycle spec, tickets 02–04):
//! pure functions from the grow history response to an `ECharts`
//! option JSON, unit-tested without a DOM. The wasm component passes
//! the result to `echarts.setOption` via `serde-wasm-bindgen`;
//! `ECharts` only draws — every axis, unit, colour, and band decision
//! lives here, and this seam serves every chart in the app.

use crate::api::{ChartSeriesData, GrowHistory};
use crate::i18n::Locale;
use crate::labels::action_label_for_kind;
use leptos_i18n::td_string;

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

/// The fixed value-axis label width every role's chart shares. The
/// section's charts join one `ECharts` connect group, and a connected
/// hover is broadcast in *pixels* — so every chart must draw the
/// same plot rectangle for the same pixels to land on the same
/// timestamp chart-to-chart. The grid therefore reserves a fixed
/// left gutter of this width plus the axis margin, instead of
/// `containLabel`, which measures each role's own label text and
/// drifts the plot origins apart. The width fits the widest role
/// label: the temperature card's dual-unit "18.0 °C / 64.4 °F"
/// measures ~89 px at the 12 px label font (negatives run ~93 px).
const CONNECT_LABEL_WIDTH: f64 = 96.0;

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
    locale: Locale,
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
        SeriesKind::Scatter => scatter_series(chart, look, locale),
        SeriesKind::StepLine => vec![line_series(chart, look, true)],
        SeriesKind::Line => vec![line_series(chart, look, false)],
    };
    // The tick series appends last, so the series indices ahead of it
    // stay stable (the pair charts keep Input/Runoff at 0/1). The
    // night shade appends after the ticks — array order is contract
    // only, the z level below is what puts it behind everything.
    let mut series = series;
    if !chart.markers.is_empty() {
        series.push(marker_series(chart, look, y_min, y_max, locale));
    }
    if !chart.nights.is_empty() {
        series.push(night_series(chart, look));
    }
    serde_json::json!({
        "animation": false,
        "grid": {
            // Pixel-broadcast geometry: a fixed left gutter (label
            // width plus the axis margin) puts every role's plot
            // origin at the same x. `containLabel` would measure each
            // role's own label text and drift the origins apart, so
            // it stays off and the insets are explicit and uniform.
            "left": CONNECT_LABEL_WIDTH + 14.0,
            "right": 16.0,
            "top": 36.0,
            "bottom": 24.0,
        },
        "tooltip": {
            "trigger": "axis",
            "confine": true,
            // The line crosshair is the connect group's shared
            // pointer: the section's charts all draw it at the same
            // timestamp.
            "axisPointer": { "type": "line" },
        },
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
            "axisLabel": {
                "show": !bare_axis,
                "hideOverlap": true,
                "color": look.text,
                // Labels sit inside the fixed grid gutter; the box
                // width (with truncation overflow) keeps a wider
                // role label from spilling over the plot area.
                "width": CONNECT_LABEL_WIDTH,
                "overflow": "truncate",
            },
        },
        "series": series,
    })
}

/// Whether the loaded window carries nothing drawable: no points, no
/// pair series, no band segments. `history_option` quiets the value
/// axis for such a window, and the chart card shows a hint line so
/// the empty frame reads as data, not breakage. The night shading is
/// context, not data — it deliberately does not count here (the hint
/// "no readings" stays true, and a degenerate value range never
/// widens for a tint that spans the full height anyway).
#[must_use]
pub fn window_is_empty(chart: &GrowHistory) -> bool {
    chart.series.points.is_empty()
        && chart
            .compare
            .as_ref()
            .is_none_or(|compare| compare.points.is_empty())
        && chart.bands.is_empty()
}

/// Whether the loaded window holds no readings on either series —
/// the main series or its pair (ticket 07). This is the card hint's
/// question ("no readings in this window"), and the Target Bands
/// never answer it: they ride every temperature, humidity, VPD and
/// soil window regardless of data, so [`window_is_empty`] — the
/// quiet-axis question, "nothing drawable" — is a stricter test
/// the hint cannot key on. A banded window with zero readings keeps
/// its labelled axis and still says the quiet is data, not breakage.
#[must_use]
pub fn series_is_empty(chart: &GrowHistory) -> bool {
    chart.series.points.is_empty()
        && chart
            .compare
            .as_ref()
            .is_none_or(|compare| compare.points.is_empty())
}

/// The main series as a line: width-2 accent stroke, the Target Band
/// regions as `markArea`, and (stepped) the staircase semantics — each
/// value holds until the next reading's x, then jumps.
fn line_series(chart: &GrowHistory, look: &ChartLook, step: bool) -> serde_json::Value {
    let mut series = serde_json::json!({
        "type": "line",
        "name": MAIN_SERIES_NAME,
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
/// the hover tooltip tells the two apart — the names come from the
/// catalog in the active locale (`env.input_series` /
/// `env.runoff_series`).
fn scatter_series(chart: &GrowHistory, look: &ChartLook, locale: Locale) -> Vec<serde_json::Value> {
    let compare_data = chart
        .compare
        .as_ref()
        .map_or_else(Vec::new, compare_ms_data);
    vec![
        serde_json::json!({
            "name": td_string!(locale, env.input_series),
            "type": "scatter",
            "symbolSize": 8,
            "itemStyle": { "color": look.accent },
            "data": series_ms_data(chart),
            "markArea": band_mark_area(chart, &look.accent),
        }),
        serde_json::json!({
            "name": td_string!(locale, env.runoff_series),
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

/// The series name every single-line history chart travels under: the
/// component's tooltip renders the main reading plain (the card title
/// already names the role), and only an intentionally chosen name
/// lets it tell that series apart from the pair charts' named
/// series — `ECharts` fills synthetic `seriesN` names for unnamed
/// series, which used to leak into the tooltip as a bogus prefix.
/// A wire token, never displayed.
pub const MAIN_SERIES_NAME: &str = "Readings";

/// The series name every chart's log-event markers travel under: the
/// component's tooltip recognises it and names the event instead of
/// formatting the tick's placement y as a reading. A wire token,
/// never displayed — the tooltip's special case renders the event's
/// own name (the action's localised caption) instead.
pub const MARKER_SERIES_NAME: &str = "Events";

/// The log-event markers as one scatter series of narrow vertical
/// ticks: `rect` symbols 2×12 in the muted gridline colour — geometry
/// carries the event, not a new hue (the Phase Accent Rule). Ticks
/// anchor just above the axis floor so they never cross the line.
/// Markers with unparseable timestamps drop out (the merge rule).
fn marker_series(
    chart: &GrowHistory,
    look: &ChartLook,
    y_min: f64,
    y_max: f64,
    locale: Locale,
) -> serde_json::Value {
    let floor = 0.05_f64.mul_add(y_max - y_min, y_min);
    let data: Vec<serde_json::Value> = chart
        .markers
        .iter()
        .filter_map(|marker| {
            window_ms(&marker.at).map(|at| {
                serde_json::json!({
                    "value": [at, floor],
                    "name": action_label_for_kind(locale, &marker.kind),
                })
            })
        })
        .collect();
    serde_json::json!({
        "name": MARKER_SERIES_NAME,
        "type": "scatter",
        "symbol": "rect",
        "symbolSize": [2, 12],
        "itemStyle": { "color": look.muted },
        "data": data,
    })
}

/// The series name the night shading travels under. An empty-data
/// series never reaches the tooltip; the name exists for debugging
/// and for the e2e selectors.
pub const NIGHT_SERIES_NAME: &str = "Night";

/// The night shading as its own silent back series: a `markArea` of
/// full-height regions — each pair names only its x edges, so
/// `ECharts` spans the whole plot height whatever the value range.
/// The series carries no data and sits at `z` 0, under the readings
/// line and the Target Band wash (both at the series default `z` 2),
/// so the tint reads as a layer of the background, not a series of
/// its own. The muted ink already carries 50 % alpha; the 0.16
/// opacity tops it to ~8 % effective — flat tonal depth, no new hue,
/// both themes (the Phase Accent Rule).
fn night_series(chart: &GrowHistory, look: &ChartLook) -> serde_json::Value {
    let regions: Vec<[serde_json::Value; 2]> = night_regions(chart)
        .into_iter()
        .map(|(start, end)| {
            [
                serde_json::json!({ "xAxis": start }),
                serde_json::json!({ "xAxis": end }),
            ]
        })
        .collect();
    serde_json::json!({
        "name": NIGHT_SERIES_NAME,
        "type": "line",
        "data": [],
        "z": 0,
        "silent": true,
        "markArea": {
            "silent": true,
            "itemStyle": { "opacity": 0.16, "color": look.muted },
            "data": regions,
        },
    })
}

/// The window's dark stretches as merged, window-clipped
/// `[start, end)` epoch-ms pairs. Segments fully outside drop;
/// unparseable bounds drop (the merge rule); butt-joined segments
/// merge so no antialiasing seam cuts a continuous night. An
/// unparseable window edge clips to nothing rather than drawing an
/// unbounded region (the band rule).
fn night_regions(chart: &GrowHistory) -> Vec<(i64, i64)> {
    let Some((from, to)) = (window_ms(&chart.series.from)).zip(window_ms(&chart.series.to)) else {
        return Vec::new();
    };
    let mut spans: Vec<(i64, i64)> = chart
        .nights
        .iter()
        .filter_map(|night| {
            let start = window_ms(&night.start)?.clamp(from, to);
            let end = window_ms(&night.end)?.clamp(from, to);
            (start < end).then_some((start, end))
        })
        .collect();
    spans.sort_unstable();
    let mut merged: Vec<(i64, i64)> = Vec::with_capacity(spans.len());
    for (start, end) in spans {
        match merged.last_mut() {
            Some((_, last_end)) if start <= *last_end => *last_end = (*last_end).max(end),
            _ => merged.push((start, end)),
        }
    }
    merged
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
    use super::{
        CONNECT_LABEL_WIDTH, ChartLook, ChartStyle, MAIN_SERIES_NAME, SeriesKind, history_option,
        role_style, series_is_empty, window_is_empty,
    };
    use crate::api::{
        BandSegmentData, ChartPointData, ChartSeriesData, EventMarkerData, GrowHistory,
        LightIntervalData,
    };
    use crate::i18n::Locale;
    use crate::labels::action_label_for_kind;

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
            markers: Vec::new(),
            nights: Vec::new(),
            degraded: false,
            bound: true,
        }
    }

    #[test]
    fn connected_charts_share_one_plot_geometry_and_a_line_crosshair() {
        // The section's charts join one `ECharts` connect group, and a
        // connected hover is broadcast in pixels. Every role's chart
        // must therefore draw the same plot rectangle: a fixed left
        // gutter for the value-axis labels pins the plot origin at
        // the same x on every card. `containLabel` would instead
        // measure each role's own label text and drift the origins
        // apart (measured: 61 px between the temperature and humidity
        // cards), so the same pixels would land on different
        // timestamps chart-to-chart. The axis pointer is pinned to
        // the line crosshair the group contract names.
        for role in ["temperature", "humidity", "ph"] {
            let option = history_option(&sample_for_role(role), &look(), &style(role), Locale::en);
            assert_eq!(
                option["grid"]["left"],
                serde_json::json!(CONNECT_LABEL_WIDTH + 14.0),
                "{role}: the shared left gutter pins the plot origin"
            );
            assert_eq!(
                option["grid"]["containLabel"],
                serde_json::json!(null),
                "{role}: containLabel would pad each plot origin by its own label width"
            );
            assert_eq!(
                option["yAxis"]["axisLabel"]["width"],
                serde_json::json!(CONNECT_LABEL_WIDTH),
                "{role}: the shared y-label gutter"
            );
            assert_eq!(option["yAxis"]["axisLabel"]["overflow"], "truncate");
            assert_eq!(option["tooltip"]["axisPointer"]["type"], "line");
        }
    }

    #[test]
    fn the_option_labels_axes_and_gridlines_and_arms_the_tooltip() {
        let option = history_option(
            &sample_history(),
            &look(),
            &style("temperature"),
            Locale::en,
        );
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
        let option = history_option(
            &sample_history(),
            &look(),
            &style("temperature"),
            Locale::en,
        );
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
        let option = history_option(&broken, &look(), &style("temperature"), Locale::en);
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
        let option = history_option(
            &sample_history(),
            &look(),
            &style("temperature"),
            Locale::en,
        );
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
        let option = history_option(&flat, &look(), &style("temperature"), Locale::en);
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
        let option = history_option(&empty, &look(), &style("ec"), Locale::en);
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
        let option = history_option(&empty, &look(), &style("ec"), Locale::en);
        assert_eq!(option["yAxis"]["name"], "µS/cm");
        assert_eq!(option["yAxis"]["axisLabel"]["show"], true);
    }

    #[test]
    fn bands_alone_do_not_make_a_window_with_readings() {
        // Target Bands ride every temperature, humidity, VPD and
        // soil window, so "nothing drawable" and "no readings yet"
        // are different questions (ticket 07): the quiet axis keys
        // on the drawable, the card's hint on the readings.
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
        assert!(!window_is_empty(&banded), "the band is drawable");
        assert!(series_is_empty(&banded), "yet no reading exists");
    }

    #[test]
    fn readings_on_either_series_answer_the_hint() {
        // A pair chart (input beside runoff) is only empty when both
        // sides are quiet; one point on either side is a reading.
        let mut paired = sample_for_role("ph");
        paired.series.points = Vec::new();
        assert!(series_is_empty(&paired), "both sides quiet");
        paired.compare = Some(ChartSeriesData {
            from: paired.series.from.clone(),
            to: paired.series.to.clone(),
            points: vec![ChartPointData {
                at: "2026-11-16T00:30:00+00:00".to_owned(),
                value: 6.8,
                source: "log".to_owned(),
            }],
        });
        assert!(!series_is_empty(&paired), "a runoff point is a reading");
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
        let option = history_option(&banded, &look(), &style("temperature"), Locale::en);
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
        let option = history_option(&banded, &look(), &style("temperature"), Locale::en);
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
        let option = history_option(&banded, &look(), &style("temperature"), Locale::en);
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
        let option = history_option(&empty, &look(), &style("temperature"), Locale::en);
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
        let option = history_option(&banded, &look(), &style("temperature"), Locale::en);
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
        let option = history_option(&sample_for_role("vpd"), &look(), &style("vpd"), Locale::en);
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
            let option = history_option(&pair_history(), &look(), &style(role), Locale::en);
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
        let option = history_option(&pair_history(), &look(), &style("ph"), Locale::en);
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
        let option = history_option(&pair_history(), &look(), &style("ph"), Locale::en);
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
        let option = history_option(&alone, &look(), &style("ph"), Locale::en);
        let runoff = option["series"][1]["data"].as_array().expect("runoff data");
        assert!(runoff.is_empty(), "no runoff, no runoff dots");
        let input = option["series"][0]["data"].as_array().expect("input data");
        assert_eq!(input.len(), 1, "the input dots survive alone");
    }

    #[test]
    fn each_pair_role_labels_its_axis_with_its_unit() {
        assert_eq!(style("ph").axis_name, "pH", "pH is dimensionless");
        assert_eq!(style("ec").axis_name, "µS/cm", "EC is canonical µS/cm");
        let option = history_option(&pair_history(), &look(), &style("ec"), Locale::en);
        assert_eq!(option["yAxis"]["name"], "µS/cm");
    }

    #[test]
    fn the_staircase_holds_each_value_until_the_next_reading() {
        let option = history_option(&light_history(), &look(), &style("light"), Locale::en);
        // `step: "end"`: from each reading the line runs horizontally
        // at its value until the next reading's x, then jumps — the
        // old `step_points` semantics, so an off at 00:10 draws on/1.0
        // only from 00:30.
        assert_eq!(option["series"][0]["step"], "end");
    }

    #[test]
    fn a_continuous_role_draws_a_plain_line_without_a_step_marker() {
        let option = history_option(&sample_history(), &look(), &style("humidity"), Locale::en);
        assert!(
            option["series"][0]["step"].is_null(),
            "no step key on plain lines"
        );
        assert_eq!(option["series"][0]["showSymbol"], false);
        assert_eq!(option["series"][0]["type"], "line");
    }

    #[test]
    fn the_series_data_are_epoch_ms_value_pairs() {
        let option = history_option(&light_history(), &look(), &style("light"), Locale::en);
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
        // CO₂ stays bare (env-section ticket 06): the backend emits no
        // Target Band, so empty bands must leave no markArea regions
        // and still yield a sane axis.
        let mut co2 = sample_for_role("co2");
        co2.bands = Vec::new();
        let option = history_option(&co2, &look(), &style("co2"), Locale::en);
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
        let option = history_option(&soil, &look(), &style("soil_moisture"), Locale::en);
        assert_eq!(option["yAxis"]["name"], "%");
        let region = &option["series"][0]["markArea"]["data"][0];
        assert_eq!(region[0]["yAxis"].as_f64(), Some(45.0), "band top");
        assert_eq!(region[1]["yAxis"].as_f64(), Some(25.0), "band bottom");
    }

    // --- Stepped day/night bands (Light Cycle spec, ticket 07) ---

    /// A night segment and a day segment with the germ-phase VPD
    /// ranges the backend emits (env-section ticket 06): different
    /// ranges, stepping at the boundary.
    fn stepped_bands() -> Vec<BandSegmentData> {
        vec![
            BandSegmentData {
                start: "2026-11-16T00:00:00+00:00".to_owned(),
                end: "2026-11-16T00:30:00+00:00".to_owned(),
                min: 0.5,
                max: 0.9,
                phase: "germination".to_owned(),
                light_on: Some(false),
            },
            BandSegmentData {
                start: "2026-11-16T00:30:00+00:00".to_owned(),
                end: "2026-11-16T01:00:00+00:00".to_owned(),
                min: 0.4,
                max: 0.8,
                phase: "germination".to_owned(),
                light_on: Some(true),
            },
        ]
    }

    #[test]
    fn stepped_bands_render_one_region_per_segment_with_its_own_range() {
        let mut stepped = sample_for_role("vpd");
        stepped.bands = stepped_bands();
        let option = history_option(&stepped, &look(), &style("vpd"), Locale::en);
        let regions = option["series"][0]["markArea"]["data"]
            .as_array()
            .expect("mark area data");
        assert_eq!(regions.len(), 2, "one region per stepped segment");
        // Night first: its ceiling (0.9) sits above the day's (0.8).
        assert_eq!(regions[0][0]["yAxis"].as_f64(), Some(0.9));
        assert_eq!(regions[0][1]["yAxis"].as_f64(), Some(0.5));
        assert_eq!(
            regions[0][0]["xAxis"].as_i64(),
            Some(1_794_787_200_000),
            "night region starts at the window"
        );
        assert_eq!(regions[1][0]["yAxis"].as_f64(), Some(0.8));
        assert_eq!(regions[1][1]["yAxis"].as_f64(), Some(0.4));
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
        let mut stepped = sample_for_role("vpd");
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
                min: if step % 2 == 0 { 0.5 } else { 0.4 },
                max: if step % 2 == 0 { 0.9 } else { 0.8 },
                phase: "germination".to_owned(),
                light_on: Some(step % 2 == 1),
            })
            .collect();
        let option = history_option(&stepped, &look(), &style("vpd"), Locale::en);
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

    // --- Temperature Target Band (env-section ticket 06) ---

    #[test]
    fn the_flat_temperature_band_tints_with_the_look_in_both_themes() {
        // The tolerance band is flat (no light state) and carries the
        // resolved accent at the fixed band opacity in either theme:
        // a light look and a dark look both tint the region with
        // their own accent.
        let mut banded = sample_history();
        banded.bands = vec![BandSegmentData {
            start: "2026-11-16T00:00:00+00:00".to_owned(),
            end: "2026-11-16T01:00:00+00:00".to_owned(),
            min: 15.6,
            max: 26.7,
            phase: "germination".to_owned(),
            light_on: None,
        }];
        let dark = ChartLook {
            accent: "#9ccc65".to_owned(),
            compare: "#8a9a5b".to_owned(),
            text: "#e1e1e1".to_owned(),
            muted: "#e1e1e180".to_owned(),
        };
        for theme_look in [look(), dark] {
            let option = history_option(&banded, &theme_look, &style("temperature"), Locale::en);
            let area = &option["series"][0]["markArea"];
            assert_eq!(
                area["itemStyle"]["color"].as_str(),
                Some(theme_look.accent.as_str()),
                "the band tint follows the resolved accent"
            );
            assert_eq!(
                area["itemStyle"]["opacity"].as_f64(),
                Some(0.15),
                "the tint opacity is theme-independent"
            );
            let regions = area["data"].as_array().expect("mark area data");
            assert_eq!(regions.len(), 1, "one region for the flat band");
        }
    }

    // ── Log-event markers (ticket 04) ───────────────────────────

    /// A chart carrying two log-event markers inside its window.
    fn history_with_markers(role: &str) -> GrowHistory {
        let mut chart = sample_for_role(role);
        chart.markers = vec![
            EventMarkerData {
                at: "2026-11-16T00:20:00+00:00".to_owned(),
                kind: "lights".to_owned(),
            },
            EventMarkerData {
                at: "2026-11-16T00:40:00+00:00".to_owned(),
                kind: "phase_change".to_owned(),
            },
        ];
        chart
    }

    #[test]
    fn markers_render_as_one_muted_tick_series_at_the_axis_floor() {
        let option = history_option(
            &history_with_markers("temperature"),
            &look(),
            &style("temperature"),
            Locale::en,
        );
        let series = option["series"].as_array().expect("series array");
        assert_eq!(series.len(), 2, "the tick series appends after the line");
        let ticks = &series[1];
        assert_eq!(ticks["name"], "Events");
        assert_eq!(ticks["type"], "scatter");
        assert_eq!(ticks["symbol"], "rect");
        assert_eq!(ticks["symbolSize"], serde_json::json!([2, 12]));
        assert_eq!(
            ticks["itemStyle"]["color"], "#1d2b1f80",
            "ticks wear the muted gridline colour, not a new hue"
        );
        let data = ticks["data"].as_array().expect("tick data");
        assert_eq!(data.len(), 2, "both in-window markers draw");
        let y_min = option["yAxis"]["min"].as_f64().expect("axis min");
        let y_max = option["yAxis"]["max"].as_f64().expect("axis max");
        let floor = 0.05_f64.mul_add(y_max - y_min, y_min);
        assert_eq!(
            data[0]["value"][0],
            serde_json::json!(1_794_788_400_000_i64)
        );
        assert_eq!(data[0]["value"][1].as_f64(), Some(floor));
        assert_eq!(data[0]["name"], "Lights");
        assert_eq!(data[1]["name"], "Phase change");
    }

    #[test]
    fn charts_without_markers_stay_marker_free() {
        let plain = history_option(
            &sample_history(),
            &look(),
            &style("temperature"),
            Locale::en,
        );
        assert_eq!(plain["series"].as_array().expect("series").len(), 1);
        // A hidden marker set is stripped before the builder sees it.
        let mut hidden = history_with_markers("temperature");
        hidden.markers.clear();
        let stripped = history_option(&hidden, &look(), &style("temperature"), Locale::en);
        assert_eq!(stripped["series"].as_array().expect("series").len(), 1);
    }

    #[test]
    fn marker_ticks_follow_the_theme_muted_colour_in_both_themes() {
        let dark = ChartLook {
            accent: "#9ccc65".to_owned(),
            compare: "#8a9a5b".to_owned(),
            text: "#e1e1e1".to_owned(),
            muted: "#e1e1e180".to_owned(),
        };
        for theme_look in [look(), dark] {
            let option = history_option(
                &history_with_markers("temperature"),
                &theme_look,
                &style("temperature"),
                Locale::en,
            );
            assert_eq!(
                option["series"][1]["itemStyle"]["color"].as_str(),
                Some(theme_look.muted.as_str()),
                "the tick colour follows the resolved muted tone"
            );
        }
    }

    #[test]
    fn pair_charts_append_the_ticks_after_their_two_series() {
        let option = history_option(
            &history_with_markers("ph"),
            &look(),
            &style("ph"),
            Locale::en,
        );
        let series = option["series"].as_array().expect("series array");
        assert_eq!(series.len(), 3, "input, runoff, ticks");
        assert_eq!(series[0]["name"], "Input");
        assert_eq!(series[1]["name"], "Runoff");
        assert_eq!(series[2]["name"], "Events");
    }

    #[test]
    fn marker_ticks_drop_unparseable_timestamps() {
        let mut broken = history_with_markers("soil_moisture");
        broken.markers[0].at = "not-a-timestamp".to_owned();
        let option = history_option(&broken, &look(), &style("soil_moisture"), Locale::en);
        let data = option["series"][1]["data"].as_array().expect("tick data");
        assert_eq!(data.len(), 1, "only the parseable marker draws");
    }

    #[test]
    fn marker_labels_name_events_and_echo_unknown_kinds() {
        assert_eq!(action_label_for_kind(Locale::en, "water"), "Water");
        assert_eq!(action_label_for_kind(Locale::en, "feed"), "Feed");
        assert_eq!(
            action_label_for_kind(Locale::en, "transplant"),
            "Transplant"
        );
        assert_eq!(action_label_for_kind(Locale::en, "lights"), "Lights");
        assert_eq!(
            action_label_for_kind(Locale::en, "light_change"),
            "Light change"
        );
        assert_eq!(action_label_for_kind(Locale::en, "flip"), "Flip 12/12");
        assert_eq!(
            action_label_for_kind(Locale::en, "phase_change"),
            "Phase change"
        );
        assert_eq!(
            action_label_for_kind(Locale::en, "seeds popped"),
            "seeds popped"
        );
        assert_eq!(action_label_for_kind(Locale::de, "water"), "Gießen");
        assert_eq!(action_label_for_kind(Locale::en, "mystery"), "mystery");
    }

    // ── Night shading (ticket 05) ─────────────────────────────

    /// A dark interval across `[start, end)`.
    fn dark(start: &str, end: &str) -> LightIntervalData {
        LightIntervalData {
            start: start.to_owned(),
            end: end.to_owned(),
            on: false,
        }
    }

    /// An Air-group sample with two dark stretches: the window's
    /// first 20 minutes and its last 20 minutes, an on stretch
    /// between (the mock staircase's shape, in miniature).
    fn history_with_nights() -> GrowHistory {
        let mut chart = sample_for_role("temperature");
        chart.nights = vec![
            dark("2026-11-16T00:00:00+00:00", "2026-11-16T00:20:00+00:00"),
            dark("2026-11-16T00:40:00+00:00", "2026-11-16T01:00:00+00:00"),
        ];
        chart
    }

    #[test]
    fn nights_render_as_a_silent_back_series_of_full_height_regions() {
        let option = history_option(
            &history_with_nights(),
            &look(),
            &style("temperature"),
            Locale::en,
        );
        let series = option["series"].as_array().expect("series array");
        assert_eq!(series.len(), 2, "the night series appends after the line");
        let shade = &series[1];
        assert_eq!(shade["name"], "Night");
        assert_eq!(shade["type"], "line");
        assert_eq!(shade["data"], serde_json::json!([]), "no points of its own");
        assert_eq!(shade["silent"], true);
        assert_eq!(
            shade["z"], 0,
            "the back layer: under the data and band wash"
        );
        let area = &shade["markArea"];
        assert_eq!(area["silent"], true);
        assert_eq!(area["itemStyle"]["color"], look().muted);
        assert_eq!(area["itemStyle"]["opacity"], 0.16);
        let regions = area["data"].as_array().expect("night regions");
        assert_eq!(regions.len(), 2);
        // Full-height regions name only the x edges — ECharts then
        // spans the plot's whole height, whatever the value range.
        for region in regions {
            for edge in region.as_array().expect("region edge pair") {
                let object = edge.as_object().expect("region edge object");
                assert_eq!(object.len(), 1, "only the xAxis key: {edge}");
                assert!(object.contains_key("xAxis"));
            }
        }
        assert_eq!(
            regions[0],
            serde_json::json!([
                { "xAxis": 1_794_787_200_000_i64 },
                { "xAxis": 1_794_788_400_000_i64 }
            ]),
            "first dark stretch: [window start, 00:20)"
        );
        assert_eq!(
            regions[1],
            serde_json::json!([
                { "xAxis": 1_794_789_600_000_i64 },
                { "xAxis": 1_794_790_800_000_i64 }
            ]),
            "second dark stretch: [00:40, window end)"
        );
    }

    #[test]
    fn nights_clip_to_the_window_and_drop_what_fell_out() {
        let mut chart = sample_for_role("temperature");
        chart.nights = vec![
            // A whole day before the window: nothing to draw.
            dark("2026-11-15T12:00:00+00:00", "2026-11-15T13:00:00+00:00"),
            // Overhangs both window edges: clamps to the edges.
            dark("2026-11-15T23:30:00+00:00", "2026-11-16T00:10:00+00:00"),
            // Unparseable bound: drops out, never panics.
            LightIntervalData {
                start: "not-a-timestamp".to_owned(),
                end: "2026-11-16T00:50:00+00:00".to_owned(),
                on: false,
            },
        ];
        let option = history_option(&chart, &look(), &style("temperature"), Locale::en);
        let regions = option["series"][1]["markArea"]["data"]
            .as_array()
            .expect("night regions");
        assert_eq!(regions.len(), 1, "only the clamped stretch survives");
        assert_eq!(
            regions[0],
            serde_json::json!([
                { "xAxis": 1_794_787_200_000_i64 },
                { "xAxis": 1_794_787_800_000_i64 }
            ]),
            "clamped to [window start, 00:10)"
        );
    }

    #[test]
    fn butt_joined_nights_merge_into_one_region() {
        let mut chart = sample_for_role("temperature");
        chart.nights = vec![
            dark("2026-11-16T00:00:00+00:00", "2026-11-16T00:20:00+00:00"),
            dark("2026-11-16T00:20:00+00:00", "2026-11-16T00:40:00+00:00"),
        ];
        let option = history_option(&chart, &look(), &style("temperature"), Locale::en);
        let regions = option["series"][1]["markArea"]["data"]
            .as_array()
            .expect("night regions");
        assert_eq!(regions.len(), 1, "one seam-free region, not two");
        assert_eq!(
            regions[0],
            serde_json::json!([
                { "xAxis": 1_794_787_200_000_i64 },
                { "xAxis": 1_794_789_600_000_i64 }
            ])
        );
    }

    #[test]
    fn night_shading_leaves_the_series_untouched_without_nights() {
        let option = history_option(
            &sample_for_role("temperature"),
            &look(),
            &style("temperature"),
            Locale::en,
        );
        let series = option["series"].as_array().expect("series array");
        assert_eq!(series.len(), 1, "no nights, no extra series");
    }

    #[test]
    fn night_shading_appends_after_the_marker_ticks() {
        let mut chart = history_with_markers("temperature");
        chart.nights = history_with_nights().nights;
        let option = history_option(&chart, &look(), &style("temperature"), Locale::en);
        let series = option["series"].as_array().expect("series array");
        assert_eq!(series.len(), 3, "line, ticks, night");
        assert_eq!(series[1]["name"], "Events");
        assert_eq!(series[2]["name"], "Night");
    }

    #[test]
    fn night_shading_alone_keeps_the_empty_window_honest() {
        // Only nights, no readings and no bands: the value axis stays
        // quiet (nights are context, not data — the hint line keeps
        // telling the truth), the tint still draws, and the y range
        // keeps its degenerate-window fallback.
        let mut chart = sample_for_role("temperature");
        chart.series.points = Vec::new();
        chart.nights = history_with_nights().nights;
        assert!(window_is_empty(&chart));
        let option = history_option(&chart, &look(), &style("temperature"), Locale::en);
        assert_eq!(option["yAxis"]["axisLabel"]["show"], false);
        // The y range is exactly what the same window without nights
        // gets: a tint spanning the full height never widens it.
        let mut bare = sample_for_role("temperature");
        bare.series.points = Vec::new();
        let bare_option = history_option(&bare, &look(), &style("temperature"), Locale::en);
        assert_eq!(option["yAxis"]["min"], bare_option["yAxis"]["min"]);
        assert_eq!(option["yAxis"]["max"], bare_option["yAxis"]["max"]);
        assert_eq!(option["series"][1]["name"], "Night");
    }

    #[test]
    fn the_main_line_travels_under_its_wire_name() {
        // Unnamed, the renderer fills a synthetic `seriesN` name and
        // the tooltip used to leak that token as a prefix (D135). The
        // line carries the wire token; the tooltip special-cases it.
        let option = history_option(
            &sample_for_role("temperature"),
            &look(),
            &style("temperature"),
            Locale::en,
        );
        assert_eq!(option["series"][0]["name"], MAIN_SERIES_NAME);
        // The step and pair charts get the same treatment where they
        // are main lines, and stay out of the pair names' way.
        let step = history_option(
            &sample_for_role("light"),
            &look(),
            &style("light"),
            Locale::en,
        );
        assert_eq!(step["series"][0]["name"], MAIN_SERIES_NAME);
        let pair = history_option(&sample_for_role("ph"), &look(), &style("ph"), Locale::en);
        assert_ne!(pair["series"][0]["name"], MAIN_SERIES_NAME);
    }
}
