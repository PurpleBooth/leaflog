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
