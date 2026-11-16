//! The `ECharts` option-builder seam (Light Cycle spec, ticket 02):
//! pure functions from the grow history response to an `ECharts` option
//! JSON, unit-tested without a DOM. The wasm component passes the
//! result to `echarts.setOption` via `serde-wasm-bindgen`; `ECharts`
//! only draws — every axis, unit, and band decision lives here.

use crate::api::GrowHistory;

/// The chart's colours, resolved from the app theme at runtime
/// (probe `text-primary` / `text-base-content` / a 50 % faded variant)
/// so both daisyUI themes render natively.
#[derive(Debug, Clone)]
pub struct ChartLook {
    /// The series line and band fill (`text-primary`).
    pub accent: String,
    /// Axis labels and the axis name (`text-base-content`).
    pub text: String,
    /// Axis lines, ticks, and gridlines (a faded variant) — subtle,
    /// not competing with the data.
    pub muted: String,
}

/// The option for one temperature history chart: a time axis with tick
/// labels, a value axis labelled with the dual °C/°F unit, horizontal
/// gridlines, and an axis-triggered hover tooltip.
#[must_use]
pub fn temperature_history_option(chart: &GrowHistory, look: &ChartLook) -> serde_json::Value {
    let (y_min, y_max) = padded_range(chart);
    // The x axis spans the whole Chart window (the old SVG geometry put
    // the window edges at the canvas edges), so sparse data cannot
    // zoom the axis in and fake density. An unparseable window bound
    // would be a backend contract break; the axis then auto-scales.
    let (x_min, x_max) = window_bounds(chart);
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
            "name": "°C / °F",
            "nameTextStyle": { "color": look.text, "align": "left" },
            "min": y_min,
            "max": y_max,
            "splitLine": { "show": true, "lineStyle": { "color": look.muted } },
            "axisLabel": { "hideOverlap": true, "color": look.text },
        },
        "series": [{
            "type": "line",
            "showSymbol": false,
            "lineStyle": { "width": 2, "color": look.accent },
            "data": series_ms_data(chart),
            "markArea": band_mark_area(chart, &look.accent),
        }],
    })
}

/// The series data as `[epoch ms, value]` pairs; points with
/// unparseable timestamps drop out (the same rule the merge applies).
fn series_ms_data(chart: &GrowHistory) -> Vec<(i64, f64)> {
    chart
        .series
        .points
        .iter()
        .filter_map(|point| {
            // Epoch ms (~1.7e12) travels through JSON.parse into an
            // f64 well below the 2^53 exact-integer bound.
            leaflog_core::chart::parse_epoch(&point.at).map(|at| (at * 1000, point.value))
        })
        .collect()
}

/// The axis bounds: the series (and band) range padded 10 % — at
/// least half a unit — so the line never touches the frame. The old
/// SVG geometry used the same padding.
fn padded_range(chart: &GrowHistory) -> (f64, f64) {
    let mut values: Vec<f64> = chart
        .series
        .points
        .iter()
        .map(|point| point.value)
        .collect();
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
    use super::{ChartLook, temperature_history_option};
    use crate::api::{BandSegmentData, ChartPointData, ChartSeriesData, GrowHistory};

    /// Fixed test colours (the fallbacks the component probes for).
    fn look() -> ChartLook {
        ChartLook {
            accent: "#2e7d4f".to_owned(),
            text: "#1d2b1f".to_owned(),
            muted: "#1d2b1f80".to_owned(),
        }
    }

    /// A loaded chart with two points, one band, and a 24-hour window.
    fn sample_history() -> GrowHistory {
        GrowHistory {
            role: "temperature".to_owned(),
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
        let option = temperature_history_option(&sample_history(), &look());
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
        let option = temperature_history_option(&sample_history(), &look());
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
        let option = temperature_history_option(&broken, &look());
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
        let option = temperature_history_option(&sample_history(), &look());
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
        let option = temperature_history_option(&flat, &look());
        let y_max = option["yAxis"]["max"].as_f64().expect("explicit max");
        let y_min = option["yAxis"]["min"].as_f64().expect("explicit min");
        assert!(
            y_max > 21.0 && y_min < 21.0,
            "flat line needs room: {y_min}..{y_max}"
        );
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
        }];
        let option = temperature_history_option(&banded, &look());
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
            },
            // Half in: must clip to the window edge.
            BandSegmentData {
                start: "2026-11-15T12:30:00+00:00".to_owned(),
                end: "2026-11-16T00:20:00+00:00".to_owned(),
                min: 20.0,
                max: 26.0,
                phase: "vegetative".to_owned(),
            },
        ];
        let option = temperature_history_option(&banded, &look());
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
        }];
        let option = temperature_history_option(&banded, &look());
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
        let option = temperature_history_option(&empty, &look());
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
        }];
        let option = temperature_history_option(&banded, &look());
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
}
