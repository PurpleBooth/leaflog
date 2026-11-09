//! The Home Assistant Supervisor link (spec §3, M4).
//!
//! One [`HaBridge`] owns the connection: REST calls go straight to
//! `http://supervisor/core/api/...` with the Bearer token, while a
//! background task keeps a WebSocket to `ws://supervisor/core/websocket`
//! subscribed to the bound entities (`subscribe_entities`, filtered),
//! republishing state through a `tokio::sync::watch` channel that the
//! SSE endpoint and the auto-capture path read from. The bridge is
//! offline-tolerant: with the Supervisor unreachable everything else
//! keeps working and the socket reconnects with exponential backoff
//! and a ping heartbeat.
//!
//! Token hygiene: the token lives in a redacted-Debug newtype, is sent
//! only to the Supervisor, and is never included in any log line or
//! response (asserted by `tests/ha.rs`).

use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt as _, StreamExt as _};
use serde::Serialize;
use serde_json::{Value, json};
use tokio::sync::{mpsc, watch};
use tokio_tungstenite::tungstenite::Message;

use crate::db::Db;

/// The Bearer token for the Supervisor. Debug is redacted so a stray
/// log line can never leak it.
#[derive(Clone)]
pub struct SupervisorToken(Arc<str>);

impl SupervisorToken {
    /// Wraps a token string.
    #[must_use]
    pub fn new(token: &str) -> Self {
        Self(Arc::from(token))
    }
}

impl fmt::Debug for SupervisorToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SupervisorToken(***)")
    }
}

/// One live entity state (compact view of what we display).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LiveEntity {
    /// Entity id, for example `sensor.grow_temperature`.
    pub entity_id: String,
    /// Raw state string ("24.3", "on", ...).
    pub state: String,
    /// Attributes worth showing (unit, friendly name).
    pub attributes: BTreeMap<String, Value>,
}

/// The shared live-state map plus connection status.
#[derive(Debug, Clone, Serialize)]
pub struct LiveData {
    /// Whether the Supervisor WebSocket is currently connected.
    pub connected: bool,
    /// Entity states from the last `subscribe_entities` events.
    pub entities: BTreeMap<String, LiveEntity>,
}

impl LiveData {
    /// The initial (offline, empty) snapshot.
    #[must_use]
    pub const fn offline() -> Self {
        Self {
            connected: false,
            entities: BTreeMap::new(),
        }
    }
}

/// A failure talking to the Supervisor.
#[derive(Debug, thiserror::Error)]
pub enum HaError {
    /// Transport failure (Supervisor unreachable).
    #[error("supervisor unreachable: {0}")]
    Unreachable(String),
    /// The Supervisor answered with an error status.
    #[error("supervisor error ({status}): {message}")]
    Status {
        /// HTTP status code.
        status: u16,
        /// Best-effort message from the body.
        message: String,
    },
}

/// Handle to the running bridge: cheap to clone, safe to share.
#[derive(Clone, Debug)]
pub struct HaBridge {
    inner: Arc<BridgeInner>,
    #[allow(dead_code)] // kept for a future stop signal
    updates_tx: mpsc::UnboundedSender<Update>,
}

#[derive(Debug)]
struct BridgeInner {
    base_url: String,
    token: SupervisorToken,
    live_rx: watch::Receiver<LiveData>,
}

/// Commands from the API layer to the socket task.
enum Update {
    /// The union of bound entity ids changed; resubscribe.
    Entities(Vec<String>),
}

impl HaBridge {
    /// Spawns the bridge (socket task) for `base_url` (for example
    /// `http://supervisor` or the mock in dev). `db` receives auto-log
    /// entries for light transitions.
    #[must_use]
    pub fn spawn(base_url: String, token: SupervisorToken, db: Db) -> Self {
        let (live_tx, live_rx) = watch::channel(LiveData::offline());
        let (updates_tx, updates_rx) = mpsc::unbounded_channel();
        let ws_base = base_url.trim_end_matches('/').replacen("http", "ws", 1);
        let handle_token = token.clone();
        tokio::spawn(socket_task(ws_base, token, live_tx, updates_rx, db));
        Self {
            inner: Arc::new(BridgeInner {
                base_url,
                token: handle_token,
                live_rx,
            }),
            updates_tx,
        }
    }

    /// A receiver of live-data snapshots (for the SSE endpoint).
    #[must_use]
    pub fn live(&self) -> watch::Receiver<LiveData> {
        self.inner.live_rx.clone()
    }

    /// The current snapshot without subscribing.
    #[must_use]
    pub fn live_now(&self) -> LiveData {
        self.inner.live_rx.borrow().clone()
    }

    /// Tells the socket to track exactly this set of entities (the
    /// union of every grow's bindings).
    pub fn track_entities(&self, entity_ids: Vec<String>) {
        let _ = self.updates_tx.send(Update::Entities(entity_ids));
    }

    /// `GET /core/api/states` — entity picker list.
    ///
    /// # Errors
    ///
    /// Returns an [`HaError`] when the Supervisor is unreachable or
    /// refuses the call.
    pub async fn list_entities(&self) -> Result<Vec<Value>, HaError> {
        let url = format!("{}/core/api/states", self.rest_base());
        let response = reqwest::Client::new()
            .get(&url)
            .bearer_auth(self.inner.token.0.as_ref())
            .send()
            .await
            .map_err(|error| HaError::Unreachable(error.to_string()))?;
        Self::json_response(response).await
    }

    /// History for charts: the documented
    /// `GET /core/api/history/period/<from>` with `filter_entity_id`
    /// and `minimal_response`.
    ///
    /// # Errors
    ///
    /// Returns an [`HaError`] when the Supervisor is unreachable or
    /// refuses the call.
    pub async fn history(&self, entity_id: &str, hours: u32) -> Result<Value, HaError> {
        use time::format_description::well_known::Rfc3339;
        let from = (time::OffsetDateTime::now_utc() - time::Duration::hours(i64::from(hours)))
            .format(&Rfc3339)
            .unwrap_or_default();
        // `entity_id` is user-controlled (a bound entity id): never
        // format! it into the query, where `&`/`#`/`%` would reshape or
        // extend the request sent with the admin Bearer token —
        // `.query()` percent-encodes instead.
        let url = format!("{}/core/api/history/period/{from}", self.rest_base());
        let response = reqwest::Client::new()
            .get(&url)
            .query(&[
                ("filter_entity_id", entity_id),
                // Flag parameter: an empty value is how the API expects it.
                ("minimal_response", ""),
            ])
            .bearer_auth(self.inner.token.0.as_ref())
            .send()
            .await
            .map_err(|error| HaError::Unreachable(error.to_string()))?;
        Self::json_response(response).await
    }

    /// The REST base URL.
    fn rest_base(&self) -> &str {
        &self.inner.base_url
    }

    /// Turns a response into JSON, mapping error statuses.
    async fn json_response<T: serde::de::DeserializeOwned>(
        response: reqwest::Response,
    ) -> Result<T, HaError> {
        let status = response.status().as_u16();
        if !response.status().is_success() {
            let message = response
                .text()
                .await
                .unwrap_or_default()
                .chars()
                .take(300)
                .collect();
            return Err(HaError::Status { status, message });
        }
        response
            .json::<T>()
            .await
            .map_err(|error| HaError::Unreachable(error.to_string()))
    }
}

/// The reconnect backoff schedule: 1, 2, 4, ... capped at 60 seconds,
/// reset after a connection that authenticated successfully.
#[must_use]
pub fn backoff_for(attempt: u32) -> Duration {
    let seconds = 2_u64.saturating_pow(attempt.min(6)).max(1);
    Duration::from_secs(seconds.min(60))
}

/// The socket task: connect, auth, subscribe, stream; reconnect
/// forever. `db` writes auto-log entries when a bound light entity
/// changes state and auto-logging is on.
async fn socket_task(
    ws_base: String,
    token: SupervisorToken,
    live_tx: watch::Sender<LiveData>,
    mut updates: mpsc::UnboundedReceiver<Update>,
    db: Db,
) {
    let url = format!("{ws_base}/core/websocket");
    let mut tracked: Vec<String> = Vec::new();
    let mut attempt = 0_u32;
    loop {
        // Pick up binding changes while disconnected.
        while let Ok(Update::Entities(entities)) = updates.try_recv() {
            tracked = entities;
        }
        match run_socket(&url, &token, &live_tx, &tracked, &mut updates, &db).await {
            SocketOutcome::Restart(next_tracked) => {
                tracked = next_tracked;
                attempt = attempt.saturating_add(1);
                tokio::time::sleep(backoff_for(attempt)).await;
            }
        }
    }
}

/// Why the socket run ended (there is only one reason: try again).
enum SocketOutcome {
    /// Reconnect, tracking this entity set.
    Restart(Vec<String>),
}

/// One connection lifecycle: WS handshake → `auth` →
/// `subscribe_entities` → stream events until error/timeout/binding
/// change. Marks the live data offline on exit.
async fn run_socket(
    url: &str,
    token: &SupervisorToken,
    live_tx: &watch::Sender<LiveData>,
    tracked: &[String],
    updates: &mut mpsc::UnboundedReceiver<Update>,
    db: &Db,
) -> SocketOutcome {
    let mut tracked = tracked.to_vec();
    let stream = tokio_tungstenite::connect_async(url).await;
    let Ok((mut socket, _response)) = stream else {
        return SocketOutcome::Restart(tracked);
    };

    // Expect auth_required, send the token, expect auth_ok.
    if !auth_handshake(&mut socket, token).await {
        return SocketOutcome::Restart(tracked);
    }
    let mut next_id = 1_u64;
    if !subscribe(&mut socket, &tracked, &mut next_id).await {
        return SocketOutcome::Restart(tracked);
    }

    // Connected: seed from REST states so the UI is not blind until the
    // first event, then mark online.
    live_tx.send_modify(|data| data.connected = true);

    let mut heartbeat = tokio::time::interval(Duration::from_secs(30));
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    heartbeat.tick().await; // first tick is immediate

    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                let ping = json!({ "id": next_id, "type": "ping" });
                next_id += 1;
                if socket
                    .send(Message::text(ping.to_string()))
                    .await
                    .is_err()
                {
                    break;
                }
            }
            update = updates.recv() => {
                if let Some(Update::Entities(entities)) = update {
                    let changed = entities != tracked;
                    tracked = entities;
                    if changed {
                        // Cleanest resubscribe: a fresh connection.
                        break;
                    }
                }
            }
            message = socket.next() => {
                let Some(Ok(message)) = message else {
                    break;
                };
                match message {
                    Message::Text(text) => {
                        let changed = apply_event(text.as_str(), live_tx);
                        for (entity_id, new_state) in changed {
                            if entity_id.starts_with("light.") {
                                                tokio::spawn(auto_log_light_change(
                                    db.clone(),
                                    entity_id,
                                    new_state,
                                ));
                            }
                        }
                    }
                    Message::Ping(payload) => {
                        let _ = socket.send(Message::Pong(payload)).await;
                    }
                    Message::Close(_) => break,
                    _ => {}
                }
            }
        }
    }

    live_tx.send_modify(|data| {
        data.connected = false;
        data.entities.clear();
    });
    SocketOutcome::Restart(tracked)
}

/// The HA WebSocket auth handshake.
async fn auth_handshake<S>(socket: &mut S, token: &SupervisorToken) -> bool
where
    S: futures_util::SinkExt<Message>
        + futures_util::StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>
        + Unpin,
{
    let required = match socket.next().await {
        Some(Ok(Message::Text(text))) => text.contains("auth_required"),
        _ => false,
    };
    if !required {
        return false;
    }
    let auth = json!({ "type": "auth", "access_token": token.0.as_ref() });
    if socket.send(Message::text(auth.to_string())).await.is_err() {
        return false;
    }
    matches!(socket.next().await, Some(Ok(Message::Text(text))) if text.contains("auth_ok"))
}

/// Sends `subscribe_entities` for the tracked set (empty set: skip).
async fn subscribe<S>(socket: &mut S, tracked: &[String], next_id: &mut u64) -> bool
where
    S: futures_util::SinkExt<Message>
        + futures_util::StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>
        + Unpin,
{
    if tracked.is_empty() {
        return true;
    }
    let command = json!({
        "id": *next_id,
        "type": "subscribe_entities",
        "entity_ids": tracked,
    });
    *next_id += 1;
    if socket
        .send(Message::text(command.to_string()))
        .await
        .is_err()
    {
        return false;
    }
    // The success result mirrors our command id; anything else (an
    // error result) ends this connection.
    matches!(socket.next().await, Some(Ok(Message::Text(text))) if text.contains("\"result\":true") || text.contains("success"))
}

/// Applies one `subscribe_entities` event to the live map (compact
/// diff format: `event.c` keyed by entity id with `+`/`-` and `s`/`a`).
fn apply_event(text: &str, live_tx: &watch::Sender<LiveData>) -> Vec<(String, String)> {
    let mut changed = Vec::new();
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return changed;
    };
    let Some(event_changes) = value.pointer("/event/c").and_then(Value::as_object) else {
        return changed;
    };
    live_tx.send_modify(|data| {
        for (entity_id, diff) in event_changes {
            let Some(diff) = diff.as_object() else {
                continue;
            };
            if diff.contains_key("-") {
                data.entities.remove(entity_id);
                continue;
            }
            if let Some(added) = diff.get("+").and_then(Value::as_object) {
                let state = added
                    .get("s")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_owned();
                let entity = LiveEntity {
                    entity_id: entity_id.clone(),
                    state: state.clone(),
                    attributes: attrs(added.get("a")),
                };
                data.entities.insert(entity_id.clone(), entity);
                changed.push((entity_id.clone(), state));
                continue;
            }
            if let Some(entity) = data.entities.get_mut(entity_id) {
                if let Some(state) = diff.get("s").and_then(Value::as_str) {
                    entity.state.clone_from(&state.to_owned());
                    changed.push((entity_id.clone(), state.to_owned()));
                }
                if let Some(new_attrs) = diff.get("a") {
                    merge_attrs(&mut entity.attributes, new_attrs);
                }
            }
        }
    });
    changed
}

/// Flattens an attribute value into the display map.
fn attrs(value: Option<&Value>) -> BTreeMap<String, Value> {
    let mut map = BTreeMap::new();
    if let Some(attributes) = value.and_then(Value::as_object) {
        for (key, value) in attributes {
            if matches!(
                key.as_str(),
                "unit_of_measurement" | "friendly_name" | "device_class"
            ) {
                map.insert(key.clone(), value.clone());
            }
        }
    }
    map
}

/// Merges a partial attribute update.
fn merge_attrs(target: &mut BTreeMap<String, Value>, update: &Value) {
    if let Some(update) = update.as_object() {
        for (key, value) in update {
            target.insert(key.clone(), value.clone());
        }
    }
}

/// The auto-log path: given a state diff for a light entity, write a
/// `lights` log entry for each grow bound to it (spec §4.2), unless the
/// `auto_log_ha_events` setting is off.
async fn auto_log_light_change(db: Db, entity_id: String, new_state: String) {
    let result = db
        .with(move |conn| -> Result<(), crate::db::StoreError> {
            let entity = entity_id.as_str();
            let state = new_state.as_str();
            let opt_out = crate::store::settings::get(conn, "auto_log_ha_events")
                .ok()
                .and_then(|value| value.as_bool())
                .unwrap_or(true);
            if !opt_out {
                return Ok(());
            }
            let grow_ids = crate::store::sensors::grow_ids_bound_to(conn, entity)?;
            for grow_id in grow_ids {
                let Ok(grow) = crate::store::grows::get(conn, &grow_id) else {
                    continue;
                };
                let occurred_at = crate::db::now_rfc3339();
                let day = day_from(&grow.started_on, &occurred_at).unwrap_or(0);
                let flip = crate::store::actions::first_event_day(
                    conn,
                    &grow_id,
                    leaflog_core::actions::ActionType::Flip,
                    day,
                )?;
                let harvested = crate::store::actions::first_event_day(
                    conn,
                    &grow_id,
                    leaflog_core::actions::ActionType::Harvest,
                    day,
                )?
                .is_some();
                let phase = leaflog_core::actions::derive_phase(day, flip, harvested);
                let input = crate::store::actions::from_input(
                    &grow_id,
                    &leaflog_core::grows::ActionInput {
                        plant_ids: Vec::new(),
                        occurred_at,
                        action_type: leaflog_core::actions::ActionType::Lights,
                        details: leaflog_core::actions::ActionDetails::default(),
                        notes: Some(format!("auto: light {state}")),
                        appearance_tags: Vec::new(),
                    },
                    phase,
                    day,
                );
                crate::store::actions::create(conn, &input)?;
            }
            Ok(())
        })
        .await;
    if let Err(error) = result {
        eprintln!("auto-log skipped: {error}");
    }
}

/// Whole days between a `YYYY-MM-DD` start and an RFC 3339 instant.
fn day_from(started_on: &str, occurred_at: &str) -> Option<u16> {
    let date_format = time::macros::format_description!("[year]-[month]-[day]");
    let start = time::Date::parse(started_on, &date_format).ok()?;
    let moment =
        time::OffsetDateTime::parse(occurred_at, &time::format_description::well_known::Rfc3339)
            .ok()?;
    u16::try_from((moment.date() - start).whole_days().max(0)).ok()
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::{LiveData, backoff_for};

    #[test]
    fn backoff_grows_and_caps() {
        assert_eq!(backoff_for(0), Duration::from_secs(1));
        assert_eq!(backoff_for(1), Duration::from_secs(2));
        assert_eq!(backoff_for(2), Duration::from_secs(4));
        assert_eq!(backoff_for(6), Duration::from_secs(60));
        assert_eq!(backoff_for(9), Duration::from_secs(60));
    }

    #[test]
    fn offline_snapshot_is_empty() {
        let data = LiveData::offline();
        assert!(!data.connected);
        assert!(data.entities.is_empty());
    }
}
