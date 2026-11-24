//! The observed Light Cycle reality (grow-page rework, ticket 03).
//!
//! The light-cycle GET derives what the grow's bound light entity
//! actually does from the same HA history the charts read: the lit
//! share of the observed span for hours-on, each local day's first
//! lit start for the on-time (leaflog-core `observed_cycle`, D102).
//! Computed lazily per request with a short-lived cache that a
//! binding change invalidates; there is no background job.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use leaflog_core::chart;
use leaflog_core::light_cycle::{ObservedCycle, ObservedReality};

use crate::AppState;

/// How far back the derivation reads: the spec's last three days,
/// falling back to whatever shorter history exists.
pub const OBSERVED_WINDOW_HOURS: u32 = 72;

/// How long a derivation stays fresh on its own. Binding changes
/// invalidate immediately; this bounds the drift between page loads.
const CACHE_TTL: Duration = Duration::from_secs(60);

#[derive(Debug)]
struct Entry {
    entity_id: String,
    cycle: Option<ObservedCycle>,
    computed_at: Instant,
}

/// The per-grow derived-reality cache. Clones share one map.
#[derive(Debug, Clone)]
pub struct ObservedCache {
    entries: Arc<Mutex<BTreeMap<String, Entry>>>,
}

impl ObservedCache {
    /// An empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: Arc::new(Mutex::new(BTreeMap::new())),
        }
    }

    /// Drops a grow's cached derivation: called when the grow's
    /// bindings change or the grow goes away.
    ///
    /// # Panics
    ///
    /// Panics only on cache-map poisoning (a lock-holder panic).
    pub fn invalidate(&self, grow_id: &str) {
        self.entries
            .lock()
            .expect("observed-reality cache poisoned")
            .remove(grow_id);
    }

    /// The cached derivation for a grow, when the entry is fresh and
    /// still bound to `entity_id`. The inner `Option` is the cached
    /// answer itself (`None` answers are cached too).
    fn fresh(&self, grow_id: &str, entity_id: &str) -> Fresh {
        let hit = self.entries.lock().ok().and_then(|entries| {
            let entry = entries.get(grow_id)?;
            (entry.entity_id == entity_id && entry.computed_at.elapsed() < CACHE_TTL)
                .then_some(entry.cycle)
        });
        hit.map_or(Fresh::Miss, Fresh::Hit)
    }

    /// Stores a derivation for a grow (possibly `None`). A poisoned
    /// map skips the store: the next GET derives again.
    fn store(&self, grow_id: &str, entity_id: &str, cycle: Option<ObservedCycle>) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.insert(
                grow_id.to_owned(),
                Entry {
                    entity_id: entity_id.to_owned(),
                    cycle,
                    computed_at: Instant::now(),
                },
            );
        }
    }
}

/// A cache lookup: a miss, or the cached answer — which may itself
/// be a learned `None` ("no usable history"), cached like any other
/// answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fresh {
    /// Derive again: no entry, an expired one, a changed binding,
    /// or a poisoned map.
    Miss,
    /// The cached answer for the current binding.
    Hit(Option<ObservedCycle>),
}

impl Default for ObservedCache {
    fn default() -> Self {
        Self::new()
    }
}

/// The observed reality for one grow, if it has one.
///
/// "Has one" means a bound light entity with usable history.
/// Unbound grows, unreachable Supervisors and histories that never
/// light up all yield `None`: the editor then shows only the
/// editable defaults (fail closed).
///
/// The first GET for a binding derives and caches the answer
/// (including the `None` answers) for [`CACHE_TTL`]; a later GET
/// reuses it unless the binding changed.
pub async fn observed_for(
    state: &AppState,
    grow_id: &str,
    tz: &str,
    now: &str,
) -> Option<ObservedReality> {
    let role_grow_id = grow_id.to_owned();
    let (loaded, stance) = state
        .db
        .with(
            move |conn| -> Result<(crate::api_chart::RolePoints, bool), crate::api::ApiError> {
                let stance =
                    crate::store::grows::get(conn, &role_grow_id)?.light_unavailable_as_off;
                let loaded = crate::api_chart::load_role(conn, &role_grow_id, "light", stance)?;
                Ok((loaded, stance))
            },
        )
        .await
        .ok()?;
    let Some(entity_id) = loaded.entity_id else {
        // Unbound: nothing to serve, and any stale entry belongs to a
        // binding that no longer exists.
        state.observed.invalidate(grow_id);
        return None;
    };

    if let Fresh::Hit(cycle) = state.observed.fresh(grow_id, &entity_id) {
        return cycle.map(|cycle| ObservedReality { entity_id, cycle });
    }

    let derived = match state.ha.history(&entity_id, OBSERVED_WINDOW_HOURS).await {
        Ok(history) => {
            let ha_points = crate::api_chart::ha_history_points("light", &history, stance);
            let merged = chart::merge_series_span(
                now,
                i64::from(OBSERVED_WINDOW_HOURS),
                &ha_points,
                &loaded.snapshots,
            );
            let staircase = crate::api_chart::series_points(&merged);
            chart::parse_epoch(now).and_then(|now_epoch| {
                leaflog_core::light_cycle::observed_cycle(&staircase, now_epoch, tz)
            })
        }
        // A dead Supervisor keeps the endpoint usable: no observed
        // block until HA answers again.
        Err(_) => None,
    };
    state.observed.store(grow_id, &entity_id, derived);
    derived.map(|cycle| ObservedReality { entity_id, cycle })
}

#[cfg(test)]
mod tests {
    use super::{CACHE_TTL, Fresh, ObservedCache};
    use leaflog_core::light_cycle::{ClockTime, ObservedCycle};
    use std::time::{Duration, Instant};

    /// An 18 h cycle switching on at `hour`:00, for cache round-trips.
    fn cycle(hour: u8) -> ObservedCycle {
        ObservedCycle {
            lights_on: ClockTime { hour, minute: 0 },
            hours_on: 18,
        }
    }

    #[test]
    fn fresh_serves_a_matching_fresh_entry() {
        let cache = ObservedCache::new();
        cache.store("g1", "switch.grow_led", Some(cycle(6)));
        assert_eq!(
            cache.fresh("g1", "switch.grow_led"),
            Fresh::Hit(Some(cycle(6)))
        );
        assert_eq!(
            cache.fresh("g1", "switch.other"),
            Fresh::Miss,
            "a different entity is a miss"
        );
        assert_eq!(
            cache.fresh("g2", "switch.grow_led"),
            Fresh::Miss,
            "an unknown grow is a miss"
        );
    }

    #[test]
    fn fresh_serves_a_cached_none_answer() {
        let cache = ObservedCache::new();
        cache.store("g1", "switch.grow_led", None);
        assert_eq!(
            cache.fresh("g1", "switch.grow_led"),
            Fresh::Hit(None),
            "None answers are cached too"
        );
    }

    #[test]
    fn an_expired_entry_is_a_miss() {
        let cache = ObservedCache::new();
        cache.store("g1", "switch.grow_led", Some(cycle(6)));
        if let Ok(mut entries) = cache.entries.lock() {
            let now = Instant::now();
            let stale = now
                .checked_sub(CACHE_TTL)
                .and_then(|fresh_until| fresh_until.checked_sub(Duration::from_secs(1)));
            entries.get_mut("g1").unwrap().computed_at = stale.unwrap_or(now);
        }
        assert_eq!(cache.fresh("g1", "switch.grow_led"), Fresh::Miss);
    }
}
