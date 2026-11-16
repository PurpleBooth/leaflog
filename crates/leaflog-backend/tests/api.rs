//! Integration tests for the JSON API: settings, onboarding, the
//! removed catalogue routes, grows, harvest weights and history.

use std::net::SocketAddr;

use axum::ServiceExt as _;
use std::path::Path;

use leaflog_backend::{AppState, Assets, Db, Mode};
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

/// The strain catalogue routes are gone (D075 feature strip, step 05):
/// list, create, read, update, delete, clone-as-phenotype and the
/// grow's yield write-back all answer 404, so a stale bookmark or an
/// old client gets a clean "not found", never a half-working endpoint.
#[tokio::test]
async fn strain_routes_are_gone() {
    let base = spawn_api().await;
    let client = reqwest::Client::new();
    let routes = [
        (reqwest::Method::GET, "/api/strains"),
        (reqwest::Method::POST, "/api/strains"),
        (reqwest::Method::GET, "/api/strains/nope"),
        (reqwest::Method::PUT, "/api/strains/nope"),
        (reqwest::Method::DELETE, "/api/strains/nope"),
        (reqwest::Method::POST, "/api/strains/nope/clone"),
        (reqwest::Method::POST, "/api/grows/nope/writeback"),
    ];
    for (method, path) in routes {
        let response = client
            .request(method, format!("{base}{path}"))
            .json(&json!({}))
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            reqwest::StatusCode::NOT_FOUND,
            "{path} must be gone"
        );
    }
}

/// The spaces and equipment routes are gone too (feature-strip D078,
/// ticket 04): list, create, read, update, delete, membership and
/// clone all answer 404.
#[tokio::test]
async fn space_and_equipment_routes_are_gone() {
    let base = spawn_api().await;
    let client = reqwest::Client::new();
    let space: Value = json!({ "name": "tent" });
    let equipment: Value = json!({ "name": "lamp", "category": "light" });
    let body_by_path = [
        ("/api/spaces", &space),
        ("/api/spaces/nope", &space),
        ("/api/spaces/nope/equipment/nope", &Value::Null),
        ("/api/equipment", &equipment),
        ("/api/equipment/nope", &equipment),
        ("/api/equipment/nope/clone", &Value::Null),
    ];
    let routes = [
        (reqwest::Method::GET, "/api/spaces"),
        (reqwest::Method::POST, "/api/spaces"),
        (reqwest::Method::GET, "/api/spaces/nope"),
        (reqwest::Method::PUT, "/api/spaces/nope"),
        (reqwest::Method::DELETE, "/api/spaces/nope"),
        (reqwest::Method::POST, "/api/spaces/nope/equipment/nope"),
        (reqwest::Method::DELETE, "/api/spaces/nope/equipment/nope"),
        (reqwest::Method::GET, "/api/equipment"),
        (reqwest::Method::POST, "/api/equipment"),
        (reqwest::Method::GET, "/api/equipment/nope"),
        (reqwest::Method::PUT, "/api/equipment/nope"),
        (reqwest::Method::DELETE, "/api/equipment/nope"),
        (reqwest::Method::POST, "/api/equipment/nope/clone"),
    ];
    for (method, path) in routes {
        let body = body_by_path
            .iter()
            .find(|(candidate, _)| candidate == &path)
            .map_or(json!({}), |(_, body)| (*body).clone());
        let response = client
            .request(method, format!("{base}{path}"))
            .json(&body)
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            reqwest::StatusCode::NOT_FOUND,
            "{path} must be gone"
        );
    }
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
async fn sensors_replace_is_atomic_when_an_insert_fails() {
    use std::collections::BTreeMap;

    use leaflog_backend::db::StoreError;
    use leaflog_backend::store::grows;
    use leaflog_backend::store::sensors::{BindingsInput, list, replace};
    use leaflog_core::grows::{GrowInput, GrowStatus, PhotoperiodType, Tolerance};

    let db = Db::open_in_memory().unwrap();

    // A grow with two live bindings.
    let (grow_id, seeded) = db
        .with(
            |conn| -> Result<(String, BTreeMap<String, String>), StoreError> {
                let grow = grows::create(
                    conn,
                    &GrowInput {
                        name: "Closet run".to_owned(),
                        feed_template_id: None,
                        planned_veg_weeks: None,
                        strain_label: String::new(),
                        photoperiod: PhotoperiodType::default(),
                        heat_tolerance: Tolerance::default(),
                        cold_tolerance: Tolerance::default(),
                        light_tolerance: Tolerance::default(),
                        real_watts: None,
                        dimmable: true,
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

async fn lamp_min_for_grow(client: &reqwest::Client, base: &str, body: Value) -> f64 {
    let started_on = started_today();
    let mut body = body;
    body["started_on"] = json!(started_on);
    let grow: Value = client
        .post(format!("{base}/api/grows"))
        .json(&body)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let advice: Value = client
        .get(format!(
            "{base}/api/grows/{}/lamp",
            grow["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    advice["height"]["min_in"].as_f64().unwrap()
}

/// Today's date, `YYYY-MM-DD`, for grows whose forecast starts now.
fn started_today() -> String {
    let today = time::OffsetDateTime::now_utc().date();
    format!(
        "{:04}-{:02}-{:02}",
        today.year(),
        u8::from(today.month()),
        today.day()
    )
}

#[tokio::test]
async fn lamp_advisor_reads_the_grow_watts_with_the_default_fallback() {
    let base = spawn_api().await;
    let client = reqwest::Client::new();

    // Seedling ranges: 28–30 in for the D021 330 W default fixture,
    // 32–36 in for a 420 W lamp — read from the grow's own watts.
    let plain = json!({
        "name": "Default run",
        "substrate": "soil",
        "status": "active",
        "notes": null,
    });
    assert_eq!(
        lamp_min_for_grow(&client, &base, plain).await,
        28.0,
        "no watts → 330 W class"
    );

    let mut lit = json!({
        "name": "Lit run",
        "substrate": "soil",
        "status": "active",
        "notes": null,
    });
    lit["real_watts"] = json!(420.0);
    assert_eq!(
        lamp_min_for_grow(&client, &base, lit).await,
        32.0,
        "grow watts → 420 W class"
    );

    let mut fractional = json!({
        "name": "Rounded run",
        "substrate": "soil",
        "status": "active",
        "notes": null,
    });
    fractional["real_watts"] = json!(419.6);
    assert_eq!(
        lamp_min_for_grow(&client, &base, fractional).await,
        32.0,
        "fractional grow watts round to the nearest whole watt"
    );
}

#[tokio::test]
async fn env_thresholds_shift_by_grow_tolerances() {
    let base = spawn_api().await;
    let client = reqwest::Client::new();

    let thresholds_for = |name: String, heat: Value, cold: Value| {
        let client = &client;
        let base = &base;
        async move {
            let mut body = json!({
                "name": name,
                "substrate": "soil",
                "started_on": started_today(),
                "status": "active",
                "notes": null,
            });
            // Null means leave the field out: the serde default (Average)
            // applies to absent fields only.
            if !heat.is_null() {
                body["heat_tolerance"] = heat;
            }
            if !cold.is_null() {
                body["cold_tolerance"] = cold;
            }
            let grow: Value = client
                .post(format!("{base}/api/grows"))
                .json(&body)
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            let env: Value = client
                .get(format!(
                    "{base}/api/grows/{}/env",
                    grow["id"].as_str().unwrap()
                ))
                .send()
                .await
                .unwrap()
                .json()
                .await
                .unwrap();
            env["thresholds"].clone()
        }
    };

    let default = thresholds_for("Plain run".to_owned(), Value::Null, Value::Null).await;
    assert!(
        (default["ceiling_c"].as_f64().unwrap() - 26.7).abs() < 0.1,
        "unset tolerances keep the unshifted ceiling"
    );
    assert!(
        (default["floor_c"].as_f64().unwrap() - 15.6).abs() < 0.1,
        "unset tolerances keep the unshifted floor"
    );

    let shifted = thresholds_for(
        "Tuned run".to_owned(),
        json!("sensitive"),
        json!("tolerant"),
    )
    .await;
    assert!(
        shifted["ceiling_c"].as_f64().unwrap() < default["ceiling_c"].as_f64().unwrap() - 1.0,
        "heat-sensitive grow lowers the ceiling"
    );
    assert!(
        shifted["floor_c"].as_f64().unwrap() < default["floor_c"].as_f64().unwrap() - 1.0,
        "cold-tolerant grow lowers the floor"
    );
}

#[tokio::test]
async fn autoflower_grow_forecasts_and_fits_without_a_strain() {
    // The grow attaches no strain record — there is no catalogue left
    // (D075): the forecast and feed-fit must run on the grow's
    // photoperiod alone.
    let db = Db::open_in_memory().unwrap();
    leaflog_backend::seed::seed_feed_presets(&db).await.unwrap();
    let base = spawn_with(db).await;
    let client = reqwest::Client::new();

    let grow: Value = client
        .post(format!("{base}/api/grows"))
        .json(&json!({
            "name": "Auto run",
            "photoperiod": "auto",
            "substrate": "soil",
            "started_on": started_today(),
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

    // The forecast projects the full auto chain on the default total
    // window: age flip at day 28, harvest 63–77 days from seed.
    let forecast: Value = client
        .get(format!("{base}/api/grows/{grow_id}/forecast"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let flower = forecast["windows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|window| window["phase"] == "flower")
        .expect("auto grow projects a flower window");
    assert_eq!(flower["actual_start"], true, "autos flip by age");
    assert_eq!(flower["earliest_end"], 63);
    assert_eq!(flower["likely_end"], 70);
    assert_eq!(flower["latest_end"], 77);

    // Feed-fit consumes the same windows: the auto grow fits on the
    // toggle alone, no record consulted.
    let templates: Vec<Value> = client
        .get(format!("{base}/api/feeds/templates"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let fit: Value = client
        .post(format!("{base}/api/grows/{grow_id}/feed/fit"))
        .json(&json!({ "template_id": templates[0]["id"] }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        !fit["after"]["days"].as_array().unwrap().is_empty(),
        "the fitted plan has days"
    );
    assert!(fit["after"]["actual_total_days"].as_u64().unwrap() > 0);

    // Contrast: an unset photoperiod grow without a plan is
    // open-ended — no flower window, and the fit refuses.
    let plain: Value = client
        .post(format!("{base}/api/grows"))
        .json(&json!({
            "name": "Photo run",
            "substrate": "soil",
            "started_on": started_today(),
            "status": "active",
            "notes": null,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let photo_id = plain["id"].as_str().unwrap();
    let forecast: Value = client
        .get(format!("{base}/api/grows/{photo_id}/forecast"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let windows = forecast["windows"].as_array().unwrap();
    assert_eq!(windows.len(), 3, "open-ended photoperiod veg");
    assert_eq!(windows.last().unwrap()["phase"], "vegetative");
    let status = client
        .post(format!("{base}/api/grows/{photo_id}/feed/fit"))
        .json(&json!({ "template_id": templates[0]["id"] }))
        .send()
        .await
        .unwrap()
        .status();
    assert_eq!(status, reqwest::StatusCode::BAD_REQUEST);
}

/// Logs one note entry carrying `tags` and returns the lamp advice's
/// dim recommendation.
async fn lamp_dim_pct_with_tags(
    client: &reqwest::Client,
    base: &str,
    name: &str,
    started_on: &str,
    tags: &[&str],
) -> Option<u64> {
    let grow: Value = client
        .post(format!("{base}/api/grows"))
        .json(&json!({
            "name": name,
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
    let grow_id = grow["id"].as_str().unwrap();
    client
        .post(format!("{base}/api/grows/{grow_id}/actions"))
        .json(&json!({
            "plant_ids": [],
            "occurred_at": format!("{started_on}T12:00:00Z"),
            "action_type": "note",
            "details": {},
            "notes": null,
            "appearance_tags": tags,
        }))
        .send()
        .await
        .unwrap();
    let advice: Value = client
        .get(format!("{base}/api/grows/{grow_id}/lamp"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    advice["dim_pct"].as_u64()
}

#[tokio::test]
async fn lamp_advisor_reacts_to_merged_and_old_stress_tags() {
    let base = spawn_api().await;
    let client = reqwest::Client::new();
    let today = time::OffsetDateTime::now_utc().date();
    let started_on = format!(
        "{:04}-{:02}-{:02}",
        today.year(),
        u8::from(today.month()),
        today.day()
    );
    let dim = |name: &'static str, tags: &'static [&'static str]| {
        lamp_dim_pct_with_tags(&client, &base, name, &started_on, tags)
    };

    // The default fixture (330 W, dimmable) has no dim advice without
    // a light-stress tag; stretch alone changes height, not dimming.
    assert_eq!(dim("No tags", &[]).await, None, "no tag → no dim advice");
    assert_eq!(
        dim("Stretch only", &["stretchy"]).await,
        None,
        "stretch raises, does not dim"
    );

    // The merged vocabulary must advise exactly like the old spellings
    // that stored entries may still carry.
    assert_eq!(
        dim("Canoeing grow", &["canoeing"]).await,
        Some(70),
        "merged canoeing tag dims to 70%"
    );
    assert_eq!(
        dim("Taco grow", &["taco"]).await,
        Some(70),
        "old taco tag still dims to 70%"
    );
    assert_eq!(
        dim("Canoe grow", &["canoe"]).await,
        Some(70),
        "old canoe tag still dims to 70%"
    );
    assert_eq!(
        dim("Bleaching grow", &["bleaching"]).await,
        Some(70),
        "bleaching keeps dimming to 70%"
    );
}

/// Feature-strip (ticket fb99a14): the seed bank is gone end to end —
/// batch and germination routes answer 404, so a stale bookmark or an
/// old client gets a clean "not found", never a half-working endpoint.
#[tokio::test]
async fn seed_bank_routes_are_gone() {
    let base = spawn_api().await;
    let client = reqwest::Client::new();
    let routes = [
        (reqwest::Method::GET, "/api/seed-batches"),
        (reqwest::Method::POST, "/api/seed-batches"),
        (reqwest::Method::GET, "/api/seed-batches/nope"),
        (reqwest::Method::PUT, "/api/seed-batches/nope"),
        (reqwest::Method::DELETE, "/api/seed-batches/nope"),
        (
            reqwest::Method::GET,
            "/api/seed-batches/nope/germination-tests",
        ),
        (
            reqwest::Method::POST,
            "/api/seed-batches/nope/germination-tests",
        ),
    ];
    for (method, path) in routes {
        let response = client
            .request(method, format!("{base}{path}"))
            .json(&json!({}))
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            reqwest::StatusCode::NOT_FOUND,
            "{path} must be gone"
        );
    }
}

#[tokio::test]
async fn onboarding_wizard_appears_runs_and_stays_dismissable() {
    // Fresh install: no grows, no dismissal — the wizard shows (spec
    // §4.12). The feed presets a fresh boot seeds are the only bundled
    // data left (D079).
    let db = Db::open_in_memory().unwrap();
    leaflog_backend::seed::seed_feed_presets(&db).await.unwrap();
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
    // The removed catalogues no longer report into the wizard state.
    assert!(
        fresh.get("has_spaces").is_none() && fresh.get("has_equipment").is_none(),
        "the wizard state must not report removed features: {fresh}"
    );

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
    // the final two-step shape: ① bind the grow's sensors, ② create
    // the grow (strain label and photoperiod inline, D075) with its
    // plants.
    let grow: Value = client
        .post(format!("{base}/api/grows"))
        .json(&json!({
            "name": "First run",
            "strain_label": "Northern Lights #5",
            "photoperiod": "photoperiod",
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
    let bindings: Value = client
        .put(format!("{base}/api/grows/{grow_id}/bindings"))
        .json(&json!({
            "bindings": { "temperature": "sensor.grow_temperature" }
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(bindings["temperature"], "sensor.grow_temperature");
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
}

#[tokio::test]
async fn harvest_weights_round_trip_and_summarise() {
    let base = spawn_api().await;
    let client = reqwest::Client::new();

    // A grow with two plants (the yield write-back left with the
    // catalogue, D075 — the per-plant weights are the record).
    let grow: Value = client
        .post(format!("{base}/api/grows"))
        .json(&json!({
            "name": "Harvest run",
            "strain_label": "My own cross",
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
}

#[tokio::test]
async fn history_accuracy_on_a_golden_grow_and_next_run_by_reference() {
    let db = Db::open_in_memory().unwrap();
    leaflog_backend::seed::seed_feed_presets(&db).await.unwrap();
    let base = spawn_with(db).await;
    let client = reqwest::Client::new();

    // A labelled grow (free-text strain label, D075) with a generic
    // feed template fitted.
    let templates: Vec<Value> = client
        .get(format!("{base}/api/feeds/templates"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let template = &templates[0];

    let grow: Value = client
        .post(format!("{base}/api/grows"))
        .json(&json!({
            "name": "NL golden run",
            "strain_label": "Northern Lights",
            "photoperiod": "photoperiod",
            "feed_template_id": template["id"],
            "planned_veg_weeks": 4,
            "substrate": "soil",
            "started_on": "2026-08-01",
            "status": "archived",
            "notes": null,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let grow_id = grow["id"].as_str().unwrap().to_owned();

    // A second grow with no label: its yield groups under the shared
    // unlabelled key instead of a strain.
    let unlabelled: Value = client
        .post(format!("{base}/api/grows"))
        .json(&json!({
            "name": "Unlabelled run",
            "substrate": "hydro",
            "started_on": "2026-08-02",
            "status": "archived",
            "notes": null,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let unlabelled_id = unlabelled["id"].as_str().unwrap().to_owned();

    // Flip on grow day 42 (2026-09-12), harvest on day 100
    // (2026-11-09).
    for (action, when) in [
        ("flip", "2026-09-12T12:00:00Z"),
        ("harvest", "2026-11-09T12:00:00Z"),
    ] {
        client
            .post(format!("{base}/api/grows/{grow_id}/actions"))
            .json(&json!({
                "plant_ids": [],
                "occurred_at": when,
                "action_type": action,
                "details": {},
                "notes": null,
                "appearance_tags": [],
            }))
            .send()
            .await
            .unwrap();
    }

    // One plant with a dry weight feeds the yield aggregates; the
    // unlabelled grow carries a second weighed plant.
    let plant: Value = client
        .post(format!("{base}/api/grows/{grow_id}/plants"))
        .json(&json!({ "name": "NL #1", "wet_g": null, "dry_g": null, "trimmed_g": null }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    client
        .put(format!(
            "{base}/api/plants/{}",
            plant["id"].as_str().unwrap()
        ))
        .json(&json!({ "name": "NL #1", "wet_g": 100.0, "dry_g": 25.0, "trimmed_g": 23.0 }))
        .send()
        .await
        .unwrap();
    let loose_plant: Value = client
        .post(format!("{base}/api/grows/{unlabelled_id}/plants"))
        .json(&json!({ "name": "Loose #1", "wet_g": null, "dry_g": null, "trimmed_g": null }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    client
        .put(format!(
            "{base}/api/plants/{}",
            loose_plant["id"].as_str().unwrap()
        ))
        .json(&json!({ "name": "Loose #1", "wet_g": 40.0, "dry_g": 8.0, "trimmed_g": 2.0 }))
        .send()
        .await
        .unwrap();

    // The golden comparison: the plan (no actuals) against the logged
    // days. The predicted numbers come from the M5-golden-verified
    // engine; here the deltas must be exactly actual − predicted.
    let report: Value = client
        .get(format!("{base}/api/history"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let row = report["accuracy"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["grow_id"] == grow_id.as_str())
        .expect("the golden grow is in the history");
    assert_eq!(row["strain"], "Northern Lights");
    let boundaries = row["boundaries"].as_array().unwrap();
    assert_eq!(boundaries.len(), 2, "flip and harvest are both logged");

    let predicted_flip = boundaries[0]["predicted_day"].as_i64().unwrap();
    assert_eq!(boundaries[0]["actual_day"], 42);
    assert_eq!(boundaries[0]["delta_days"], 42 - predicted_flip);
    let predicted_harvest = boundaries[1]["predicted_day"].as_i64().unwrap();
    assert_eq!(boundaries[1]["actual_day"], 100);
    assert_eq!(boundaries[1]["delta_days"], 100 - predicted_harvest);
    // The plan's flip sits after germination + seedling + default veg;
    // the harvest lands inside the default photoperiod flower window.
    assert!(predicted_flip > 0 && predicted_harvest > predicted_flip);

    // Yields group by the grow's strain label; the unlabelled grow
    // groups under the shared key, never under a strain name.
    let nl_yield = report["yield_by_label"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["label"] == "Northern Lights")
        .expect("yield grouped by strain label");
    assert_eq!(nl_yield["total_dry_g"], 25.0);
    assert_eq!(nl_yield["grows"], 1);
    let unlabelled_yield = report["yield_by_label"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["label"] == "unlabelled")
        .expect("unlabelled grows group together");
    assert_eq!(unlabelled_yield["total_dry_g"], 8.0);
    let soil_yield = report["yield_by_substrate"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["label"] == "soil")
        .expect("yield grouped by substrate");
    assert_eq!(soil_yield["total_wet_g"], 100.0);
    let template_yield = report["yield_by_feed_template"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["label"] == template["name"].as_str().unwrap())
        .expect("yield grouped by feed template");
    assert_eq!(template_yield["total_trimmed_g"], 23.0);

    // The per-plant view ties the plant to its weights.
    let view: Value = client
        .get(format!(
            "{base}/api/plants/{}/view",
            plant["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(view["plant"]["name"], "NL #1");
    assert_eq!(view["yield_g"]["total_dry_g"], 25.0);

    // One-tap next run: the label and photoperiod travel verbatim (the
    // label is free text now — there is no catalogue entry to
    // reference), and the space is reused by reference, not copied.
    let next: Value = client
        .post(format!("{base}/api/grows/{grow_id}/next-run"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(next["name"], "NL golden run (next run)");
    assert_eq!(next["strain_label"], "Northern Lights");
    assert_eq!(next["photoperiod"], "photoperiod");
    assert_eq!(next["feed_template_id"], template["id"]);
    assert_eq!(next["substrate"], "soil");
    assert_eq!(next["status"], "active");
}

#[tokio::test]
async fn unavailable_bound_entities_render_no_data_and_can_rebind() {
    // The bridge is offline: nothing is tracked, so every bound entity
    // is unavailable. The snapshot must say so per binding and degrade
    // to no-data — never "in range" (spec §4.1/§4.9).
    let base = spawn_api().await;
    let client = reqwest::Client::new();

    let grow: Value = client
        .post(format!("{base}/api/grows"))
        .json(&json!({
            "name": "Drift run",
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
    let grow_id = grow["id"].as_str().unwrap().to_owned();
    client
        .put(format!("{base}/api/grows/{grow_id}/bindings"))
        .json(&json!({
            "bindings": {
                "temperature": "sensor.gone_temp",
                "humidity": "sensor.gone_hum",
            }
        }))
        .send()
        .await
        .unwrap();

    let env: Value = client
        .get(format!("{base}/api/grows/{grow_id}/env"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let bindings = env["bindings"].as_array().unwrap();
    assert_eq!(
        bindings.len(),
        2,
        "each binding reports its drift: {bindings:?}"
    );
    assert!(
        bindings
            .iter()
            .all(|binding| binding["available"] == json!(false))
    );
    assert_eq!(env["temperature_c"], Value::Null, "no data, not zero");
    assert_eq!(env["temp_ok"], Value::Null, "never 'in range' without data");
    assert_eq!(env["vpd_ok"], Value::Null);

    // Rebinding is the fix: point the roles at different entities and
    // the snapshot follows (still no data offline, but the binding
    // moved — the affordance works end to end).
    client
        .put(format!("{base}/api/grows/{grow_id}/bindings"))
        .json(&json!({
            "bindings": { "temperature": "sensor.new_temp" }
        }))
        .send()
        .await
        .unwrap();
    let env: Value = client
        .get(format!("{base}/api/grows/{grow_id}/env"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let bindings = env["bindings"].as_array().unwrap();
    assert_eq!(bindings.len(), 1, "the humidity role was released");
    assert_eq!(bindings[0]["entity_id"], "sensor.new_temp");
    assert_eq!(bindings[0]["available"], false, "still offline");
}
