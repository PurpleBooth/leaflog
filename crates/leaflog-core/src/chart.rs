//! Chart series engine (spec 2fe2dcc ticket 05).
//!
//! Merges Home Assistant history points with Leaflog's own
//! `sensor_snapshot` rows into one windowed series per sensor role,
//! and builds the per-phase band corridor (min/max presets) that the
//! chart draws behind the line. Pure: timestamps in, series out; no
//! I/O. Windows are rolling (24 h / 7 d / 30 d) ending at `now`.
//!
//! Boundary rule: the window is `[from, now]` inclusive on both ends.
//! Phase bands key off the phase each instant falls in (derived from
//! the grow's forecast windows), so pinned boundaries (ticket 02's
//! flip/harvest pins) step the corridor the moment they exist.

use serde::{Deserialize, Serialize};

use crate::Phase;
use crate::forecast::PhaseWindow;

/// The rolling window length behind a chart (day/week/month toggles
/// map to rolling 24 h / 7 d / 30 d).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChartWindow {
    /// Rolling 24 hours.
    Day,
    /// Rolling 7 days.
    Week,
    /// Rolling 30 days.
    Month,
}

impl ChartWindow {
    /// All windows in toggle order.
    #[must_use]
    pub const fn all() -> [Self; 3] {
        [Self::Day, Self::Week, Self::Month]
    }

    /// The window length in hours (24 / 168 / 720).
    #[must_use]
    pub const fn hours(self) -> i64 {
        match self {
            Self::Day => 24,
            Self::Week => 24 * 7,
            Self::Month => 24 * 30,
        }
    }

    /// The API label (`day` / `week` / `month`).
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Day => "day",
            Self::Week => "week",
            Self::Month => "month",
        }
    }

    /// Parses an API label; the inverse of [`ChartWindow::label`].
    #[must_use]
    pub fn parse(label: &str) -> Option<Self> {
        let candidate = match label {
            "day" => Self::Day,
            "week" => Self::Week,
            "month" => Self::Month,
            _ => return None,
        };
        Some(candidate)
    }
}

/// Where a series point came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PointSource {
    /// Home Assistant's history API.
    Ha,
    /// A Leaflog `sensor_snapshot` row captured at log time.
    Snapshot,
}

/// One raw (timestamp, value) input point; SI units, RFC 3339 UTC.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RawPoint {
    /// When the value was recorded, RFC 3339 UTC.
    pub at: String,
    /// The value in SI units (°C for temperature).
    pub value: f64,
}

/// One merged series point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SeriesPoint {
    /// When the value was recorded, RFC 3339 UTC.
    pub at: String,
    /// The value in SI units.
    pub value: f64,
    /// Which capture path produced it.
    pub source: PointSource,
}

/// One corridor segment: the phase's min/max range between two
/// instants, already clipped to the window.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BandSegment {
    /// Segment start, RFC 3339 UTC (inclusive).
    pub start: String,
    /// Segment end, RFC 3339 UTC (the next segment's start, or the
    /// window end).
    pub end: String,
    /// Lower band edge in SI units.
    pub min: f64,
    /// Upper band edge in SI units.
    pub max: f64,
    /// The phase the segment belongs to.
    pub phase: Phase,
}

/// A phase boundary: the phase runs from `start` until the next
/// span's start (the last span runs open-ended).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PhaseSpan {
    /// When the phase begins, RFC 3339 UTC.
    pub start: String,
    /// Which phase begins here.
    pub phase: Phase,
}

/// The merged windowed series for one role.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChartSeries {
    /// Window start, RFC 3339 UTC (`now − window`), inclusive.
    pub from: String,
    /// Window end (the `now` the caller passed), inclusive.
    pub to: String,
    /// Points ascending by time; exact duplicates resolved in favour
    /// of snapshots (the app-verified capture path).
    pub points: Vec<SeriesPoint>,
}

/// Parses an RFC 3339 instant into Unix seconds; `None` when it does
/// not parse.
#[must_use]
pub fn parse_epoch(at: &str) -> Option<i64> {
    at.parse::<jiff::Timestamp>()
        .ok()
        .map(jiff::Timestamp::as_second)
}

/// Formats Unix seconds back to an RFC 3339 UTC instant.
fn format_epoch(seconds: i64) -> String {
    jiff::Timestamp::from_second(seconds)
        .map(|timestamp| timestamp.to_string())
        .unwrap_or_default()
}

/// The window start: `now − window`, RFC 3339 UTC. `None` when `now`
/// does not parse.
#[must_use]
pub fn window_from(now: &str, window: ChartWindow) -> Option<String> {
    let moment: jiff::Timestamp = now.parse().ok()?;
    let span = jiff::Span::new().hours(window.hours());
    Some((moment - span).to_string())
}

/// Merges HA history points and Leaflog snapshot rows into one
/// windowed series (rolling `[now − window, now]`, inclusive on both
/// ends).
///
/// Rules: points outside the window, with unparseable timestamps or
/// non-finite values are dropped; the output is ascending by time; an
/// exact timestamp tie between HA and a snapshot keeps the snapshot
/// (the app-verified capture path). An unparseable `now` yields an
/// empty series (fail closed).
#[must_use]
pub fn merge_series(
    now: &str,
    window: ChartWindow,
    ha: &[RawPoint],
    snapshots: &[RawPoint],
) -> ChartSeries {
    let from = window_from(now, window).unwrap_or_default();
    let Some(from_epoch) = parse_epoch(&from) else {
        return ChartSeries {
            from,
            to: now.to_owned(),
            points: Vec::new(),
        };
    };
    let to_epoch = parse_epoch(now).unwrap_or(from_epoch);

    // (epoch, preference, point): snapshots win exact ties, so they
    // sort first (lower rank) and dedup keeps them.
    let mut ordered: Vec<(i64, u8, SeriesPoint)> = Vec::new();
    for (batch, source, rank) in [
        (ha, PointSource::Ha, 1_u8),
        (snapshots, PointSource::Snapshot, 0_u8),
    ] {
        for raw in batch {
            let Some(epoch) = parse_epoch(&raw.at) else {
                continue;
            };
            if !raw.value.is_finite() || epoch < from_epoch || epoch > to_epoch {
                continue;
            }
            ordered.push((
                epoch,
                rank,
                SeriesPoint {
                    at: raw.at.clone(),
                    value: raw.value,
                    source,
                },
            ));
        }
    }
    ordered.sort_by_key(|(epoch, rank, _)| (*epoch, *rank));
    ordered.dedup_by(|later, kept| later.0 == kept.0);

    ChartSeries {
        from,
        to: now.to_owned(),
        points: ordered.into_iter().map(|(_, _, point)| point).collect(),
    }
}

/// Builds the band corridor for one window: each phase span overlapping
/// `[from, to]` becomes a segment of its corridor range, clipped to the
/// window, stepping exactly at the span boundaries.
///
/// `corridor` maps a phase to its `(min, max)` preset, or `None` when
/// the role has no corridor (the bands list stays empty). Spans with
/// unparseable starts are skipped; the last span runs open-ended until
/// the window end.
#[must_use]
pub fn build_bands(
    from: &str,
    to: &str,
    spans: &[PhaseSpan],
    corridor: impl Fn(Phase) -> Option<(f64, f64)>,
) -> Vec<BandSegment> {
    let Some(from_epoch) = parse_epoch(from) else {
        return Vec::new();
    };
    let Some(to_epoch) = parse_epoch(to) else {
        return Vec::new();
    };
    let mut sorted: Vec<(i64, &PhaseSpan)> = spans
        .iter()
        .filter_map(|span| parse_epoch(&span.start).map(|epoch| (epoch, span)))
        .collect();
    sorted.sort_by_key(|(epoch, _)| *epoch);

    let mut bands = Vec::new();
    for (index, (start_epoch, span)) in sorted.iter().enumerate() {
        // A span ends where the next begins; the last runs to the
        // window end.
        let span_end = sorted
            .get(index + 1)
            .map_or(to_epoch, |(next_epoch, _)| *next_epoch);
        let segment_start = (*start_epoch).max(from_epoch);
        let segment_end = span_end.min(to_epoch);
        if segment_start > segment_end {
            continue;
        }
        let Some((min, max)) = corridor(span.phase) else {
            continue;
        };
        bands.push(BandSegment {
            start: format_epoch(segment_start),
            end: format_epoch(segment_end),
            min,
            max,
            phase: span.phase,
        });
    }
    bands
}

/// Maps a grow's forecast windows to phase-boundary instants: each
/// window's likely start becomes the local midnight (in `tz_name`) of
/// `started_on + likely_start` days, as plain RFC 3339 UTC.
///
/// Windows must be the forecast engine's chronological output. Fail
/// closed: an unparseable start date, unknown zone or absurd window
/// yields an empty list (the caller serves no bands).
#[must_use]
pub fn phase_spans(started_on: &str, tz_name: &str, windows: &[PhaseWindow]) -> Vec<PhaseSpan> {
    let Ok(zone) = jiff::tz::TimeZone::get(tz_name) else {
        return Vec::new();
    };
    let Ok(start) = started_on.parse::<jiff::civil::Date>() else {
        return Vec::new();
    };
    let mut spans = Vec::with_capacity(windows.len());
    for window in windows {
        let Ok(day) = start.checked_add(jiff::Span::new().days(window.likely_start)) else {
            continue;
        };
        let Ok(midnight) = day.at(0, 0, 0, 0).to_zoned(zone.clone()) else {
            continue;
        };
        spans.push(PhaseSpan {
            start: midnight.timestamp().to_string(),
            phase: window.phase,
        });
    }
    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: &str = "2026-11-16T12:00:00Z";

    fn point(at: &str, value: f64) -> RawPoint {
        RawPoint {
            at: at.to_owned(),
            value,
        }
    }

    #[test]
    fn window_toggles_map_to_rolling_hours() {
        assert_eq!(ChartWindow::Day.hours(), 24);
        assert_eq!(ChartWindow::Week.hours(), 24 * 7);
        assert_eq!(ChartWindow::Month.hours(), 24 * 30);
        for window in ChartWindow::all() {
            assert_eq!(ChartWindow::parse(window.label()), Some(window));
        }
        assert_eq!(ChartWindow::parse("year"), None);
    }

    #[test]
    fn window_start_is_exactly_n_hours_before_now() {
        // Worked example: 2026-11-16T12:00Z minus 24 h is 2026-11-15T12:00Z.
        let from = window_from(NOW, ChartWindow::Day).unwrap();
        assert_eq!(parse_epoch(&from), parse_epoch("2026-11-15T12:00:00Z"));
        let week = window_from(NOW, ChartWindow::Week).unwrap();
        assert_eq!(parse_epoch(&week), parse_epoch("2026-11-09T12:00:00Z"));
        assert_eq!(window_from("not-a-time", ChartWindow::Day), None);
    }

    #[test]
    fn merge_combines_sources_ascending_inside_window() {
        let ha = [
            point("2026-11-15T18:00:00Z", 21.0),
            point("2026-11-16T06:00:00Z", 23.5),
        ];
        let snapshots = [point("2026-11-15T20:00:00Z", 22.0)];
        let series = merge_series(NOW, ChartWindow::Day, &ha, &snapshots);
        let at: Vec<&str> = series.points.iter().map(|p| p.at.as_str()).collect();
        assert_eq!(
            at,
            [
                "2026-11-15T18:00:00Z",
                "2026-11-15T20:00:00Z",
                "2026-11-16T06:00:00Z"
            ]
        );
        assert_eq!(series.points[0].source, PointSource::Ha);
        assert_eq!(series.points[1].source, PointSource::Snapshot);
        // Window edges echo back for the chart's x scale.
        assert_eq!(
            parse_epoch(&series.from),
            parse_epoch("2026-11-15T12:00:00Z")
        );
        assert_eq!(parse_epoch(&series.to), parse_epoch(NOW));
    }

    #[test]
    fn merge_drops_points_outside_the_window() {
        // Rolling window is [now − 24 h, now]: both edges inclusive.
        let ha = [
            point("2026-11-15T12:00:00Z", 20.0), // exactly from: kept
            point("2026-11-16T12:00:00Z", 24.0), // exactly to: kept
            point("2026-11-15T11:59:59Z", 19.0), // one second early: dropped
            point("2026-11-16T12:00:01Z", 25.0), // one second late: dropped
        ];
        let series = merge_series(NOW, ChartWindow::Day, &ha, &[]);
        let values: Vec<f64> = series.points.iter().map(|p| p.value).collect();
        assert_eq!(values, [20.0, 24.0]);
    }

    #[test]
    fn merge_resolves_exact_ties_in_favour_of_snapshots() {
        let ha = [point("2026-11-16T08:00:00Z", 99.0)];
        let snapshots = [point("2026-11-16T08:00:00Z", 22.5)];
        let series = merge_series(NOW, ChartWindow::Day, &ha, &snapshots);
        assert_eq!(series.points.len(), 1);
        assert_eq!(series.points[0].value, 22.5);
        assert_eq!(series.points[0].source, PointSource::Snapshot);
    }

    #[test]
    fn merge_drops_unparseable_and_non_finite_points() {
        let ha = [
            point("not-a-timestamp", 21.0),
            point("2026-11-16T06:00:00Z", f64::NAN),
            point("2026-11-16T07:00:00Z", 22.0),
        ];
        let series = merge_series(NOW, ChartWindow::Day, &ha, &[]);
        assert_eq!(series.points.len(), 1);
        assert_eq!(series.points[0].value, 22.0);
    }

    #[test]
    fn merge_fails_closed_on_an_unparseable_now() {
        let ha = [point("2026-11-16T06:00:00Z", 22.0)];
        let series = merge_series("garbage", ChartWindow::Day, &ha, &[]);
        assert_eq!(series.points, Vec::new());
    }

    // --- band corridor ---

    fn span(start: &str, phase: Phase) -> PhaseSpan {
        PhaseSpan {
            start: start.to_owned(),
            phase,
        }
    }

    #[test]
    fn bands_step_at_phase_boundaries_inside_the_window() {
        let spans = [
            span("2026-11-10T00:00:00Z", Phase::Vegetative),
            span("2026-11-14T00:00:00Z", Phase::Flower),
        ];
        let from = "2026-11-12T00:00:00Z";
        let to = "2026-11-16T00:00:00Z";
        let corridor = |phase: Phase| match phase {
            Phase::Vegetative => Some((20.0, 28.0)),
            Phase::Flower => Some((18.0, 26.0)),
            _ => None,
        };
        let bands = build_bands(from, to, &spans, corridor);
        assert_eq!(bands.len(), 2);
        // The vegetative band is clipped to the window start; the
        // flower band starts exactly at the boundary and runs to the
        // window end. Segments butt-join with no gap.
        assert_eq!(parse_epoch(&bands[0].start), parse_epoch(from));
        assert_eq!(
            parse_epoch(&bands[0].end),
            parse_epoch("2026-11-14T00:00:00Z")
        );
        assert_eq!(bands[0].phase, Phase::Vegetative);
        assert_eq!((bands[0].min, bands[0].max), (20.0, 28.0));
        assert_eq!(
            parse_epoch(&bands[1].start),
            parse_epoch("2026-11-14T00:00:00Z")
        );
        assert_eq!(parse_epoch(&bands[1].end), parse_epoch(to));
        assert_eq!(bands[1].phase, Phase::Flower);
        assert_eq!((bands[1].min, bands[1].max), (18.0, 26.0));
        assert_eq!(bands[0].end, bands[1].start);
    }

    #[test]
    fn bands_skip_roles_without_a_corridor() {
        let spans = [span("2026-11-10T00:00:00Z", Phase::Flower)];
        let bands = build_bands(
            "2026-11-12T00:00:00Z",
            "2026-11-16T00:00:00Z",
            &spans,
            |_| None,
        );
        assert_eq!(bands, Vec::new());
    }

    #[test]
    fn bands_ignore_spans_that_never_overlap_the_window() {
        // A span starting after the window end contributes nothing;
        // an unparseable start is skipped fail-closed.
        let spans = [
            span("nope", Phase::Seedling),
            span("2026-12-01T00:00:00Z", Phase::Curing),
        ];
        let bands = build_bands(
            "2026-11-12T00:00:00Z",
            "2026-11-16T00:00:00Z",
            &spans,
            |_| Some((10.0, 30.0)),
        );
        assert_eq!(bands, Vec::new());
    }

    #[test]
    fn bands_cover_the_window_from_a_span_that_started_before_it() {
        let spans = [span("2026-11-01T00:00:00Z", Phase::Germination)];
        let bands = build_bands(
            "2026-11-12T00:00:00Z",
            "2026-11-16T00:00:00Z",
            &spans,
            |_| Some((21.0, 26.7)),
        );
        assert_eq!(bands.len(), 1);
        assert_eq!(
            parse_epoch(&bands[0].start),
            parse_epoch("2026-11-12T00:00:00Z")
        );
        assert_eq!(
            parse_epoch(&bands[0].end),
            parse_epoch("2026-11-16T00:00:00Z")
        );
        assert_eq!(bands[0].phase, Phase::Germination);
    }

    #[test]
    fn bands_fail_closed_on_unparseable_window_edges() {
        let spans = [span("2026-11-10T00:00:00Z", Phase::Flower)];
        let corridor = |_| Some((0.0, 1.0));
        assert_eq!(
            build_bands("garbage", "2026-11-16T00:00:00Z", &spans, corridor),
            Vec::new()
        );
        assert_eq!(
            build_bands("2026-11-12T00:00:00Z", "garbage", &spans, corridor),
            Vec::new()
        );
    }

    // --- forecast windows to phase spans ---

    fn phase_window(phase: Phase, likely_start: i64) -> PhaseWindow {
        PhaseWindow {
            phase,
            earliest_start: likely_start,
            likely_start,
            latest_start: likely_start,
            earliest_end: likely_start + 1,
            likely_end: likely_start + 1,
            latest_end: likely_start + 1,
            actual_start: false,
        }
    }

    #[test]
    fn phase_spans_anchor_to_local_midnight_in_the_grow_zone() {
        // Window starts are grow-day offsets; each span begins at that
        // day's midnight in the grow's zone. Berlin in November is
        // UTC+1, so 2026-11-01 00:00 local is 2026-10-31T23:00Z.
        let windows = [
            phase_window(Phase::Germination, 0),
            phase_window(Phase::Seedling, 5),
        ];
        let spans = phase_spans("2026-11-01", "Europe/Berlin", &windows);
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].phase, Phase::Germination);
        assert_eq!(parse_epoch(&spans[0].start), Some(1_793_487_600));
        assert_eq!(spans[1].phase, Phase::Seedling);
        // Five days later: 2026-11-06 00:00 local is 2026-11-05T23:00Z.
        assert_eq!(
            parse_epoch(&spans[1].start),
            parse_epoch("2026-11-05T23:00:00Z")
        );
    }

    #[test]
    fn phase_spans_respect_summer_time() {
        // Berlin 2026-06-01 is UTC+2: local midnight is 22:00Z the day
        // before.
        let windows = [phase_window(Phase::Vegetative, 0)];
        let spans = phase_spans("2026-06-01", "Europe/Berlin", &windows);
        assert_eq!(spans.len(), 1);
        assert_eq!(
            parse_epoch(&spans[0].start),
            parse_epoch("2026-05-31T22:00:00Z")
        );
    }

    #[test]
    fn phase_spans_fail_closed_on_unknown_zone_or_date() {
        let windows = [phase_window(Phase::Germination, 0)];
        assert_eq!(
            phase_spans("2026-11-01", "Mars/Olympus", &windows),
            Vec::new()
        );
        assert_eq!(phase_spans("soon", "Europe/Berlin", &windows), Vec::new());
    }
}
