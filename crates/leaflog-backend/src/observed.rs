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
    let loaded = state
        .db
        .with(move |conn| crate::api_chart::load_role(conn, &role_grow_id, "light"))
        .await
        .ok()?;
    let Some(entity_id) = loaded.entity_id else {
        // Unbound: nothing to serve, and any stale entry belongs to a
        // binding that no longer exists.
        state.observed.invalidate(grow_id);
        return None;
    };

    if let Ok(entries) = state.observed.entries.lock()
        && let Some(entry) = entries.get(grow_id)
        && entry.entity_id == entity_id
        && entry.computed_at.elapsed() < CACHE_TTL
    {
        return entry
            .cycle
            .map(|cycle| ObservedReality { entity_id, cycle });
    }

    let derived = match state.ha.history(&entity_id, OBSERVED_WINDOW_HOURS).await {
        Ok(history) => {
            let ha_points = crate::api_chart::ha_history_points("light", &history);
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
    if let Ok(mut entries) = state.observed.entries.lock() {
        entries.insert(
            grow_id.to_owned(),
            Entry {
                entity_id: entity_id.clone(),
                cycle: derived,
                computed_at: Instant::now(),
            },
        );
    }
    derived.map(|cycle| ObservedReality { entity_id, cycle })
}
