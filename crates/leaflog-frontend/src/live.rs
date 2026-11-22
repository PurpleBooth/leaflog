//! Live sensor values for the env strip (grow-page rework ticket 02).
//!
//! The old `LivePanel` section is gone: the environment strip is the
//! single live surface. Its values come from whichever snapshot is
//! newest — the backend env snapshot or the live-data SSE stream
//! ([`subscribe_live`]) — and there is deliberately no status line
//! and no polling: `EventSource` reconnects natively, and while it
//! reconnects the tiles keep showing their last values (the
//! quiet-degradation rule: backend values or "—", never an alarm).

use crate::api;
use crate::bindings::{ROLE_CO2, ROLE_HUMIDITY, ROLE_SOIL_MOISTURE, ROLE_TEMPERATURE};
use crate::dashboard::{BindingStatus, EnvSnapshot};
use leptos::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen::closure::Closure;

/// Subscribes to the backend's live-data SSE stream (`api/live`, a
/// relative URL so it resolves under the ingress base) with the
/// browser's native `EventSource`: exactly one connection per page,
/// automatic reconnect, and no polling. Call this from a component
/// body — the source (and its handler) close with the component.
///
/// Returns the signal holding the latest received snapshot. The
/// `EventSource` path has exactly three UI states, all rendered by the
/// env strip alone: before the first event (backend snapshot
/// values), live (SSE values), and reconnecting (the last values
/// stand — there is no status line to flip).
pub fn subscribe_live() -> RwSignal<Option<api::LiveData>> {
    let latest: RwSignal<Option<api::LiveData>> = RwSignal::new(None);
    // A browser without EventSource keeps the quiet-degradation rule:
    // the strip stays on the backend snapshot's values.
    let Ok(source) = web_sys::EventSource::new("api/live") else {
        return latest;
    };
    let on_message =
        Closure::<dyn FnMut(web_sys::MessageEvent)>::new(move |event: web_sys::MessageEvent| {
            let Some(text) = event.data().as_string() else {
                return;
            };
            // A malformed payload leaves the previous snapshot in
            // place; the next event refreshes it.
            if let Ok(data) = serde_json::from_str::<api::LiveData>(&text) {
                latest.set(Some(data));
            }
        });
    source.set_onmessage(Some(on_message.as_ref().unchecked_ref()));
    // `on_cleanup` demands `Send + Sync`, which wasm handles are
    // not: the pair lives in an owner-local stored value instead
    // (the chart.rs pattern), and cleanup drops it exactly once.
    let subscription = StoredValue::new_local(Some((source, on_message)));
    on_cleanup(move || {
        subscription.update_value(|slot| {
            if let Some((source, on_message)) = slot.take() {
                source.close();
                drop(on_message);
            }
        });
    });
    latest
}

/// The values the env strip's six tiles show, merged from the newest
/// live-data SSE snapshot and the backend env snapshot (whichever is
/// fresher for a reading wins; the SSE snapshot always wins when it
/// carries the bound entity).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct StripValues {
    /// Air temperature, °C.
    pub temperature_c: Option<f64>,
    /// Relative humidity, %.
    pub humidity_pct: Option<f64>,
    /// VPD, kPa (recomputed from the merged temp and humidity).
    pub vpd_kpa: Option<f64>,
    /// RH window for the merged temperature and the phase band.
    pub rh_window: (f64, f64),
    /// CO₂, ppm (live only — the env snapshot carries no CO₂).
    pub co2_ppm: Option<f64>,
    /// Soil moisture, % (live only — the env snapshot carries none).
    pub soil_moisture_pct: Option<f64>,
    /// Whether the merged temperature sits inside the thresholds.
    pub temp_ok: Option<bool>,
    /// Whether the merged VPD sits inside the phase band.
    pub vpd_ok: Option<bool>,
}

/// Reads one bound entity's state as a number from the live-data
/// snapshot. `None` when the role is unbound, the entity carries no
/// state (outage), or the state is not numeric (dead or binary).
fn live_number(
    live: Option<&api::LiveData>,
    bindings: &[BindingStatus],
    role: &str,
) -> Option<f64> {
    let entity_id = bindings
        .iter()
        .find(|binding| binding.role == role)?
        .entity_id
        .as_str();
    let entity = live?.entities.get(entity_id)?;
    let value = entity.state.parse::<f64>().ok()?;
    if role == ROLE_TEMPERATURE {
        let unit = entity
            .attributes
            .get("unit_of_measurement")
            .and_then(serde_json::Value::as_str);
        leaflog_core::units::sensor_celsius(value, unit)
    } else {
        Some(value)
    }
}

/// The frame classes a tile wears for an ok flag: in-band (success
/// tint), out-of-band (warning tint), unknown (neutral). Full literal
/// strings — Tailwind only compiles literals it can scan out of the
/// sources, so no format! fragments.
#[must_use]
pub const fn tile_class(ok: Option<bool>) -> &'static str {
    match ok {
        Some(true) => {
            "rounded-box border border-success/50 bg-success/10 px-4 py-3 flex flex-wrap gap-x-1 gap-y-0.5"
        }
        Some(false) => {
            "rounded-box border border-warning/60 bg-warning/10 px-4 py-3 flex flex-wrap gap-x-1 gap-y-0.5"
        }
        None => {
            "rounded-box border border-base-300 bg-base-100 px-4 py-3 flex flex-wrap gap-x-1 gap-y-0.5"
        }
    }
}

/// The dead bound entity for one tile role, if any: the entity id
/// behind a binding that exists but currently reports no data (spec
/// §4.1, surfaced per tile since D146). An unbound role or a healthy
/// binding yields `None` — unbound is the not-measured card's word,
/// not dead data.
#[must_use]
pub fn dead_entity(bindings: &[BindingStatus], role: &str) -> Option<String> {
    bindings
        .iter()
        .find(|binding| binding.role == role)
        .filter(|binding| !binding.available)
        .map(|binding| binding.entity_id.clone())
}

/// Merges the env snapshot with the live-data snapshot into the
/// strip's tile values (see [`StripValues`]). Derived readings (VPD,
/// RH window, both ok flags) recompute from the merged inputs so the
/// colours never lie about a stale snapshot.
#[must_use]
pub fn strip_values(env: &EnvSnapshot, live: Option<&api::LiveData>) -> StripValues {
    let band = env.vpd_band;
    let live_temperature = live_number(live, &env.bindings, ROLE_TEMPERATURE);
    let live_humidity = live_number(live, &env.bindings, ROLE_HUMIDITY);
    // The SSE snapshot wins when it carries the bound entity; the
    // backend snapshot stands in for everything else (and for the
    // readings it never carries — CO₂ and soil moisture — the tile
    // shows "—").
    let temperature_c = live_temperature.or(env.temperature_c);
    let humidity_pct = live_humidity.or(env.humidity_pct);
    // Derived readings (VPD, RH window, both ok flags) recompute from
    // the merged inputs whenever a live reading replaced an input,
    // so the derived tiles track the live stream and the colours
    // never lie about a stale snapshot. With no live reading the
    // backend's own derived values stand verbatim.
    let derived_live = live_temperature.is_some() || live_humidity.is_some();
    let (vpd_kpa, rh_window, temp_ok, vpd_ok) = if derived_live {
        let vpd_kpa = match (temperature_c, humidity_pct) {
            (Some(temp), Some(humidity)) => Some(leaflog_core::env::vpd_kpa(
                temp,
                humidity,
                leaflog_core::env::DEFAULT_LEAF_OFFSET_C,
            )),
            _ => None,
        };
        let rh_window = temperature_c.map_or(env.rh_window, |temp| {
            leaflog_core::env::rh_window(temp, leaflog_core::env::DEFAULT_LEAF_OFFSET_C, band)
        });
        let temp_ok = temperature_c
            .map(|temp| temp >= env.thresholds.floor_c && temp <= env.thresholds.ceiling_c);
        let vpd_ok = vpd_kpa.map(|vpd| vpd >= band.0 && vpd <= band.1);
        (vpd_kpa, rh_window, temp_ok, vpd_ok)
    } else {
        (env.vpd_kpa, env.rh_window, env.temp_ok, env.vpd_ok)
    };
    StripValues {
        temperature_c,
        humidity_pct,
        vpd_kpa,
        rh_window,
        co2_ppm: live_number(live, &env.bindings, ROLE_CO2),
        soil_moisture_pct: live_number(live, &env.bindings, ROLE_SOIL_MOISTURE),
        temp_ok,
        vpd_ok,
    }
}

#[cfg(test)]
mod tests {
    use super::strip_values;
    use crate::api::{LiveData, LiveEntity};
    use crate::dashboard::{BindingStatus, EnvSnapshot, Thresholds};
    use std::collections::BTreeMap;

    /// A snapshot with every reading present and healthy: temp 24.3 °C
    /// inside an 18–29 °C band, VPD 1.00 kPa inside its 0.8–1.4 kPa
    /// band, humidity 58 %.
    fn healthy_snapshot() -> EnvSnapshot {
        EnvSnapshot {
            bindings: vec![
                BindingStatus {
                    role: "temperature".to_owned(),
                    entity_id: "sensor.grow_temperature".to_owned(),
                    available: true,
                },
                BindingStatus {
                    role: "humidity".to_owned(),
                    entity_id: "sensor.grow_humidity".to_owned(),
                    available: true,
                },
                BindingStatus {
                    role: "co2".to_owned(),
                    entity_id: "sensor.grow_co2".to_owned(),
                    available: true,
                },
                BindingStatus {
                    role: "soil_moisture".to_owned(),
                    entity_id: "sensor.grow_soil_moisture".to_owned(),
                    available: true,
                },
            ],
            temperature_c: Some(24.3),
            humidity_pct: Some(58.0),
            vpd_kpa: Some(1.0),
            vpd_band: (0.8, 1.4),
            rh_window: (45.0, 65.0),
            thresholds: Thresholds {
                floor_c: 18.0,
                ceiling_c: 29.0,
            },
            phase: "Vegetative".to_owned(),
            temp_ok: Some(true),
            vpd_ok: Some(true),
        }
    }

    /// A live-data snapshot carrying the four bound grow sensors.
    fn live_data(temperature: &str, humidity: &str, co2: &str, soil_moisture: &str) -> LiveData {
        let entity = |entity_id: &str, state: &str| {
            (
                entity_id.to_owned(),
                LiveEntity {
                    entity_id: entity_id.to_owned(),
                    state: state.to_owned(),
                    attributes: BTreeMap::new(),
                },
            )
        };
        LiveData {
            connected: true,
            entities: BTreeMap::from([
                entity("sensor.grow_temperature", temperature),
                entity("sensor.grow_humidity", humidity),
                entity("sensor.grow_co2", co2),
                entity("sensor.grow_soil_moisture", soil_moisture),
            ]),
        }
    }

    #[test]
    fn live_state_overrides_the_snapshot_and_recomputes_derived_tiles() {
        let env = healthy_snapshot();
        // The SSE push runs hot: 30.5 °C is above the 29 °C ceiling the
        // snapshot's own (healthy) flags still claim.
        let live = live_data("30.5", "60.0", "850", "31");
        let values = strip_values(&env, Some(&live));

        assert_eq!(values.temperature_c, Some(30.5));
        assert_eq!(values.humidity_pct, Some(60.0));
        // The colours never lie: the recomputed flag flips against the
        // snapshot's stale true.
        assert_eq!(values.temp_ok, Some(false));
        // VPD rises with temperature at constant humidity, so the
        // recomputed reading must beat the snapshot's 1.00 kPa.
        let vpd = values.vpd_kpa.expect("vpd from merged readings");
        assert!(vpd > 1.0, "recomputed vpd {vpd} beats the snapshot's 1.0");
        // The RH window follows the merged temperature upward.
        assert!(
            values.rh_window.0 > 45.0,
            "rh window {:?} follows the hotter reading",
            values.rh_window,
        );
    }

    #[test]
    fn co2_and_soil_read_their_bound_entities_only_live() {
        let env = healthy_snapshot();
        let live = live_data("24.3", "58.0", "850", "31");
        let values = strip_values(&env, Some(&live));
        assert_eq!(values.co2_ppm, Some(850.0));
        assert_eq!(values.soil_moisture_pct, Some(31.0));

        // A dead entity (HA's unavailable) is not a number: the tile
        // gets "—", never a stale or zero reading.
        let dead = live_data("24.3", "58.0", "unavailable", "31");
        let values = strip_values(&env, Some(&dead));
        assert_eq!(values.co2_ppm, None);
    }

    #[test]
    fn without_a_live_snapshot_the_backend_values_stand() {
        let env = healthy_snapshot();
        let values = strip_values(&env, None);
        assert_eq!(values.temperature_c, Some(24.3));
        assert_eq!(values.humidity_pct, Some(58.0));
        assert_eq!(values.vpd_kpa, Some(1.0));
        assert_eq!(values.rh_window, (45.0, 65.0));
        assert_eq!(values.temp_ok, Some(true));
        assert_eq!(values.vpd_ok, Some(true));
        // No backend fallback exists for the live-only readings.
        assert_eq!(values.co2_ppm, None);
        assert_eq!(values.soil_moisture_pct, None);
    }

    #[test]
    fn stale_snapshot_flags_correct_themselves_from_live_readings() {
        let mut env = healthy_snapshot();
        // The snapshot's own flags lag its (healthy) readings.
        env.temp_ok = Some(false);
        env.vpd_ok = Some(false);
        // The live stream carries readings equal to the snapshot's —
        // recomputation, not the stale flags, must drive the tiles.
        let live = live_data("24.3", "58.0", "850", "31");
        let values = strip_values(&env, Some(&live));
        assert_eq!(values.temp_ok, Some(true));
        assert_eq!(values.vpd_ok, Some(true));
    }

    #[test]
    fn an_outage_cleared_live_map_falls_back_to_the_snapshot() {
        let env = healthy_snapshot();
        // The backend clears its live map on an HA outage, so the SSE
        // stream pushes a connected: false, entities: {} snapshot.
        let outage = LiveData {
            connected: false,
            entities: BTreeMap::new(),
        };
        let values = strip_values(&env, Some(&outage));
        assert_eq!(values.temperature_c, Some(24.3));
        assert_eq!(values.vpd_kpa, Some(1.0));
        // The snapshot's own ok flags stand too (the tile colours fall
        // back with the values).
        assert_eq!(values.temp_ok, Some(true));
        assert_eq!(values.vpd_ok, Some(true));
        assert_eq!(values.co2_ppm, None);
    }

    #[test]
    fn tile_classes_match_the_ok_flag() {
        use super::tile_class;
        assert_eq!(
            tile_class(Some(true)),
            "rounded-box border border-success/50 bg-success/10 px-4 py-3 flex flex-wrap gap-x-1 gap-y-0.5",
        );
        assert_eq!(
            tile_class(Some(false)),
            "rounded-box border border-warning/60 bg-warning/10 px-4 py-3 flex flex-wrap gap-x-1 gap-y-0.5",
        );
        assert_eq!(
            tile_class(None),
            "rounded-box border border-base-300 bg-base-100 px-4 py-3 flex flex-wrap gap-x-1 gap-y-0.5",
        );
    }

    #[test]
    fn dead_entity_only_names_dead_bound_roles() {
        let bindings = vec![
            BindingStatus {
                role: "temperature".to_owned(),
                entity_id: "sensor.ghost".to_owned(),
                available: false,
            },
            BindingStatus {
                role: "humidity".to_owned(),
                entity_id: "sensor.grow_humidity".to_owned(),
                available: true,
            },
        ];
        assert_eq!(
            super::dead_entity(&bindings, "temperature").as_deref(),
            Some("sensor.ghost"),
            "a dead bound role names its entity"
        );
        assert_eq!(
            super::dead_entity(&bindings, "humidity"),
            None,
            "a healthy binding never marks its tile"
        );
        assert_eq!(
            super::dead_entity(&bindings, "co2"),
            None,
            "an unbound role is not dead data"
        );
    }
}
