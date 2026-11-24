//! Calendar-day diary pages and historical environment summaries.

use axum::extract::{Path, Query, State};
use axum::{Json, Router, routing::get};
use jiff::{Timestamp, ToSpan, civil::Date, tz::TimeZone};
use leaflog_core::grows::{ActionEntry, Photo};
use leaflog_core::story::{Period, StoryDay, StoryPage};
use leaflog_core::{
    actions::{ActionDetails, ActionType},
    env,
};
use serde::Deserialize;
use serde_json::Value;

use crate::{api::ApiError, app::AppState, store};

const PAGE_DAYS: i64 = 7;
// Do not carry a numeric sensor value through a long reporting gap.
const MAX_READING_GAP: i64 = 3600;
const ROLES: [&str; 3] = ["temperature", "humidity", "light"];

pub fn router() -> Router<AppState> {
    Router::new().route("/api/grows/{id}/story", get(story))
}

#[derive(Debug, Deserialize)]
struct StoryQuery {
    before: Option<String>,
}

#[derive(Debug)]
struct LocalHistory {
    actions: Vec<ActionEntry>,
    photos: Vec<Photo>,
    bindings: store::sensors::Bindings,
    snapshots: [Vec<store::sensors::SensorSnapshot>; 3],
}

async fn story(
    State(state): State<AppState>,
    Path(id): Path<String>,
    Query(query): Query<StoryQuery>,
) -> Result<Json<StoryPage>, ApiError> {
    let timezone = crate::app::canonical_timezone(&state).await;
    let zone = TimeZone::get(&timezone).unwrap_or(TimeZone::UTC);
    let query_zone = zone.clone();
    let query_id = id.clone();
    let earliest = state
        .db
        .with(move |conn| -> Result<Option<Date>, ApiError> {
            // A seven-day page must know about older records; the
            // aggregates walk the grow's time indexes, not every row.
            let action = store::actions::earliest_timestamp(conn, &query_id)?
                .and_then(|at| local_date(&at, &query_zone));
            let photo = store::photos::earliest_timestamp(conn, &query_id)?
                .and_then(|at| local_date(&at, &query_zone));
            Ok(action.into_iter().chain(photo).min())
        })
        .await?;
    let (start, end, now) = page_window(&query, &zone)?;
    let from = midnight(start, &zone)?;
    let to = midnight(end, &zone)?;
    let (grow, windows, local) = state
        .db
        .with(move |conn| -> Result<_, ApiError> {
            let (from_s, to_s) = (from.to_string(), to.to_string());
            let grow = store::grows::get(conn, &id)?;
            let windows = crate::api_chart::load_windows(conn, &grow)?;
            let local = LocalHistory {
                actions: store::actions::list_for_story(conn, &id, from, to)?,
                photos: store::photos::list_in_window(conn, &id, from, to)?,
                bindings: store::sensors::list(conn, &id)?,
                snapshots: [
                    store::sensors::snapshots_for_role_between(
                        conn, &id, ROLES[0], &from_s, &to_s,
                    )?,
                    store::sensors::snapshots_for_role_between(
                        conn, &id, ROLES[1], &from_s, &to_s,
                    )?,
                    store::sensors::snapshots_for_role_between(
                        conn, &id, ROLES[2], &from_s, &to_s,
                    )?,
                ],
            };
            Ok((grow, windows, local))
        })
        .await?;
    let started: Date = grow.started_on.parse().map_err(invalid_date)?;
    // Backdated records can predate the grow anchor. Include their
    // calendar days, but keep the first displayed grow day at zero.
    let earliest = local
        .actions
        .iter()
        .filter_map(|entry| local_date(&entry.occurred_at, &zone))
        .chain(
            local
                .photos
                .iter()
                .filter_map(|photo| local_date(&photo.captured_at, &zone)),
        )
        .min()
        .map_or(started, |date| date.min(started))
        .min(earliest.unwrap_or(started));
    let from_ha = from.max(midnight(started, &zone)?);
    let history = fetch_window(&state, &local, from_ha, to.min(now)).await;
    let mut days = Vec::new();
    let mut date = end;
    while date > start.max(earliest) {
        date = date.yesterday().map_err(invalid_date)?;
        let lo = midnight(date, &zone)?;
        let hi = midnight(date.tomorrow().map_err(invalid_date)?, &zone)?;
        let day_number = (date - started).get_days().max(0);
        // Diary membership is calendar-correct: an entry the grower
        // dated within this day belongs here even while its wall clock
        // is still ahead (phase picks stamp noon, D073). Only the
        // environment summary is clipped to now, because sensors
        // cannot observe the future.
        let (actions, automatic) = day_actions(&local.actions, lo, hi);
        let photos: Vec<_> = local
            .photos
            .iter()
            .filter(|photo| in_day(&photo.captured_at, lo, hi))
            .cloned()
            .collect();
        // Sensor weighting remains second-based; calendar membership does not.
        let summary = history.summary(lo.as_second(), hi.min(now).as_second());
        if actions.is_empty() && photos.is_empty() && !automatic && !summary.has_observations {
            continue;
        }
        days.push(StoryDay {
            date: date.to_string(),
            grow_day: u16::try_from(day_number).unwrap_or(u16::MAX),
            phase: crate::api_phase::phase_at(&windows, i64::from(day_number)),
            actions,
            photos,
            automatic,
            has_observations: summary.has_observations,
            light_hours: summary.light_hours,
            light_partial: summary.light_partial,
            periods: summary.periods,
        });
    }
    Ok(Json(StoryPage {
        timezone,
        days,
        before: (start > earliest).then(|| start.to_string()),
    }))
}

fn page_window(query: &StoryQuery, zone: &TimeZone) -> Result<(Date, Date, Timestamp), ApiError> {
    let now = Timestamp::now();
    let tomorrow = now
        .to_zoned(zone.clone())
        .date()
        .tomorrow()
        .map_err(invalid_date)?;
    let end = query
        .before
        .as_deref()
        .map(str::parse::<Date>)
        .transpose()
        .map_err(invalid_date)?
        .unwrap_or(tomorrow)
        .min(tomorrow);
    let start = end.checked_sub(PAGE_DAYS.days()).map_err(invalid_date)?;
    Ok((start, end, now))
}

fn invalid_date(_: jiff::Error) -> ApiError {
    ApiError::BadRequest("invalid diary date".to_owned())
}

fn midnight(date: Date, zone: &TimeZone) -> Result<Timestamp, ApiError> {
    date.at(0, 0, 0, 0)
        .to_zoned(zone.clone())
        .map(|zoned| zoned.timestamp())
        .map_err(invalid_date)
}

fn local_date(at: &str, zone: &TimeZone) -> Option<Date> {
    at.parse::<Timestamp>()
        .ok()
        .map(|at| at.to_zoned(zone.clone()).date())
}

fn in_day(at: &str, lo: Timestamp, hi: Timestamp) -> bool {
    at.parse::<Timestamp>()
        .is_ok_and(|at| (lo..hi).contains(&at))
}

fn day_actions(entries: &[ActionEntry], lo: Timestamp, hi: Timestamp) -> (Vec<ActionEntry>, bool) {
    let mut automatic = false;
    let actions = entries
        .iter()
        .filter(|entry| {
            if !in_day(&entry.occurred_at, lo, hi) {
                return false;
            }
            if matches!(
                entry.action_type,
                ActionType::LightChange | ActionType::Lights
            ) && matches!(
                entry.notes.as_deref(),
                Some("auto: light on" | "auto: light off")
            ) {
                automatic = true;
                return false;
            }
            entry.action_type != ActionType::Photo
                || entry
                    .notes
                    .as_ref()
                    .is_some_and(|note| !note.trim().is_empty())
                || entry.details != ActionDetails::default()
                || !entry.appearance_tags.is_empty()
        })
        .cloned()
        .collect();
    (actions, automatic)
}

#[derive(Debug, Default)]
struct History {
    recorded: [Vec<Reading>; 3],
    snapshots: [Vec<Reading>; 3],
}

#[derive(Debug, Clone, Copy)]
struct Reading {
    at: i64,
    value: Option<f64>,
}

async fn fetch_window(
    state: &AppState,
    local: &LocalHistory,
    from: Timestamp,
    to: Timestamp,
) -> History {
    let fetch = |role: &'static str| async move {
        let Some(entity) = local.bindings.get(role).filter(|_| from < to) else {
            return Vec::new();
        };
        let from = from.to_string();
        let to = to.to_string();
        let response = tokio::time::timeout(
            std::time::Duration::from_secs(4),
            state.ha.history_between(entity, &from, Some(&to)),
        )
        .await;
        response
            .ok()
            .and_then(Result::ok)
            .map_or_else(Vec::new, |history| history_readings(role, &history))
    };
    let (temperature, humidity, light) =
        tokio::join!(fetch(ROLES[0]), fetch(ROLES[1]), fetch(ROLES[2]));
    let snapshots = std::array::from_fn(|i| {
        local.snapshots[i]
            .iter()
            .filter_map(|snap| {
                let at = leaflog_core::chart::parse_epoch(&snap.captured_at)?;
                (from.as_second()..to.as_second())
                    .contains(&at)
                    .then_some(())?;
                // A sensor's current unit may differ from its unit at capture time.
                let unit = snap.unit.as_deref();
                if ROLES[i] == "temperature" && unit.is_none() {
                    return None;
                }
                Some(Reading {
                    at,
                    value: reading_value(ROLES[i], &snap.state, unit),
                })
            })
            .collect()
    });
    History {
        recorded: (temperature, humidity, light).into(),
        snapshots,
    }
}

fn history_readings(role: &str, history: &Value) -> Vec<Reading> {
    let mut readings = Vec::new();
    for states in history
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(Value::as_array)
    {
        let mut unit = None;
        for state in states {
            unit = state
                .pointer("/attributes/unit_of_measurement")
                .and_then(Value::as_str)
                .or(unit);
            let at = ["lu", "last_changed", "last_updated"]
                .iter()
                .find_map(|key| {
                    state.get(*key).and_then(|value| {
                        value
                            .as_str()
                            .and_then(leaflog_core::chart::parse_epoch)
                            .or_else(|| {
                                value.as_f64().and_then(leaflog_core::chart::parse_ha_epoch)
                            })
                    })
                });
            if let Some(at) = at {
                let value = state
                    .get("state")
                    .or_else(|| state.get("s"))
                    .and_then(Value::as_str)
                    .and_then(|text| reading_value(role, text, unit));
                readings.push(Reading { at, value });
            }
        }
    }
    readings.sort_by_key(|reading| reading.at);
    readings
}

fn reading_value(role: &str, text: &str, unit: Option<&str>) -> Option<f64> {
    if role == "light" {
        return match text {
            "on" | "1" => Some(1.0),
            "off" | "0" => Some(0.0),
            _ => None,
        };
    }
    let value = text.parse::<f64>().ok().filter(|value| value.is_finite())?;
    match (role, unit) {
        ("temperature", _) => leaflog_core::units::sensor_celsius(value, unit),
        ("humidity", _) if (0.0..=100.0).contains(&value) => Some(value),
        _ => None,
    }
}

#[derive(Debug, Default, Clone, Copy)]
struct Summary {
    has_observations: bool,
    light_hours: Option<f64>,
    light_partial: bool,
    periods: [Period; 2],
}

fn current(readings: &[Reading], at: i64, max_age: i64) -> Option<f64> {
    let index = readings
        .partition_point(|reading| reading.at <= at)
        .checked_sub(1)?;
    let reading = readings[index];
    (at - reading.at < max_age)
        .then_some(reading.value)
        .flatten()
}

fn metrics(temp: Option<f64>, rh: Option<f64>, on: bool) -> [Option<f64>; 3] {
    let leaf_offset = if on {
        env::DEFAULT_LEAF_OFFSET_C
    } else {
        env::NIGHT_LEAF_OFFSET_C
    };
    let vpd = temp
        .zip(rh)
        .map(|(t, rh)| env::vpd_kpa(t, rh, leaf_offset))
        .filter(|vpd| vpd.is_finite());
    [temp, rh, vpd]
}

const fn seconds(value: i64) -> f64 {
    // No underflow/overflow: summary spans are bounded by one civil day.
    #[allow(
        clippy::cast_precision_loss,
        reason = "a day in seconds is exactly representable"
    )]
    {
        value as f64
    }
}

impl History {
    fn summary(&self, lo: i64, hi: i64) -> Summary {
        if hi <= lo {
            return Summary::default();
        }
        let [temperature, humidity, light] = &self.recorded;
        let mut cuts = vec![lo, hi];
        for readings in &self.recorded {
            for reading in readings {
                if (lo..hi).contains(&reading.at) {
                    cuts.push(reading.at);
                }
                if (lo..hi).contains(&(reading.at + MAX_READING_GAP)) {
                    cuts.push(reading.at + MAX_READING_GAP);
                }
            }
        }
        cuts.sort_unstable();
        cuts.dedup();
        let mut light_time = [0; 2];
        let mut weights = [[0; 3]; 2];
        let mut sums = [[0.0; 3]; 2];
        let mut has_observations = false;
        for pair in cuts.windows(2) {
            let at = pair[0];
            let duration = pair[1] - at;
            let temp = current(temperature, at, MAX_READING_GAP);
            let rh = current(humidity, at, MAX_READING_GAP);
            let light = current(light, at, i64::MAX);
            has_observations |= temp.is_some() || rh.is_some() || light.is_some();
            if let Some(light) = light {
                let on = light > 0.5;
                let period = usize::from(!on);
                light_time[period] += duration;
                has_observations = true;
                for (metric, value) in metrics(temp, rh, on).into_iter().enumerate() {
                    if let Some(value) = value {
                        weights[period][metric] += duration;
                        sums[period][metric] =
                            f64::mul_add(value, seconds(duration), sums[period][metric]);
                    }
                }
            }
        }
        let [snap_temp, snap_rh, snap_light] = &self.snapshots;
        let mut spots = [[(0.0, 0_u32); 3]; 2];
        let mut times: Vec<_> = self
            .snapshots
            .iter()
            .flatten()
            .filter(|reading| (lo..hi).contains(&reading.at))
            .map(|reading| reading.at)
            .collect();
        times.sort_unstable();
        times.dedup();
        has_observations |= !times.is_empty();
        for at in times {
            let Some(light) = current(light, at, i64::MAX).or_else(|| current(snap_light, at, 1))
            else {
                continue;
            };
            let on = light > 0.5;
            let period = usize::from(!on);
            for (i, value) in metrics(current(snap_temp, at, 1), current(snap_rh, at, 1), on)
                .into_iter()
                .enumerate()
            {
                if let Some(value) = value {
                    spots[period][i].0 += value;
                    spots[period][i].1 += 1;
                }
            }
        }
        let periods = std::array::from_fn(|p| {
            let avg = |i: usize| {
                if weights[p][i] > 0 {
                    Some(sums[p][i] / seconds(weights[p][i]))
                } else {
                    let (sum, count) = spots[p][i];
                    (count > 0).then(|| sum / f64::from(count))
                }
            };
            Period {
                temperature: avg(0),
                humidity: avg(1),
                vpd: avg(2),
                sparse: (0..3).any(|i| {
                    weights[p][i] < light_time[p] || (weights[p][i] == 0 && spots[p][i].1 > 0)
                }),
            }
        });
        let observed = light_time.iter().sum::<i64>();
        Summary {
            has_observations,
            light_hours: (observed > 0).then(|| seconds(light_time[0]) / 3600.0),
            light_partial: observed < hi - lo,
            periods,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(second: i64, value: f64) -> Reading {
        Reading {
            at: second,
            value: Some(value),
        }
    }

    fn light(second: i64, on: bool) -> Reading {
        at(second, if on { 1.0 } else { 0.0 })
    }

    #[test]
    fn history_parses_unavailable_and_converts_fahrenheit() {
        let readings = history_readings(
            "temperature",
            &serde_json::json!([[{
                "last_updated": "1970-01-01T00:00:00+00:00", "state": "68",
                "attributes": {"unit_of_measurement": "°F"}
            }, {
                "last_updated": "1970-01-01T01:00:00+00:00", "state": "unavailable"
            }, {
                "last_updated": "1970-01-01T02:00:00+00:00", "state": "70"
            }]]),
        );
        assert_eq!(readings[0].value, Some(20.0));
        assert_eq!(readings[1].value, None);
        assert!((readings[2].value.unwrap_or_default() - 21.111).abs() < 0.01);
    }

    #[test]
    fn history_parses_last_changed_and_light_states() {
        let readings = history_readings(
            "light",
            &serde_json::json!([[{
                "last_changed": "1970-01-01T00:00:00+00:00", "state": "on"
            }, {
                "last_changed": "1970-01-01T01:00:00+00:00", "state": "unknown"
            }]]),
        );
        assert_eq!(readings[0].value, Some(1.0));
        assert_eq!(readings[1].value, None);
    }

    #[test]
    fn minimal_response_numeric_timestamps_are_unix_seconds() {
        let readings = history_readings(
            "temperature",
            &serde_json::json!([[{"lu": 3600, "s": "20.0",
                "attributes": {"unit_of_measurement": "°C"}}]]),
        );
        assert_eq!(readings.len(), 1);
        assert_eq!(readings[0].at, 3600);
        assert_eq!(readings[0].value, Some(20.0));
    }

    #[test]
    fn averages_are_weighted_by_observed_time() {
        let history = History {
            recorded: [
                vec![at(0, 10.0), at(3600, 30.0)],
                vec![at(0, 50.0), at(3600, 70.0)],
                vec![light(0, true), light(7200, false)],
            ],
            snapshots: std::array::from_fn(|_| Vec::new()),
        };
        let summary = history.summary(0, 7200);
        assert!((summary.periods[0].temperature.unwrap_or_default() - 20.0).abs() < 0.001);
        assert!((summary.periods[0].humidity.unwrap_or_default() - 60.0).abs() < 0.001);
        assert_eq!(summary.light_hours, Some(2.0));
        assert!(!summary.light_partial);
        assert!(!summary.periods[0].sparse);
        assert!(!summary.periods[1].sparse);
    }

    #[test]
    fn missing_metric_does_not_zero_or_contaminate_other_metric() {
        let history = History {
            recorded: [
                vec![
                    at(0, 20.0),
                    Reading {
                        at: 1800,
                        value: None,
                    },
                    at(3600, 40.0),
                ],
                vec![at(0, 50.0)],
                vec![light(0, true)],
            ],
            snapshots: std::array::from_fn(|_| Vec::new()),
        };
        let summary = history.summary(0, 3600);
        assert_eq!(summary.periods[0].humidity, Some(50.0));
        assert_eq!(summary.periods[0].temperature, Some(20.0));
        assert!(summary.periods[0].sparse);
    }

    #[test]
    fn explicit_gap_stops_averages_after_one_hour() {
        let history = History {
            recorded: [
                vec![
                    at(0, 20.0),
                    Reading {
                        at: 1800,
                        value: None,
                    },
                ],
                vec![
                    at(0, 50.0),
                    Reading {
                        at: 1800,
                        value: None,
                    },
                ],
                vec![light(0, true), light(7200, false)],
            ],
            snapshots: std::array::from_fn(|_| Vec::new()),
        };
        let summary = history.summary(0, 7200);
        assert_eq!(summary.periods[0].temperature, Some(20.0));
        assert_eq!(summary.periods[0].humidity, Some(50.0));
        assert!(summary.periods[0].sparse);
        assert!(!summary.periods[1].sparse);
    }

    #[test]
    fn day_boundary_observes_prior_light_without_extending_sensor_values() {
        let history = History {
            recorded: [
                vec![at(-3601, 20.0)],
                vec![at(-3601, 50.0)],
                vec![light(-3600, true)],
            ],
            snapshots: std::array::from_fn(|_| Vec::new()),
        };
        let summary = history.summary(0, 3600);
        assert_eq!(summary.periods[0].temperature, None);
        assert_eq!(summary.periods[0].humidity, None);
        assert_eq!(summary.light_hours, Some(1.0));
        assert!(summary.has_observations);
    }

    #[test]
    fn snapshots_are_spot_values_not_all_day_observations() {
        let history = History {
            recorded: [
                Vec::new(),
                Vec::new(),
                vec![
                    light(0, true),
                    Reading {
                        at: 1800,
                        value: None,
                    },
                ],
            ],
            snapshots: [vec![at(60, 20.0)], vec![at(60, 50.0)], Vec::new()],
        };
        let summary = history.summary(0, 3600);
        assert_eq!(summary.periods[0].temperature, Some(20.0));
        assert!(summary.periods[0].sparse);
        assert!(summary.periods[1].temperature.is_none());
        assert!(summary.light_partial);
    }

    #[test]
    fn an_unavailable_sensor_does_not_count_as_an_observation() {
        let history = History {
            recorded: [Vec::new(), Vec::new(), vec![Reading { at: 0, value: None }]],
            snapshots: [
                vec![Reading {
                    at: 60,
                    value: None,
                }],
                Vec::new(),
                Vec::new(),
            ],
        };
        let summary = history.summary(0, 3600);
        assert!(summary.has_observations);
        assert_eq!(summary.periods[0].temperature, None);
        assert!(!summary.periods[0].sparse);
    }

    #[test]
    fn hidden_light_entries_and_empty_photo_actions_are_grouped() {
        let entry = |kind, notes: Option<&str>| ActionEntry {
            id: "id".to_owned(),
            grow_id: "g".to_owned(),
            plant_ids: Vec::new(),
            occurred_at: "1970-01-01T00:10:00Z".to_owned(),
            action_type: kind,
            phase: None,
            grow_day: 0,
            details: ActionDetails::default(),
            notes: notes.map(str::to_owned),
            appearance_tags: Vec::new(),
            created_at: String::new(),
            updated_at: String::new(),
        };
        let (actions, automatic) = day_actions(
            &[
                entry(ActionType::LightChange, Some("auto: light on")),
                entry(ActionType::Photo, None),
                entry(ActionType::Photo, Some("new leaves")),
            ],
            "1970-01-01T00:00:00Z".parse().expect("valid UTC timestamp"),
            "1970-01-01T01:00:00Z".parse().expect("valid UTC timestamp"),
        );
        assert!(automatic);
        assert_eq!(actions.len(), 1);
    }
}
