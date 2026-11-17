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
    /// Computed from other series (the derived VPD chart).
    Derived,
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
    /// Whether the grow lights were on during the segment. `None`
    /// when no light state applies: the unlit phases (drying,
    /// curing), or no schedule and no light history to speak of.
    pub light_on: Option<bool>,
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
    points: &[(i64, f64)],
    spans: &[PhaseSpan],
    corridor: impl Fn(Phase, Option<f64>) -> Option<(f64, f64)>,
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
        // The segment's mean point value gives context-dependent
        // corridors their input (an RH window needs the mean
        // temperature); None when the segment carries no points.
        let mut sum = 0.0;
        let mut count = 0;
        for &(at, value) in points {
            if at >= segment_start && at <= segment_end {
                sum += value;
                count += 1;
            }
        }
        let mean = if count == 0 {
            None
        } else {
            let mean = sum / f64::from(u32::try_from(count).unwrap_or(u32::MAX));
            Some(mean)
        };
        let Some((min, max)) = corridor(span.phase, mean) else {
            continue;
        };
        bands.push(BandSegment {
            start: format_epoch(segment_start),
            end: format_epoch(segment_end),
            min,
            max,
            phase: span.phase,
            light_on: None,
        });
    }
    bands
}

// ---------------------------------------------------------------------------
// Light-state timeline (Light Cycle spec, ticket 07)
// ---------------------------------------------------------------------------

/// One lights-on/off interval: the state holds from `start` (inclusive)
/// to `end` (exclusive), RFC 3339 UTC.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LightInterval {
    /// Interval start, RFC 3339 UTC (inclusive).
    pub start: String,
    /// Interval end, RFC 3339 UTC (exclusive; the next interval's
    /// start, or the window end).
    pub end: String,
    /// Whether the lights are on during the interval.
    pub on: bool,
}

/// The phase in effect at an instant, from the sorted span epochs.
/// `None` when no span has started yet (the caller serves no schedule).
fn phase_at(spans: &[(i64, Phase)], at: i64) -> Option<Phase> {
    spans
        .iter()
        .rev()
        .find(|(start, _)| *start <= at)
        .map(|(_, phase)| *phase)
}

/// Expands a grow's Light Cycle into the lights-on/off timeline for
/// the window (the schedule that fills gaps and the whole unbound
/// window).
///
/// The schedule anchors in the grow's canonical timezone: each local
/// day whose phase is one of [`crate::light_cycle::LIT_PHASES`] gets
/// its on-period at the resolved lights-on wall-clock time, lasting
/// `hours_on` absolute hours — so the on-length survives a DST
/// transition while the anchor stays local, the same convention
/// [`phase_spans`] uses for phase boundaries. Off periods fill the
/// rest of the window. Days in the unlit phases, before the first
/// phase span, or outside a parseable window contribute nothing; an
/// unknown zone fails closed to an empty list.
#[must_use]
pub fn schedule_light_periods(
    from: &str,
    to: &str,
    tz_name: &str,
    spans: &[PhaseSpan],
    cycle: &crate::light_cycle::LightCycle,
    photoperiod: crate::strain::PhotoperiodType,
) -> Vec<LightInterval> {
    let (Some(from_epoch), Some(to_epoch)) = (parse_epoch(from), parse_epoch(to)) else {
        return Vec::new();
    };
    let Ok(zone) = jiff::tz::TimeZone::get(tz_name) else {
        return Vec::new();
    };
    let (Ok(from_zoned), Ok(to_zoned)) = (
        jiff::Timestamp::from_second(from_epoch).map(|ts| ts.to_zoned(zone.clone())),
        jiff::Timestamp::from_second(to_epoch).map(|ts| ts.to_zoned(zone.clone())),
    ) else {
        return Vec::new();
    };
    let mut sorted: Vec<(i64, Phase)> = spans
        .iter()
        .filter_map(|span| parse_epoch(&span.start).map(|epoch| (epoch, span.phase)))
        .collect();
    sorted.sort_by_key(|(epoch, _)| *epoch);

    let on_time = cycle.lights_on_resolved();
    // ClockTime validates its bounds, so the i8 conversions jiff wants
    // are always in range.
    let on_hour = i8::try_from(on_time.hour).unwrap_or(0);
    let on_minute = i8::try_from(on_time.minute).unwrap_or(0);
    // Start one local day early so an on-period wrapped over midnight
    // still covers the window's first hours.
    let mut day = from_zoned.datetime().date();
    if let Ok(earlier) = day.yesterday() {
        day = earlier;
    }
    let last_day = to_zoned.datetime().date();
    let mut periods = Vec::new();
    loop {
        if let Ok(midnight) = day.at(0, 0, 0, 0).to_zoned(zone.clone())
            && let Some(phase) = phase_at(&sorted, midnight.timestamp().as_second())
            && let Some(hours) = cycle.hours_on_for(phase, photoperiod)
            && let Ok(on_zoned) = day.at(on_hour, on_minute, 0, 0).to_zoned(zone.clone())
            && let Ok(off_zoned) = on_zoned.checked_add(jiff::Span::new().hours(i64::from(hours)))
        {
            let start = on_zoned.timestamp().as_second().max(from_epoch);
            let end = off_zoned.timestamp().as_second().min(to_epoch);
            if start < end {
                periods.push(LightInterval {
                    start: format_epoch(start),
                    end: format_epoch(end),
                    on: true,
                });
            }
        }
        if day >= last_day {
            break;
        }
        match day.tomorrow() {
            Ok(next) => day = next,
            Err(_) => break,
        }
    }
    // Interleave the off periods: between and around the on-periods.
    // A window whose phases are all unlit yields no on-periods and
    // therefore no schedule at all.
    if periods.is_empty() {
        return Vec::new();
    }
    complete_off_periods(from_epoch, to_epoch, &mut periods);
    periods
}

/// Fills the gaps between and around `on_periods` (ascending,
/// non-overlapping) with off periods so the timeline covers the whole
/// window edge to edge.
fn complete_off_periods(from_epoch: i64, to_epoch: i64, on_periods: &mut Vec<LightInterval>) {
    let mut complete = Vec::with_capacity(on_periods.len() * 2 + 1);
    let mut cursor = from_epoch;
    for period in on_periods.drain(..) {
        let start = parse_epoch(&period.start).unwrap_or(cursor);
        let end = parse_epoch(&period.end).unwrap_or(to_epoch);
        if start > cursor {
            complete.push(LightInterval {
                start: format_epoch(cursor),
                end: format_epoch(start),
                on: false,
            });
        }
        complete.push(period);
        cursor = cursor.max(end);
    }
    if cursor < to_epoch {
        complete.push(LightInterval {
            start: format_epoch(cursor),
            end: format_epoch(to_epoch),
            on: false,
        });
    }
    *on_periods = complete;
}

/// Builds the light-state intervals from the bound light entity's
/// staircase readings (in-window `(epoch, value)` pairs; any positive
/// value is a lit fixture).
///
/// Staircase semantics: each reading's state holds until the next
/// reading, the last until the window end — the timeline covers
/// `[first reading, to]`. The stretch before the first reading is
/// deliberately uncovered: the schedule fills it (the precedence
/// rule), so this returns nothing there.
#[must_use]
pub fn sensor_light_periods(from: &str, to: &str, staircase: &[(i64, f64)]) -> Vec<LightInterval> {
    let (Some(from_epoch), Some(to_epoch)) = (parse_epoch(from), parse_epoch(to)) else {
        return Vec::new();
    };
    let mut reads: Vec<(i64, bool)> = staircase
        .iter()
        .copied()
        .filter(|(at, value)| value.is_finite() && *at >= from_epoch && *at <= to_epoch)
        .map(|(at, value)| (at, value > 0.0))
        .collect();
    reads.sort_by_key(|(at, _)| *at);
    // Collapse runs of repeated state: the run's first reading anchors
    // its whole stretch.
    reads.dedup_by(|later, kept| later.1 == kept.1);

    let mut periods = Vec::new();
    for (index, (at, on)) in reads.iter().enumerate() {
        let end = reads
            .get(index + 1)
            .map_or(to_epoch, |(next_at, _)| *next_at);
        if at >= &end {
            continue;
        }
        periods.push(LightInterval {
            start: format_epoch(*at),
            end: format_epoch(end),
            on: *on,
        });
    }
    periods
}

/// Resolves the window's light-state timeline by precedence.
///
/// The sensor staircase wins wherever it has readings; the schedule
/// fills the uncovered stretch before the first reading. When neither
/// exists there is no light state at all (`None` — the caller keeps a
/// single band).
#[must_use]
pub fn resolve_light_periods(
    from: &str,
    _to: &str,
    schedule: &[LightInterval],
    sensor: &[LightInterval],
) -> Option<Vec<LightInterval>> {
    if sensor.is_empty() {
        return (!schedule.is_empty()).then(|| schedule.to_vec());
    }
    let from_epoch = parse_epoch(from)?;
    let first_sensor = sensor
        .iter()
        .filter_map(|interval| parse_epoch(&interval.start))
        .min()?;
    // Schedule intervals clipped to the pre-sensor stretch.
    let mut resolved: Vec<LightInterval> = schedule
        .iter()
        .filter_map(|interval| {
            let start = parse_epoch(&interval.start)?;
            let end = parse_epoch(&interval.end)?
                .min(first_sensor)
                .max(from_epoch);
            (start < end && start < first_sensor).then_some(LightInterval {
                start: format_epoch(start),
                end: format_epoch(end),
                on: interval.on,
            })
        })
        .collect();
    resolved.extend(sensor.iter().cloned());
    Some(resolved)
}

/// Builds the stepped band corridor for the window.
///
/// Each phase span overlapping `[from, to]` is split at the
/// light-state changes, and every piece gets its corridor range for
/// (phase, light state, mean point value). Adjacent pieces with
/// identical `(min, max, phase, light_on)` merge, so a month of
/// steady nights is one region, not hundreds.
///
/// The unlit phases (drying, curing) never split. `light` being `None`
/// (no schedule and no sensor state) or an uncovered stretch yields
/// pieces with `light_on: None` — the corridor decides what an unknown
/// state means (today: the lights-on preset).
#[must_use]
pub fn build_light_bands(
    from: &str,
    to: &str,
    points: &[(i64, f64)],
    spans: &[PhaseSpan],
    light: Option<&[LightInterval]>,
    corridor: impl Fn(Phase, Option<bool>, Option<f64>) -> Option<(f64, f64)>,
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

    let mut bands: Vec<BandSegment> = Vec::new();
    for (index, (start_epoch, span)) in sorted.iter().enumerate() {
        let span_end = sorted
            .get(index + 1)
            .map_or(to_epoch, |(next_epoch, _)| *next_epoch);
        let segment_start = (*start_epoch).max(from_epoch);
        let segment_end = span_end.min(to_epoch);
        if segment_start > segment_end {
            continue;
        }
        for (piece_start, piece_end, light_on) in
            light_pieces(segment_start, segment_end, span.phase, light)
        {
            let mut sum = 0.0;
            let mut count = 0;
            for &(at, value) in points {
                if at >= piece_start && at <= piece_end {
                    sum += value;
                    count += 1;
                }
            }
            let mean =
                (count > 0).then(|| sum / f64::from(u32::try_from(count).unwrap_or(u32::MAX)));
            let Some((min, max)) = corridor(span.phase, light_on, mean) else {
                continue;
            };
            push_merged(
                &mut bands,
                BandSegment {
                    start: format_epoch(piece_start),
                    end: format_epoch(piece_end),
                    min,
                    max,
                    phase: span.phase,
                    light_on,
                },
            );
        }
    }
    bands
}

/// Splits one phase segment at the light-state changes. The unlit
/// phases never split; a missing timeline or an uncovered stretch
/// yields `None` light-state pieces.
fn light_pieces(
    segment_start: i64,
    segment_end: i64,
    phase: Phase,
    light: Option<&[LightInterval]>,
) -> Vec<(i64, i64, Option<bool>)> {
    if !crate::light_cycle::LIT_PHASES.contains(&phase) {
        return vec![(segment_start, segment_end, None)];
    }
    let Some(intervals) = light else {
        return vec![(segment_start, segment_end, None)];
    };
    let mut ordered: Vec<(i64, i64, bool)> = intervals
        .iter()
        .filter_map(|interval| {
            Some((
                parse_epoch(&interval.start)?,
                parse_epoch(&interval.end)?,
                interval.on,
            ))
        })
        .collect();
    ordered.sort_by_key(|(start, _, _)| *start);
    let mut pieces = Vec::new();
    let mut cursor = segment_start;
    for (start, end, on) in ordered {
        let start = start.max(segment_start);
        let end = end.min(segment_end);
        if start >= end {
            continue;
        }
        if start > cursor {
            pieces.push((cursor, start, None));
        }
        let end = end.max(cursor);
        pieces.push((cursor.max(start), end, Some(on)));
        cursor = end;
    }
    if cursor < segment_end {
        pieces.push((cursor, segment_end, None));
    }
    pieces
}

/// Appends a segment, fusing it into the previous one when phase,
/// light state and range all agree and the pieces are contiguous.
#[allow(clippy::float_cmp)] // range edges are preset constants copied bit-identical; see docs/DECISIONS.md
#[allow(clippy::suspicious_operation_groupings)] // end == start is the butt-join check, on purpose
fn push_merged(bands: &mut Vec<BandSegment>, segment: BandSegment) {
    if let Some(last) = bands.last_mut()
        && last.phase == segment.phase
        && last.light_on == segment.light_on
        && last.min == segment.min
        && last.max == segment.max
        && last.end == segment.start
    {
        last.end = segment.end;
        return;
    }
    bands.push(segment);
}

/// Tolerance for pairing a humidity point to a temperature point when
/// deriving the VPD series (five minutes).
pub const VPD_PAIRING_TOLERANCE_SECS: i64 = 300;

/// Derives the VPD series from the merged temperature and humidity
/// series.
///
/// Each temperature point pairs with the nearest-in-time humidity
/// point within [`VPD_PAIRING_TOLERANCE_SECS`] and converts via
/// [`crate::env::vpd_kpa`]; unpaired points drop out. The window
/// bounds copy the temperature series.
#[must_use]
pub fn derive_vpd_series(
    temperature: &ChartSeries,
    humidity: &ChartSeries,
    leaf_offset_c: f64,
) -> ChartSeries {
    let rh_points: Vec<(i64, f64)> = humidity
        .points
        .iter()
        .filter_map(|point| parse_epoch(&point.at).map(|at| (at, point.value)))
        .collect();
    let points = temperature
        .points
        .iter()
        .filter_map(|point| {
            let at = parse_epoch(&point.at)?;
            let nearest = rh_points
                .iter()
                .min_by_key(|(rh_at, _)| (rh_at - at).abs())?;
            if (nearest.0 - at).abs() > VPD_PAIRING_TOLERANCE_SECS {
                return None;
            }
            Some(SeriesPoint {
                at: point.at.clone(),
                value: crate::env::vpd_kpa(point.value, nearest.1, leaf_offset_c),
                source: PointSource::Derived,
            })
        })
        .collect();
    ChartSeries {
        from: temperature.from.clone(),
        to: temperature.to.clone(),
        points,
    }
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
        let corridor = |phase: Phase, _mean: Option<f64>| match phase {
            Phase::Vegetative => Some((20.0, 28.0)),
            Phase::Flower => Some((18.0, 26.0)),
            _ => None,
        };
        let bands = build_bands(from, to, &[], &spans, corridor);
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
            &[],
            &spans,
            |_, _| None,
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
            &[],
            &spans,
            |_, _| Some((10.0, 30.0)),
        );
        assert_eq!(bands, Vec::new());
    }

    #[test]
    fn bands_cover_the_window_from_a_span_that_started_before_it() {
        let spans = [span("2026-11-01T00:00:00Z", Phase::Germination)];
        let bands = build_bands(
            "2026-11-12T00:00:00Z",
            "2026-11-16T00:00:00Z",
            &[],
            &spans,
            |_, _| Some((21.0, 26.7)),
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
        let corridor = |_, _| Some((0.0, 1.0));
        assert_eq!(
            build_bands("garbage", "2026-11-16T00:00:00Z", &[], &spans, corridor),
            Vec::new()
        );
        assert_eq!(
            build_bands("2026-11-12T00:00:00Z", "garbage", &[], &spans, corridor),
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

#[cfg(test)]
mod light_timeline_tests {
    use super::{
        LightInterval, PhaseSpan, build_light_bands, parse_epoch, resolve_light_periods,
        schedule_light_periods, sensor_light_periods,
    };
    use crate::Phase;
    use crate::light_cycle::{ClockTime, LightCycle, PhaseHours};
    use crate::strain::PhotoperiodType;

    /// A cycle with one lights-on time and one phase's hours set.
    fn cycle(lights_on: ClockTime, hours: u8) -> LightCycle {
        LightCycle {
            lights_on: Some(lights_on),
            hours_on: PhaseHours {
                germination: Some(hours),
                ..PhaseHours::default()
            },
        }
    }

    fn span(start: &str, phase: Phase) -> PhaseSpan {
        PhaseSpan {
            start: start.to_owned(),
            phase,
        }
    }

    fn interval(start: &str, end: &str, on: bool) -> LightInterval {
        LightInterval {
            start: start.to_owned(),
            end: end.to_owned(),
            on,
        }
    }

    /// The on-intervals of a resolved timeline, as (start, end) epoch
    /// pairs — the readable shape for stepping assertions.
    fn on_pairs(periods: &[LightInterval]) -> Vec<(i64, i64)> {
        periods
            .iter()
            .filter(|period| period.on)
            .map(|period| {
                (
                    parse_epoch(&period.start).unwrap(),
                    parse_epoch(&period.end).unwrap(),
                )
            })
            .collect()
    }

    fn epochs(texts: &[&str]) -> Vec<i64> {
        texts
            .iter()
            .map(|text| parse_epoch(text).unwrap())
            .collect()
    }

    #[test]
    fn schedule_periods_alternate_on_and_off_across_the_window() {
        // Worked example: Berlin in November is UTC+1, so a 08:00
        // lights-on with 12 h on runs 07:00Z–19:00Z. The off periods
        // fill everything else, clipped to the window.
        let spans = [span("2026-11-01T00:00:00Z", Phase::Germination)];
        let periods = schedule_light_periods(
            "2026-11-16T00:00:00Z",
            "2026-11-17T00:00:00Z",
            "Europe/Berlin",
            &spans,
            &cycle(ClockTime { hour: 8, minute: 0 }, 12),
            PhotoperiodType::Photoperiod,
        );
        assert_eq!(
            on_pairs(&periods),
            [(
                parse_epoch("2026-11-16T07:00:00Z").unwrap(),
                parse_epoch("2026-11-16T19:00:00Z").unwrap()
            )]
        );
        // Three alternating periods: off, on, off — the off edges
        // exactly the window edges and the on edges.
        assert_eq!(periods.len(), 3);
        assert_eq!(
            epochs(&[&periods[0].start, &periods[0].end]),
            epochs(&["2026-11-16T00:00:00Z", "2026-11-16T07:00:00Z"])
        );
        assert!(!periods[0].on);
        assert_eq!(
            epochs(&[&periods[2].start, &periods[2].end]),
            epochs(&["2026-11-16T19:00:00Z", "2026-11-17T00:00:00Z"])
        );
        assert!(!periods[2].on);
    }

    #[test]
    fn schedule_periods_wrap_midnight_and_reach_back_one_day() {
        // 23:00 on with 4 h on spans midnight: the previous day's
        // on-period must still cover the window's first hours.
        let spans = [span("2026-11-01T00:00:00Z", Phase::Germination)];
        let periods = schedule_light_periods(
            "2026-11-16T00:00:00Z",
            "2026-11-17T00:00:00Z",
            "Europe/Berlin",
            &spans,
            &cycle(
                ClockTime {
                    hour: 23,
                    minute: 0,
                },
                4,
            ),
            PhotoperiodType::Photoperiod,
        );
        assert_eq!(
            on_pairs(&periods),
            [
                (
                    parse_epoch("2026-11-16T00:00:00Z").unwrap(),
                    parse_epoch("2026-11-16T02:00:00Z").unwrap()
                ),
                (
                    parse_epoch("2026-11-16T22:00:00Z").unwrap(),
                    parse_epoch("2026-11-17T00:00:00Z").unwrap()
                ),
            ]
        );
    }

    #[test]
    fn schedule_periods_follow_the_phase_in_effect() {
        // Veg 18 h until 2026-11-17 local midnight, then flower 12 h:
        // the on-length changes at the phase boundary, not the
        // calendar, and both phases anchor at the same local on-time.
        let spans = [
            span("2026-11-14T23:00:00Z", Phase::Vegetative), // Nov 15 00:00 local
            span("2026-11-16T23:00:00Z", Phase::Flower),     // Nov 17 00:00 local
        ];
        let cycle = LightCycle {
            lights_on: Some(ClockTime { hour: 6, minute: 0 }),
            hours_on: PhaseHours {
                vegetative: Some(18),
                flower: Some(12),
                ..PhaseHours::default()
            },
        };
        let periods = schedule_light_periods(
            "2026-11-16T00:00:00Z",
            "2026-11-17T23:00:00Z",
            "Europe/Berlin",
            &spans,
            &cycle,
            PhotoperiodType::Photoperiod,
        );
        assert_eq!(
            on_pairs(&periods),
            [
                (
                    parse_epoch("2026-11-16T05:00:00Z").unwrap(),
                    parse_epoch("2026-11-16T23:00:00Z").unwrap()
                ),
                (
                    parse_epoch("2026-11-17T05:00:00Z").unwrap(),
                    parse_epoch("2026-11-17T17:00:00Z").unwrap()
                ),
            ]
        );
    }

    #[test]
    fn unlit_phases_and_spanless_windows_get_no_schedule() {
        let drying = [span("2026-11-01T00:00:00Z", Phase::Drying)];
        assert_eq!(
            schedule_light_periods(
                "2026-11-16T00:00:00Z",
                "2026-11-17T00:00:00Z",
                "Europe/Berlin",
                &drying,
                &cycle(ClockTime { hour: 6, minute: 0 }, 12),
                PhotoperiodType::Photoperiod,
            ),
            Vec::new()
        );
        // A window entirely before the first phase span has no phase in
        // effect, hence no schedule.
        let early = [span("2026-12-01T00:00:00Z", Phase::Germination)];
        assert_eq!(
            schedule_light_periods(
                "2026-11-16T00:00:00Z",
                "2026-11-17T00:00:00Z",
                "Europe/Berlin",
                &early,
                &cycle(ClockTime { hour: 6, minute: 0 }, 12),
                PhotoperiodType::Photoperiod,
            ),
            Vec::new()
        );
    }

    #[test]
    fn schedule_periods_anchor_to_local_summer_time() {
        // Berlin 2026-06 is UTC+2: 06:00 local is 04:00Z.
        let spans = [span("2026-06-01T00:00:00Z", Phase::Germination)];
        let periods = schedule_light_periods(
            "2026-06-15T00:00:00Z",
            "2026-06-16T00:00:00Z",
            "Europe/Berlin",
            &spans,
            &cycle(ClockTime { hour: 6, minute: 0 }, 18),
            PhotoperiodType::Photoperiod,
        );
        assert_eq!(
            on_pairs(&periods),
            [(
                parse_epoch("2026-06-15T04:00:00Z").unwrap(),
                parse_epoch("2026-06-15T22:00:00Z").unwrap()
            )]
        );
    }

    #[test]
    fn schedule_periods_cross_the_dst_transition_on_local_time() {
        // Berlin springs forward 2026-03-29 (02:00 → 03:00): the
        // 06:00 anchor lands at 05:00Z on Saturday and 04:00Z on
        // Sunday — local wall-clock, absolute hours-on.
        let spans = [span("2026-03-01T00:00:00Z", Phase::Vegetative)];
        let cycle = LightCycle {
            lights_on: Some(ClockTime { hour: 6, minute: 0 }),
            hours_on: PhaseHours {
                vegetative: Some(18),
                ..PhaseHours::default()
            },
        };
        let periods = schedule_light_periods(
            "2026-03-28T00:00:00Z",
            "2026-03-30T00:00:00Z",
            "Europe/Berlin",
            &spans,
            &cycle,
            PhotoperiodType::Photoperiod,
        );
        assert_eq!(
            on_pairs(&periods),
            [
                (
                    parse_epoch("2026-03-28T05:00:00Z").unwrap(),
                    parse_epoch("2026-03-28T23:00:00Z").unwrap()
                ),
                (
                    parse_epoch("2026-03-29T04:00:00Z").unwrap(),
                    parse_epoch("2026-03-29T22:00:00Z").unwrap()
                ),
            ]
        );
    }

    #[test]
    fn schedule_periods_fail_closed_on_bad_zone_or_window() {
        let spans = [span("2026-11-01T00:00:00Z", Phase::Germination)];
        let cycle = cycle(ClockTime { hour: 6, minute: 0 }, 12);
        assert_eq!(
            schedule_light_periods(
                "2026-11-16T00:00:00Z",
                "2026-11-17T00:00:00Z",
                "Mars/Olympus",
                &spans,
                &cycle,
                PhotoperiodType::Photoperiod,
            ),
            Vec::new()
        );
        assert_eq!(
            schedule_light_periods(
                "garbage",
                "2026-11-17T00:00:00Z",
                "Europe/Berlin",
                &spans,
                &cycle,
                PhotoperiodType::Photoperiod,
            ),
            Vec::new()
        );
    }

    // --- sensor staircase ---

    #[test]
    fn sensor_periods_hold_each_reading_until_the_next() {
        // The staircase: off until the first reading (not covered —
        // the schedule fills that gap), then each reading's state
        // holds until the next, the last until the window end.
        let staircase = [
            (parse_epoch("2026-11-16T02:00:00Z").unwrap(), 1.0),
            (parse_epoch("2026-11-16T06:00:00Z").unwrap(), 0.0),
            (parse_epoch("2026-11-16T10:00:00Z").unwrap(), 1.0),
        ];
        let periods =
            sensor_light_periods("2026-11-16T00:00:00Z", "2026-11-16T12:00:00Z", &staircase);
        assert_eq!(
            periods,
            vec![
                interval("2026-11-16T02:00:00Z", "2026-11-16T06:00:00Z", true),
                interval("2026-11-16T06:00:00Z", "2026-11-16T10:00:00Z", false),
                interval("2026-11-16T10:00:00Z", "2026-11-16T12:00:00Z", true),
            ]
        );
    }

    #[test]
    fn sensor_periods_treat_any_positive_value_as_on() {
        // A dimmer at 42 % is a lit fixture; exactly zero is dark.
        let staircase = [
            (parse_epoch("2026-11-16T02:00:00Z").unwrap(), 42.0),
            (parse_epoch("2026-11-16T06:00:00Z").unwrap(), 0.0),
            (parse_epoch("2026-11-16T08:00:00Z").unwrap(), 0.0),
        ];
        let periods =
            sensor_light_periods("2026-11-16T00:00:00Z", "2026-11-16T12:00:00Z", &staircase);
        assert_eq!(
            periods,
            vec![
                interval("2026-11-16T02:00:00Z", "2026-11-16T06:00:00Z", true),
                interval("2026-11-16T06:00:00Z", "2026-11-16T12:00:00Z", false),
            ]
        );
    }

    #[test]
    fn sensor_periods_drop_unparseable_reads_and_empty_windows() {
        assert_eq!(
            sensor_light_periods("2026-11-16T00:00:00Z", "2026-11-16T12:00:00Z", &[]),
            Vec::new()
        );
        assert_eq!(
            sensor_light_periods("garbage", "2026-11-16T12:00:00Z", &[]),
            Vec::new()
        );
    }

    // --- precedence resolve ---

    #[test]
    fn resolve_prefers_the_sensor_and_fills_the_leading_gap_from_the_schedule() {
        let from = "2026-11-16T00:00:00Z";
        let to = "2026-11-16T12:00:00Z";
        let schedule = vec![
            interval(from, "2026-11-16T04:00:00Z", true),
            interval("2026-11-16T04:00:00Z", to, false),
        ];
        let sensor = vec![
            interval("2026-11-16T02:00:00Z", "2026-11-16T08:00:00Z", false),
            interval("2026-11-16T08:00:00Z", to, true),
        ];
        let resolved = resolve_light_periods(from, to, &schedule, &sensor).unwrap();
        // The schedule's on-period is clipped at the sensor's first
        // reading; from there the staircase owns the timeline.
        assert_eq!(
            resolved,
            vec![
                interval(from, "2026-11-16T02:00:00Z", true),
                interval("2026-11-16T02:00:00Z", "2026-11-16T08:00:00Z", false),
                interval("2026-11-16T08:00:00Z", to, true),
            ]
        );
    }

    #[test]
    fn resolve_falls_back_to_the_schedule_without_sensor_reads() {
        let from = "2026-11-16T00:00:00Z";
        let to = "2026-11-16T12:00:00Z";
        let schedule = vec![interval(from, to, true)];
        assert_eq!(
            resolve_light_periods(from, to, &schedule, &[]),
            Some(schedule)
        );
        // Neither schedule nor sensor: no light state at all.
        assert_eq!(resolve_light_periods(from, to, &[], &[]), None);
    }

    // --- stepped bands ---

    /// The test corridor: day and night ranges per light state, the
    /// lights-on range when the state is unknown (today's behaviour).
    fn corridor(phase: Phase, light_on: Option<bool>, _mean: Option<f64>) -> Option<(f64, f64)> {
        match (phase, light_on) {
            (Phase::Germination, Some(true) | None) => Some((21.1, 26.7)),
            (Phase::Germination, Some(false)) => Some((20.0, 23.3)),
            (Phase::Drying, _) => Some((15.6, 20.0)),
            _ => None,
        }
    }

    #[test]
    fn bands_step_between_day_and_night_at_the_light_changes() {
        let spans = [span("2026-11-01T00:00:00Z", Phase::Germination)];
        let light = [
            interval("2026-11-16T07:00:00Z", "2026-11-16T19:00:00Z", true),
            interval("2026-11-16T19:00:00Z", "2026-11-17T00:00:00Z", false),
            interval("2026-11-16T00:00:00Z", "2026-11-16T07:00:00Z", false),
        ];
        let bands = build_light_bands(
            "2026-11-16T00:00:00Z",
            "2026-11-17T00:00:00Z",
            &[],
            &spans,
            Some(&light),
            corridor,
        );
        assert_eq!(bands.len(), 3);
        assert_eq!(
            epochs(&[&bands[0].start, &bands[0].end]),
            epochs(&["2026-11-16T00:00:00Z", "2026-11-16T07:00:00Z"])
        );
        assert_eq!(
            (bands[0].min, bands[0].max, bands[0].light_on),
            (20.0, 23.3, Some(false))
        );
        assert_eq!(
            epochs(&[&bands[1].start, &bands[1].end]),
            epochs(&["2026-11-16T07:00:00Z", "2026-11-16T19:00:00Z"])
        );
        assert_eq!(
            (bands[1].min, bands[1].max, bands[1].light_on),
            (21.1, 26.7, Some(true))
        );
        assert_eq!(
            epochs(&[&bands[2].start, &bands[2].end]),
            epochs(&["2026-11-16T19:00:00Z", "2026-11-17T00:00:00Z"])
        );
        assert_eq!(bands[2].light_on, Some(false));
        // Segments butt-join with no gap.
        assert_eq!(bands[0].end, bands[1].start);
        assert_eq!(bands[1].end, bands[2].start);
    }

    #[test]
    fn bands_merge_adjacent_segments_that_agree_on_range_phase_and_light() {
        let spans = [span("2026-11-01T00:00:00Z", Phase::Germination)];
        // Two adjacent on-intervals draw as one band; the off pieces
        // around them cannot merge across the on stretch.
        let light = [
            interval("2026-11-16T07:00:00Z", "2026-11-16T09:00:00Z", true),
            interval("2026-11-16T09:00:00Z", "2026-11-16T19:00:00Z", true),
            interval("2026-11-16T19:00:00Z", "2026-11-17T00:00:00Z", false),
            interval("2026-11-16T00:00:00Z", "2026-11-16T07:00:00Z", false),
        ];
        let bands = build_light_bands(
            "2026-11-16T00:00:00Z",
            "2026-11-17T00:00:00Z",
            &[],
            &spans,
            Some(&light),
            corridor,
        );
        assert_eq!(bands.len(), 3, "the identical on neighbours merge");
        assert_eq!(
            epochs(&[&bands[1].start, &bands[1].end]),
            epochs(&["2026-11-16T07:00:00Z", "2026-11-16T19:00:00Z"]),
            "one band across both on-intervals"
        );
        assert_eq!(bands[0].light_on, Some(false));
        assert_eq!(bands[1].light_on, Some(true));
        assert_eq!(bands[2].light_on, Some(false));
    }

    #[test]
    fn unlit_phases_stay_single_band_despite_light_intervals() {
        let spans = [span("2026-11-01T00:00:00Z", Phase::Drying)];
        let light = [interval(
            "2026-11-16T00:00:00Z",
            "2026-11-17T00:00:00Z",
            true,
        )];
        let bands = build_light_bands(
            "2026-11-16T00:00:00Z",
            "2026-11-17T00:00:00Z",
            &[],
            &spans,
            Some(&light),
            corridor,
        );
        assert_eq!(bands.len(), 1);
        assert_eq!(bands[0].light_on, None, "drying never splits");
        assert_eq!((bands[0].min, bands[0].max), (15.6, 20.0));
    }

    #[test]
    fn bands_without_a_light_timeline_stay_single() {
        let spans = [span("2026-11-01T00:00:00Z", Phase::Germination)];
        let bands = build_light_bands(
            "2026-11-16T00:00:00Z",
            "2026-11-17T00:00:00Z",
            &[],
            &spans,
            None,
            corridor,
        );
        assert_eq!(bands.len(), 1);
        assert_eq!(bands[0].light_on, None);
        assert_eq!((bands[0].min, bands[0].max), (21.1, 26.7));
    }

    #[test]
    fn uncovered_regions_of_a_lit_phase_carry_no_light_state() {
        // Light intervals cover only the middle of the day: the edges
        // are unknown (no schedule filled them), so they keep the
        // lights-on band.
        let spans = [span("2026-11-01T00:00:00Z", Phase::Germination)];
        let light = [interval(
            "2026-11-16T07:00:00Z",
            "2026-11-16T19:00:00Z",
            false,
        )];
        let bands = build_light_bands(
            "2026-11-16T00:00:00Z",
            "2026-11-17T00:00:00Z",
            &[],
            &spans,
            Some(&light),
            corridor,
        );
        assert_eq!(bands.len(), 3);
        assert_eq!(bands[0].light_on, None);
        assert_eq!(
            bands[0].max, 26.7,
            "unknown state keeps the lights-on range"
        );
        assert_eq!(bands[1].light_on, Some(false));
        assert_eq!(bands[2].light_on, None);
    }

    #[test]
    fn the_humidity_band_solves_at_the_segments_actual_mean_temperature() {
        // The points are *temperatures*: the warm day segment's RH
        // window differs from the cool night's, and each solves at its
        // own mean — never at a humidity reading fed as °C.
        let spans = [span("2026-11-01T00:00:00Z", Phase::Germination)];
        let light = [
            interval("2026-11-16T00:00:00Z", "2026-11-16T12:00:00Z", true),
            interval("2026-11-16T12:00:00Z", "2026-11-17T00:00:00Z", false),
        ];
        let temperatures = [
            (parse_epoch("2026-11-16T06:00:00Z").unwrap(), 26.0),
            (parse_epoch("2026-11-16T18:00:00Z").unwrap(), 14.0),
        ];
        let band = |phase: Phase, light_on: Option<bool>, mean: Option<f64>| {
            let _ = phase;
            let offset = match light_on {
                Some(false) => crate::env::NIGHT_LEAF_OFFSET_C,
                _ => crate::env::DEFAULT_LEAF_OFFSET_C,
            };
            let Some(mean) = mean else {
                return Some((0.0, 100.0));
            };
            let (lo, hi) = crate::env::rh_window(mean, offset, (0.4, 0.8));
            Some((lo, hi))
        };
        let bands = build_light_bands(
            "2026-11-16T00:00:00Z",
            "2026-11-17T00:00:00Z",
            &temperatures,
            &spans,
            Some(&light),
            band,
        );
        assert_eq!(bands.len(), 2);
        let (day_lo, day_hi) = (bands[0].min, bands[0].max);
        let (night_lo, night_hi) = (bands[1].min, bands[1].max);
        // The same VPD band at 26 °C solves to a higher RH window than
        // at 14 °C (cooler air holds less vapour): the segment's
        // temperature, not a humidity reading, drove both.
        assert!(day_hi > night_hi, "day {day_hi} > night {night_hi}");
        assert!(day_lo > night_lo, "day {day_lo} > night {night_lo}");
        assert!((0.0..=100.0).contains(&day_lo) && (0.0..=100.0).contains(&day_hi));
    }

    #[test]
    fn bands_stay_inside_the_window_and_fail_closed() {
        let spans = [span("2026-11-01T00:00:00Z", Phase::Germination)];
        let light = [interval(
            "2026-11-01T00:00:00Z",
            "2026-12-01T00:00:00Z",
            true,
        )];
        let bands = build_light_bands(
            "2026-11-16T00:00:00Z",
            "2026-11-17T00:00:00Z",
            &[],
            &spans,
            Some(&light),
            corridor,
        );
        assert_eq!(bands.len(), 1);
        assert_eq!(
            epochs(&[&bands[0].start, &bands[0].end]),
            epochs(&["2026-11-16T00:00:00Z", "2026-11-17T00:00:00Z"]),
            "an interval spanning the window clips to it"
        );
        // Unparseable window edges yield no bands (fail closed).
        assert_eq!(
            build_light_bands(
                "nope",
                "2026-11-17T00:00:00Z",
                &[],
                &spans,
                Some(&light),
                corridor
            ),
            Vec::new()
        );
    }
}

#[cfg(test)]
mod vpd_derive_tests {
    use super::{
        ChartWindow, PointSource, RawPoint, SeriesPoint, VPD_PAIRING_TOLERANCE_SECS,
        derive_vpd_series, merge_series, parse_epoch,
    };

    const NOW: &str = "2026-11-16T12:00:00Z";

    fn point(at: &str, value: f64) -> RawPoint {
        RawPoint {
            at: at.to_owned(),
            value,
        }
    }

    #[test]
    fn vpd_pairs_within_the_tolerance_and_drops_strays() {
        let temp_series = merge_series(
            NOW,
            ChartWindow::Day,
            &[
                point("2026-11-15T18:00:00Z", 24.0),
                point("2026-11-15T18:10:00Z", 25.0), // pairs with 18:08 RH
                point("2026-11-15T18:20:00Z", 26.0), // no RH within 5 min: dropped
            ],
            &[],
        );
        let rh_series = merge_series(
            NOW,
            ChartWindow::Day,
            &[
                point("2026-11-15T17:56:00Z", 60.0), // 4 min from 18:00: pairs
                point("2026-11-15T18:08:00Z", 50.0),
            ],
            &[],
        );
        let derived = derive_vpd_series(&temp_series, &rh_series, -2.0);
        assert_eq!(derived.points.len(), 2);
        assert_eq!(derived.points[0].source, PointSource::Derived);
        assert!(derived.points[0].value > 0.0);
        assert!(
            derived.points[1].value > derived.points[0].value,
            "warmer and drier is a higher vpd"
        );
    }

    #[test]
    fn vpd_is_zero_at_saturation() {
        // 100 % RH with a zero leaf offset saturates the air: VPD 0.
        let temp_series = merge_series(
            NOW,
            ChartWindow::Day,
            &[point("2026-11-15T18:00:00Z", 20.0)],
            &[],
        );
        let rh_series = merge_series(
            NOW,
            ChartWindow::Day,
            &[point("2026-11-15T18:00:00Z", 100.0)],
            &[],
        );
        let derived = derive_vpd_series(&temp_series, &rh_series, 0.0);
        assert_eq!(derived.points.len(), 1);
        assert_eq!(derived.points[0].value, 0.0);

        // With a negative leaf offset the raw difference goes below
        // zero (the leaf's saturation pressure sits under the air's
        // actual vapour pressure); the derivation clamps at zero so no
        // consumer can see a negative VPD.
        let offsets = [
            crate::env::DEFAULT_LEAF_OFFSET_C,
            crate::env::NIGHT_LEAF_OFFSET_C,
        ];
        for offset in offsets {
            let derived = derive_vpd_series(&temp_series, &rh_series, offset);
            assert_eq!(derived.points.len(), 1);
            assert_eq!(
                derived.points[0].value, 0.0,
                "saturation at offset {offset} must clamp at zero"
            );
        }
    }

    #[test]
    fn window_bounds_copy_the_temperature_series() {
        let temp_series = merge_series(
            NOW,
            ChartWindow::Day,
            &[point("2026-11-15T18:00:00Z", 24.0)],
            &[],
        );
        let rh_series = merge_series(
            NOW,
            ChartWindow::Day,
            &[point("2026-11-15T18:00:00Z", 60.0)],
            &[],
        );
        let derived = derive_vpd_series(&temp_series, &rh_series, -2.0);
        assert_eq!(parse_epoch(&derived.from), parse_epoch(&temp_series.from));
        assert_eq!(parse_epoch(&derived.to), parse_epoch(&temp_series.to));
    }

    #[test]
    fn pairing_tolerance_is_five_minutes() {
        assert_eq!(VPD_PAIRING_TOLERANCE_SECS, 300);
    }

    // Silence the unused-import lint when SeriesPoint is only
    // constructed inside the implementation.
    #[allow(dead_code)]
    fn shape_check(_: SeriesPoint) {}
}
