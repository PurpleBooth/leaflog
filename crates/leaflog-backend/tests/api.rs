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
    leaflog_backend::seed::seed_strains(&db).await.unwrap();
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
                        seed_batch_id: None,
                        feed_template_id: None,
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
    leaflog_backend::seed::seed_strains(&db).await.unwrap();
    leaflog_backend::seed::seed_equipment(&db).await.unwrap();
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

#[tokio::test]
async fn sensor_items_come_from_ha_entities_only() {
    // The bridge is offline (spawn_api): creating sensor items must not
    // need HA — the picker defers, nothing else blocks (spec §4.1).
    let base = spawn_api().await;
    let client = reqwest::Client::new();

    let mut sensor = sample_equipment("Tent temperature");
    sensor["category"] = json!("sensor");
    // No entity at all: rejected.
    let response = client
        .post(format!("{base}/api/equipment"))
        .json(&sensor)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);

    // Entity without a device class: rejected.
    sensor["ha_entity_id"] = json!("sensor.grow_temperature");
    let response = client
        .post(format!("{base}/api/equipment"))
        .json(&sensor)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);

    // An unsupported device class: rejected.
    sensor["ha_device_class"] = json!("co2");
    let response = client
        .post(format!("{base}/api/equipment"))
        .json(&sensor)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);

    // Malformed or non-sensor entity ids: rejected.
    for bad in ["grow_temperature", "switch.grow_temperature", "sensor."] {
        sensor["ha_entity_id"] = json!(bad);
        sensor["ha_device_class"] = json!("temperature");
        let response = client
            .post(format!("{base}/api/equipment"))
            .json(&sensor)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST, "{bad}");
    }

    // A picked temperature entity round-trips with its identity.
    sensor["ha_entity_id"] = json!("sensor.grow_temperature");
    let created: Value = client
        .post(format!("{base}/api/equipment"))
        .json(&sensor)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(created["category"], "sensor");
    assert_eq!(created["ha_entity_id"], "sensor.grow_temperature");
    assert_eq!(created["ha_device_class"], "temperature");

    // A device class on a non-sensor item is rejected; a plain entity
    // binding on one is fine.
    let mut light = sample_equipment("Bound light");
    light["ha_device_class"] = json!("temperature");
    let response = client
        .post(format!("{base}/api/equipment"))
        .json(&light)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
    light["ha_device_class"] = Value::Null;
    light["ha_entity_id"] = json!("light.grow_led");
    let created: Value = client
        .post(format!("{base}/api/equipment"))
        .json(&light)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(created["ha_entity_id"], "light.grow_led");
}

fn sample_batch(strain_id: Option<&str>, acquisition_on: &str) -> Value {
    json!({
        "strain_id": strain_id,
        "quantity": 10,
        "acquired_from": "Royal Queen Seeds",
        "acquisition_on": acquisition_on,
        "storage_method": "cool_dark_dry",
        "notes": null,
    })
}

#[tokio::test]
async fn seedbank_lists_oldest_first_with_viability() {
    let base = spawn_api().await;
    let client = reqwest::Client::new();

    let strain: Value = client
        .post(format!("{base}/api/strains"))
        .json(&sample_strain("Royal Cookies"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let strain_id = strain["id"].as_str().unwrap().to_owned();

    // Three batches: newest and oldest share the strain, one is
    // unstrained.
    let fresh: Value = client
        .post(format!("{base}/api/seed-batches"))
        .json(&sample_batch(Some(&strain_id), "2026-06-01"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(fresh["quantity"], 10);
    assert_eq!(fresh["viability"]["window_months"], json!([12, 36]));
    assert_eq!(fresh["viability"]["past_prime"], false);
    assert!(
        fresh["viability"]["source"]
            .as_str()
            .is_some_and(|source| !source.is_empty()),
        "the preset window carries its source label"
    );

    let mut old = sample_batch(Some(&strain_id), "2024-01-01");
    old["storage_method"] = json!("room_temp_dark");
    old["quantity"] = json!(3);
    let old: Value = client
        .post(format!("{base}/api/seed-batches"))
        .json(&old)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    // Room temp, dark drawer: 6-12 month window, acquired nearly three
    // years ago — well past prime.
    assert_eq!(old["viability"]["window_months"], json!([6, 12]));
    assert_eq!(old["viability"]["past_prime"], true);

    let unstrained: Value = client
        .post(format!("{base}/api/seed-batches"))
        .json(&sample_batch(None, "2025-03-01"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // Oldest first within the strain; the unstrained batch trails.
    let list: Vec<Value> = client
        .get(format!("{base}/api/seed-batches"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(list.len(), 3);
    assert_eq!(list[0]["id"], old["id"], "oldest seeds listed first");
    assert_eq!(list[1]["id"], fresh["id"]);
    assert_eq!(list[2]["id"], unstrained["id"], "unstrained batches last");

    // Update rewrites the payload; delete clears the batch.
    let mut edited = sample_batch(Some(&strain_id), "2026-06-01");
    edited["quantity"] = json!(7);
    edited["storage_method"] = json!("refrigerated");
    let updated: Value = client
        .put(format!(
            "{}/api/seed-batches/{}",
            base,
            fresh["id"].as_str().unwrap()
        ))
        .json(&edited)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(updated["quantity"], 7);
    assert_eq!(updated["viability"]["window_months"], json!([60, 120]));

    let status = client
        .delete(format!(
            "{}/api/seed-batches/{}",
            base,
            fresh["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(status.status(), reqwest::StatusCode::NO_CONTENT);
    let missing = client
        .get(format!(
            "{}/api/seed-batches/{}",
            base,
            fresh["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), reqwest::StatusCode::NOT_FOUND);

    // Validation: a bad date and an unknown strain.
    let mut bad_date = sample_batch(None, "last summer");
    bad_date["acquisition_on"] = json!("last summer");
    let response = client
        .post(format!("{base}/api/seed-batches"))
        .json(&bad_date)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
    let response = client
        .post(format!("{base}/api/seed-batches"))
        .json(&sample_batch(Some("nope"), "2026-01-01"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn linked_plant_starts_decrement_the_batch() {
    let base = spawn_api().await;
    let client = reqwest::Client::new();

    let strain: Value = client
        .post(format!("{base}/api/strains"))
        .json(&sample_strain("Euforia"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let strain_id = strain["id"].as_str().unwrap().to_owned();
    let mut batch = sample_batch(Some(&strain_id), "2026-09-01");
    batch["quantity"] = json!(5);
    let batch: Value = client
        .post(format!("{base}/api/seed-batches"))
        .json(&batch)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let batch_id = batch["id"].as_str().unwrap().to_owned();

    // A grow names its batch; the link round-trips.
    let grow: Value = client
        .post(format!("{base}/api/grows"))
        .json(&json!({
            "name": "Euforia run",
            "strain_id": strain_id,
            "grow_space_id": null,
            "seed_batch_id": batch_id,
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
    assert_eq!(grow["seed_batch_id"], batch_id.as_str());

    // Two plants started: 5 -> 3, floored at zero, never raised.
    for name in ["Plant A", "Plant B"] {
        let status = client
            .post(format!("{base}/api/grows/{grow_id}/plants"))
            .json(&json!({ "name": name }))
            .send()
            .await
            .unwrap();
        assert_eq!(status.status(), reqwest::StatusCode::CREATED);
    }
    let after: Value = client
        .get(format!("{base}/api/seed-batches/{batch_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(after["quantity"], 3, "each started plant decrements once");

    // Manual edits always win: resetting to 10 makes the next plant
    // start land on 9.
    let mut edited = sample_batch(Some(&strain_id), "2026-09-01");
    edited["quantity"] = json!(10);
    client
        .put(format!("{base}/api/seed-batches/{batch_id}"))
        .json(&edited)
        .send()
        .await
        .unwrap();
    client
        .post(format!("{base}/api/grows/{grow_id}/plants"))
        .json(&json!({ "name": "Plant C" }))
        .send()
        .await
        .unwrap();
    let after: Value = client
        .get(format!("{base}/api/seed-batches/{batch_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(after["quantity"], 9);

    // An unknown batch on grow creation is rejected...
    let response = client
        .post(format!("{base}/api/grows"))
        .json(&json!({
            "name": "Nowhere run",
            "strain_id": null,
            "grow_space_id": null,
            "seed_batch_id": "nope",
            "substrate": "soil",
            "started_on": "2026-10-01",
            "status": "active",
            "notes": null,
        }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);

    // ...and deleting the batch keeps the grow running, unlinked.
    client
        .delete(format!("{base}/api/seed-batches/{batch_id}"))
        .send()
        .await
        .unwrap();
    let survived: Value = client
        .get(format!("{base}/api/grows/{grow_id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(survived["name"], "Euforia run");
    assert_eq!(survived["seed_batch_id"], Value::Null);
}

#[tokio::test]
async fn onboarding_wizard_appears_runs_and_stays_dismissable() {
    // Fresh install: no spaces, no grows, no dismissal — the wizard
    // shows (spec §4.12). The catalogue carries the bundled presets a
    // fresh boot seeds.
    let db = Db::open_in_memory().unwrap();
    leaflog_backend::seed::seed_strains(&db).await.unwrap();
    leaflog_backend::seed::seed_equipment(&db).await.unwrap();
    let base = spawn_with(db).await;
    let client = reqwest::Client::new();
    let fresh: Value = client
        .get(format!("{base}/api/onboarding"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(fresh["show_wizard"], true);

    // Dismissal hides it (the settings flag is the dismissal), and the
    // empty-state restart clears the flag again.
    client
        .put(format!("{base}/api/settings"))
        .json(&json!({ "onboarding_dismissed": true }))
        .send()
        .await
        .unwrap();
    let dismissed: Value = client
        .get(format!("{base}/api/onboarding"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(dismissed["show_wizard"], false);
    assert_eq!(dismissed["dismissed"], true);
    client
        .put(format!("{base}/api/settings"))
        .json(&json!({ "onboarding_dismissed": false }))
        .send()
        .await
        .unwrap();
    let restarted: Value = client
        .get(format!("{base}/api/onboarding"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(restarted["show_wizard"], true, "the restart card works");

    // The wizard's own call sequence, exactly as the frontend drives
    // it: equipment → space → strain → plants → grow.
    let equipment: Vec<Value> = client
        .get(format!("{base}/api/equipment"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let tent = equipment
        .iter()
        .find(|item| item["category"] == "tent_room")
        .expect("a bundled tent is picked in step 1")["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let space: Value = client
        .post(format!("{base}/api/spaces"))
        .json(&sample_space("The tent"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let space_id = space["id"].as_str().unwrap().to_owned();
    client
        .post(format!("{base}/api/spaces/{space_id}/equipment/{tent}"))
        .send()
        .await
        .unwrap();

    let strains: Vec<Value> = client
        .get(format!("{base}/api/strains"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    // The wizard offers the bundled records; step 3 may also be skipped
    // entirely (a grow without a strain is valid).
    let strain_id = strains[0]["id"].as_str().unwrap().to_owned();

    let grow: Value = client
        .post(format!("{base}/api/grows"))
        .json(&json!({
            "name": "First run",
            "strain_id": strain_id,
            "grow_space_id": space_id,
            "seed_batch_id": null,
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
    let grow_id = grow["id"].as_str().unwrap().to_owned();
    for name in ["Plant 1", "Plant 2"] {
        client
            .post(format!("{base}/api/grows/{grow_id}/plants"))
            .json(&json!({ "name": name }))
            .send()
            .await
            .unwrap();
    }

    // A grow exists: the wizard never shows again, dismissal or not.
    let done: Value = client
        .get(format!("{base}/api/onboarding"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(done["show_wizard"], false);
    assert_eq!(done["has_grows"], true);
    assert_eq!(done["has_spaces"], true);
}

#[tokio::test]
async fn harvest_weights_round_trip_summarise_and_write_back() {
    let base = spawn_api().await;
    let client = reqwest::Client::new();

    // A grow with a user strain and two plants, plus a second grow on a
    // bundled strain for the write-back rejection.
    let strain: Value = client
        .post(format!("{base}/api/strains"))
        .json(&sample_strain("My own cross"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let grow: Value = client
        .post(format!("{base}/api/grows"))
        .json(&json!({
            "name": "Harvest run",
            "strain_id": strain["id"],
            "grow_space_id": null,
            "seed_batch_id": null,
            "feed_template_id": null,
            "substrate": "soil",
            "started_on": "2026-08-01",
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
    let mut plants = Vec::new();
    for name in ["NL #1", "NL #2"] {
        let plant: Value = client
            .post(format!("{base}/api/grows/{grow_id}/plants"))
            .json(&json!({ "name": name }))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        plants.push(plant["id"].as_str().unwrap().to_owned());
    }

    // Weights are grams end to end (spec §4.14) and round-trip per
    // plant; unset weights stay null, never zero.
    let updated: Value = client
        .put(format!("{base}/api/plants/{}", plants[0]))
        .json(&json!({
            "name": "NL #1",
            "wet_g": 120.5,
            "dry_g": 30.0,
            "trimmed_g": 28.0,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(updated["wet_g"], 120.5);
    assert_eq!(updated["trimmed_g"], 28.0);
    client
        .put(format!("{base}/api/plants/{}", plants[1]))
        .json(&json!({
            "name": "NL #2",
            "wet_g": 99.5,
            "dry_g": null,
            "trimmed_g": 25.0,
        }))
        .send()
        .await
        .unwrap();

    let summary: Value = client
        .get(format!("{base}/api/grows/{grow_id}/yield"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(summary["total_wet_g"], 220.0);
    assert_eq!(summary["total_dry_g"], 30.0, "only the weighed plant sums");
    assert_eq!(summary["total_trimmed_g"], 53.0);
    let ratio = summary["wet_to_dry_ratio"].as_f64().unwrap();
    assert!(
        (ratio - 220.0 / 30.0).abs() < 1e-9,
        "wet→dry ratio: {ratio}"
    );
    assert_eq!(summary["plants"].as_array().unwrap().len(), 2);

    // Validation: negative weights and blank names are rejected; an
    // unknown plant is a 404.
    let response = client
        .put(format!("{base}/api/plants/{}", plants[0]))
        .json(&json!({ "name": "x", "wet_g": -1.0, "dry_g": null, "trimmed_g": null }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
    let response = client
        .put(format!("{base}/api/plants/{}", plants[0]))
        .json(&json!({ "name": "  ", "wet_g": null, "dry_g": null, "trimmed_g": null }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
    let response = client
        .put(format!("{base}/api/plants/nope"))
        .json(&json!({ "name": "x", "wet_g": null, "dry_g": null, "trimmed_g": null }))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);

    // Write-back reaches the user strain exactly once per grow.
    let written: Value = client
        .post(format!("{base}/api/grows/{grow_id}/writeback"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let notes = written["yield_notes"].as_str().unwrap();
    assert!(notes.contains("achieved"), "notes: {notes}");
    assert!(notes.contains("30 g dry"), "notes: {notes}");
    assert!(notes.contains(&format!("[grow:{grow_id}]")));
    client
        .post(format!("{base}/api/grows/{grow_id}/writeback"))
        .send()
        .await
        .unwrap();
    let strain_after: Value = client
        .get(format!(
            "{}/api/strains/{}",
            base,
            strain["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let notes = strain_after["yield_notes"].as_str().unwrap();
    assert_eq!(
        notes.matches("achieved").count(),
        1,
        "re-writing replaces the line, it does not pile up: {notes}"
    );

    // A grow on a bundled strain never writes back (§4.4), and a
    // strainless grow says so.
    let db_seeded = Db::open_in_memory().unwrap();
    leaflog_backend::seed::seed_strains(&db_seeded)
        .await
        .unwrap();
    let seeded_base = spawn_with(db_seeded).await;
    let bundled: Vec<Value> = reqwest::get(format!("{seeded_base}/api/strains"))
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let bundled_id = bundled
        .iter()
        .find(|s| s["is_builtin"] == json!(true))
        .unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let bundled_grow: Value = reqwest::Client::new()
        .post(format!("{seeded_base}/api/grows"))
        .json(&json!({
            "name": "Preset run",
            "strain_id": bundled_id,
            "grow_space_id": null,
            "seed_batch_id": null,
            "feed_template_id": null,
            "substrate": "soil",
            "started_on": "2026-08-01",
            "status": "active",
            "notes": null,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let response = reqwest::Client::new()
        .post(format!(
            "{}/api/grows/{}/writeback",
            seeded_base,
            bundled_grow["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::CONFLICT);

    let response = client
        .post(format!("{base}/api/grows/nope/writeback"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::NOT_FOUND);
}
