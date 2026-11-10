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
                "light.grow_led",
                if self.light_on { "on" } else { "off" },
                "",
                "Grow light"
            ),
        ])
    }

    /// One `subscribe_entities` initial-change batch.
    fn initial_changes(&self) -> Value {
        json!({
            "sensor.grow_temperature": { "+": { "s": format!("{:.1}", self.temperature), "a": { "unit_of_measurement": "°C" } } },
            "sensor.grow_humidity": { "+": { "s": format!("{:.0}", self.humidity), "a": { "unit_of_measurement": "%" } } },
            "sensor.grow_soil_moisture": { "+": { "s": format!("{:.0}", self.soil_moisture), "a": { "unit_of_measurement": "%" } } },
            "light.grow_led": { "+": { "s": if self.light_on { "on" } else { "off" }, "a": {} } },
        })
    }

    /// Nudge the fake sensors (called each tick).
    fn tick(&mut self) {
        let delta = if self.light_on { 0.2_f64 } else { -0.1_f64 };
        self.temperature += delta;
        self.humidity = (self.humidity - 0.3).max(35.0);
        self.soil_moisture = (self.soil_moisture - 0.8).max(15.0);
    }

    /// The next change batch after a tick.
    fn tick_changes(&self) -> Value {
        json!({
            "sensor.grow_temperature": { "s": format!("{:.1}", self.temperature) },
            "sensor.grow_humidity": { "s": format!("{:.0}", self.humidity) },
            "sensor.grow_soil_moisture": { "s": format!("{:.0}", self.soil_moisture) },
        })
    }
}

/// One HA-shaped state object.
fn entity_state(entity_id: &str, state: &str, unit: &str, name: &str) -> Value {
    let mut attributes = json!({ "friendly_name": name });
    if !unit.is_empty() {
        attributes["unit_of_measurement"] = json!(unit);
    }
    // Real HA exposes the device class the sensor-item picker filters
    // on (spec §4.1); the grow sensors carry theirs.
    if let Some(class) = entity_id
        .strip_prefix("sensor.grow_")
        .filter(|class| *class != "soil_moisture")
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
#[derive(Debug, Clone, Default)]
pub struct MockState {
    /// The mutable fake room (ticked by WS sessions).
    pub room: Arc<Mutex<FakeRoom>>,
    /// Notifications received on the `persistent_notification` service
    /// (asserted by tests).
    pub notifications: Arc<Mutex<Vec<String>>>,
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
async fn config() -> Json<Value> {
    // The documented config surface Leaflog reads (spec §2): the
    // canonical timezone for display and grow-day numbering.
    json!({
        "timezone": "Europe/Berlin",
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
    let (base, temperature, humidity, soil_moisture) = {
        let room = state.room.lock().await;
        (
            room.states(),
            room.temperature,
            room.humidity,
            room.soil_moisture,
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
    let mut series = vec![full];
    for step in 1..12 {
        let value = match entity.as_str() {
            "sensor.grow_humidity" => (-0.3_f64).mul_add(f64::from(step), humidity),
            "sensor.grow_soil_moisture" => (-0.8_f64).mul_add(f64::from(step), soil_moisture),
            _ => (-0.1_f64).mul_add(f64::from(step), temperature),
        };
        series.push(json!({ "s": format!("{value:.1}"), "lu": "2026-11-08T11:00:00+00:00" }));
    }
    json!([series]).into()
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
                    CommandOutcome::Subscribed => subscribed = true,
                    CommandOutcome::Continue => {}
                }
            }
            _ = ticker.tick() => {
                if !subscribed { continue; }
                let mut room = state.room.lock().await;
                room.tick();
                let changes = room.tick_changes();
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
                    "event": { "c": room.initial_changes() },
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

/// Exposes the initial-change batch for tests.
#[must_use]
pub fn initial_changes(room: &FakeRoom) -> Value {
    room.initial_changes()
}
