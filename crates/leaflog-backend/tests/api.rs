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

#[tokio::test]
async fn sensors_replace_is_atomic_when_an_insert_fails() {
    use std::collections::BTreeMap;

    use leaflog_backend::db::StoreError;
    use leaflog_backend::store::grows;
    use leaflog_backend::store::sensors::{BindingsInput, list, replace};
    use leaflog_core::grows::{GrowInput, GrowStatus};

    let db = Db::open_in_memory().unwrap();

    // A grow with two live bindings.
    let (grow_id, seeded) = db
        .with(
            |conn| -> Result<(String, BTreeMap<String, String>), StoreError> {
                let grow = grows::create(
                    conn,
                    &GrowInput {
                        name: "Closet run".to_owned(),
                        strain_id: None,
                        grow_space_id: None,
                        substrate: "soil".to_owned(),
                        started_on: "2026-10-01".to_owned(),
                        status: GrowStatus::Active,
                        notes: None,
                    },
                )?;
                let input = BindingsInput {
                    bindings: BTreeMap::from([
                        ("temperature".to_owned(), "sensor.grow_temp".to_owned()),
                        ("humidity".to_owned(), "sensor.grow_hum".to_owned()),
                    ]),
                };
                replace(conn, &grow.id, &input)?;
                Ok((grow.id, input.bindings))
            },
        )
        .await
        .unwrap();

    // Make every INSERT into sensor_binding abort mid-replacement, then
    // remove the trigger again so later reads are unaffected.
    let trigger_grow_id = grow_id.clone();
    let replace_failed = db
        .with(move |conn| -> Result<bool, StoreError> {
            conn.execute_batch(
                "CREATE TRIGGER abort_binding BEFORE INSERT ON sensor_binding
                 BEGIN SELECT RAISE(ABORT, 'boom'); END",
            )?;
            let result = replace(
                conn,
                &trigger_grow_id,
                &BindingsInput {
                    bindings: BTreeMap::from([("co2".to_owned(), "sensor.grow_co2".to_owned())]),
                },
            );
            conn.execute_batch("DROP TRIGGER abort_binding")?;
            Ok(result.is_err())
        })
        .await
        .unwrap();
    assert!(replace_failed, "replace must fail while INSERTs abort");

    // The failed replacement must not have destroyed the previous
    // bindings: the DELETE inside `replace` has to roll back with the
    // aborted INSERT.
    let survivors = db.with(move |conn| list(conn, &grow_id)).await.unwrap();
    assert_eq!(survivors, seeded);
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

// --- Grow spaces (spec §4.1) -------------------------------------------

fn sample_space(name: &str) -> Value {
    json!({
        "name": name,
        "width_cm": 60.0,
        "depth_cm": 60.0,
        "height_cm": 180.0,
        "notes": null,
    })
}

/// Creates an equipment entry via the API and returns its id.
async fn create_equipment_at(
    client: &reqwest::Client,
    base: &str,
    name: &str,
    real_watts: Option<f64>,
) -> String {
    let mut body = sample_equipment(name);
    body["real_watts"] = json!(real_watts);
    let created: Value = client
        .post(format!("{base}/api/equipment"))
        .json(&body)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    created["id"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn spaces_crud_and_equipment_membership_round_trip() {
    let base = spawn_api().await;
    let client = reqwest::Client::new();

    // Create.
    let created: Value = client
        .post(format!("{base}/api/spaces"))
        .json(&sample_space("60×60 tent"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = created["id"].as_str().unwrap().to_owned();
    assert_eq!(created["name"], "60×60 tent");
    assert_eq!(created["height_cm"], 180.0);

    // Membership: add two entries; re-adding is an idempotent no-op.
    let light = create_equipment_at(&client, &base, "SE420", Some(420.0)).await;
    let fan = create_equipment_at(&client, &base, "Clip fan", None).await;
    for _ in 0..2 {
        let status = client
            .post(format!("{base}/api/spaces/{id}/equipment/{light}"))
            .send()
            .await
            .unwrap();
        assert_eq!(status.status(), reqwest::StatusCode::NO_CONTENT);
    }
    let status = client
        .post(format!("{base}/api/spaces/{id}/equipment/{fan}"))
        .send()
        .await
        .unwrap();
    assert_eq!(status.status(), reqwest::StatusCode::NO_CONTENT);

    // Detail and list resolve the equipment, ordered by name.
    let detail: Value = client
        .get(format!("{base}/api/spaces/{id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(detail["space"]["id"], id.as_str());
    assert_eq!(detail["equipment"].as_array().unwrap().len(), 2);
    assert_eq!(detail["equipment"][0]["name"], "Clip fan");
    assert_eq!(detail["grows"].as_array().unwrap().len(), 0);
    let list: Vec<Value> = client
        .get(format!("{base}/api/spaces"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0]["equipment"].as_array().unwrap().len(), 2);

    // Unknown space or equipment on membership is a 404.
    let status = client
        .post(format!("{base}/api/spaces/{id}/equipment/nope"))
        .send()
        .await
        .unwrap();
    assert_eq!(status.status(), reqwest::StatusCode::NOT_FOUND);
    let status = client
        .post(format!("{base}/api/spaces/nope/equipment/{light}"))
        .send()
        .await
        .unwrap();
    assert_eq!(status.status(), reqwest::StatusCode::NOT_FOUND);

    // Removing is idempotent and shrinks the set.
    for _ in 0..2 {
        let status = client
            .delete(format!("{base}/api/spaces/{id}/equipment/{fan}"))
            .send()
            .await
            .unwrap();
        assert_eq!(status.status(), reqwest::StatusCode::NO_CONTENT);
    }
    let detail: Value = client
        .get(format!("{base}/api/spaces/{id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(detail["equipment"].as_array().unwrap().len(), 1);
    assert_eq!(detail["equipment"][0]["name"], "SE420");

    // Deleting equipment removes it from every space (cascade).
    let status = client
        .delete(format!("{base}/api/equipment/{light}"))
        .send()
        .await
        .unwrap();
    assert_eq!(status.status(), reqwest::StatusCode::NO_CONTENT);
    let detail: Value = client
        .get(format!("{base}/api/spaces/{id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(detail["equipment"].as_array().unwrap().is_empty());

    // Update (rename + resize), preserving id and creation time.
    let mut renamed = sample_space("Big tent");
    renamed["height_cm"] = json!(200.0);
    let updated: Value = client
        .put(format!("{base}/api/spaces/{id}"))
        .json(&renamed)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(updated["name"], "Big tent");
    assert_eq!(updated["height_cm"], 200.0);
    assert_eq!(updated["created_at"], created["created_at"]);

    // Validation: blank name and non-positive dimensions are 400s.
    let mut no_name = sample_space("x");
    no_name["name"] = json!("  ");
    let response = client
        .post(format!("{base}/api/spaces"))
        .json(&no_name)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
    let mut flat = sample_space("Flat tent");
    flat["height_cm"] = json!(-1.0);
    let response = client
        .post(format!("{base}/api/spaces"))
        .json(&flat)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);

    // Delete: 204 then 404.
    let status = client
        .delete(format!("{base}/api/spaces/{id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(status.status(), reqwest::StatusCode::NO_CONTENT);
    let status = client
        .get(format!("{base}/api/spaces/{id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(status.status(), reqwest::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn grows_belong_to_spaces() {
    let base = spawn_api().await;
    let client = reqwest::Client::new();

    let space: Value = client
        .post(format!("{base}/api/spaces"))
        .json(&sample_space("60×60 tent"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let space_id = space["id"].as_str().unwrap().to_owned();

    // Create a grow inside the space.
    let grow: Value = client
        .post(format!("{base}/api/grows"))
        .json(&json!({
            "name": "RC #1",
            "strain_id": null,
            "grow_space_id": space_id,
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
    let grow_id = grow["id"].as_str().unwrap().to_owned();
    assert_eq!(grow["grow_space_id"], space_id.as_str());

    // The space lists the grow.
    let detail: Value = client
        .get(format!("{base}/api/spaces/{space_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let grows = detail["grows"].as_array().unwrap();
    assert_eq!(grows.len(), 1);
    assert_eq!(grows[0]["id"], grow_id.as_str());

    // An unknown space id is rejected on create and update.
    let response = client
        .post(format!("{base}/api/grows"))
        .json(&json!({
            "name": "Nowhere run",
            "strain_id": null,
            "grow_space_id": "nope",
            "substrate": "soil",
            "started_on": "2026-10-01",
            "status": "active",
            "notes": null,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);

    // Clearing the space via PUT round-trips.
    let cleared: Value = client
        .put(format!("{base}/api/grows/{grow_id}"))
        .json(&json!({
            "name": "RC #1",
            "strain_id": null,
            "grow_space_id": null,
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
    assert_eq!(cleared["grow_space_id"], Value::Null);

    // Re-assign, then delete the space: the grow survives, unspaced.
    let response = client
        .put(format!("{base}/api/grows/{grow_id}"))
        .json(&json!({
            "name": "RC #1",
            "strain_id": null,
            "grow_space_id": space_id,
            "substrate": "soil",
            "started_on": "2026-10-01",
            "status": "active",
            "notes": null,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
    let status = client
        .delete(format!("{base}/api/spaces/{space_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(status.status(), reqwest::StatusCode::NO_CONTENT);
    let survived: Value = client
        .get(format!("{base}/api/grows/{grow_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(survived["name"], "RC #1");
    assert_eq!(survived["grow_space_id"], Value::Null);
}

#[tokio::test]
async fn lamp_advisor_uses_the_grow_space_light() {
    let base = spawn_api().await;
    let client = reqwest::Client::new();

    // A grow started today has no flip/harvest, so the advice reflects
    // the seedling range of the advising wattage class: 28–30 in for
    // the historical 330 W default, 32–36 in for a 420 W fixture.
    let today = time::OffsetDateTime::now_utc().date();
    let started_on = format!(
        "{:04}-{:02}-{:02}",
        today.year(),
        u8::from(today.month()),
        today.day()
    );

    let plain: Value = client
        .post(format!("{base}/api/grows"))
        .json(&json!({
            "name": "Default run",
            "strain_id": null,
            "grow_space_id": null,
            "substrate": "soil",
            "started_on": started_on,
            "status": "active",
            "notes": null,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let advice: Value = client
        .get(format!(
            "{base}/api/grows/{}/lamp",
            plain["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(advice["height"]["min_in"], 28.0, "no space → 330 W class");

    let light = create_equipment_at(&client, &base, "SE420", Some(420.0)).await;
    let space: Value = client
        .post(format!("{base}/api/spaces"))
        .json(&sample_space("60×60 tent"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let space_id = space["id"].as_str().unwrap().to_owned();
    let status = client
        .post(format!("{base}/api/spaces/{space_id}/equipment/{light}"))
        .send()
        .await
        .unwrap();
    assert_eq!(status.status(), reqwest::StatusCode::NO_CONTENT);

    let lit: Value = client
        .post(format!("{base}/api/grows"))
        .json(&json!({
            "name": "Lit run",
            "strain_id": null,
            "grow_space_id": space_id,
            "substrate": "soil",
            "started_on": started_on,
            "status": "active",
            "notes": null,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let advice: Value = client
        .get(format!(
            "{base}/api/grows/{}/lamp",
            lit["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        advice["height"]["min_in"], 32.0,
        "space light → 420 W class"
    );
}

#[tokio::test]
async fn bundled_records_are_read_only_and_clone_to_user_entries() {
    let db = Db::open_in_memory().unwrap();
    leaflog_backend::seed::seed_strains_if_empty(&db)
        .await
        .unwrap();
    leaflog_backend::seed::seed_equipment_if_empty(&db)
        .await
        .unwrap();
    let base = spawn_with(db).await;
    let client = reqwest::Client::new();

    // -- Bundled equipment: edit and delete are rejected, clone works.
    let equipment: Vec<Value> = client
        .get(format!("{base}/api/equipment"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let bundled = equipment
        .iter()
        .find(|item| item["is_builtin"] == json!(true))
        .expect("bundled equipment is seeded")
        .clone();
    let id = bundled["id"].as_str().unwrap().to_owned();

    let response = client
        .put(format!("{base}/api/equipment/{id}"))
        .json(&sample_equipment("Hostile takeover"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::CONFLICT);
    let response = client
        .delete(format!("{base}/api/equipment/{id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::CONFLICT);
    let survivor: Value = client
        .get(format!("{base}/api/equipment/{id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(survivor["name"], bundled["name"], "the preset is intact");

    let clone: Value = client
        .post(format!("{base}/api/equipment/{id}/clone"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        clone["name"],
        format!("{} (copy)", bundled["name"].as_str().unwrap())
    );
    assert_eq!(clone["is_builtin"], json!(false), "clones are user entries");
    assert_eq!(clone["dimmable"], bundled["dimmable"]);
    assert_eq!(clone["category"], bundled["category"]);
    assert_ne!(clone["id"], id.as_str());

    // Cloning a user entry is the "two identical lights" flow (§4.1).
    let user = create_equipment_at(&client, &base, "Second SE1500", Some(150.0)).await;
    let second: Value = client
        .post(format!("{base}/api/equipment/{user}/clone"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(second["real_watts"], 150.0);
    assert_ne!(second["id"], user.as_str());

    // -- Bundled strains: same contract.
    let strains: Vec<Value> = client
        .get(format!("{base}/api/strains"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let bundled_strain = strains
        .iter()
        .find(|strain| strain["is_builtin"] == json!(true))
        .expect("bundled strain is seeded")
        .clone();
    let strain_id = bundled_strain["id"].as_str().unwrap().to_owned();

    let response = client
        .put(format!("{base}/api/strains/{strain_id}"))
        .json(&sample_strain("Rewritten history"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::CONFLICT);
    let response = client
        .delete(format!("{base}/api/strains/{strain_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::CONFLICT);

    let phenotype: Value = client
        .post(format!("{base}/api/strains/{strain_id}/clone"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(phenotype["is_builtin"], json!(false));
    assert_eq!(phenotype["is_phenotype_of"], strain_id.as_str());

    // User strains stay fully mutable.
    let own: Value = client
        .post(format!("{base}/api/strains"))
        .json(&sample_strain("My own cross"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(own["is_builtin"], json!(false));
    let response = client
        .put(format!(
            "{}/api/strains/{}",
            base,
            own["id"].as_str().unwrap()
        ))
        .json(&sample_strain("My own cross v2"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);
}
