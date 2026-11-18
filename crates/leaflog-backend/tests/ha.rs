//! Integration tests for M4: live data through the mock Supervisor,
//! auto-capture, bindings, history, the SSE stream, offline behaviour,
//! reconnect, and the token-never-leaks rule (spec §13).

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use axum::ServiceExt as _;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response;
use futures_util::{SinkExt as _, StreamExt as _};
use leaflog_backend::ha::{HaBridge, SupervisorToken};
use leaflog_backend::{AppState, Assets, Db, Mode, app};
use serde_json::{Value, json};

use std::sync::Mutex as StdMutex;

static TEST_BRIDGE: std::sync::LazyLock<StdMutex<HashMap<String, Arc<HaBridge>>>> =
    std::sync::LazyLock::new(|| StdMutex::new(HashMap::new()));

const SECRET: &str = "super-secret-supervisor-token-do-not-leak";

/// Spawns the mock Supervisor on an ephemeral port and returns its URL.
async fn spawn_mock() -> (String, mock_ha::MockState) {
    let state = mock_ha::MockState {
        room: Arc::new(tokio::sync::Mutex::new(mock_ha::FakeRoom::new())),
        notifications: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        language: Arc::new(tokio::sync::Mutex::new("en".to_owned())),
    };
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let router = mock_ha::app(state.clone());
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    (format!("http://{addr}"), state)
}

/// Spawns the Leaflog API with the bridge pointed at the mock.
async fn spawn_api(mock_url: &str) -> (String, Db, Arc<HaBridge>) {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dist");
    let db = Db::open_in_memory().unwrap();
    let bridge = Arc::new(HaBridge::spawn(
        mock_url.to_owned(),
        SupervisorToken::new(SECRET),
        db.clone(),
    ));
    let bridge_handle = bridge.clone();
    let state = AppState::new(
        Assets::Disk(fixture),
        Mode::Dev,
        None,
        db.clone(),
        leaflog_backend::photos::PhotoStore::new(std::env::temp_dir()),
        bridge,
    );
    let make = app::service(state).into_make_service_with_connect_info::<SocketAddr>();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, make).await.unwrap();
    });
    (format!("http://{addr}"), db, bridge_handle)
}

fn state_ha_bridge(base: &str) -> Arc<HaBridge> {
    // Stored in a global for test access; the URL is the key.
    TEST_BRIDGE
        .lock()
        .expect("bridge registry")
        .get(base)
        .expect("bridge registered")
        .clone()
}

async fn create_grow(base: &str) -> String {
    reqwest::Client::new()
        .post(format!("{base}/api/grows"))
        .json(&json!({
            "name": "HA run", "substrate": "soil",
            "started_on": "2026-10-01", "status": "active", "notes": null,
        }))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

async fn wait_connected(bridge: &HaBridge) {
    for _ in 0..40 {
        if bridge.live_now().connected {
            return;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    panic!("bridge never connected to the mock");
}

async fn wait_state_contains(bridge: &HaBridge, needle: &str) {
    for _ in 0..40 {
        let live = bridge.live_now();
        if live.entities.values().any(|entity| entity.state == needle) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    panic!("live map never contained {needle:?}");
}

#[tokio::test]
async fn live_data_flows_from_mock_to_sse() {
    let (mock_url, _mock) = spawn_mock().await;
    let bridge = HaBridge::spawn(
        mock_url.clone(),
        SupervisorToken::new(SECRET),
        Db::open_in_memory().unwrap(),
    );
    bridge.track_entities(vec![
        "sensor.grow_temperature".to_owned(),
        "sensor.grow_humidity".to_owned(),
    ]);
    wait_connected(&bridge).await;
    // The mock ticks every 2s; temperature moves away from 24.3.
    wait_state_contains(&bridge, "24.2").await;

    // SSE stream: the first event is the current snapshot.
    let stream = reqwest::get(format!("{mock_url}/core/api/states"))
        .await
        .unwrap();
    assert!(stream.status().is_success());
}

#[tokio::test]
async fn bindings_auto_capture_and_entities_endpoints() {
    let (mock_url, _mock) = spawn_mock().await;
    let (base, db, bridge) = spawn_api(&mock_url).await;
    TEST_BRIDGE
        .lock()
        .expect("bridge registry")
        .insert(base.clone(), bridge);
    let grow_id = create_grow(&base).await;
    let client = reqwest::Client::new();

    // Bind temperature + light to the grow.
    let bindings: Value = client
        .put(format!("{base}/api/grows/{grow_id}/bindings"))
        .json(&json!({
            "bindings": {
                "temperature": "sensor.grow_temperature",
                "light": "light.grow_led",
            }
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(bindings["temperature"], "sensor.grow_temperature");

    // Entity picker works through the API (Supervisor reachable).
    let entities: Value = client
        .get(format!("{base}/api/ha/entities"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(entities["source"], "supervisor");
    let list = entities["entities"].as_array().unwrap();
    assert!(
        list.iter()
            .any(|entity| entity["entity_id"] == "sensor.grow_temperature")
    );

    // History passthrough.
    let history: Value = client
        .get(format!(
            "{base}/api/ha/history?entity_id=sensor.grow_temperature&hours=12"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(history.as_array().is_some());

    // Wait for the bridge to have live values (it resubscribes after
    // the bindings change, which restarts the socket).
    for _ in 0..60 {
        let live = state_ha_bridge(&base).live_now();
        if live.entities.contains_key("sensor.grow_temperature") {
            break;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    let live = state_ha_bridge(&base).live_now();
    assert!(
        live.entities.contains_key("sensor.grow_temperature"),
        "bridge must have live temperature before auto-capture (connected: {})",
        live.connected
    );
    let entry: Value = client
        .post(format!("{base}/api/grows/{grow_id}/actions"))
        .json(&json!({
            "plant_ids": [], "occurred_at": "2026-11-08T10:00:00Z",
            "action_type": "water", "details": {}, "notes": null,
            "appearance_tags": [],
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // The snapshot must reference the bound entity.
    let snapshots = db
        .with(move |conn| {
            leaflog_backend::store::sensors::snapshots_for(conn, entry["id"].as_str().unwrap())
        })
        .await
        .unwrap();
    assert!(
        snapshots
            .iter()
            .any(|snapshot| snapshot.entity_id == "sensor.grow_temperature"),
        "auto-capture must snapshot the bound temperature entity"
    );
}

#[tokio::test]
async fn bindings_put_while_connected_resubscribes_and_lands_the_states() {
    // The rebind path of a live session: the earlier cases PUT so soon
    // after spawn that the track set drains before the first connect.
    // Connected-first (as after minutes of real use), a bindings PUT
    // must tear down the socket, resubscribe on the new set, and seed
    // the live map — the state the env endpoint's availability reads.
    let (mock_url, _mock) = spawn_mock().await;
    let (base, _db, bridge) = spawn_api(&mock_url).await;
    let grow_id = create_grow(&base).await;
    wait_connected(bridge.as_ref()).await;

    let client = reqwest::Client::new();
    client
        .put(format!("{base}/api/grows/{grow_id}/bindings"))
        .json(&json!({
            "bindings": {
                "temperature": "sensor.grow_temperature",
                "humidity": "sensor.grow_humidity",
            }
        }))
        .send()
        .await
        .unwrap();

    // The resubscribe reconnects without a failure backoff (HA is
    // healthy); poll like the callers of the live map (env snapshot,
    // auto-capture) do.
    for _ in 0..40 {
        let live = bridge.live_now();
        if live.entities.contains_key("sensor.grow_temperature")
            && live.entities.contains_key("sensor.grow_humidity")
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    let live = bridge.live_now();
    assert!(
        live.entities.contains_key("sensor.grow_temperature")
            && live.entities.contains_key("sensor.grow_humidity"),
        "rebound entities never landed in the live map (connected: {})",
        live.connected
    );
}

#[tokio::test]
async fn app_works_with_supervisor_unreachable() {
    // Point at a dead port; every API endpoint must still answer.
    let dead_url = "http://127.0.0.1:1".to_owned();
    let (base, _db, _bridge) = spawn_api(&dead_url).await;
    let client = reqwest::Client::new();

    let grow_id = create_grow(&base).await;
    let entry: Value = client
        .post(format!("{base}/api/grows/{grow_id}/actions"))
        .json(&json!({
            "plant_ids": [], "occurred_at": "2026-11-08T10:00:00Z",
            "action_type": "note", "details": {}, "notes": "offline test",
            "appearance_tags": [],
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(entry["notes"], "offline test");

    // Entities falls back to the (empty) live map, not an error.
    let entities: Value = client
        .get(format!("{base}/api/ha/entities"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(entities["source"], "live");

    // History reports unreachable as a client error, not a crash.
    let history = client
        .get(format!("{base}/api/ha/history?entity_id=sensor.x"))
        .send()
        .await
        .unwrap();
    assert_eq!(history.status(), reqwest::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn token_never_appears_in_responses_or_logs() {
    let (mock_url, _mock) = spawn_mock().await;
    let (base, _db, _bridge) = spawn_api(&mock_url).await;
    let client = reqwest::Client::new();
    let grow_id = create_grow(&base).await;

    // Exercise the surface that talks to the Supervisor.
    let mut bodies = Vec::new();
    for url in [
        format!("{base}/api/health"),
        format!("{base}/api/ha/entities"),
        format!("{base}/api/ha/history?entity_id=sensor.grow_temperature"),
        format!("{base}/api/grows/{grow_id}/bindings"),
    ] {
        let response = client.get(&url).send().await.unwrap();
        let status = response.status();
        let text = response.text().await.unwrap();
        assert!(!text.contains(SECRET), "token leaked from {url}");
        assert!(status.is_success(), "{url} failed with {status}");
        bodies.push((url, text));
    }

    // The SPA shell and its assets must not contain it either.
    let index = client
        .get(format!("{base}/"))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(!index.contains(SECRET));

    // SSE stream: read only the first chunk (the stream stays open).
    let mut sse = reqwest::get(format!("{base}/api/live")).await.unwrap();
    let chunk = tokio::time::timeout(Duration::from_secs(5), sse.chunk()).await;
    if let Ok(Ok(Some(bytes))) = chunk {
        let text = String::from_utf8_lossy(&bytes).to_string();
        assert!(!text.contains(SECRET), "token leaked from SSE");
    }

    // The bridge actually authenticated (the mock rejects tokenless
    // sessions), so the token IS in use - just never exposed.
    let entities: Value = client
        .get(format!("{base}/api/ha/entities"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(entities["source"], "supervisor");
}

#[tokio::test]
async fn history_percent_encodes_entity_id_in_supervisor_url() {
    // A capturing stand-in Supervisor: record the raw request target of
    // every request it sees and answer history with an empty array.
    let captured: Arc<StdMutex<Vec<String>>> = Arc::new(StdMutex::new(Vec::new()));
    let recorder = captured.clone();
    let router = axum::Router::new().fallback(move |uri: axum::http::Uri| {
        let recorder = recorder.clone();
        async move {
            recorder.lock().expect("capture lock").push(uri.to_string());
            axum::Json(json!([]))
        }
    });
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    let bridge = HaBridge::spawn(
        format!("http://{addr}"),
        SupervisorToken::new(SECRET),
        Db::open_in_memory().unwrap(),
    );
    // An entity id whose `&`/`=` would, unencoded, flip minimal_response
    // off and append an extra query param to the admin-token request.
    bridge
        .history("sensor.x&minimal_response=false&evil=1", 1)
        .await
        .expect("history call against the capturing supervisor");

    let target = captured
        .lock()
        .expect("capture lock")
        .iter()
        .find(|target| target.contains("history/period"))
        .cloned()
        .expect("supervisor saw no history request");
    assert!(
        target.contains("%26"),
        "entity_id ampersands must be percent-encoded in the supervisor URL: {target}"
    );
    assert!(
        !target.contains("&minimal_response=false") && !target.contains("&evil=1"),
        "raw `&` from entity_id reshaped the supervisor query: {target}"
    );
}

#[tokio::test]
async fn bridge_reconnects_after_the_socket_drops() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (mock_state_tx, _mock_state_rx) = tokio::sync::oneshot::channel::<mock_ha::MockState>();
    let state = mock_ha::MockState {
        room: Arc::new(tokio::sync::Mutex::new(mock_ha::FakeRoom::new())),
        notifications: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        language: Arc::new(tokio::sync::Mutex::new("en".to_owned())),
    };
    let router = mock_ha::app(state.clone());
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let _ = mock_state_tx.send(state);

    let bridge = HaBridge::spawn(
        format!("http://{addr}"),
        SupervisorToken::new(SECRET),
        Db::open_in_memory().unwrap(),
    );
    bridge.track_entities(vec!["sensor.grow_temperature".to_owned()]);
    wait_connected(&bridge).await;

    // Kill the server; the bridge must notice and go offline...
    server.abort();
    for _ in 0..40 {
        if !bridge.live_now().connected {
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    // Even if the socket close has not been noticed yet (TCP half-open
    // takes a heartbeat cycle), the reconnect behaviour is what
    // matters; skip the strict offline assert.
    tokio::time::sleep(Duration::from_secs(3)).await;

    // ...then a fresh mock on the same port brings it back (backoff
    // schedule starts at 1s, so this lands within the retry loop).
    let listener = tokio::net::TcpListener::bind(addr).await.unwrap();
    let router = mock_ha::app(mock_ha::MockState {
        room: Arc::new(tokio::sync::Mutex::new(mock_ha::FakeRoom::new())),
        notifications: Arc::new(tokio::sync::Mutex::new(Vec::new())),
        language: Arc::new(tokio::sync::Mutex::new("en".to_owned())),
    });
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    wait_connected(&bridge).await;
}

/// A scripted Supervisor socket: the auth handshake succeeds, but
/// `subscribe_entities` is answered with an error result (as the real
/// Supervisor does for, say, a bound entity id that no longer exists).
/// The connection stays open afterwards, so a bridge that treats the
/// failed subscription as live has nothing to fall back on.
async fn rejecting_subscribe_socket(socket: WebSocket, replied: Arc<AtomicUsize>) {
    let (mut sender, mut receiver) = socket.split();
    if sender
        .send(Message::text(
            json!({ "type": "auth_required" }).to_string(),
        ))
        .await
        .is_err()
    {
        return;
    }
    let Some(Ok(message)) = receiver.next().await else {
        return;
    };
    if !message
        .into_text()
        .unwrap_or_default()
        .contains("access_token")
    {
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
    // Every connection (the bridge retries with backoff) gets the same
    // rejection; other commands (pings) are ignored.
    while let Some(Ok(message)) = receiver.next().await {
        let Ok(text) = message.into_text() else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<Value>(&text) else {
            continue;
        };
        if value["type"].as_str() == Some("subscribe_entities") {
            let error = json!({
                "id": value["id"], "type": "result", "success": false,
                "error": { "code": "entity_not_found", "message": "entity no longer exists" }
            });
            if sender.send(Message::text(error.to_string())).await.is_err() {
                break;
            }
            replied.fetch_add(1, Ordering::SeqCst);
        }
    }
}

/// The WS route handing each connection to the scripted socket.
// Extractors are taken by value because Axum requires it.
async fn rejecting_websocket(
    State(replied): State<Arc<AtomicUsize>>,
    upgrade: WebSocketUpgrade,
) -> Response {
    upgrade.on_upgrade(move |socket| rejecting_subscribe_socket(socket, replied))
}

#[tokio::test]
async fn failed_subscribe_entities_result_leaves_bridge_disconnected() {
    let replied = Arc::new(AtomicUsize::new(0));
    let router = axum::Router::new()
        .route("/core/websocket", axum::routing::get(rejecting_websocket))
        .with_state(replied.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });

    let bridge = HaBridge::spawn(
        format!("http://{addr}"),
        SupervisorToken::new(SECRET),
        Db::open_in_memory().unwrap(),
    );
    bridge.track_entities(vec!["sensor.grow_temperature".to_owned()]);

    // Wait until the scripted Supervisor really answered the failed
    // subscription, so a "disconnected" verdict cannot come from the
    // handshake never having run.
    for _ in 0..40 {
        if replied.load(Ordering::SeqCst) > 0 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    assert!(
        replied.load(Ordering::SeqCst) > 0,
        "scripted supervisor never saw a subscribe_entities command"
    );

    // A rejected subscription is not live data: the bridge must report
    // itself disconnected for the whole window.
    for _ in 0..20 {
        assert!(
            !bridge.live_now().connected,
            "bridge reported connected although subscribe_entities failed"
        );
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
}

#[tokio::test]
async fn entity_picker_filters_by_device_class() {
    let (mock_url, _mock) = spawn_mock().await;
    let (base, _db, _bridge) = spawn_api(&mock_url).await;
    let client = reqwest::Client::new();

    let ids = |payload: &Value| -> Vec<String> {
        payload["entities"]
            .as_array()
            .map(|entities| {
                entities
                    .iter()
                    .map(|entity| entity["entity_id"].as_str().unwrap_or_default().to_owned())
                    .collect()
            })
            .unwrap_or_default()
    };

    // The sensor-item picker (spec §4.1): temperature and humidity are
    // the two classes v1 offers, and nothing else sneaks in.
    let temperature: Value = client
        .get(format!("{base}/api/ha/entities?device_class=temperature"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(temperature["source"], "supervisor");
    assert_eq!(ids(&temperature), ["sensor.grow_temperature"]);

    let humidity: Value = client
        .get(format!("{base}/api/ha/entities?device_class=humidity"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(ids(&humidity), ["sensor.grow_humidity"]);

    // Unfiltered still lists everything.
    let all: Value = client
        .get(format!("{base}/api/ha/entities"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(ids(&all).contains(&"light.grow_led".to_owned()));
    assert!(ids(&all).contains(&"sensor.grow_soil_moisture".to_owned()));
}

#[tokio::test]
async fn grow_days_count_local_calendar_days_in_the_ha_timezone() {
    let (mock_url, _mock) = spawn_mock().await;
    let (base, _db, bridge) = spawn_api(&mock_url).await;
    let client = reqwest::Client::new();

    // The bridge refreshes the canonical zone from
    // GET /core/api/config on connect; the mock serves Europe/Berlin.
    let mut waited = 0_u32;
    while bridge.cached_timezone().as_deref() != Some("Europe/Berlin") {
        assert!(waited < 100, "timezone never arrived from the mock");
        tokio::time::sleep(Duration::from_millis(50)).await;
        waited += 1;
    }

    // A grow started 2026-11-10 with an entry at 23:30Z — 00:30 local
    // on the 11th in Berlin, but still the 10th in UTC. The local
    // calendar day wins (spec §2): grow day 1, not 0.
    let grow: Value = client
        .post(format!("{base}/api/grows"))
        .json(&json!({
            "name": "TZ run",
            "feed_template_id": null,
            "substrate": "soil",
            "started_on": "2026-11-10",
            "status": "active",
            "notes": null,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let entry: Value = client
        .post(format!(
            "{}/api/grows/{}/actions",
            base,
            grow["id"].as_str().unwrap()
        ))
        .json(&json!({
            "plant_ids": [],
            "occurred_at": "2026-11-10T23:30:00Z",
            "action_type": "water",
            "details": {},
            "notes": null,
            "appearance_tags": [],
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(entry["grow_day"], 1, "Berlin local day, not the UTC day 0");
}

#[tokio::test]
async fn env_alerts_route_to_ha_under_optin_rate_limited_and_quiet() {
    use std::collections::HashMap;

    let (mock_url, mock) = spawn_mock().await;
    let (base, db, bridge) = spawn_api(&mock_url).await;
    let client = reqwest::Client::new();

    // Out of band from the start: 32 °C against the ~29 °C ceiling.
    *mock.room.lock().await = mock_ha::FakeRoom {
        temperature: 32.0,
        humidity: 40.0,
        soil_moisture: 30.0,
        co2: 850.0,
        light_on: true,
    };

    // Opt in, and a grow watching the mock's sensors.
    client
        .put(format!("{base}/api/settings"))
        .json(&json!({ "env_alert_notifications": true }))
        .send()
        .await
        .unwrap();
    let grow: Value = client
        .post(format!("{base}/api/grows"))
        .json(&json!({
            "name": "Hot run",
            "feed_template_id": null,
            "substrate": "soil",
            "started_on": "2026-10-01",
            "status": "active",
            "notes": null,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    client
        .put(format!(
            "{}/api/grows/{}/bindings",
            base,
            grow["id"].as_str().unwrap()
        ))
        .json(&json!({
            "bindings": {
                "temperature": "sensor.grow_temperature",
                "humidity": "sensor.grow_humidity",
            }
        }))
        .send()
        .await
        .unwrap();

    // Wait until the live map carries the hot reading, then run one
    // evaluator pass by hand (the ticker runs the same function).
    let mut waited = 0_u32;
    loop {
        let live = bridge.live_now();
        let hot = live
            .entities
            .get("sensor.grow_temperature")
            .and_then(|entity| entity.state.parse::<f64>().ok())
            .is_some_and(|celsius| celsius > 30.0);
        if hot || waited > 100 {
            break;
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
        waited += 1;
    }
    assert!(waited <= 100, "the hot reading never arrived");

    let mut last_sent = HashMap::new();
    leaflog_backend::alerts::evaluate(&db, &bridge, &mut last_sent)
        .await
        .unwrap();
    let sent = mock.notifications.lock().await.clone();
    assert!(
        sent.iter().any(|body| body.contains("temperature")),
        "an out-of-band temperature notifies: {sent:?}"
    );

    // Rate limit: an immediate second pass adds nothing.
    leaflog_backend::alerts::evaluate(&db, &bridge, &mut last_sent)
        .await
        .unwrap();
    assert_eq!(mock.notifications.lock().await.len(), sent.len());

    // Quiet hours bracketing this exact minute block even fresh
    // alerts (a one-minute window containing now, DST-zone aware).
    let now_minutes =
        leaflog_core::tz::local_wall_minutes_now("Europe/Berlin").expect("zone from the mock");
    let start = (now_minutes, now_minutes / 60, now_minutes % 60);
    let end_minute = (now_minutes + 1) % 1440;
    let end = (end_minute, end_minute / 60, end_minute % 60);
    client
        .put(format!("{base}/api/settings"))
        .json(&json!({
            "quiet_hours": format!("{:02}:{:02}-{:02}:{:02}", start.1, start.2, end.1, end.2)
        }))
        .send()
        .await
        .unwrap();
    let mut fresh = HashMap::new();
    leaflog_backend::alerts::evaluate(&db, &bridge, &mut fresh)
        .await
        .unwrap();
    assert_eq!(
        mock.notifications.lock().await.len(),
        sent.len(),
        "quiet hours suppress the notification"
    );

    // And with the opt-in off, nothing routes at all.
    client
        .put(format!("{base}/api/settings"))
        .json(&json!({ "env_alert_notifications": false }))
        .send()
        .await
        .unwrap();
    let mut fresh = HashMap::new();
    leaflog_backend::alerts::evaluate(&db, &bridge, &mut fresh)
        .await
        .unwrap();
    assert_eq!(mock.notifications.lock().await.len(), sent.len());
}

// --- Grow history chart (grow-overview ticket 05) ---

#[tokio::test]
async fn grow_history_validates_role_and_window() {
    // The endpoint must reject unknown parameters loudly (400), while
    // an unknown grow stays 404 — before any Home Assistant call.
    let dead_url = "http://127.0.0.1:1".to_owned();
    let (base, _db, _bridge) = spawn_api(&dead_url).await;
    let client = reqwest::Client::new();
    let grow_id = create_grow(&base).await;

    for query in [
        "window=day",                   // missing role
        "role=temperature",             // missing window
        "role=moisture&window=day",     // unknown role
        "role=temperature&window=year", // unknown window
    ] {
        let response = client
            .get(format!("{base}/api/grows/{grow_id}/history?{query}"))
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            reqwest::StatusCode::BAD_REQUEST,
            "query {query} must be rejected"
        );
    }
    let response = client
        .get(format!(
            "{base}/api/grows/no-such-grow/history?role=temperature&window=day"
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn grow_history_degrades_to_snapshots_when_supervisor_unreachable() {
    // Dead binding rule: HA unreachable means snapshot-only with
    // degraded=true, never an error wall. A role without a binding is
    // an explicit bound=false no-data state, not an error either.
    let dead_url = "http://127.0.0.1:1".to_owned();
    let (base, db, _bridge) = spawn_api(&dead_url).await;
    let client = reqwest::Client::new();
    let grow_id = create_grow(&base).await;

    client
        .put(format!("{base}/api/grows/{grow_id}/bindings"))
        .json(&json!({ "bindings": { "temperature": "sensor.grow_temperature" } }))
        .send()
        .await
        .unwrap();
    let entry: Value = client
        .post(format!("{base}/api/grows/{grow_id}/actions"))
        .json(&json!({
            "plant_ids": [], "occurred_at": "2026-11-16T10:00:00Z",
            "action_type": "note", "details": {}, "notes": "snapshot seed",
            "appearance_tags": [],
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let entry_id = entry["id"].as_str().unwrap().to_owned();

    // Inject the snapshot the dead HA cannot provide: a log-time
    // capture of the bound temperature sensor, taken now.
    db.with(move |conn| {
        leaflog_backend::store::sensors::snapshot(
            conn,
            &entry_id,
            "temperature",
            "sensor.grow_temperature",
            "22.5",
        )
    })
    .await
    .unwrap();

    let body: Value = client
        .get(format!(
            "{base}/api/grows/{grow_id}/history?role=temperature&window=day"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["degraded"], true);
    assert_eq!(body["bound"], true);
    assert_eq!(body["role"], "temperature");
    assert_eq!(body["window"], "day");
    let points = body["series"]["points"].as_array().unwrap();
    assert_eq!(points.len(), 1, "exactly the injected snapshot point");
    assert_eq!(points[0]["source"], "snapshot");
    assert_eq!(points[0]["value"], 22.5);
    // The corridor is preset-driven and survives the outage.
    let bands = body["bands"].as_array().unwrap();
    assert!(!bands.is_empty(), "temperature bands must exist offline");
    for band in bands {
        assert!(band["min"].as_f64().unwrap() < band["max"].as_f64().unwrap());
    }

    // An unbound role is an explicit no-sensor state, not an error.
    let body: Value = client
        .get(format!(
            "{base}/api/grows/{grow_id}/history?role=co2&window=week"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["bound"], false);
    assert_eq!(body["degraded"], false);
    assert_eq!(body["series"]["points"].as_array().unwrap().len(), 0);
}

#[tokio::test]
async fn grow_history_serves_live_series_with_phase_bands() {
    // Against the mock Supervisor the endpoint answers 200 with a
    // bound flag, snapshot points from the log-time capture, and the
    // temperature Target Band — the tolerance-shifted alert
    // thresholds (env-section ticket 06).
    let (mock_url, _mock) = spawn_mock().await;
    let (base, _db, bridge) = spawn_api(&mock_url).await;
    wait_connected(&bridge).await;
    let client = reqwest::Client::new();
    // A grow started today keeps the band assertion clock-robust: its
    // current phase is germination on any run date.
    let today = time::OffsetDateTime::now_utc().date();
    let grow: Value = client
        .post(format!("{base}/api/grows"))
        .json(&json!({
            "name": "Chart run", "substrate": "soil",
            "started_on": today.to_string(), "status": "active", "notes": null,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let grow_id = grow["id"].as_str().unwrap().to_owned();

    client
        .put(format!("{base}/api/grows/{grow_id}/bindings"))
        .json(&json!({
            "bindings": {
                "temperature": "sensor.grow_temperature",
                "humidity": "sensor.grow_humidity",
            }
        }))
        .send()
        .await
        .unwrap();
    // PUT resubscribes the bridge (which clears and refills the live
    // map); auto-capture only sees entities already in the map, so
    // the action must wait until the bound sensors have landed.
    for _ in 0..40 {
        if bridge
            .live_now()
            .entities
            .contains_key("sensor.grow_temperature")
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    assert!(
        bridge
            .live_now()
            .entities
            .contains_key("sensor.grow_temperature"),
        "bound sensor never landed in the live map"
    );
    client
        .post(format!("{base}/api/grows/{grow_id}/actions"))
        .json(&json!({
            "plant_ids": [], "occurred_at": "2026-11-16T10:00:00Z",
            "action_type": "note", "details": {}, "notes": "capture me",
            "appearance_tags": [],
        }))
        .send()
        .await
        .unwrap();

    // Auto-capture lands asynchronously after the binding resubscribe;
    // poll until the merged series carries the snapshot point.
    let mut body = Value::Null;
    for _ in 0..40 {
        body = client
            .get(format!(
                "{base}/api/grows/{grow_id}/history?role=temperature&window=month"
            ))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let has_snapshot = body["series"]["points"]
            .as_array()
            .is_some_and(|points| points.iter().any(|point| point["source"] == "snapshot"));
        if has_snapshot {
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    assert_eq!(body["degraded"], false, "mock Supervisor is reachable");
    assert_eq!(body["bound"], true);
    let points = body["series"]["points"].as_array().unwrap();
    assert!(
        points.iter().any(|point| point["source"] == "snapshot"),
        "the auto-captured snapshot must back the series"
    );
    // The temperature Target Band (env-section ticket 06): the
    // tolerance-shifted alert thresholds, a flat corridor with no
    // light state. The grow started today (germination) with default
    // Average tolerances, so every segment carries the default
    // 60–80 °F thresholds; how many segments appear depends only on
    // the phase spans in the window.
    let bands = body["bands"].as_array().unwrap();
    assert!(!bands.is_empty());
    for pair in bands.windows(2) {
        assert_eq!(pair[0]["end"], pair[1]["start"], "segments butt-join");
    }
    let expected_band = leaflog_core::env::temperature_target_band(
        leaflog_core::grows::Tolerance::Average,
        leaflog_core::grows::Tolerance::Average,
    );
    for band in bands {
        assert_eq!(band["phase"], "germination");
        assert_eq!(
            band["light_on"],
            Value::Null,
            "the flat tolerance band carries no light state"
        );
        assert_eq!(
            (band["min"].as_f64().unwrap(), band["max"].as_f64().unwrap()),
            expected_band,
            "every segment carries the tolerance Target Band"
        );
    }

    // Humidity's corridor is the phase's VPD band converted at the
    // segment's mean temperature (a clamped RH percentage). With the
    // default cycle the window steps day/night, so multiple segments
    // appear and every one stays a sane RH window.
    let body: Value = client
        .get(format!(
            "{base}/api/grows/{grow_id}/history?role=humidity&window=month"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["degraded"], false);
    assert_eq!(body["bound"], true);
    let humidity_bands = body["bands"].as_array().unwrap();
    assert!(!humidity_bands.is_empty());
    for band in humidity_bands {
        assert_eq!(band["phase"], "germination");
        let min = band["min"].as_f64().unwrap();
        let max = band["max"].as_f64().unwrap();
        assert!((0.0..=100.0).contains(&min));
        assert!((0.0..=100.0).contains(&max));
        assert!(min <= max);
    }
}

// --- Stepped day/night bands (Light Cycle spec, ticket 07) ---

/// Creates a grow whose germination phase covers any day window:
/// started three days ago, so the first phase span predates the
/// window on every run date.
async fn create_germinating_grow(base: &str, name: &str) -> String {
    let started = (time::OffsetDateTime::now_utc() - time::Duration::days(3)).date();
    reqwest::Client::new()
        .post(format!("{base}/api/grows"))
        .json(&json!({
            "name": name, "strain_id": null, "substrate": "soil",
            "started_on": started.to_string(), "status": "active", "notes": null,
        }))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// The germ-phase lights-on VPD band a lit segment must carry.
const VPD_DAY_BAND: (f64, f64) = (0.4, 0.8);
/// The germ-phase lights-off VPD band a dark segment must carry.
const VPD_NIGHT_BAND: (f64, f64) = (0.5, 0.9);

/// Asserts a band list steps cleanly: alternating light states,
/// butt-joined segments, both states present in a day window.
/// Returns the light states in order, for the caller's per-state
/// corridor checks.
fn assert_germination_stepping(bands: &[Value]) -> Vec<bool> {
    let mut states = Vec::new();
    for band in bands {
        assert_eq!(band["phase"], "germination");
        let min = band["min"].as_f64().unwrap();
        let max = band["max"].as_f64().unwrap();
        assert!(min < max);
        let light_on = band["light_on"]
            .as_bool()
            .expect("the schedule always yields a light state here");
        if let Some(&previous) = states.last() {
            assert_ne!(previous, light_on, "identical neighbours must merge");
        }
        states.push(light_on);
    }
    for pair in bands.windows(2) {
        assert_eq!(pair[0]["end"], pair[1]["start"], "segments butt-join");
    }
    assert!(
        states.iter().any(|&on| on) && states.iter().any(|&on| !on),
        "a day window must contain both a lights-on and a lights-off stretch"
    );
    states
}

#[tokio::test]
async fn grow_history_steps_day_and_night_by_the_light_cycle() {
    // No light bound: the grow's Light Cycle drives the split. The
    // schedule runs 08:00–20:00 UTC in germination (12 h on), so the
    // day window shows both states with their own corridors, and the
    // humidity chart solves at the temperature series — not at a
    // humidity reading fed in as °C.
    let dead_url = "http://127.0.0.1:1".to_owned();
    let (base, db, _bridge) = spawn_api(&dead_url).await;
    let client = reqwest::Client::new();
    let grow_id = create_germinating_grow(&base, "Stepped schedule").await;

    let put = client
        .put(format!("{base}/api/grows/{grow_id}/light-cycle"))
        .json(&json!({ "lights_on": "08:00", "hours_on": { "germination": 12 } }))
        .send()
        .await
        .unwrap();
    assert_eq!(put.status(), 200);

    client
        .put(format!("{base}/api/grows/{grow_id}/bindings"))
        .json(&json!({
            "bindings": {
                "temperature": "sensor.grow_temperature",
                "humidity": "sensor.grow_humidity",
            }
        }))
        .send()
        .await
        .unwrap();

    // One log entry whose auto-capture is replaced by exact known
    // snapshots: temperature 22.0 °C is the mean the humidity window
    // must solve at; the humidity reading (70.0) must NOT shape it.
    let entry: Value = client
        .post(format!("{base}/api/grows/{grow_id}/actions"))
        .json(&json!({
            "plant_ids": [], "occurred_at": "2026-11-16T10:00:00Z",
            "action_type": "note", "details": {}, "notes": "snapshot seed",
            "appearance_tags": [],
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let entry_id = entry["id"].as_str().unwrap().to_owned();
    db.with(move |conn| {
        leaflog_backend::store::sensors::snapshot(
            conn,
            &entry_id,
            "temperature",
            "sensor.grow_temperature",
            "22.0",
        )?;
        leaflog_backend::store::sensors::snapshot(
            conn,
            &entry_id,
            "humidity",
            "sensor.grow_humidity",
            "70.0",
        )
    })
    .await
    .unwrap();

    let history = |role: &'static str| {
        let base = base.clone();
        let grow_id = grow_id.clone();
        let client = client.clone();
        async move {
            client
                .get(format!(
                    "{base}/api/grows/{grow_id}/history?role={role}&window=day"
                ))
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()
        }
    };

    // VPD is the reference stepping (env-section ticket 06): the
    // Target Band steps between the phase's day range and the
    // sourced lights-off range with the light state, and every
    // segment carries its state's range.
    let vpd = history("vpd").await;
    let vpd_bands = vpd["bands"].as_array().unwrap().to_vec();
    let steps = assert_germination_stepping(&vpd_bands);
    for (band, &on) in vpd_bands.iter().zip(&steps) {
        assert_eq!(
            (band["min"].as_f64().unwrap(), band["max"].as_f64().unwrap()),
            if on { VPD_DAY_BAND } else { VPD_NIGHT_BAND },
            "VPD range must match the light state (on: {on})"
        );
    }
    // The band covering the window end (the snapshot lands there too:
    // its captured_at is the server's wall-clock now).
    let to_epoch = leaflog_core::chart::parse_epoch(vpd["series"]["to"].as_str().unwrap()).unwrap();
    let covering = vpd_bands
        .iter()
        .find(|band| {
            let start = leaflog_core::chart::parse_epoch(band["start"].as_str().unwrap()).unwrap();
            let end = leaflog_core::chart::parse_epoch(band["end"].as_str().unwrap()).unwrap();
            start <= to_epoch && to_epoch <= end
        })
        .expect("bands cover the window end")
        .clone();

    // Temperature: a flat tolerance Target Band (env-section ticket
    // 06) — the same thresholds the env snapshot's temp_ok checks,
    // with no light state, across the whole day window.
    let temperature = history("temperature").await;
    assert_eq!(temperature["degraded"], true, "dead supervisor");
    let temp_bands = temperature["bands"].as_array().unwrap();
    assert_eq!(
        temp_bands.len(),
        1,
        "one flat band across the germination day window"
    );
    assert_eq!(temp_bands[0]["light_on"], Value::Null);
    assert_eq!(
        (
            temp_bands[0]["min"].as_f64().unwrap(),
            temp_bands[0]["max"].as_f64().unwrap()
        ),
        leaflog_core::env::temperature_target_band(
            leaflog_core::grows::Tolerance::Average,
            leaflog_core::grows::Tolerance::Average,
        ),
        "the tolerance thresholds are the band edges"
    );

    // CO₂: bare (env-section ticket 06) — no Target Band at all.
    let co2 = history("co2").await;
    assert_eq!(
        co2["bands"].as_array().unwrap().len(),
        0,
        "CO₂ renders without a Target Band"
    );

    // Humidity: the RH window solved at the segment's actual mean
    // temperature with the day/night leaf offset. The one injected
    // temperature snapshot (22.0 °C, at wall-clock now) gives exactly
    // one light segment a mean, so exactly one humidity band exists —
    // and it must equal the independently tested core solver at 22.0
    // °C with that segment's leaf offset (the 70.0 humidity reading
    // never feeds the solver as °C).
    let humidity = history("humidity").await;
    let humidity_bands = humidity["bands"].as_array().unwrap();
    assert_eq!(
        humidity_bands.len(),
        1,
        "only the snapshot's segment has a mean"
    );
    let humid_band = &humidity_bands[0];
    let on_at_snapshot = covering["light_on"].as_bool().unwrap();
    let offset = match on_at_snapshot {
        true => leaflog_core::env::DEFAULT_LEAF_OFFSET_C,
        false => leaflog_core::env::NIGHT_LEAF_OFFSET_C,
    };
    let vpd_band = match on_at_snapshot {
        true => leaflog_core::env::vpd_band(leaflog_core::Phase::Germination),
        false => {
            let night = leaflog_core::env::night_vpd_band(leaflog_core::Phase::Germination)
                .expect("germination night band");
            (night.min_kpa, night.max_kpa)
        }
    };
    let (min, max) = leaflog_core::env::rh_window(22.0, offset, vpd_band);
    assert_eq!(
        (
            humid_band["min"].as_f64().unwrap(),
            humid_band["max"].as_f64().unwrap()
        ),
        (min, max),
        "humidity band must solve at 22.0 °C (offset {offset}), light on: {on_at_snapshot}"
    );
    // The humidity band segments with the temperature band at the
    // snapshot: same stretch, so the step boundaries come from the
    // same light timeline.
    assert_eq!(humid_band["start"], covering["start"]);
    assert_eq!(humid_band["end"], covering["end"]);
}

#[tokio::test]
async fn grow_history_steps_follow_the_bound_light_history() {
    // A bound light entity wins over the schedule. The grow's cycle is
    // 24 h on — under the schedule alone every band would be lit — so
    // every dark band below is a fixture-driven override. The mock's
    // staircase: off at now−23 h, on at now−20 h, off at now−18 h.
    let (mock_url, _mock) = spawn_mock().await;
    let (base, _db, bridge) = spawn_api(&mock_url).await;
    wait_connected(&bridge).await;
    let client = reqwest::Client::new();
    let grow_id = create_germinating_grow(&base, "Stepped by fixture").await;

    client
        .put(format!("{base}/api/grows/{grow_id}/bindings"))
        .json(&json!({
            "bindings": {
                "temperature": "sensor.grow_temperature",
                "humidity": "sensor.grow_humidity",
                "light": "light.grow_led",
            }
        }))
        .send()
        .await
        .unwrap();

    let put = client
        .put(format!("{base}/api/grows/{grow_id}/light-cycle"))
        .json(&json!({ "lights_on": "06:00", "hours_on": { "germination": 24 } }))
        .send()
        .await
        .unwrap();
    assert_eq!(put.status(), 200);

    // VPD is the stepped role (env-section ticket 06): its day/night
    // Target Band follows the light timeline.
    let body: Value = client
        .get(format!(
            "{base}/api/grows/{grow_id}/history?role=vpd&window=day"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["degraded"], false);
    assert_eq!(body["bound"], true);
    let bands = body["bands"].as_array().unwrap();

    // Four stretches: the schedule-lit prefix, then the staircase.
    assert_eq!(bands.len(), 4, "schedule prefix + off + on + off");
    let states: Vec<bool> = bands
        .iter()
        .map(|band| band["light_on"].as_bool().expect("light state"))
        .collect();
    assert_eq!(states, [true, false, true, false]);
    for (index, band) in bands.iter().enumerate() {
        let expected = if states[index] {
            VPD_DAY_BAND
        } else {
            VPD_NIGHT_BAND
        };
        assert_eq!(
            (band["min"].as_f64().unwrap(), band["max"].as_f64().unwrap()),
            expected,
            "band {index} must carry the VPD range of its light state"
        );
        assert_eq!(band["phase"], "germination");
    }
    // The step instants are the fixture's staircase times (the mock
    // stamps them relative to its own request-time now; allow a small
    // skew against this test's now).
    let now = time::OffsetDateTime::now_utc();
    let boundary = |hours_ago: i64| now - time::Duration::hours(hours_ago);
    let assert_near = |band: &Value, key: &str, expected: time::OffsetDateTime| {
        let epoch = leaflog_core::chart::parse_epoch(band[key].as_str().unwrap()).unwrap();
        let instant = time::OffsetDateTime::from_unix_timestamp(epoch).unwrap();
        let skew = (instant - expected).abs();
        assert!(
            skew < time::Duration::minutes(5),
            "{key} at {instant} must be the fixture time {expected}"
        );
    };
    assert_near(&bands[0], "end", boundary(23));
    assert_near(&bands[1], "start", boundary(23));
    assert_near(&bands[1], "end", boundary(20));
    assert_near(&bands[2], "start", boundary(20));
    assert_near(&bands[2], "end", boundary(18));
    assert_near(&bands[3], "start", boundary(18));
    // The last dark stretch runs to the window end.
    let to = leaflog_core::chart::parse_epoch(body["series"]["to"].as_str().unwrap()).unwrap();
    assert_eq!(
        leaflog_core::chart::parse_epoch(bands[3]["end"].as_str().unwrap()),
        Some(to)
    );
}

#[tokio::test]
async fn grow_history_steps_follow_any_on_off_entity_bound_to_light() {
    // The Light binding role is domain-agnostic: any on/off entity
    // (light.*, switch.*, binary_sensor.*) is pickable, and its
    // on/off history drives the staircase exactly like the light.*
    // fixture. The mock serves switch.grow_panel the same
    // request-anchored staircase, and the 24 h schedule alone would
    // keep every band lit — so every dark band is switch-driven.
    let (mock_url, _mock) = spawn_mock().await;
    let (base, _db, bridge) = spawn_api(&mock_url).await;
    wait_connected(&bridge).await;
    let client = reqwest::Client::new();
    let grow_id = create_germinating_grow(&base, "Stepped by switch").await;

    let put = client
        .put(format!("{base}/api/grows/{grow_id}/bindings"))
        .json(&json!({
            "bindings": {
                "temperature": "sensor.grow_temperature",
                "humidity": "sensor.grow_humidity",
                "light": "switch.grow_panel",
            }
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(put.status(), 200);

    client
        .put(format!("{base}/api/grows/{grow_id}/light-cycle"))
        .json(&json!({ "lights_on": "06:00", "hours_on": { "germination": 24 } }))
        .send()
        .await
        .unwrap();

    // VPD steps with the switch-driven light timeline exactly like
    // the light.* fixture above.
    let body: Value = client
        .get(format!(
            "{base}/api/grows/{grow_id}/history?role=vpd&window=day"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["degraded"], false);
    assert_eq!(body["bound"], true);
    let bands = body["bands"].as_array().unwrap();
    let states = assert_germination_stepping(bands);
    for (band, &on) in bands.iter().zip(&states) {
        let expected = if on { VPD_DAY_BAND } else { VPD_NIGHT_BAND };
        assert_eq!(
            (band["min"].as_f64().unwrap(), band["max"].as_f64().unwrap()),
            expected,
            "band must carry the VPD range of the switch's light state (on: {on})"
        );
    }
}

#[tokio::test]
async fn grow_history_temperature_band_follows_the_grow_tolerances() {
    // The temperature Target Band personalizes with the grow's heat
    // and cold tolerances (env-section ticket 06): a doubly sensitive
    // grow draws a tighter corridor than the defaults. The supervisor
    // stays dead — the band needs no HA history, only the grow row.
    let dead_url = "http://127.0.0.1:1".to_owned();
    let (base, _db, _bridge) = spawn_api(&dead_url).await;
    let client = reqwest::Client::new();
    let started = (time::OffsetDateTime::now_utc() - time::Duration::days(3)).date();
    let grow: Value = client
        .post(format!("{base}/api/grows"))
        .json(&json!({
            "name": "Sensitive grow", "strain_id": null, "substrate": "soil",
            "started_on": started.to_string(), "status": "active", "notes": null,
            "heat_tolerance": "sensitive", "cold_tolerance": "sensitive",
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let grow_id = grow["id"].as_str().unwrap().to_owned();

    let body: Value = client
        .get(format!(
            "{base}/api/grows/{grow_id}/history?role=temperature&window=day"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let bands = body["bands"].as_array().unwrap();
    assert!(!bands.is_empty());
    let expected = leaflog_core::env::temperature_target_band(
        leaflog_core::grows::Tolerance::Sensitive,
        leaflog_core::grows::Tolerance::Sensitive,
    );
    for band in bands {
        assert_eq!(band["light_on"], Value::Null, "flat band, no light state");
        assert_eq!(
            (band["min"].as_f64().unwrap(), band["max"].as_f64().unwrap()),
            expected,
            "the sensitive grow's thresholds are the band edges"
        );
    }
    // Tighter than the default Average tolerances on both edges.
    let (default_min, default_max) = leaflog_core::env::temperature_target_band(
        leaflog_core::grows::Tolerance::Average,
        leaflog_core::grows::Tolerance::Average,
    );
    assert!(expected.0 > default_min && expected.1 < default_max);
}

// --- Coach light state (ticket 08) ---

/// The VPD tip's title out of a coach response.
fn vpd_tip(coach: &Value) -> String {
    coach["tips"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tip| tip["title"].as_str().unwrap().to_owned())
        .find(|title| title.contains("Keep VPD"))
        .expect("the coach must carry the VPD tip")
}

/// The coach reads the bound light entity's live state: the fake room
/// starts dark, so the VPD tip carries the researched night wording;
/// flipping the fixture flips the tip (spec §4.9, D096).
#[tokio::test]
async fn coach_tips_follow_the_bound_light_state() {
    let (mock_url, mock) = spawn_mock().await;
    let (base, _db, bridge) = spawn_api(&mock_url).await;
    wait_connected(&bridge).await;
    let client = reqwest::Client::new();
    let grow_id = create_germinating_grow(&base, "Coach light run").await;
    client
        .put(format!("{base}/api/grows/{grow_id}/bindings"))
        .json(&json!({ "bindings": { "light": "light.grow_led" } }))
        .send()
        .await
        .unwrap();

    // The mock room starts with the light off; wait until the live
    // map carries it.
    assert!(!mock.room.lock().await.light_on);
    for _ in 0..40 {
        if bridge
            .live_now()
            .entities
            .get("light.grow_led")
            .is_some_and(|entity| entity.state == "off")
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    let coach: Value = client
        .get(format!("{base}/api/grows/{grow_id}/coach"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let tip = vpd_tip(&coach);
    assert!(
        tip.contains("(lights off: keep close to daytime; watch the RH spike)"),
        "dark room must give the night wording, got {tip:?}"
    );
    assert!(
        !tip.contains("run higher"),
        "never 'run higher', got {tip:?}"
    );

    // Flip the fixture: the tip follows the live state.
    mock.room.lock().await.toggle_light();
    for _ in 0..40 {
        if bridge
            .live_now()
            .entities
            .get("light.grow_led")
            .is_some_and(|entity| entity.state == "on")
        {
            break;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    let coach: Value = client
        .get(format!("{base}/api/grows/{grow_id}/coach"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        vpd_tip(&coach).contains("(lights on)"),
        "lit room must give the day wording, got {:?}",
        vpd_tip(&coach)
    );
}

#[tokio::test]
async fn grow_history_carries_role_relevant_windowed_markers() {
    // Ticket 04: the history response carries the action log's
    // role-relevant events inside the window as `markers` — root-zone
    // roles get water/feed/transplant, air roles lights/light-change/
    // flip/phase-change, and every other kind (and out-of-window
    // event) drops. Timestamps run relative to the real clock so the
    // day/week windows filter like they do in production.
    let dead_url = "http://127.0.0.1:1".to_owned();
    let (base, db, _bridge) = spawn_api(&dead_url).await;
    let client = reqwest::Client::new();
    let grow_id = create_grow(&base).await;

    // The acceptance shape: a grow with bound sensors, snapshot
    // points on the chart, and log events — the markers must ride a
    // real history response, not an empty one.
    client
        .put(format!("{base}/api/grows/{grow_id}/bindings"))
        .json(&json!({
            "bindings": {
                "temperature": "sensor.grow_temperature",
                "soil_moisture": "sensor.grow_soil_moisture",
            }
        }))
        .send()
        .await
        .unwrap();

    let now = time::OffsetDateTime::now_utc();
    let at = |hours_ago: i64| {
        (now - time::Duration::hours(hours_ago))
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap()
    };
    let events = [
        ("water", json!({}), 2),
        ("feed", json!({}), 3),
        ("transplant", json!({}), 5),
        ("lights", json!({}), 4),
        ("phase_change", json!({"target_phase": "vegetative"}), 6),
        ("water", json!({}), 30), // week-only: outside the day window
        ("note", json!({}), 1),   // never a marker
    ];
    let mut snapshot_entry_id = String::new();
    for (action_type, details, hours_ago) in events {
        let entry: Value = client
            .post(format!("{base}/api/grows/{grow_id}/actions"))
            .json(&json!({
                "plant_ids": [], "occurred_at": at(hours_ago),
                "action_type": action_type, "details": details, "notes": null,
                "appearance_tags": [],
            }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        assert_eq!(
            entry["action_type"],
            json!(action_type),
            "{action_type} must log"
        );
        // The note entry anchors the snapshots (a log-time capture).
        if action_type == "note" {
            snapshot_entry_id = entry["id"].as_str().unwrap().to_owned();
        }
    }

    // Inject the snapshot captures the dead HA cannot provide: one
    // reading per bound role, taken now (inside both windows).
    db.with(move |conn| {
        leaflog_backend::store::sensors::snapshot(
            conn,
            &snapshot_entry_id,
            "temperature",
            "sensor.grow_temperature",
            "22.5",
        )?;
        leaflog_backend::store::sensors::snapshot(
            conn,
            &snapshot_entry_id,
            "soil_moisture",
            "sensor.grow_soil_moisture",
            "41",
        )
    })
    .await
    .unwrap();

    async fn fetch_history(
        client: &reqwest::Client,
        base: &str,
        grow_id: &str,
        role: &str,
        window: &str,
    ) -> Value {
        client
            .get(format!(
                "{base}/api/grows/{grow_id}/history?role={role}&window={window}"
            ))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap()
    }

    // Root zone, day window: transplant, feed, water ascending, each
    // `at` inside the window; lights, phase picks and notes drop.
    // The chart carries its snapshot point alongside the markers.
    let soil_day = fetch_history(&client, &base, &grow_id, "soil_moisture", "day").await;
    let kinds = |body: &Value| -> Vec<String> {
        body["markers"]
            .as_array()
            .expect("markers array")
            .iter()
            .map(|marker| marker["kind"].as_str().unwrap().to_owned())
            .collect()
    };

    assert!(
        !soil_day["series"]["points"]
            .as_array()
            .expect("points array")
            .is_empty()
    );
    assert_eq!(kinds(&soil_day), ["transplant", "feed", "water"]);
    let soil_ats: Vec<i64> = soil_day["markers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|marker| {
            time::OffsetDateTime::parse(
                marker["at"].as_str().unwrap(),
                &time::format_description::well_known::Rfc3339,
            )
            .unwrap()
            .unix_timestamp()
        })
        .collect();
    let window = soil_ats.windows(2).all(|pair| pair[0] <= pair[1]);
    assert!(window, "markers must ascend: {soil_ats:?}");

    // Air roles read light and phase events only.
    let air_day = fetch_history(&client, &base, &grow_id, "temperature", "day").await;
    assert_eq!(kinds(&air_day), ["phase_change", "lights"]);

    // The week window also holds the 30-hour-old watering.
    let soil_week = fetch_history(&client, &base, &grow_id, "soil_moisture", "week").await;
    assert_eq!(kinds(&soil_week), ["water", "transplant", "feed", "water"]);

    // Roles outside the environment section take no markers at all.
    let light = fetch_history(&client, &base, &grow_id, "light", "day").await;
    assert_eq!(light["markers"], json!([]));
}

/// SPEC review a2: `MARKER_SCAN_LIMIT` bounds the newest rows the
/// marker read considers INSIDE the requested window, not across it.
/// A log dense with out-of-window rows — here future-dated
/// waterings, the log's newest rows — must not crowd an in-window
/// marker out of the scan.
#[tokio::test]
async fn grow_history_marker_scan_filters_the_window_before_its_limit() {
    let dead_url = "http://127.0.0.1:1".to_owned();
    let (base, db, _bridge) = spawn_api(&dead_url).await;
    let grow_id = create_grow(&base).await;

    let now = time::OffsetDateTime::now_utc();
    let at = move |offset: time::Duration| {
        (now + offset)
            .format(&time::format_description::well_known::Rfc3339)
            .unwrap()
    };

    // Seeded straight through the store: 1000+ API round-trips buy
    // nothing. `MARKER_SCAN_LIMIT` + 5 future-dated waterings are the
    // log's newest rows; the transplant is 20 days old, well inside
    // the month window. Under the pre-fix scan (newest 1000 rows
    // first, window filter second) the waterings fill the read and
    // the marker is lost.
    let marker_grow_id = grow_id.clone();
    db.with(move |conn| {
        use leaflog_backend::store::actions;
        use leaflog_core::actions::{ActionDetails, ActionType};
        let seed = |occurred_at: String,
                    action_type: ActionType|
         -> Result<(), leaflog_backend::db::StoreError> {
            let input = leaflog_core::grows::ActionInput {
                plant_ids: Vec::new(),
                occurred_at,
                action_type,
                details: ActionDetails::default(),
                notes: None,
                appearance_tags: Vec::new(),
            };
            actions::create(
                conn,
                &actions::from_input(&marker_grow_id, &input, leaflog_core::Phase::Vegetative, 20),
            )
        };
        let extra = i64::from(leaflog_backend::api_chart::MARKER_SCAN_LIMIT) + 5;
        for hour in 0..extra {
            seed(
                at(time::Duration::hours(hour) + time::Duration::minutes(1)),
                ActionType::Water,
            )?;
        }
        seed(at(-time::Duration::days(20)), ActionType::Transplant)
    })
    .await
    .unwrap();

    let client = reqwest::Client::new();
    let history: Value = client
        .get(format!(
            "{base}/api/grows/{grow_id}/history?role=soil_moisture&window=month"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let kinds: Vec<&str> = history["markers"]
        .as_array()
        .expect("markers array")
        .iter()
        .map(|marker| marker["kind"].as_str().unwrap())
        .collect();
    assert_eq!(
        kinds,
        ["transplant"],
        "the in-window marker must survive a log of newer out-of-window rows: {history:?}"
    );
}

// --- Night shading behind the Air charts (env-section ticket 05) ---

/// Parses one response night interval's `[start, end)` epoch pair.
fn night_bounds(entry: &Value) -> (i64, i64) {
    (
        leaflog_core::chart::parse_epoch(entry["start"].as_str().unwrap()).unwrap(),
        leaflog_core::chart::parse_epoch(entry["end"].as_str().unwrap()).unwrap(),
    )
}

/// A grow's night shading follows the Light Cycle when no light
/// entity is bound: the schedule's off stretches, inside the chart
/// window, and every edge a real cycle boundary. Editing the cycle
/// moves the shading on the next fetch.
#[tokio::test]
async fn grow_history_shades_nights_from_the_light_cycle_when_unbound() {
    let dead_url = "http://127.0.0.1:1".to_owned();
    let (base, _db, _bridge) = spawn_api(&dead_url).await;
    let client = reqwest::Client::new();
    let grow_id = create_germinating_grow(&base, "Nights unbound").await;

    client
        .put(format!("{base}/api/grows/{grow_id}/bindings"))
        .json(&json!({ "bindings": { "temperature": "sensor.grow_temperature" } }))
        .send()
        .await
        .unwrap();
    let put = client
        .put(format!("{base}/api/grows/{grow_id}/light-cycle"))
        .json(&json!({ "lights_on": "08:00", "hours_on": { "germination": 12 } }))
        .send()
        .await
        .unwrap();
    assert_eq!(put.status(), 200);

    let fetch_day = |role: &'static str| {
        let base = base.clone();
        let grow_id = grow_id.clone();
        let client = client.clone();
        async move {
            client
                .get(format!(
                    "{base}/api/grows/{grow_id}/history?role={role}&window=day"
                ))
                .send()
                .await
                .unwrap()
                .json::<Value>()
                .await
                .unwrap()
        }
    };

    let body = fetch_day("temperature").await;
    let from = leaflog_core::chart::parse_epoch(body["series"]["from"].as_str().unwrap()).unwrap();
    let to = leaflog_core::chart::parse_epoch(body["series"]["to"].as_str().unwrap()).unwrap();

    let nights = body["nights"].as_array().expect("nights present");
    assert!(
        !nights.is_empty(),
        "a 12 h cycle leaves nights in a day window"
    );
    let mut total_dark = 0_i64;
    for entry in nights {
        assert_eq!(entry["on"], false, "only dark intervals are nights");
        let (start, end) = night_bounds(entry);
        assert!(
            from <= start && start < end && end <= to,
            "night stays inside the window: [{start}, {end}) vs [{from}, {to})"
        );
        total_dark += end - start;
        // Interior edges are cycle boundaries: nights start at
        // lights-off (20:00) and end at lights-on (08:00). The
        // canonical timezone is UTC when HA is unreachable.
        if start > from {
            let wall = time::OffsetDateTime::from_unix_timestamp(start).unwrap();
            assert_eq!(
                (wall.hour(), wall.minute()),
                (20, 0),
                "an interior night starts at lights-off"
            );
        }
        if end < to {
            let wall = time::OffsetDateTime::from_unix_timestamp(end).unwrap();
            assert_eq!(
                (wall.hour(), wall.minute()),
                (8, 0),
                "an interior night ends at lights-on"
            );
        }
    }
    assert_eq!(total_dark, 12 * 3600, "half the day window is dark");

    // Editing the Light Cycle moves the shading on the next fetch:
    // nights now run 08:00–20:00 instead.
    let put = client
        .put(format!("{base}/api/grows/{grow_id}/light-cycle"))
        .json(&json!({ "lights_on": "20:00", "hours_on": { "germination": 12 } }))
        .send()
        .await
        .unwrap();
    assert_eq!(put.status(), 200);
    let shifted = fetch_day("temperature").await;
    let to_shift =
        leaflog_core::chart::parse_epoch(shifted["series"]["to"].as_str().unwrap()).unwrap();
    let from_shift =
        leaflog_core::chart::parse_epoch(shifted["series"]["from"].as_str().unwrap()).unwrap();
    let nights = shifted["nights"].as_array().unwrap();
    assert!(!nights.is_empty());
    for entry in nights {
        let (start, end) = night_bounds(entry);
        if start > from_shift {
            let wall = time::OffsetDateTime::from_unix_timestamp(start).unwrap();
            assert_eq!(
                (wall.hour(), wall.minute()),
                (8, 0),
                "after the edit nights start at lights-on 08:00"
            );
        }
        if end < to_shift {
            let wall = time::OffsetDateTime::from_unix_timestamp(end).unwrap();
            assert_eq!(
                (wall.hour(), wall.minute()),
                (20, 0),
                "after the edit nights end at lights-off 20:00"
            );
        }
    }

    // Only the Air roles carry nights: the root-zone and light roles
    // stay bare.
    let soil = fetch_day("soil_moisture").await;
    assert_eq!(soil["nights"], json!([]), "root-zone roles shade nothing");
    let light = fetch_day("light").await;
    assert_eq!(
        light["nights"],
        json!([]),
        "the light role itself shades nothing"
    );
}

/// A bound light entity's observed history drives the night shading:
/// the mock's staircase (off at now−23 h, on at now−20 h, off at
/// now−18 h) is the timeline, with the 24 h-on schedule covering only
/// the pre-sensor stretch lit. Every Air role carries the same
/// nights.
#[tokio::test]
async fn grow_history_shades_nights_from_the_bound_light_history() {
    let (mock_url, _mock) = spawn_mock().await;
    let (base, _db, bridge) = spawn_api(&mock_url).await;
    wait_connected(&bridge).await;
    let client = reqwest::Client::new();
    let grow_id = create_germinating_grow(&base, "Nights by fixture").await;

    client
        .put(format!("{base}/api/grows/{grow_id}/bindings"))
        .json(&json!({
            "bindings": {
                "temperature": "sensor.grow_temperature",
                "co2": "sensor.grow_co2",
                "light": "light.grow_led",
            }
        }))
        .send()
        .await
        .unwrap();
    let put = client
        .put(format!("{base}/api/grows/{grow_id}/light-cycle"))
        .json(&json!({ "lights_on": "06:00", "hours_on": { "germination": 24 } }))
        .send()
        .await
        .unwrap();
    assert_eq!(put.status(), 200);

    let body: Value = client
        .get(format!(
            "{base}/api/grows/{grow_id}/history?role=temperature&window=day"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(body["degraded"], false, "the mock Supervisor is reachable");

    let to = leaflog_core::chart::parse_epoch(body["series"]["to"].as_str().unwrap()).unwrap();
    let nights = body["nights"].as_array().expect("nights present");
    assert_eq!(
        nights.len(),
        2,
        "the staircase's two dark stretches, schedule-lit prefix"
    );
    for entry in nights {
        assert_eq!(entry["on"], false);
    }
    // The mock stamps and the response `now` run off slightly
    // different clocks; a two-minute slack absorbs the drift.
    let slack = 120;
    let (first_start, first_end) = night_bounds(&nights[0]);
    let (second_start, second_end) = night_bounds(&nights[1]);
    assert!(
        (first_start - (to - 23 * 3600)).abs() <= slack
            && (first_end - (to - 20 * 3600)).abs() <= slack,
        "first dark stretch [now−23 h, now−20 h): got [{first_start}, {first_end}) with to {to}"
    );
    assert!(
        (second_start - (to - 18 * 3600)).abs() <= slack && second_end == to,
        "second dark stretch [now−18 h, now]: got [{second_start}, {second_end}) with to {to}"
    );

    // Every Air role shades the same timeline — CO₂ here; the
    // root-zone roles stay bare (asserted by the unbound test).
    let co2: Value = client
        .get(format!(
            "{base}/api/grows/{grow_id}/history?role=co2&window=day"
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(co2["nights"], body["nights"], "Air roles share the shading");
}

// --- Locale endpoint (D112) ----------------------------------------------

/// Polls `GET /api/locale` until the reported language matches `want`
/// (the config fetch races the test start).
async fn wait_language(base: &str, want: &str) {
    let client = reqwest::Client::new();
    for _ in 0..40 {
        let body: Value = client
            .get(format!("{base}/api/locale"))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        if body["language"].as_str() == Some(want) {
            return;
        }
        tokio::time::sleep(Duration::from_millis(250)).await;
    }
    panic!("locale endpoint never reported {want:?}");
}

/// The locale endpoint reflects the mock Supervisor's configured user
/// language (D112): the mock's `language` config field flows through
/// the bridge config refresh into `GET /api/locale`.
#[tokio::test]
async fn locale_endpoint_reports_mock_language() {
    let (mock_url, mock_state) = spawn_mock().await;
    *mock_state.language.lock().await = "de-DE".to_owned();
    let (base, _db, bridge) = spawn_api(&mock_url).await;
    wait_connected(&bridge).await;
    wait_language(&base, "de-DE").await;
}

/// With the mock left at its default (`en`), the endpoint reports
/// English — the chain's fallback anchor for German-forced browsers.
#[tokio::test]
async fn locale_endpoint_defaults_to_english() {
    let (mock_url, _mock) = spawn_mock().await;
    let (base, _db, bridge) = spawn_api(&mock_url).await;
    wait_connected(&bridge).await;
    wait_language(&base, "en").await;
}
