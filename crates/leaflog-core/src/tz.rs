//! Canonical-timezone day arithmetic (spec §2).
//!
//! UTC is stored; Home Assistant's timezone (or the settings
//! override) is canonical for display and grow-day numbering, and day
//! numbers count **local calendar days** — DST-safe, never UTC day
//! flips.

use jiff::Zoned;
use serde::{Deserialize, Serialize};

/// The fallback zone when nothing better is known.
pub const UTC: &str = "UTC";

/// The grow day of a UTC instant, in local calendar days.
///
/// Whole days between the grow's local start date and the instant's
/// local date in `tz_name`; `None` when the date, the timestamp or the
/// zone name does not parse (callers fall back to UTC day math).
///
/// The start date is a local calendar date already (the grower picked
/// it off a calendar), so only the instant needs converting.
#[must_use]
pub fn day_number_local(started_on: &str, occurred_at_utc: &str, tz_name: &str) -> Option<u16> {
    let zone = jiff::tz::TimeZone::get(tz_name).ok()?;
    let timestamp: jiff::Timestamp = occurred_at_utc.parse().ok()?;
    let occurred: Zoned = timestamp.to_zoned(zone);
    let start = parse_local_date(started_on)?;
    let days = (occurred.date() - start).get_days();
    u16::try_from(days.max(0)).ok()
}

/// A quiet-hours window on the grower's wall clock (spec §4.9).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuietHours {
    /// Start, minutes since local midnight.
    pub start_minutes: u32,
    /// End, minutes since local midnight.
    pub end_minutes: u32,
}

impl QuietHours {
    /// Whether a local wall-clock minute falls inside the window.
    /// Windows may wrap midnight (22:00–07:00); equal bounds mean
    /// "always quiet".
    #[must_use]
    pub const fn contains(&self, minutes: u32) -> bool {
        if self.start_minutes <= self.end_minutes {
            minutes >= self.start_minutes && minutes < self.end_minutes
        } else {
            minutes >= self.start_minutes || minutes < self.end_minutes
        }
    }
}

/// Parses a `YYYY-MM-DD` local date (validated, leap-day aware).
fn parse_local_date(date: &str) -> Option<jiff::civil::Date> {
    date.parse().ok()
}

#[cfg(test)]
mod tests {
    use super::{QuietHours, UTC, day_number_local};

    #[test]
    fn utc_days_match_plain_date_math() {
        assert_eq!(
            day_number_local("2026-10-01", "2026-10-04T12:00:00Z", UTC),
            Some(3)
        );
        assert_eq!(
            day_number_local("2026-10-04", "2026-10-04T23:00:00Z", UTC),
            Some(0)
        );
        assert_eq!(
            day_number_local("nonsense", "2026-10-04T23:00:00Z", UTC),
            None
        );
        assert_eq!(day_number_local("2026-10-04", "nonsense", UTC), None);
        assert_eq!(
            day_number_local("2026-10-04", "2026-10-04T23:00:00Z", "Mars/Olympus"),
            None,
            "unknown zones do not pretend"
        );
    }

    #[test]
    fn spring_forward_counts_local_calendar_days() {
        // Europe/Berlin springs forward 2026-03-29 at 02:00 local
        // (01:00Z): the local day 03-29 is 23 hours long. A grow
        // started Saturday 2026-03-28 with an event Monday
        // 2026-03-30T23:30Z is 01:30 CEST on Tuesday 03-31 local —
        // day 3 — where naive UTC-date math would say 2.
        assert_eq!(
            day_number_local("2026-03-28", "2026-03-30T23:30:00Z", "Europe/Berlin"),
            Some(3)
        );
        // Late on the jump day itself the local date is still 03-29.
        assert_eq!(
            day_number_local("2026-03-28", "2026-03-29T21:30:00Z", "Europe/Berlin"),
            Some(1)
        );
    }

    #[test]
    fn fall_back_also_counts_local_days() {
        // Europe/Berlin falls back 2026-10-25 at 03:00 local (01:00Z).
        // Just before the jump Berlin is CEST (UTC+2): 2026-10-24T23:30Z
        // is 01:30 local on 10-25 — day 5, not the naive UTC day 4.
        assert_eq!(
            day_number_local("2026-10-20", "2026-10-24T23:30:00Z", "Europe/Berlin"),
            Some(5),
            "01:30 CEST local on 10-25 before the jump"
        );
        assert_eq!(
            day_number_local("2026-10-20", "2026-10-26T22:30:00Z", "Europe/Berlin"),
            Some(6),
            "23:30 local on 10-26"
        );
    }

    #[test]
    fn quiet_hours_follow_wall_clock_and_wrap_midnight() {
        let night = QuietHours {
            start_minutes: 22 * 60,
            end_minutes: 7 * 60,
        };
        assert!(night.contains(23 * 60));
        assert!(night.contains(3 * 60));
        assert!(!night.contains(12 * 60));
        let day = QuietHours {
            start_minutes: 13 * 60,
            end_minutes: 15 * 60,
        };
        assert!(day.contains(14 * 60));
        assert!(!day.contains(15 * 60), "the window is half-open");
    }
}
