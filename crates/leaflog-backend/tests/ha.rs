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
            "name": "HA run", "strain_id": null, "substrate": "soil",
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

    // The resubscribe reconnects with one backoff step; poll like the
    // callers of the live map (env snapshot, auto-capture) do.
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
            "strain_id": null,
            "grow_space_id": null,
            "seed_batch_id": null,
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
            "strain_id": null,
            "grow_space_id": null,
            "seed_batch_id": null,
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
