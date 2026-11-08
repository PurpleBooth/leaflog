//! Integration tests for the M2 JSON API: equipment CRUD, strain CRUD
//! and clone, seeding, settings, and validation errors.

use std::net::SocketAddr;

use axum::ServiceExt as _;
use std::path::Path;

use leaflog_backend::{AppState, Assets, Db, Mode};
use leaflog_core::equipment::EquipmentInput;
use leaflog_core::strain::StrainInput;
use serde_json::{Value, json};

/// Spawns the API on an ephemeral port with an empty in-memory database.
async fn spawn_api() -> String {
    spawn_with(Db::open_in_memory().unwrap()).await
}

/// Spawns the API on an ephemeral port with the given database (used to
/// test seeding).
async fn spawn_with(db: Db) -> String {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dist");
    let state = AppState::new(
        Assets::Disk(fixture),
        Mode::Dev,
        None,
        db,
        leaflog_backend::photos::PhotoStore::new(std::env::temp_dir()),
        offline_bridge(),
    );
    let app =
        leaflog_backend::app::service(state).into_make_service_with_connect_info::<SocketAddr>();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    format!("http://{addr}")
}

fn sample_equipment(name: &str) -> Value {
    json!({
        "name": name,
        "category": "light",
        "brand_model": "Spider Farmer SE-3000",
        "real_watts": 285.0,
        "dimmable": true,
        "purchase_date": "2026-05-01",
        "purchase_price": 289.99,
        "currency": "EUR",
        "notes": null,
    })
}

fn sample_strain(name: &str) -> Value {
    json!({
        "name": name,
        "breeder": "Test Breeder",
        "photoperiod": "photoperiod",
        "sex": "feminized",
        "indica_pct": 55,
        "landrace_tags": ["haze"],
        "flower_days_min": 56,
        "flower_days_max": 63,
        "stretch_factor": 1.5,
        "veg_weeks_min": null,
        "veg_weeks_max": null,
        "height_class": "medium",
        "auto_total_days_min": null,
        "auto_total_days_max": null,
        "yield_notes": null,
        "terpene_notes": null,
        "effect_notes": null,
        "heat_tolerance": "average",
        "cold_tolerance": "average",
        "light_tolerance": "average",
        "user_rating": null,
        "notes": null,
    })
}

#[tokio::test]
async fn equipment_crud_round_trip() {
    let base = spawn_api().await;
    let client = reqwest::Client::new();

    // Create.
    let created: Value = client
        .post(format!("{base}/api/equipment"))
        .json(&sample_equipment("Mars Hydro TS-1000"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = created["id"].as_str().unwrap().to_owned();
    assert_eq!(created["name"], "Mars Hydro TS-1000");
    assert_eq!(created["category"], "light");
    assert_eq!(created["dimmable"], true);

    // Read (list + get).
    let list: Vec<Value> = client
        .get(format!("{base}/api/equipment"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list.len(), 1);
    let fetched: Value = client
        .get(format!("{base}/api/equipment/{id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(fetched["id"], id.as_str());
    assert_eq!(fetched["real_watts"], 285.0);

    // Category filter matches and misses.
    let lights: Vec<Value> = client
        .get(format!("{base}/api/equipment?category=light"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(lights.len(), 1);
    let fans: Vec<Value> = client
        .get(format!("{base}/api/equipment?category=fan"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(fans.is_empty());
    let bad_filter = client
        .get(format!("{base}/api/equipment?category=lightsabre"))
        .send()
        .await
        .unwrap();
    assert_eq!(bad_filter.status(), reqwest::StatusCode::BAD_REQUEST);

    // Update (rename + change watts).
    let mut updated_input = sample_equipment("Mars Hydro TS-1000 (measured)");
    updated_input["real_watts"] = json!(150.0);
    updated_input["category"] = json!("light");
    let updated: Value = client
        .put(format!("{base}/api/equipment/{id}"))
        .json(&updated_input)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(updated["name"], "Mars Hydro TS-1000 (measured)");
    assert_eq!(updated["real_watts"], 150.0);
    assert_eq!(updated["created_at"], created["created_at"]);
    assert_ne!(updated["updated_at"], created["updated_at"]);

    // Delete: 204 then 404.
    let deleted = client
        .delete(format!("{base}/api/equipment/{id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(deleted.status(), reqwest::StatusCode::NO_CONTENT);
    let missing = client
        .get(format!("{base}/api/equipment/{id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), reqwest::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn equipment_validation_rejects_bad_input() {
    let base = spawn_api().await;
    let client = reqwest::Client::new();

    let mut no_name = sample_equipment("x");
    no_name["name"] = json!("  ");
    let response = client
        .post(format!("{base}/api/equipment"))
        .json(&no_name)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);

    let mut negative_watts = sample_equipment("Free energy");
    negative_watts["real_watts"] = json!(-1.0);
    let response = client
        .post(format!("{base}/api/equipment"))
        .json(&negative_watts)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);

    let mut bad_category = sample_equipment("Alien tech");
    bad_category["category"] = json!("warp_drive");
    let response = client
        .post(format!("{base}/api/equipment"))
        .json(&bad_category)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn strain_crud_and_clone_as_phenotype() {
    let base = spawn_api().await;
    let client = reqwest::Client::new();

    let created: Value = client
        .post(format!("{base}/api/strains"))
        .json(&sample_strain("Test Haze"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = created["id"].as_str().unwrap().to_owned();
    assert_eq!(created["flower_days_min"], 56);
    // User-entered numbers are tagged grower-reported; tolerances inferred.
    assert_eq!(created["evidence"]["flower_days_min"], "grower_reported");
    assert_eq!(created["evidence"]["heat_tolerance"], "inferred");
    // No provenance on user records.
    assert!(created["source_url"].is_null());

    // Update.
    let mut updated_input = sample_strain("Test Haze v2");
    updated_input["flower_days_max"] = json!(70);
    let updated: Value = client
        .put(format!("{base}/api/strains/{id}"))
        .json(&updated_input)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(updated["name"], "Test Haze v2");
    assert_eq!(updated["flower_days_max"], 70);

    // Clone as phenotype.
    let clone: Value = client
        .post(format!("{base}/api/strains/{id}/clone"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(clone["name"], "Test Haze v2 (my phenotype)");
    assert_eq!(clone["is_phenotype_of"], id.as_str());
    assert!(clone["source_url"].is_null());

    // Strain validation: inverted range.
    let mut inverted = sample_strain("Bad range");
    inverted["flower_days_min"] = json!(70);
    inverted["flower_days_max"] = json!(56);
    let response = client
        .post(format!("{base}/api/strains"))
        .json(&inverted)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);

    // Delete both.
    for strain_id in [&id, clone["id"].as_str().unwrap()] {
        let response = client
            .delete(format!("{base}/api/strains/{strain_id}"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::NO_CONTENT);
    }
    let empty: Vec<Value> = client
        .get(format!("{base}/api/strains"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(empty.is_empty());
}

#[tokio::test]
async fn strain_inputs_round_trip_through_core_types() {
    // Proves the JSON the API accepts deserializes into the shared
    // domain type and back without loss (frontend uses the same shape).
    let payload = sample_strain("Round Trip");
    let parsed: StrainInput = serde_json::from_value(payload.clone()).unwrap();
    assert_eq!(parsed.name, "Round Trip");
    assert_eq!(parsed.flower_days_max, Some(63));
    let equipment_payload = sample_equipment("Round Trip Light");
    let parsed: EquipmentInput = serde_json::from_value(equipment_payload).unwrap();
    assert_eq!(parsed.real_watts, Some(285.0));
}

#[tokio::test]
async fn settings_round_trip() {
    let base = spawn_api().await;
    let client = reqwest::Client::new();

    let empty: Value = client
        .get(format!("{base}/api/settings"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(empty, json!({}));

    let saved: Value = client
        .put(format!("{base}/api/settings"))
        .json(&json!({ "tds_scale": "0.5", "log_level": "info" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(saved["tds_scale"], "0.5");

    // Upsert one key, keep the other.
    let saved: Value = client
        .put(format!("{base}/api/settings"))
        .json(&json!({ "tds_scale": "0.7" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(saved["tds_scale"], "0.7");
    assert_eq!(saved["log_level"], "info");

    let non_object = client
        .put(format!("{base}/api/settings"))
        .json(&json!([1, 2]))
        .send()
        .await
        .unwrap();
    assert_eq!(non_object.status(), reqwest::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn seeds_arrive_once_and_carry_provenance() {
    let db = Db::open_in_memory().unwrap();
    leaflog_backend::seed::seed_strains_if_empty(&db)
        .await
        .unwrap();
    let base = spawn_with(db).await;

    let strains: Vec<Value> = reqwest::get(format!("{base}/api/strains"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(strains.len() >= 5, "five bundled records expected");
    for strain in &strains {
        assert!(strain["source_url"].as_str().is_some_and(|u| !u.is_empty()));
        assert!(
            strain["source_licence"]
                .as_str()
                .is_some_and(|l| !l.is_empty())
        );
    }
    let northern_lights = strains
        .iter()
        .find(|s| s["name"] == "Northern Lights")
        .expect("Northern Lights is bundled");
    assert_eq!(northern_lights["flower_days_min"], 45);
    assert_eq!(northern_lights["evidence"]["flower_days_min"], "breeder");
    let auto = strains
        .iter()
        .find(|s| s["name"] == "GG4 Auto")
        .expect("an autoflower is bundled");
    assert_eq!(auto["photoperiod"], "auto");
    assert_eq!(auto["auto_total_days_max"], 84);
}

/// A bridge pointed at an unroutable address: REST/WS fail fast and the
/// app must still work (spec: works with HA unreachable).
fn offline_bridge() -> std::sync::Arc<leaflog_backend::ha::HaBridge> {
    std::sync::Arc::new(leaflog_backend::ha::HaBridge::spawn(
        "http://127.0.0.1:1".to_owned(),
        leaflog_backend::ha::SupervisorToken::new("test-token"),
        leaflog_backend::Db::open_in_memory().unwrap(),
    ))
}
