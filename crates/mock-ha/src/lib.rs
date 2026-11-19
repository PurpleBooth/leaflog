//! Mock Home Assistant Supervisor (library): REST states + history and
//! the WebSocket protocol with fake, slowly-changing sensor data
//! (spec §3, milestone M4).

#![warn(clippy::nursery)]
#![deny(
    unused,
    nonstandard_style,
    future_incompatible,
    missing_copy_implementations,
    missing_debug_implementations,
    missing_docs,
    clippy::cargo,
    clippy::complexity,
    clippy::correctness,
    clippy::perf,
    clippy::style,
    clippy::suspicious,
    clippy::pedantic,
    non_fmt_panics
)]
#![allow(clippy::multiple_crate_versions)]

use std::sync::Arc;
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::Response;
use axum::routing::get;
use axum::{Json, Router};
use futures_util::{SinkExt as _, StreamExt as _};
use serde_json::{Value, json};
use tokio::sync::Mutex;

/// The fake grow room the mock replays.
#[derive(Debug, Clone, Copy, Default)]
pub struct FakeRoom {
    /// temperature °C
    pub temperature: f64,
    /// humidity %
    pub humidity: f64,
    /// soil moisture %
    pub soil_moisture: f64,
    /// CO₂ ppm
    pub co2: f64,
    /// light on/off
    pub light_on: bool,
}

impl FakeRoom {
    /// The canned starting values.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            temperature: 24.3,
            humidity: 58.0,
            soil_moisture: 31.0,
            co2: 850.0,
            light_on: false,
        }
    }

    /// All fake entities as HA state objects.
    #[must_use]
    pub fn states(&self) -> Value {
        json!([
            entity_state(
                "sensor.grow_temperature",
                &format!("{:.1}", self.temperature),
                "°C",
                "Grow tent temperature"
            ),
            entity_state(
                "sensor.grow_humidity",
                &format!("{:.0}", self.humidity),
                "%",
                "Grow tent humidity"
            ),
            entity_state(
                "sensor.grow_soil_moisture",
                &format!("{:.0}", self.soil_moisture),
                "%",
                "Soil moisture"
            ),
            entity_state(
                "sensor.grow_co2",
                &format!("{:.0}", self.co2),
                "ppm",
                "Grow tent CO₂"
            ),
            entity_state(
                "light.grow_led",
                if self.light_on { "on" } else { "off" },
                "",
                "Grow light"
            ),
            // A second on/off entity of a non-light domain: the Light
            // binding role must accept any on/off entity, not only a
            // light.* fixture.
            entity_state("switch.grow_panel", "on", "", "Grow panel switch"),
        ])
    }

    /// One `subscribe_entities` initial batch: full compressed
    /// states (`{entity: {s, a}}`), as real HA sends them under
    /// `event.a` — no `+` diff wrapper here.
    fn initial_states(&self) -> Value {
        json!({
            "sensor.grow_temperature": { "s": format!("{:.1}", self.temperature), "a": { "unit_of_measurement": "°C" } },
            "sensor.grow_humidity": { "s": format!("{:.0}", self.humidity), "a": { "unit_of_measurement": "%" } },
            "sensor.grow_soil_moisture": { "s": format!("{:.0}", self.soil_moisture), "a": { "unit_of_measurement": "%" } },
            "sensor.grow_co2": { "s": format!("{:.0}", self.co2), "a": { "unit_of_measurement": "ppm" } },
            "light.grow_led": { "s": if self.light_on { "on" } else { "off" }, "a": {} },
        })
    }

    /// Nudge the fake sensors (called each tick).
    fn tick(&mut self) {
        let delta = if self.light_on { 0.2_f64 } else { -0.1_f64 };
        self.temperature += delta;
        // The tent dries down to 35 %, then a humidifier kicks in
        // and the sawtooth restarts — a clamped floor would freeze
        // the live RH tile on one reading forever.
        self.humidity = if self.humidity <= 35.0 {
            44.9
        } else {
            self.humidity - 0.3
        };
        // The wet–dry cycle the coach preaches: dry out to 15 %,
        // then a watering resets the pot. Every tick still moves, so
        // the live soil-moisture tile never freezes on its floor.
        self.soil_moisture = if self.soil_moisture <= 15.0 {
            27.0
        } else {
            (self.soil_moisture - 0.8).max(15.0)
        };
        // Photosynthesis with the light on draws CO₂ down; a dark
        // tent accumulates it. Always non-zero, so every push gives
        // the live CO₂ tile a fresh value.
        let co2_delta = if self.light_on { -4.0_f64 } else { 2.0_f64 };
        self.co2 += co2_delta;
    }

    /// The next diff event after a tick. Home Assistant only
    /// pushes what changed, wrapped per entity in `+` additions
    /// (`event.c`), so the binary light enters the batch only when
    /// the session has not yet pushed its current state —
    /// `toggle_light` therefore reaches subscribers on the next tick
    /// (the contract `toggle_light` documents).
    fn tick_changes(&self, light_changed: bool) -> Value {
        let mut changes = json!({
            "sensor.grow_temperature": { "+": { "s": format!("{:.1}", self.temperature) } },
            "sensor.grow_humidity": { "+": { "s": format!("{:.0}", self.humidity) } },
            "sensor.grow_soil_moisture": { "+": { "s": format!("{:.0}", self.soil_moisture) } },
            "sensor.grow_co2": { "+": { "s": format!("{:.0}", self.co2) } },
        });
        if light_changed {
            changes["light.grow_led"] =
                json!({ "+": { "s": if self.light_on { "on" } else { "off" } } });
        }
        changes
    }
}

/// One HA-shaped state object.
fn entity_state(entity_id: &str, state: &str, unit: &str, name: &str) -> Value {
    let mut attributes = json!({ "friendly_name": name });
    if !unit.is_empty() {
        attributes["unit_of_measurement"] = json!(unit);
    }
    // Real HA exposes the device class the sensor-item picker filters
    // on (spec §4.1); the grow sensors carry theirs. CO₂ joins soil
    // moisture in the exclusion: its real enum is carbon_dioxide, not
    // the entity-id suffix, and a wrong class would lie harder than
    // none.
    if let Some(class) = entity_id
        .strip_prefix("sensor.grow_")
        .filter(|class| *class != "soil_moisture" && *class != "co2")
    {
        attributes["device_class"] = json!(class);
    }
    json!({
        "entity_id": entity_id,
        "state": state,
        "attributes": attributes,
        "last_changed": "2026-11-08T12:00:00.000000+00:00",
        "last_updated": "2026-11-08T12:00:00.000000+00:00",
    })
}

/// Shared mock state.
#[derive(Debug, Clone)]
pub struct MockState {
    /// The mutable fake room (ticked by WS sessions).
    pub room: Arc<Mutex<FakeRoom>>,
    /// Notifications received on the `persistent_notification` service
    /// (asserted by tests).
    pub notifications: Arc<Mutex<Vec<String>>>,
    /// The user language reported by `GET /core/api/config` (BCP-47
    /// tag, default `en`; tests set it to exercise the locale chain).
    pub language: Arc<Mutex<String>>,
}

impl Default for MockState {
    fn default() -> Self {
        Self {
            room: Arc::new(Mutex::new(FakeRoom::default())),
            notifications: Arc::new(Mutex::new(Vec::new())),
            language: Arc::new(Mutex::new("en".to_owned())),
        }
    }
}

/// Builds the mock Supervisor router (REST + WS).
pub fn app(state: MockState) -> Router {
    Router::new()
        .route("/", get(root))
        .route("/core/api/", get(ping))
        .route("/core/api/states", get(states))
        .route("/core/api/config", get(config))
        .route(
            "/core/api/services/notify/persistent_notification",
            axum::routing::post(notify),
        )
        .route("/core/api/history/period/{from}", get(history))
        .route("/core/websocket", get(websocket))
        .with_state(state)
}

/// Query accepted on the `history` endpoint.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(default)]
#[allow(dead_code)] // fields exist to accept the documented query
pub struct HistoryQuery {
    /// `filter_entity_id`
    filter_entity_id: Option<String>,
    /// `minimal_response` flag presence
    minimal_response: Option<String>,
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn root() -> Json<Value> {
    json!({
        "app": "mock-ha",
        "endpoints": ["/core/api/", "/core/api/states", "/core/api/history/period/*", "/core/websocket"],
    })
    .into()
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn ping() -> Json<Value> {
    json!({ "message": "API running." }).into()
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn config(State(state): State<MockState>) -> Json<Value> {
    // The documented config surface Leaflog reads (spec §2): the
    // canonical timezone for display and grow-day numbering, and the
    // user language (D112, default `en`, settable for tests). An
    // empty `MOCK_HA_LANGUAGE` means "no language": HA reports none,
    // so the frontend locale chain falls through to the browser
    // locale — that is the step the e2e German case exercises.
    let language = state.language.lock().await.clone();
    json!({
        "timezone": "Europe/Berlin",
        "language": if language.is_empty() {
            Value::Null
        } else {
            json!(language)
        },
        "unit_system": { "temperature": "°C", "length": "mm", "volume": "L" },
    })
    .into()
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn notify(State(state): State<MockState>, body: String) -> StatusCode {
    state.notifications.lock().await.push(body);
    StatusCode::OK
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn states(State(state): State<MockState>) -> Json<Value> {
    let room = state.room.lock().await;
    room.states().into()
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn history(State(state): State<MockState>, Query(query): Query<HistoryQuery>) -> Json<Value> {
    let entity = query
        .filter_entity_id
        .unwrap_or_else(|| "sensor.grow_temperature".to_owned());
    if entity == "light.grow_led" || entity == "switch.grow_panel" {
        return Json(on_off_history(&entity));
    }
    let (base, temperature, humidity, soil_moisture, co2) = {
        let room = state.room.lock().await;
        (
            room.states(),
            room.temperature,
            room.humidity,
            room.soil_moisture,
            room.co2,
        )
    };
    // minimal_response shape: first full object, then {"s": ..., "lu": ...}.
    let full = base
        .as_array()
        .and_then(|all| {
            all.iter()
                .find(|candidate| candidate["entity_id"] == entity.as_str())
                .cloned()
        })
        .unwrap_or_else(|| {
            entity_state(
                "sensor.grow_temperature",
                "24.3",
                "°C",
                "Grow tent temperature",
            )
        });
    // The eleven loop points spread two hours apart, anchored at
    // request time from now−23 h to now−3 h. Every rolling window
    // (24 h / 7 d / 30 d) carries them, and the charts get distinct
    // timestamps to read — a fixed far-past date put every point on
    // one instant, which no crosshair-sync test could discriminate.
    let now = jiff::Timestamp::now();
    let mut series = vec![full];
    for step in 1..12 {
        let value = match entity.as_str() {
            "sensor.grow_humidity" => (-0.3_f64).mul_add(f64::from(step), humidity),
            "sensor.grow_soil_moisture" => (-0.8_f64).mul_add(f64::from(step), soil_moisture),
            "sensor.grow_co2" => (-2.0_f64).mul_add(f64::from(step), co2),
            _ => (-0.1_f64).mul_add(f64::from(step), temperature),
        };
        let last_update = now - jiff::Span::new().hours(25 - 2 * i64::from(step));
        series.push(json!({ "s": format!("{value:.1}"), "lu": last_update.to_string() }));
    }
    json!([series]).into()
}

/// An on/off entity's staircase for `history/period`, anchored at
/// request time so every window contains it.
///
/// Served for the light fixture and the panel switch alike — any
/// on/off entity bound to the Light role must drive the same
/// timeline. The stamps run off at now−23 h, on at now−20 h, off at
/// now−18 h: in a rolling day window that is a schedule-driven
/// prefix, a dark override, a one-hour on stretch, and a dark run to
/// now — the shape the stepped-band tests assert.
fn on_off_history(entity_id: &str) -> Value {
    let now = jiff::Timestamp::now();
    let stamp = |hours_ago: i64| (now - jiff::Span::new().hours(hours_ago)).to_string();
    // minimal_response shape: one full state object first, then
    // short {s, lu} objects.
    json!([
        [
            {
                "entity_id": entity_id,
                "state": "off",
                "attributes": { "friendly_name": "Grow light" },
                "last_changed": stamp(23),
                "last_updated": stamp(23),
            },
            { "s": "on", "lu": stamp(20) },
            { "s": "off", "lu": stamp(18) },
        ]
    ])
}

// Extractors are taken by value because Axum requires it.
#[allow(clippy::needless_pass_by_value)] // Axum handler extractor contract; see docs/DECISIONS.md
async fn websocket(State(state): State<MockState>, upgrade: WebSocketUpgrade) -> Response {
    upgrade.on_upgrade(move |socket| socket_loop(socket, state))
}

/// One WS session: HA auth handshake, then command handling and a
/// periodic fake-data push.
async fn socket_loop(socket: WebSocket, state: MockState) {
    let (mut sender, mut receiver) = socket.split();
    let mut subscribed = false;
    let mut next_event_id = 1_u64;
    // The light state this session has already pushed; HA pushes a
    // binary entity only when its state changes.
    let mut pushed_light = false;

    // Send auth_required.
    if sender
        .send(Message::text(
            json!({ "type": "auth_required" }).to_string(),
        ))
        .await
        .is_err()
    {
        return;
    }
    // Await auth.
    let Some(Ok(message)) = receiver.next().await else {
        return;
    };
    let Ok(text) = message.into_text() else {
        return;
    };
    if !text.contains("access_token") {
        let _ = sender
            .send(Message::text(
                json!({ "type": "auth_invalid", "message": "no token" }).to_string(),
            ))
            .await;
        return;
    }
    if sender
        .send(Message::text(
            json!({ "type": "auth_ok", "ha_version": "2026.11.0" }).to_string(),
        ))
        .await
        .is_err()
    {
        return;
    }

    // Push the initial states once subscribed; then tick every 2s.
    let mut ticker = tokio::time::interval(Duration::from_secs(2));
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    ticker.tick().await; // first tick is immediate
    loop {
        tokio::select! {
            message = receiver.next() => {
                let Some(Ok(message)) = message else {
                    break;
                };
                let Ok(text) = message.into_text() else {
                    continue;
                };
                let Ok(value) = serde_json::from_str::<Value>(&text) else {
                    continue;
                };
                match handle_command(&mut sender, &value, &state, &mut next_event_id).await {
                    CommandOutcome::Close => break,
                    CommandOutcome::Subscribed => {
                        subscribed = true;
                        // The subscribe batch carries the light's
                        // current state; only a later flip is news.
                        pushed_light = state.room.lock().await.light_on;
                    }
                    CommandOutcome::Continue => {}
                }
            }
            _ = ticker.tick() => {
                if !subscribed { continue; }
                let mut room = state.room.lock().await;
                room.tick();
                let light_changed = room.light_on != pushed_light;
                let changes = room.tick_changes(light_changed);
                pushed_light = room.light_on;
                drop(room);
                let event = json!({
                    "id": next_event_id,
                    "type": "event",
                    "event": { "c": changes },
                });
                next_event_id += 1;
                if sender.send(Message::text(event.to_string())).await.is_err() {
                    break;
                }
            }
        }
    }
}
/// What happened while handling one client command.
enum CommandOutcome {
    /// The session should close.
    Close,
    /// A subscription became active.
    Subscribed,
    /// Nothing special.
    Continue,
}

/// Handles one client command.
async fn handle_command<S>(
    sender: &mut S,
    value: &Value,
    state: &MockState,
    next_event_id: &mut u64,
) -> CommandOutcome
where
    S: futures_util::SinkExt<Message> + Unpin,
{
    match value["type"].as_str() {
        Some("subscribe_entities") => {
            let result = json!({
                "id": value["id"], "type": "result", "success": true, "result": null
            });
            if sender
                .send(Message::text(result.to_string()))
                .await
                .is_err()
            {
                return CommandOutcome::Close;
            }
            let event = {
                let room = state.room.lock().await;
                json!({
                    "id": *next_event_id, "type": "event",
                    "event": { "a": room.initial_states() },
                })
            };
            *next_event_id += 1;
            if sender.send(Message::text(event.to_string())).await.is_err() {
                return CommandOutcome::Close;
            }
            CommandOutcome::Subscribed
        }
        Some("ping") => {
            let pong = json!({ "id": value["id"], "type": "pong" });
            if sender.send(Message::text(pong.to_string())).await.is_err() {
                return CommandOutcome::Close;
            }
            CommandOutcome::Continue
        }
        _ => {
            let error = json!({
                "id": value["id"], "type": "result", "success": false,
                "error": { "code": "unknown_command" }
            });
            if sender.send(Message::text(error.to_string())).await.is_err() {
                return CommandOutcome::Close;
            }
            CommandOutcome::Continue
        }
    }
}

// The light helper used by tests to flip the fake light and force a
// state-changed push on the next tick.
impl FakeRoom {
    /// Flips the fake light.
    pub const fn toggle_light(&mut self) {
        self.light_on = !self.light_on;
    }
}

/// Exposes the initial state batch for tests.
#[must_use]
pub fn initial_states(room: &FakeRoom) -> Value {
    room.initial_states()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The CO₂ fixture feeds the env strip's live CO₂ tile (grow-page
    /// rework ticket 02): a bound grow must see a ppm reading that
    /// moves every tick, through the initial batch, the tick pushes
    /// and the REST states alike.
    #[test]
    fn co2_fixture_serves_a_moving_ppm_reading() {
        let room = FakeRoom::new();
        let states = room.states();
        let co2 = states
            .as_array()
            .expect("states is an array")
            .iter()
            .find(|entity| entity["entity_id"] == "sensor.grow_co2")
            .expect("states carry sensor.grow_co2")
            .clone();
        assert_eq!(co2["attributes"]["unit_of_measurement"], "ppm");
        assert_eq!(co2["state"], "850");

        // The subscribe_entities initial batch seeds the same reading
        // as full compressed states (real HA's `event.a` shape).
        let initial = initial_states(&room);
        assert_eq!(
            initial["sensor.grow_co2"]["s"], "850",
            "initial batch seeds CO₂"
        );
        assert_eq!(
            initial["sensor.grow_co2"]["a"]["unit_of_measurement"],
            "ppm"
        );

        // Every tick pushes a fresh CO₂ state as a `+` addition (real
        // HA's `event.c` shape): the live tile has something new to
        // show on each push (the light starts off, so a dark tent
        // accumulates CO₂).
        let mut after_tick = room;
        after_tick.tick();
        let changes = after_tick.tick_changes(false);
        assert_eq!(changes["sensor.grow_co2"]["+"]["s"], "852");
    }

    /// RH and soil moisture cycle instead of clamping (grow-page
    /// rework ticket 02): a floor-frozen reading would freeze the
    /// live tiles on one value forever, so the humidifier restarts
    /// the RH sawtooth and a watering restarts the soil's wet–dry
    /// cycle — every tick still moves.
    #[test]
    fn humidity_and_soil_cycle_instead_of_freezing() {
        let mut room = FakeRoom::new();
        // Deterministic dry-down from just above the floor: dip,
        // clamp, refill — the cycle that keeps the tile moving.
        room.soil_moisture = 16.5;
        room.tick();
        assert!(
            room.soil_moisture > 15.0 && room.soil_moisture <= 15.7,
            "dries down toward the floor"
        );
        room.tick();
        assert_eq!(room.soil_moisture, 15.0, "the floor clamps the dip");
        room.tick();
        assert_eq!(room.soil_moisture, 27.0, "a watering restarts the cycle");

        // The RH sawtooth: dry to the floor, humidifier back to 44.9 %.
        let mut tent = FakeRoom::new();
        tent.humidity = 35.3;
        tent.tick();
        assert_eq!(tent.humidity, 35.0, "dries to the floor");
        tent.tick();
        assert_eq!(tent.humidity, 44.9, "the humidifier restarts the sawtooth");
        tent.tick();
        assert_eq!(tent.humidity, 44.6);
    }
}
