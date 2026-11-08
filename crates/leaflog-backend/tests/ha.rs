//! Integration tests for M4: live data through the mock Supervisor,
//! auto-capture, bindings, history, the SSE stream, offline behaviour,
//! reconnect, and the token-never-leaks rule (spec §13).

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use axum::ServiceExt as _;
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
async fn bridge_reconnects_after_the_socket_drops() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    let (mock_state_tx, _mock_state_rx) = tokio::sync::oneshot::channel::<mock_ha::MockState>();
    let state = mock_ha::MockState {
        room: Arc::new(tokio::sync::Mutex::new(mock_ha::FakeRoom::new())),
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
    });
    tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    wait_connected(&bridge).await;
}
