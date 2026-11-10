//! Environment-alert routing (spec §4.9).
//!
//! Under an explicit opt-in, out-of-band temperature and VPD readings
//! raise a Home Assistant notification — rate-limited to one per grow
//! and alert kind per hour, and never during quiet hours.

use std::collections::HashMap;
use std::sync::Arc;

use leaflog_core::tz::QuietHours;
use tokio::time::Instant;

use crate::ha::HaBridge;
use crate::store::settings;

/// Everything one evaluation pass needs from the database, gathered in
/// a single blocking visit.
struct Pass {
    /// Canonical timezone name.
    tz: String,
    /// Whether the opt-in setting is on.
    enabled: bool,
    /// The quiet-hours window, when configured.
    quiet: Option<QuietHours>,
    /// (grow id, grow name, verdicts) of every active grow.
    grows: Vec<(String, String, Verdict)>,
}

/// Whole minutes, in the only unit std's `Duration` constructors
/// speak.
const fn minutes(count: u64) -> std::time::Duration {
    std::time::Duration::from_secs(count * 60)
}

/// The opt-in setting (spec §4.2/§4.9): routing is off unless this is
/// `true`.
const OPT_IN_SETTING: &str = "env_alert_notifications";

/// Quiet hours, `HH:MM-HH:MM` local wall clock (unset: never quiet).
const QUIET_HOURS_SETTING: &str = "quiet_hours";

/// One evaluation pass's verdicts for a grow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Verdict {
    /// Whether the temperature is out of band (`None` = no data).
    pub temp_out: Option<bool>,
    /// Whether the VPD is out of band (`None` = no data).
    pub vpd_out: Option<bool>,
}

impl Verdict {
    /// The alert kinds due: temperature and/or vpd.
    #[must_use]
    pub fn kinds(&self) -> Vec<&'static str> {
        let mut kinds = Vec::new();
        if self.temp_out == Some(false) {
            kinds.push("temperature");
        }
        if self.vpd_out == Some(false) {
            kinds.push("vpd");
        }
        kinds
    }
}

/// The send gate: rate limit (one per grow and kind per hour) and the
/// quiet-hours window on the grower's local wall clock. `None` wall
/// minutes (unknown zone) never blocks.
#[must_use]
pub fn should_send(
    last_sent: Option<Instant>,
    now: Instant,
    quiet: Option<QuietHours>,
    wall_minutes: Option<u32>,
) -> bool {
    if let Some(last) = last_sent
        && now.duration_since(last) < std::time::Duration::from_secs(3600)
    {
        return false;
    }
    !quiet.is_some_and(|quiet| wall_minutes.is_some_and(|minutes| quiet.contains(minutes)))
}

/// Parses the `HH:MM-HH:MM` quiet-hours setting.
#[must_use]
pub fn parse_quiet_hours(text: &str) -> Option<QuietHours> {
    let (start, end) = text.split_once('-')?;
    Some(QuietHours {
        start_minutes: leaflog_core::tz::parse_hhmm(start.trim())?,
        end_minutes: leaflog_core::tz::parse_hhmm(end.trim())?,
    })
}

/// Spawns the evaluator.
///
/// Every minute it checks each active grow's environment and routes
/// out-of-band verdicts to Home Assistant under the opt-in.
/// Rate-limit state is in memory (a restart resets it, which still
/// honours "at most one per hour").
pub fn spawn(db: crate::db::Db, bridge: Arc<HaBridge>) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(minutes(1));
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut last_sent: HashMap<String, Instant> = HashMap::new();
        loop {
            ticker.tick().await;
            if let Err(error) = evaluate(&db, &bridge, &mut last_sent).await {
                eprintln!("env-alert pass skipped: {error}");
            }
        }
    });
}

/// One evaluation pass; separated from [`spawn`] so tests can drive it
/// with their own rate-limit map.
///
/// # Errors
///
/// Returns a [`crate::db::StoreError`] when the database read fails;
/// notification failures are logged and never abort the pass.
pub async fn evaluate<S: std::hash::BuildHasher>(
    db: &crate::db::Db,
    bridge: &Arc<HaBridge>,
    last_sent: &mut std::collections::HashMap<String, Instant, S>,
) -> Result<(), crate::db::StoreError> {
    let now = Instant::now();
    let bridge_handle = Arc::clone(bridge);
    let pass = db
        .with(move |conn| -> Result<Pass, crate::db::StoreError> {
            let enabled = settings::get(conn, OPT_IN_SETTING)
                .ok()
                .and_then(|value| value.as_bool())
                .unwrap_or(false);
            let tz = settings::get(conn, "display_timezone")
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_else(|| {
                    bridge_handle
                        .cached_timezone()
                        .unwrap_or_else(|| leaflog_core::tz::UTC.to_owned())
                });
            let quiet = settings::get(conn, QUIET_HOURS_SETTING)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .and_then(|text| parse_quiet_hours(&text));
            let live = bridge_handle.live_now();
            let mut grows = Vec::new();
            for grow in crate::store::grows::list(conn)? {
                if grow.status != leaflog_core::grows::GrowStatus::Active {
                    continue;
                }
                let Ok(snapshot) = crate::api_env::snapshot(conn, &grow.id, &live, &tz) else {
                    continue;
                };
                grows.push((
                    grow.id,
                    grow.name,
                    Verdict {
                        temp_out: snapshot.temp_ok,
                        vpd_out: snapshot.vpd_ok,
                    },
                ));
            }
            Ok(Pass {
                tz,
                enabled,
                quiet,
                grows,
            })
        })
        .await?;
    if !pass.enabled {
        return Ok(());
    }
    let wall_minutes = leaflog_core::tz::local_wall_minutes_now(&pass.tz);
    for (grow_id, grow_name, verdict) in pass.grows {
        for kind in verdict.kinds() {
            let key = format!("{grow_id}:{kind}");
            if !should_send(last_sent.get(&key).copied(), now, pass.quiet, wall_minutes) {
                continue;
            }
            let message =
                format!("Leaflog: {kind} out of band in \"{grow_name}\" — check the panel.");
            match bridge.notify("Leaflog environment", &message).await {
                Ok(()) => {
                    last_sent.insert(key, now);
                }
                Err(error) => eprintln!("env-alert notification skipped: {error}"),
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{QuietHours, Verdict, parse_quiet_hours, should_send};
    use std::time::Duration;
    use tokio::time::Instant;

    fn minutes_ago(count: u64) -> Instant {
        Instant::now() - Duration::from_secs(count * 60)
    }

    #[test]
    fn rate_limit_allows_one_per_hour_per_kind() {
        let now = Instant::now();
        assert!(should_send(None, now, None, None));
        assert!(!should_send(Some(minutes_ago(30)), now, None, None));
        assert!(should_send(Some(minutes_ago(61)), now, None, None));
    }

    #[test]
    fn quiet_hours_block_only_inside_the_window() {
        let night = Some(QuietHours {
            start_minutes: 22 * 60,
            end_minutes: 7 * 60,
        });
        let now = Instant::now();
        assert!(!should_send(None, now, night, Some(23 * 60)));
        assert!(!should_send(None, now, night, Some(2 * 60)));
        assert!(should_send(None, now, night, Some(12 * 60)));
        // Unknown wall clock never blocks.
        assert!(should_send(None, now, night, None));
    }

    #[test]
    fn verdicts_name_their_alert_kinds() {
        let hot_and_dry = Verdict {
            temp_out: Some(false),
            vpd_out: Some(false),
        };
        assert_eq!(hot_and_dry.kinds(), ["temperature", "vpd"]);
        let fine = Verdict {
            temp_out: Some(true),
            vpd_out: None,
        };
        assert!(fine.kinds().is_empty(), "in range or no data: no alert");
        assert_eq!(
            parse_quiet_hours("22:00 - 07:00"),
            Some(QuietHours {
                start_minutes: 1320,
                end_minutes: 420
            })
        );
        assert_eq!(parse_quiet_hours("whenever"), None);
    }
}
