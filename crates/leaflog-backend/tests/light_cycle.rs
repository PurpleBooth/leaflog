//! Integration tests for the per-grow Light Cycle endpoints
//! (ticket 05): get/put/delete with validation, reset-to-default and
//! persistence across a backend restart.

use std::net::SocketAddr;
use std::path::PathBuf;

use axum::ServiceExt as _;
use leaflog_backend::photos::PhotoStore;
use leaflog_backend::{AppState, Assets, Db, Mode, app, store};
use serde_json::{Value, json};

/// The API fixture dir the other suites use (no frontend under test
/// here, but the state wants an asset source).
fn fixture() -> PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dist")
}

fn offline_bridge() -> std::sync::Arc<leaflog_backend::ha::HaBridge> {
    std::sync::Arc::new(leaflog_backend::ha::HaBridge::spawn(
        "http://127.0.0.1:1".to_owned(),
        leaflog_backend::ha::SupervisorToken::new("test-token"),
        Db::open_in_memory().unwrap(),
    ))
}

/// Spawns the API on an in-memory DB; returns the base URL.
async fn spawn_memory() -> String {
    let db = Db::open_in_memory().unwrap();
    spawn_with_db(
        db,
        std::env::temp_dir().join(format!("lc-{}", leaflog_backend::db::new_id())),
    )
    .await
}

/// Spawns the API on a file-backed DB; returns the base URL and the
/// file path, so a test can drop the app and reopen the file.
async fn spawn_file(tag: &str) -> (String, PathBuf) {
    let dir = std::env::temp_dir().join(format!(
        "leaflog-lc-{tag}-{}",
        leaflog_backend::db::new_id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("leaflog.db");
    let db = Db::open(&path).unwrap();
    let base = spawn_with_db(db, dir.join("photos")).await;
    (base, path)
}

async fn spawn_with_db(db: Db, photo_root: PathBuf) -> String {
    let state = AppState::new(
        Assets::Disk(fixture()),
        Mode::Dev,
        None,
        db,
        PhotoStore::new(photo_root),
        offline_bridge(),
    );
    let service = app::service(state);
    let make = service.into_make_service_with_connect_info::<SocketAddr>();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, make).await.unwrap();
    });
    format!("http://{addr}")
}

async fn create_grow(base: &str) -> String {
    let client = reqwest::Client::new();
    let grow: Value = client
        .post(format!("{base}/api/grows"))
        .json(&json!({
            "name": "Light cycle run",
            "strain_id": null,
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
    grow["id"].as_str().unwrap().to_owned()
}

/// The empty cycle a fresh grow answers: every part on its default.
fn empty_cycle() -> Value {
    json!({
        "lights_on": null,
        "hours_on": {
            "germination": null,
            "seedling": null,
            "vegetative": null,
            "flower": null,
        },
    })
}

#[tokio::test]
async fn fresh_grow_answers_the_empty_cycle() {
    let base = spawn_memory().await;
    let client = reqwest::Client::new();
    let id = create_grow(&base).await;

    let cycle: Value = client
        .get(format!("{base}/api/grows/{id}/light-cycle"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(cycle, empty_cycle());
}

// --- Observed reality (grow-page rework, ticket 03) ---------------------

/// Spawns the mock Supervisor and the API against it (the staircase
/// fixture entities light.grow_led / switch.grow_panel), ready for
/// the bridge to connect.
async fn spawn_mock_api() -> (String, std::sync::Arc<leaflog_backend::ha::HaBridge>) {
    let mock_state = mock_ha::MockState {
        room: std::sync::Arc::new(tokio::sync::Mutex::new(mock_ha::FakeRoom::new())),
        notifications: std::sync::Arc::new(tokio::sync::Mutex::new(Vec::new())),
    };
    let mock_listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mock_addr = mock_listener.local_addr().unwrap();
    let mock_router = mock_ha::app(mock_state);
    tokio::spawn(async move {
        axum::serve(mock_listener, mock_router).await.unwrap();
    });

    let db = Db::open_in_memory().unwrap();
    let bridge = std::sync::Arc::new(leaflog_backend::ha::HaBridge::spawn(
        format!("http://{mock_addr}"),
        leaflog_backend::ha::SupervisorToken::new("light-cycle-secret"),
        db.clone(),
    ));
    let state = AppState::new(
        Assets::Disk(fixture()),
        Mode::Dev,
        None,
        db,
        PhotoStore::new(std::env::temp_dir()),
        bridge.clone(),
    );
    let service = app::service(state);
    let make = service.into_make_service_with_connect_info::<SocketAddr>();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, make).await.unwrap();
    });
    (format!("http://{addr}"), bridge)
}

async fn wait_connected(bridge: &leaflog_backend::ha::HaBridge) {
    for _ in 0..40 {
        if bridge.live_now().connected {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(250)).await;
    }
    panic!("bridge never connected to the mock");
}

async fn bind_light(client: &reqwest::Client, base: &str, id: &str, entity: &str) {
    let _: Value = client
        .put(format!("{base}/api/grows/{id}/bindings"))
        .json(&json!({ "bindings": { "light": entity } }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
}

async fn get_cycle(client: &reqwest::Client, base: &str, id: &str) -> Value {
    client
        .get(format!("{base}/api/grows/{id}/light-cycle"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap()
}

/// Minutes since midnight, wrapping (for near-midnight skew).
fn minute_of(text: &str) -> i64 {
    let (hour, minute) = text.split_once(':').unwrap();
    hour.parse::<i64>().unwrap() * 60 + minute.parse::<i64>().unwrap()
}

#[tokio::test]
async fn bound_light_serves_the_observed_block_from_the_staircase() {
    let (base, bridge) = spawn_mock_api().await;
    wait_connected(&bridge).await;
    let client = reqwest::Client::new();
    let id = create_grow(&base).await;
    bind_light(&client, &base, &id, "light.grow_led").await;

    let cycle = get_cycle(&client, &base, &id).await;
    let observed = &cycle["observed"];
    assert_eq!(observed["entity_id"], "light.grow_led", "cycle: {cycle}");
    // The staircase is off -> on (now - 20 h) -> off (now - 18 h):
    // two lit hours across the roughly 23 h span.
    assert_eq!(observed["hours_on"], 2);
    // The learned on-time is that on-transition's local wall time in
    // the mock's Europe/Berlin zone. Mock and test each take their
    // own "now", so allow a minute of skew across a roll-over.
    let learned = observed["lights_on"].as_str().unwrap().to_owned();
    let local = (jiff::Timestamp::now() - jiff::Span::new().hours(20))
        .to_zoned(jiff::tz::TimeZone::get("Europe/Berlin").unwrap());
    let expected = format!("{:02}:{:02}", local.hour(), local.minute());
    let skew = (minute_of(&learned) - minute_of(&expected)).abs();
    let skew = skew.min(1440 - skew);
    assert!(skew <= 1, "learned {learned}, expected {expected}");
}

#[tokio::test]
async fn unbinding_drops_the_observed_block() {
    let (base, bridge) = spawn_mock_api().await;
    wait_connected(&bridge).await;
    let client = reqwest::Client::new();
    let id = create_grow(&base).await;
    bind_light(&client, &base, &id, "light.grow_led").await;

    let bound = get_cycle(&client, &base, &id).await;
    assert!(bound.get("observed").is_some_and(Value::is_object));

    // An empty bindings map unbinds every role.
    let _: Value = client
        .put(format!("{base}/api/grows/{id}/bindings"))
        .json(&json!({ "bindings": {} }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let unbound = get_cycle(&client, &base, &id).await;
    assert_eq!(unbound, empty_cycle());
}

#[tokio::test]
async fn rebinding_serves_the_new_entity_immediately() {
    let (base, bridge) = spawn_mock_api().await;
    wait_connected(&bridge).await;
    let client = reqwest::Client::new();
    let id = create_grow(&base).await;
    bind_light(&client, &base, &id, "light.grow_led").await;

    // First GET fills the cache for the first entity.
    let first = get_cycle(&client, &base, &id).await;
    assert_eq!(first["observed"]["entity_id"], "light.grow_led");

    // The very next GET after rebinding must answer for the new
    // entity: a binding change invalidates any cached derivation.
    bind_light(&client, &base, &id, "switch.grow_panel").await;
    let second = get_cycle(&client, &base, &id).await;
    assert_eq!(second["observed"]["entity_id"], "switch.grow_panel");
    assert_eq!(second["observed"]["hours_on"], 2);
}

#[tokio::test]
async fn observed_block_fails_closed_when_ha_is_unreachable() {
    // spawn_memory points the bridge at a dead port: bound but no
    // answer, so the GET keeps serving the plain editable cycle.
    let base = spawn_memory().await;
    let client = reqwest::Client::new();
    let id = create_grow(&base).await;
    bind_light(&client, &base, &id, "light.grow_led").await;

    let cycle = get_cycle(&client, &base, &id).await;
    assert_eq!(cycle, empty_cycle());
}

#[tokio::test]
async fn put_stores_the_cycle_and_grow_payload_carries_it() {
    let base = spawn_memory().await;
    let client = reqwest::Client::new();
    let id = create_grow(&base).await;

    let stored: Value = client
        .put(format!("{base}/api/grows/{id}/light-cycle"))
        .json(&json!({
            "lights_on": "07:30",
            "hours_on": { "vegetative": 20 },
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(stored["lights_on"], "07:30");
    assert_eq!(stored["hours_on"]["vegetative"], 20);
    // Unset phases stay null (they keep their preset defaults).
    assert_eq!(stored["hours_on"]["flower"], Value::Null);

    // The grow payload itself carries the cycle for readers that
    // already hold a grow.
    let grow: Value = client
        .get(format!("{base}/api/grows/{id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(grow["light_cycle"]["lights_on"], "07:30");

    // Editing the grow must not drop the cycle: update() rewrites the
    // row, so the column has to survive.
    let mut edited = grow.clone();
    edited["name"] = json!("Renamed");
    let _: Value = client
        .put(format!("{base}/api/grows/{id}"))
        .json(&json!({
            "name": "Renamed",
            "strain_id": null,
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
    let grow: Value = client
        .get(format!("{base}/api/grows/{id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(grow["name"], "Renamed");
    assert_eq!(grow["light_cycle"]["lights_on"], "07:30");
}

#[tokio::test]
async fn delete_resets_to_the_default_cycle() {
    let base = spawn_memory().await;
    let client = reqwest::Client::new();
    let id = create_grow(&base).await;

    let put = client
        .put(format!("{base}/api/grows/{id}/light-cycle"))
        .json(&json!({ "lights_on": "21:00", "hours_on": { "flower": 11 } }))
        .send()
        .await
        .unwrap();
    assert_eq!(put.status(), 200);

    let deleted = client
        .delete(format!("{base}/api/grows/{id}/light-cycle"))
        .send()
        .await
        .unwrap();
    assert_eq!(deleted.status(), 204);

    let cycle: Value = client
        .get(format!("{base}/api/grows/{id}/light-cycle"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(cycle, empty_cycle());
}

#[tokio::test]
async fn light_cycle_persists_across_a_restart() {
    let (base, db_path) = spawn_file("restart").await;
    let client = reqwest::Client::new();
    let id = create_grow(&base).await;
    let stored: Value = client
        .put(format!("{base}/api/grows/{id}/light-cycle"))
        .json(&json!({
            "lights_on": "05:45",
            "hours_on": { "germination": 6, "flower": 12 },
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(stored["lights_on"], "05:45");

    // Drop the app and reopen the same database file: what a fresh
    // backend process would read off disk.
    let reopened = Db::open(&db_path).unwrap();
    let grow_id = id.clone();
    let grow = reopened
        .with(move |conn| store::grows::get(conn, &grow_id))
        .await
        .unwrap();
    let cycle = grow.light_cycle.expect("cycle must be stored");
    assert_eq!(
        cycle.lights_on.map(|time| time.to_string()),
        Some("05:45".to_owned())
    );
    assert_eq!(cycle.hours_on.germination, Some(6));
    assert_eq!(cycle.hours_on.flower, Some(12));
    assert_eq!(cycle.hours_on.vegetative, None);
}

#[tokio::test]
async fn put_rejects_a_bad_lights_on_time_with_a_clear_error() {
    let base = spawn_memory().await;
    let client = reqwest::Client::new();
    let id = create_grow(&base).await;

    for bad in ["25:00", "abc", "12", "12:60"] {
        let (status, body) = reject(
            &client,
            &format!("{base}/api/grows/{id}/light-cycle"),
            &json!({ "lights_on": bad, "hours_on": {} }),
        )
        .await;
        assert_eq!(status, 400, "{bad} must be rejected");
        let text = body["error"].as_str().unwrap_or_default();
        assert!(
            text.contains("HH:MM"),
            "message for {bad} must say the accepted shape, got {text:?}"
        );
    }
}

#[tokio::test]
async fn put_rejects_out_of_range_hours_with_a_clear_error() {
    let base = spawn_memory().await;
    let client = reqwest::Client::new();
    let id = create_grow(&base).await;

    for (phase, hours) in [("vegetative", 0), ("flower", 30), ("germination", -3)] {
        let (status, body) = reject(
            &client,
            &format!("{base}/api/grows/{id}/light-cycle"),
            &json!({ "lights_on": null, "hours_on": { phase: hours } }),
        )
        .await;
        assert_eq!(status, 400, "{phase}={hours} must be rejected");
        let text = body["error"].as_str().unwrap_or_default();
        assert!(
            text.contains("1–24"),
            "message must name the accepted range, got {text:?}"
        );
        assert!(
            text.contains(phase),
            "message must name the phase, got {text:?}"
        );
    }

    // A missing grow is still a 404, not a validation error.
    let missing = client
        .put(format!("{base}/api/grows/nope/light-cycle"))
        .json(&json!({ "lights_on": "06:00", "hours_on": {} }))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), 404);
}

/// PUTs a body and returns the status plus the JSON error body
/// (400/404 responses carry `{"error": ...}`).
async fn reject(client: &reqwest::Client, url: &str, body: &Value) -> (u16, Value) {
    let response = client.put(url).json(body).send().await.unwrap();
    let status = response.status().as_u16();
    (status, response.json().await.unwrap_or(Value::Null))
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

/// `HH:MM` in the canonical zone (UTC in this suite: the offline
/// bridge never learns a timezone and no display one is set).
fn hhmm(moment: time::OffsetDateTime) -> String {
    format!("{:02}:{:02}", moment.hour(), moment.minute())
}

/// Without a bound light entity the coach evaluates the grow's Light
/// Cycle for "now": a cycle switched on ten minutes ago (one hour on,
/// every lit phase) reads as lights on; rewinding it two hours reads
/// as lights off with the researched night wording (D096).
#[tokio::test]
async fn coach_asks_the_light_cycle_when_nothing_is_bound() {
    let base = spawn_memory().await;
    let client = reqwest::Client::new();
    let grow_id = create_grow(&base).await;
    let now = time::OffsetDateTime::now_utc();
    let hours = json!({ "germination": 1, "seedling": 1, "vegetative": 1, "flower": 1 });

    client
        .put(format!("{base}/api/grows/{grow_id}/light-cycle"))
        .json(&json!({
            "lights_on": hhmm(now - time::Duration::minutes(10)),
            "hours_on": hours,
        }))
        .send()
        .await
        .unwrap();
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
        "an on-period covering now must read as lit, got {:?}",
        vpd_tip(&coach)
    );

    // Same grow, cycle wound back two hours: dark for the last hour.
    client
        .put(format!("{base}/api/grows/{grow_id}/light-cycle"))
        .json(&json!({
            "lights_on": hhmm(now - time::Duration::hours(2)),
            "hours_on": hours,
        }))
        .send()
        .await
        .unwrap();
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
        "an ended on-period must read as dark, got {tip:?}"
    );
    assert!(!tip.contains("run higher"));
}

/// A bound light entity whose live state is unknown (the bridge is
/// offline here) falls back to the newest auto-captured snapshot —
/// which also beats a schedule that would say otherwise (D096).
#[tokio::test]
async fn coach_reads_the_newest_snapshot_when_the_live_map_is_empty() {
    let db = Db::open_in_memory().unwrap();
    let base = spawn_with_db(
        db.clone(),
        std::env::temp_dir().join(format!("coach-{}", leaflog_backend::db::new_id())),
    )
    .await;
    let client = reqwest::Client::new();
    let grow_id = create_grow(&base).await;

    client
        .put(format!("{base}/api/grows/{grow_id}/bindings"))
        .json(&json!({ "bindings": { "light": "light.grow_led" } }))
        .send()
        .await
        .unwrap();
    // The cycle would keep the lights on all day — the snapshot wins.
    client
        .put(format!("{base}/api/grows/{grow_id}/light-cycle"))
        .json(&json!({
            "lights_on": "00:00",
            "hours_on": { "germination": 24, "seedling": 24, "vegetative": 24, "flower": 24 },
        }))
        .send()
        .await
        .unwrap();

    // An action to hang the snapshot off, then the snapshot itself.
    let action: Value = client
        .post(format!("{base}/api/grows/{grow_id}/actions"))
        .json(&json!({
            "plant_ids": [],
            "occurred_at": leaflog_backend::db::now_rfc3339(),
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
    let action_id = action["id"].as_str().unwrap().to_owned();
    db.with(move |conn| {
        store::sensors::snapshot(conn, &action_id, "light", "light.grow_led", "off")
    })
    .await
    .unwrap();

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
        "the newest snapshot's off state must win, got {tip:?}"
    );
}
