//! The Home Assistant Supervisor link (spec §3, M4).
//!
//! One [`HaBridge`] owns the connection: REST calls go straight to
//! `http://supervisor/core/api/...` with the Bearer token. A
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
use leaflog_core::units::UnitSystem;
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

impl LiveEntity {
    /// Reads a temperature sensor in canonical Celsius.
    #[must_use]
    pub fn temperature_c(&self) -> Option<f64> {
        let value = self.state.parse().ok()?;
        let unit = self
            .attributes
            .get("unit_of_measurement")
            .and_then(Value::as_str);
        leaflog_core::units::sensor_celsius(value, unit)
    }

    /// A dimmable light's level in percent of full output: HA reports
    /// `brightness` on a 0–255 scale and keeps it while the light is
    /// off, so the lamp advice does not flip every night. `None` when
    /// the entity reports no brightness.
    #[must_use]
    pub fn brightness_pct(&self) -> Option<u8> {
        let brightness = self.attributes.get("brightness")?.as_u64()?;
        u8::try_from(brightness.min(255) * 100 / 255).ok()
    }
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
    updates_tx: mpsc::UnboundedSender<Update>,
}

#[derive(Debug)]
struct BridgeInner {
    base_url: String,
    token: SupervisorToken,
    live_rx: watch::Receiver<LiveData>,
    timezone_rx: watch::Receiver<Option<String>>,
    language_rx: watch::Receiver<Option<String>>,
    units_rx: watch::Receiver<UnitSystem>,
    /// Shared REST client (audit D145): one connection pool for all
    /// Supervisor calls instead of a fresh client per call.
    http: reqwest::Client,
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
        Self::spawn_with_heartbeat(base_url, token, db, HEARTBEAT)
    }

    /// Like [`Self::spawn`], with a custom ping interval. The socket is
    /// dropped once it stays silent for 2.5 intervals. Tests use short
    /// intervals; production takes [`HEARTBEAT`].
    #[must_use]
    pub fn spawn_with_heartbeat(
        base_url: String,
        token: SupervisorToken,
        db: Db,
        heartbeat: Duration,
    ) -> Self {
        let (live_tx, live_rx) = watch::channel(LiveData::offline());
        let (timezone_tx, timezone_rx) = watch::channel(None);
        let (language_tx, language_rx) = watch::channel(None);
        let (units_tx, units_rx) = watch::channel(UnitSystem::default());
        let (updates_tx, updates_rx) = mpsc::unbounded_channel();
        let ws_base = base_url.trim_end_matches('/').replacen("http", "ws", 1);
        let rest_base = base_url.trim_end_matches('/').to_owned();
        let handle_token = token.clone();
        // One shared client for every REST call to the Supervisor:
        // connection reuse instead of a fresh dial per request
        // (D145 audit fix). Bounded waits: without deadlines a stalled
        // Supervisor would park the reconnect loop's config refresh
        // and every picker or history request forever.
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .unwrap_or_default();
        tokio::spawn(socket_task(
            ws_base,
            live_tx,
            updates_rx,
            db,
            SocketRest {
                rest_base,
                token,
                http: http.clone(),
                heartbeat,
            },
            ConfigWatches {
                timezone: timezone_tx,
                language: language_tx,
                units: units_tx,
            },
        ));
        Self {
            inner: Arc::new(BridgeInner {
                base_url,
                token: handle_token,
                live_rx,
                timezone_rx,
                language_rx,
                units_rx,
                http,
            }),
            updates_tx,
        }
    }

    /// The Home Assistant timezone (IANA name) from the documented
    /// `GET /core/api/config`, refreshed with every reconnect; `None`
    /// until fetched or when HA is unreachable (callers fall back to
    /// the settings override, then UTC).
    #[must_use]
    pub fn cached_timezone(&self) -> Option<String> {
        self.inner.timezone_rx.borrow().clone()
    }

    /// The Home Assistant user language (BCP-47 tag) from the
    /// documented `GET /core/api/config`, refreshed with every
    /// reconnect; `None` until fetched or when HA is unreachable (the
    /// locale chain falls through to the browser locale, then
    /// English). Read-only plumbing — the backend never translates.
    #[must_use]
    pub fn cached_language(&self) -> Option<String> {
        self.inner.language_rx.borrow().clone()
    }

    /// The last known HA unit system, or metric before a config fetch.
    #[must_use]
    pub fn cached_units(&self) -> UnitSystem {
        *self.inner.units_rx.borrow()
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

    /// Raises a persistent notification in Home Assistant (spec §4.9
    /// env-alert routing): the documented
    /// `POST /core/api/services/notify/persistent_notification`.
    ///
    /// # Errors
    ///
    /// Returns an [`HaError`] when the Supervisor is unreachable or
    /// refuses the call.
    pub async fn notify(&self, title: &str, message: &str) -> Result<(), HaError> {
        let url = format!(
            "{}/core/api/services/notify/persistent_notification",
            self.rest_base()
        );
        let response = self
            .inner
            .http
            .post(&url)
            .bearer_auth(self.inner.token.0.as_ref())
            .json(&serde_json::json!({ "title": title, "message": message }))
            .send()
            .await
            .map_err(|error| HaError::Unreachable(error.to_string()))?;
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
        Ok(())
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
        let response = self
            .inner
            .http
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
        self.history_between(entity_id, &from, None).await
    }

    /// Fetches one entity's recorded history over a bounded interval.
    ///
    /// # Errors
    ///
    /// Returns an error when Home Assistant cannot serve the request.
    pub async fn history_between(
        &self,
        entity_id: &str,
        from: &str,
        to: Option<&str>,
    ) -> Result<Value, HaError> {
        let url = format!("{}/core/api/history/period/{from}", self.rest_base());
        let mut query = vec![("filter_entity_id", entity_id), ("minimal_response", "")];
        if let Some(to) = to {
            query.push(("end_time", to));
        }
        let response = self
            .inner
            .http
            .get(&url)
            .query(&query)
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

/// The watch publishers for the config fields refreshed on every
/// reconnect: the canonical timezone (spec §2) and the user language
/// (D112), bundled so `socket_task` stays within its argument budget.
#[derive(Debug)]
struct ConfigWatches {
    /// Canonical IANA timezone of the HA instance.
    timezone: watch::Sender<Option<String>>,
    /// BCP-47 user language of the HA instance.
    language: watch::Sender<Option<String>>,
    /// Unit system configured in HA.
    units: watch::Sender<UnitSystem>,
}

/// The REST client and socket timing of the socket task, bundled so
/// it stays within its argument budget (same pattern as
/// [`ConfigWatches`]).
#[derive(Debug)]
struct SocketRest {
    /// `http://supervisor`-style base for the REST calls.
    rest_base: String,
    /// The Supervisor token for those calls.
    token: SupervisorToken,
    /// The bridge's shared HTTP client (connection reuse, D145).
    http: reqwest::Client,
    /// The socket's ping interval ([`HEARTBEAT`] in production).
    heartbeat: Duration,
}

/// How often the socket pings the Supervisor. Every inbound frame
/// counts as a sign of life; a socket silent for 2.5 intervals is
/// dropped, because a half-open TCP connection never delivers an
/// error on its own.
const HEARTBEAT: Duration = Duration::from_secs(30);

/// The socket task: connect, auth, subscribe, stream; reconnect
/// forever. `db` writes auto-log entries when a bound light entity
/// changes state and auto-logging is on.
async fn socket_task(
    ws_base: String,
    live_tx: watch::Sender<LiveData>,
    mut updates: mpsc::UnboundedReceiver<Update>,
    db: Db,
    rest: SocketRest,
    watches: ConfigWatches,
) {
    let url = format!("{ws_base}/core/websocket");
    let mut tracked: Vec<String> = Vec::new();
    let mut attempt = 0_u32;
    loop {
        // Pick up binding changes while disconnected.
        while let Ok(Update::Entities(entities)) = updates.try_recv() {
            tracked = entities;
        }
        // Refresh the zone, language and units on every reconnect.
        // If HA is unreachable, keep the last known values.
        refresh_config(&rest.rest_base, &rest.token, &rest.http, &watches).await;
        let timezone = watches.timezone.borrow().clone();
        let outcome = tokio::select! {
            outcome = run_socket(&url, &rest, &live_tx, &tracked, &mut updates, &db, timezone) => outcome,
            () = refresh_config_periodically(&rest, &watches) => unreachable!("config loop never ends"),
        };
        match outcome {
            SocketOutcome::Retry(next_tracked) => {
                tracked = next_tracked;
                attempt = attempt.saturating_add(1);
                tokio::time::sleep(backoff_for(attempt)).await;
            }
            SocketOutcome::Resubscribe(next_tracked) => {
                tracked = next_tracked;
                // The connection authenticated and streamed — HA is
                // reachable, so the failure backoff starts over.
                attempt = 0;
            }
        }
    }
}

// HA can change settings without dropping the socket.
async fn refresh_config_periodically(rest: &SocketRest, watches: &ConfigWatches) {
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
        refresh_config(&rest.rest_base, &rest.token, &rest.http, watches).await;
    }
}

/// Publishes the timezone, language and unit system from
/// `GET /core/api/config`. Failures keep the last known values.
async fn refresh_config(
    rest_base: &str,
    token: &SupervisorToken,
    http: &reqwest::Client,
    watches: &ConfigWatches,
) {
    let url = format!("{rest_base}/core/api/config");
    let fetched = http
        .get(&url)
        .bearer_auth(token.0.as_ref())
        .send()
        .await
        .and_then(reqwest::Response::error_for_status);
    let fetched = match fetched {
        Ok(response) => HaBridge::json_response::<serde_json::Value>(response).await,
        Err(error) => Err(HaError::Unreachable(error.to_string())),
    };
    let Ok(config) = fetched else {
        return;
    };
    if let Some(zone) = config["timezone"].as_str() {
        let _ = watches.timezone.send(Some(zone.to_owned()));
    }
    // HA's metric and US customary systems advertise Celsius and Fahrenheit.
    let units = match config["unit_system"]["temperature"].as_str() {
        Some("°F") => Some(UnitSystem::Imperial),
        Some("°C") => Some(UnitSystem::Metric),
        _ => None,
    };
    if let Some(units) = units {
        let _ = watches.units.send(units);
    }
    if let Some(language) = config["language"].as_str() {
        let _ = watches.language.send(Some(language.to_owned()));
        // The API error wrappers read the language from the catalog's
        // process-global (D113) — published alongside the watch.
        crate::catalog::publish_language(Some(language));
    }
}

/// Why the socket run ended: retry with backoff, or resubscribe now.
enum SocketOutcome {
    /// The connection failed (dial, auth, subscribe, stream error):
    /// reconnect with failure backoff, tracking this entity set.
    Retry(Vec<String>),
    /// The bound-entity set changed on a live connection:
    /// resubscribe immediately — HA is healthy, so no backoff, and a
    /// connection that got this far resets the failure backoff.
    Resubscribe(Vec<String>),
}

/// One connection lifecycle: WS handshake → `auth` →
/// `subscribe_entities` → stream events until error, silence past
/// 2.5 heartbeats, or a binding change. Marks the live data offline
/// on exit.
async fn run_socket(
    url: &str,
    rest: &SocketRest,
    live_tx: &watch::Sender<LiveData>,
    tracked: &[String],
    updates: &mut mpsc::UnboundedReceiver<Update>,
    db: &Db,
    timezone: Option<String>,
) -> SocketOutcome {
    let mut tracked = tracked.to_vec();
    let stream = tokio_tungstenite::connect_async(url).await;
    let Ok((mut socket, _response)) = stream else {
        return SocketOutcome::Retry(tracked);
    };

    // A socket that goes quiet anywhere before or during the stream
    // loop is dropped after the same idle limit.
    let idle_after = rest.heartbeat.saturating_mul(5) / 2;

    // Expect auth_required, send the token, expect auth_ok.
    if !auth_handshake(&mut socket, &rest.token, idle_after).await {
        return SocketOutcome::Retry(tracked);
    }
    let mut next_id = 1_u64;
    if !subscribe(&mut socket, &tracked, &mut next_id, idle_after).await {
        return SocketOutcome::Retry(tracked);
    }

    // Connected. HA's `subscribe_entities` initial batch (an
    // `event.a` payload) seeds the live map the moment it arrives,
    // so there is nothing to fetch from REST here.
    live_tx.send_modify(|data| data.connected = true);

    // A binding-set change on this live connection exits the stream
    // loop with a resubscribe (no failure backoff); every other break
    // is a connection failure and retries with backoff.
    let mut outcome = SocketOutcome::Retry(tracked.clone());

    // Process-local debounce for light auto-logs (audit D145): a
    // flickering or looping automation must not flood the timeline.
    // Keyed by (entity, state), so a genuine on→off→on within the
    // window still logs each distinct transition once.
    let mut last_light_log: std::collections::HashMap<(String, String), std::time::Instant> =
        std::collections::HashMap::new();

    let mut heartbeat = tokio::time::interval(rest.heartbeat);
    heartbeat.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    heartbeat.tick().await; // first tick is immediate
    let mut last_seen = tokio::time::Instant::now();

    loop {
        tokio::select! {
            _ = heartbeat.tick() => {
                // The pong is the sign of life we wait for. Checking
                // the clock instead of matching pong ids tolerates
                // any frame order and needs no extra state.
                if last_seen.elapsed() >= idle_after {
                    break;
                }
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
                        // HA is healthy — this is not a failure, so
                        // the reconnect skips the backoff and the
                        // failure counter resets.
                        outcome = SocketOutcome::Resubscribe(tracked.clone());
                        break;
                    }
                }
            }
            message = socket.next() => {
                let Some(Ok(message)) = message else {
                    break;
                };
                last_seen = tokio::time::Instant::now();
                match message {
                    Message::Text(text) => {
                        let changed = apply_event(text.as_str(), live_tx);
                        for (entity_id, new_state) in changed {
                            if entity_id.starts_with("light.") {
                                let key = (entity_id.clone(), new_state.clone());
                                let now = std::time::Instant::now();
                                // Fresh transition (never seen, or the
                                // window has passed): log it and start
                                // a new window. A repeat inside the
                                // window is dropped.
                                let log_it = !last_light_log.get(&key).is_some_and(|seen| {
                                    now.duration_since(*seen) < LIGHT_LOG_DEBOUNCE
                                });
                                if log_it {
                                    last_light_log.insert(key, now);
                                    tokio::spawn(auto_log_light_change(
                                        db.clone(),
                                        entity_id,
                                        new_state,
                                        timezone.clone(),
                                    ));
                                }
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
    outcome
}

/// The HA WebSocket auth handshake.
async fn auth_handshake<S>(socket: &mut S, token: &SupervisorToken, idle_after: Duration) -> bool
where
    S: futures_util::SinkExt<Message>
        + futures_util::StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>
        + Unpin,
{
    let required = match tokio::time::timeout(idle_after, socket.next()).await {
        Ok(Some(Ok(Message::Text(text)))) => text.contains("auth_required"),
        _ => false,
    };
    if !required {
        return false;
    }
    let auth = json!({ "type": "auth", "access_token": token.0.as_ref() });
    if socket.send(Message::text(auth.to_string())).await.is_err() {
        return false;
    }
    matches!(
        tokio::time::timeout(idle_after, socket.next()).await,
        Ok(Some(Ok(Message::Text(text)))) if text.contains("auth_ok")
    )
}

/// Sends `subscribe_entities` for the tracked set (empty set: skip).
async fn subscribe<S>(
    socket: &mut S,
    tracked: &[String],
    next_id: &mut u64,
    idle_after: Duration,
) -> bool
where
    S: futures_util::SinkExt<Message>
        + futures_util::StreamExt<Item = Result<Message, tokio_tungstenite::tungstenite::Error>>
        + Unpin,
{
    if tracked.is_empty() {
        return true;
    }
    let command_id = *next_id;
    let command = json!({
        "id": command_id,
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
    // Only a `result` for our command id with `success == true` counts
    // as subscribed. An error result fails, ending this connection so
    // the reconnect path with backoff runs instead of idling on a dead
    // subscription. (HA accepts unknown entity ids silently — they
    // simply never appear in the initial batch — so an error here is
    // rare: a malformed command, or the user losing access.)
    match tokio::time::timeout(idle_after, socket.next()).await {
        Ok(Some(Ok(Message::Text(text)))) => {
            let Ok(value) = serde_json::from_str::<Value>(text.as_str()) else {
                return false;
            };
            value.get("id").and_then(Value::as_u64) == Some(command_id)
                && value.get("success").and_then(Value::as_bool) == Some(true)
        }
        _ => false,
    }
}

/// Applies one `subscribe_entities` event to the live map. HA's
/// compact dialect (websocket `messages.py::_state_diff_event`) has
/// three shapes:
///
/// - `event.a` seeds full compressed states `{entity: {s, a, ...}}` —
///   the initial batch after subscribing and entities added while
///   subscribed;
/// - `event.c` carries per-entity diffs: `+` additions (a new state
///   and/or attribute values), `-` the attribute keys removed;
/// - `event.r` lists entity ids dropped from the state machine.
///
/// Seeding lands entities in the map but is not a state change; only
/// `c`-diff state updates are returned as `(entity, state)` pairs, so
/// the light auto-log records flips, not reconnects. Bare `s`/`a` keys
/// at the entity level (an older abridged shape) are still merged.
fn apply_event(text: &str, live_tx: &watch::Sender<LiveData>) -> Vec<(String, String)> {
    let mut changed = Vec::new();
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return changed;
    };
    live_tx.send_modify(|data| {
        // Seed: the initial batch and mid-subscription additions are
        // full compressed states without a diff wrapper.
        let added = value.pointer("/event/a").and_then(Value::as_object);
        for (entity_id, compressed) in added.into_iter().flatten() {
            let Some(state) = compressed.get("s").and_then(Value::as_str) else {
                continue;
            };
            let entity = data
                .entities
                .entry(entity_id.clone())
                .or_insert_with(|| LiveEntity {
                    entity_id: entity_id.clone(),
                    state: String::new(),
                    attributes: BTreeMap::new(),
                });
            state.clone_into(&mut entity.state);
            entity.attributes.extend(attrs(compressed.get("a")));
        }

        // Diffs: `+` additions and `-` attribute-key removals (a `-`
        // here never removes the entity — that is `event.r`).
        let diffs = value.pointer("/event/c").and_then(Value::as_object);
        for (entity_id, diff) in diffs.into_iter().flatten() {
            if let Some(keys) = diff.pointer("/-/a").and_then(Value::as_array)
                && let Some(entity) = data.entities.get_mut(entity_id)
            {
                for key in keys.iter().filter_map(Value::as_str) {
                    entity.attributes.remove(key);
                }
            }
            // `+` additions, or tolerated bare `s`/`a` at the entity
            // level (an older abridged shape).
            let updates = diff.get("+").unwrap_or(diff);
            let Some(updates) = updates.as_object() else {
                continue;
            };
            let entity = data
                .entities
                .entry(entity_id.clone())
                .or_insert_with(|| LiveEntity {
                    entity_id: entity_id.clone(),
                    state: String::new(),
                    attributes: BTreeMap::new(),
                });
            if let Some(state) = updates.get("s").and_then(Value::as_str) {
                state.clone_into(&mut entity.state);
                changed.push((entity_id.clone(), state.to_owned()));
            }
            if let Some(new_attrs) = updates.get("a") {
                merge_attrs(&mut entity.attributes, new_attrs);
            }
        }

        // Removals: entity ids dropped from the state machine.
        let removed = value.pointer("/event/r").and_then(Value::as_array);
        for entity_id in removed.into_iter().flatten() {
            if let Some(id) = entity_id.as_str() {
                data.entities.remove(id);
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
                "unit_of_measurement" | "friendly_name" | "device_class" | "brightness"
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

/// How long the same light state on the same entity stays suppressed
/// after being logged (audit D145): a flickering or looping
/// automation must not flood the timeline. Normal operation is two
/// transitions a day, so a one-minute window costs nothing.
const LIGHT_LOG_DEBOUNCE: std::time::Duration = std::time::Duration::from_secs(60);

/// The auto-log path: given a state diff for a light entity, write a
/// `lights` log entry for each grow bound to it (spec §4.2), unless
/// the `auto_log_ha_events` setting is off or the same transition was
/// just logged ([`LIGHT_LOG_DEBOUNCE`] window).
async fn auto_log_light_change(
    db: Db,
    entity_id: String,
    new_state: String,
    timezone: Option<String>,
) {
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
                // Local calendar days in the canonical zone (spec §2);
                // UTC math when no zone is known.
                let day = timezone
                    .as_deref()
                    .and_then(|tz| crate::app::day_number_in(&grow.started_on, &occurred_at, tz))
                    .or_else(|| crate::app::day_number_utc(&grow.started_on, &occurred_at))
                    .unwrap_or(0);
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

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use serde_json::json;
    use tokio::sync::watch;

    use std::collections::BTreeMap;

    use super::{LiveData, LiveEntity, apply_event, backoff_for};

    #[test]
    fn brightness_is_reported_while_the_light_is_off() {
        let mut attributes = BTreeMap::new();
        attributes.insert("brightness".to_owned(), json!(128));
        let led = LiveEntity {
            entity_id: "light.grow_led".to_owned(),
            state: "off".to_owned(),
            attributes,
        };
        assert_eq!(led.brightness_pct(), Some(50));

        let plain = LiveEntity {
            entity_id: "light.grow_led".to_owned(),
            state: "on".to_owned(),
            attributes: BTreeMap::new(),
        };
        assert_eq!(plain.brightness_pct(), None);
    }

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

    /// Applies events to a fresh offline live map and returns the
    /// receiver plus the pairs the last event reported as changes.
    fn apply(events: &[serde_json::Value]) -> (watch::Receiver<LiveData>, Vec<(String, String)>) {
        let (tx, rx) = watch::channel(LiveData::offline());
        let mut changed = Vec::new();
        for event in events {
            changed = apply_event(&event.to_string(), &tx);
        }
        (rx, changed)
    }

    /// The bug behind the "Keine Daten" banner: real HA seeds
    /// subscribers with a full batch under `event.a` — a shape the
    /// old parser dropped, so steady entities never reached the map.
    #[test]
    fn the_subscribe_seed_populates_the_map_without_reporting_changes() {
        let seed = json!({ "event": { "a": {
            "light.grow_led": { "s": "on", "a": { "friendly_name": "Grow LED" } },
        } } });
        let (rx, changed) = apply(&[seed]);
        assert_eq!(
            changed,
            [] as [(String, String); 0],
            "seeding is not a state change"
        );
        let light = rx
            .borrow()
            .entities
            .get("light.grow_led")
            .cloned()
            .expect("the seed lands the light");
        assert_eq!(light.state, "on");
        assert_eq!(
            light.attributes.get("friendly_name"),
            Some(&json!("Grow LED"))
        );
    }

    #[test]
    fn a_plus_diff_updates_the_state_and_reports_the_change() {
        let seed = json!({ "event": { "a": { "light.grow_led": { "s": "off" } } } });
        let flip = json!({ "event": { "c": { "light.grow_led": { "+": { "s": "on" } } } } });
        let (rx, changed) = apply(&[seed, flip]);
        assert_eq!(
            changed,
            vec![("light.grow_led".to_owned(), "on".to_owned())]
        );
        assert_eq!(rx.borrow().entities["light.grow_led"].state, "on");
    }

    #[test]
    fn attribute_only_diffs_merge_without_reporting_a_change() {
        let seed = json!({ "event": { "a": {
            "sensor.grow_humidity": { "s": "44", "a": { "unit_of_measurement": "%" } },
        } } });
        let attrs = json!({ "event": { "c": {
            "sensor.grow_humidity": { "+": { "a": { "unit_of_measurement": " %" } } },
        } } });
        let (rx, changed) = apply(&[seed, attrs]);
        assert_eq!(changed, [] as [(String, String); 0]);
        let sensor = rx.borrow().entities["sensor.grow_humidity"].clone();
        assert_eq!(sensor.state, "44", "the state survives an attrs-only diff");
        assert_eq!(
            sensor.attributes.get("unit_of_measurement"),
            Some(&json!(" %"))
        );
    }

    /// A `-` inside a diff removes attribute keys only; the entity
    /// itself is dropped by `event.r` (tested below).
    #[test]
    fn minus_in_a_diff_drops_attribute_keys_but_keeps_the_entity() {
        let seed = json!({ "event": { "a": {
            "light.grow_led": { "s": "on", "a": { "friendly_name": "Grow LED", "effect": "bloom" } },
        } } });
        let drop = json!({ "event": { "c": {
            "light.grow_led": { "-": { "a": ["effect"] } },
        } } });
        let (rx, changed) = apply(&[seed, drop]);
        assert_eq!(changed, [] as [(String, String); 0]);
        let light = rx.borrow().entities["light.grow_led"].clone();
        assert_eq!(light.state, "on", "the entity survives the diff");
        assert!(!light.attributes.contains_key("effect"));
        assert_eq!(
            light.attributes.get("friendly_name"),
            Some(&json!("Grow LED"))
        );
    }

    #[test]
    fn removals_drop_the_entity_from_the_map() {
        let seed = json!({ "event": { "a": { "light.grow_led": { "s": "on" } } } });
        let removal = json!({ "event": { "r": ["light.grow_led"] } });
        let (rx, changed) = apply(&[seed, removal]);
        assert_eq!(changed, [] as [(String, String); 0]);
        assert!(!rx.borrow().entities.contains_key("light.grow_led"));
    }

    /// HA also emits `c` diffs for entities created while subscribed
    /// (old state unknown) — the map grows instead of ignoring them.
    #[test]
    fn a_diff_on_an_unknown_entity_creates_it() {
        let diff = json!({ "event": { "c": { "light.new_lamp": { "+": { "s": "on" } } } } });
        let (rx, changed) = apply(&[diff]);
        assert_eq!(
            changed,
            vec![("light.new_lamp".to_owned(), "on".to_owned())]
        );
        assert_eq!(rx.borrow().entities["light.new_lamp"].state, "on");
    }

    /// Older abridged messages carried bare `s`/`a` at the entity
    /// level without a `+` wrapper; tolerate them alongside the
    /// documented shapes.
    #[test]
    fn bare_s_and_a_keys_are_still_merged() {
        let seed = json!({ "event": { "a": { "sensor.grow_co2": { "s": "850" } } } });
        let legacy = json!({ "event": { "c": {
            "sensor.grow_co2": { "s": "852", "a": { "unit_of_measurement": "ppm" } },
        } } });
        let (rx, changed) = apply(&[seed, legacy]);
        assert_eq!(
            changed,
            vec![("sensor.grow_co2".to_owned(), "852".to_owned())]
        );
        let sensor = rx.borrow().entities["sensor.grow_co2"].clone();
        assert_eq!(sensor.state, "852");
        assert_eq!(
            sensor.attributes.get("unit_of_measurement"),
            Some(&json!("ppm"))
        );
    }
}
