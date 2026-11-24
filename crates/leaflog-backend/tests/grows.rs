//! Integration tests for M3: grows, plants, the action log and the
//! photo pipeline (upload, EXIF strip, thumbnails, serving).

use std::net::SocketAddr;
use std::path::PathBuf;

use axum::ServiceExt as _;
use leaflog_backend::photos::PhotoStore;
use leaflog_backend::{AppState, Assets, Db, Mode, app};
use serde_json::{Value, json};

mod common;

use common::started_today;

/// Spawns the API with an empty in-memory DB and a temp photo store;
/// returns the base URL and the store root.
async fn spawn() -> (String, PathBuf) {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dist");
    let photo_root =
        std::env::temp_dir().join(format!("leaflog-m3-{}", leaflog_backend::db::new_id()));
    let db = Db::open_in_memory().unwrap();
    let state = AppState::new(
        Assets::Disk(fixture),
        Mode::Dev,
        None,
        db,
        PhotoStore::new(photo_root.clone()),
        offline_bridge(),
    );
    let service = app::service(state);
    let make = service.into_make_service_with_connect_info::<SocketAddr>();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, make).await.unwrap();
    });
    (format!("http://{addr}"), photo_root)
}

fn grow_payload(name: &str) -> Value {
    json!({
        "name": name,
        "substrate": "soil",
        "started_on": "2026-10-01",
        "status": "active",
        "notes": null,
    })
}

/// The grow-carried strain and lamp facts (D075 feature strip, step
/// 01), in the shape the create/update forms send.
fn strain_lamp_fields(label: &str, photoperiod: &str, watts: f64, dimmable: bool) -> Value {
    json!({
        "strain_label": label,
        "photoperiod": photoperiod,
        "heat_tolerance": "sensitive",
        "cold_tolerance": "tolerant",
        "light_tolerance": "tolerant",
        "real_watts": watts,
        "dimmable": dimmable,
    })
}

/// Asserts one stored grow carries exactly these strain/lamp facts.
fn assert_strain_lamp_fields(grow: &Value, fields: &Value) {
    for key in [
        "strain_label",
        "photoperiod",
        "heat_tolerance",
        "cold_tolerance",
        "light_tolerance",
        "real_watts",
        "dimmable",
    ] {
        assert_eq!(
            grow[key], fields[key],
            "field {key} did not round-trip (response: {grow})"
        );
    }
}

fn action_payload(kind: &str, occurred: &str) -> Value {
    json!({
        "plant_ids": [],
        "occurred_at": occurred,
        "action_type": kind,
        "details": {},
        "notes": null,
        "appearance_tags": [],
    })
}

/// Each file keeps its own captured time and plant links. The upload
/// rejects a plant from another grow, rather than leaking that grow's
/// identity into this diary.
#[tokio::test]
async fn photo_upload_has_independent_plants_and_local_capture_time() {
    let (base, _root) = spawn().await;
    let client = reqwest::Client::new();
    let grow: Value = client
        .post(format!("{base}/api/grows"))
        .json(&grow_payload("Independent photos"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = grow["id"].as_str().unwrap();
    let mut plant_ids = Vec::new();
    for name in ["One", "Two"] {
        let plant: Value = client
            .post(format!("{base}/api/grows/{id}/plants"))
            .json(&json!({"name": name}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        plant_ids.push(plant["id"].as_str().unwrap().to_owned());
    }
    let other: Value = client
        .post(format!("{base}/api/grows"))
        .json(&grow_payload("Other grow"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let other_plant: Value = client
        .post(format!(
            "{base}/api/grows/{}/plants",
            other["id"].as_str().unwrap()
        ))
        .json(&json!({"name": "Elsewhere"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let form = reqwest::multipart::Form::new()
        .part(
            "file",
            reqwest::multipart::Part::bytes(plain_jpeg(16, 16, [200, 30, 30]))
                .file_name("plant.jpg")
                .mime_str("image/jpeg")
                .unwrap(),
        )
        .text("captured_at", "2026-10-20T09:30")
        .text("plant_id", plant_ids[0].clone())
        .text("plant_ids", plant_ids[1].clone());
    let response = client
        .post(format!("{base}/api/grows/{id}/photos"))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::CREATED);
    let photo: Value = response.json().await.unwrap();
    assert_eq!(photo["captured_at"], "2026-10-20T09:30:00Z");
    let mut linked = photo["plant_ids"].as_array().unwrap().clone();
    linked.sort_by_key(Value::to_string);
    plant_ids.sort();
    assert_eq!(linked, json!(plant_ids).as_array().unwrap().clone());
    assert_eq!(photo["plant_id"], plant_ids[0]);

    let invalid = reqwest::multipart::Form::new()
        .part(
            "file",
            reqwest::multipart::Part::bytes(plain_jpeg(16, 16, [200, 30, 30]))
                .file_name("other.jpg")
                .mime_str("image/jpeg")
                .unwrap(),
        )
        .text("plant_id", other_plant["id"].as_str().unwrap().to_owned());
    let response = client
        .post(format!("{base}/api/grows/{id}/photos"))
        .multipart(invalid)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn actions_reject_plants_from_another_grow_and_use_canonical_local_time() {
    let (base, _root) = spawn().await;
    let client = reqwest::Client::new();
    let grow: Value = client
        .post(format!("{base}/api/grows"))
        .json(&grow_payload("Local note"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = grow["id"].as_str().unwrap();
    let other: Value = client
        .post(format!("{base}/api/grows"))
        .json(&grow_payload("Other grow"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let plant: Value = client
        .post(format!(
            "{base}/api/grows/{}/plants",
            other["id"].as_str().unwrap()
        ))
        .json(&json!({"name": "Elsewhere"}))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let response = client
        .post(format!("{base}/api/grows/{id}/actions"))
        .json(&action_payload("note", "2026-10-20T09:30:00Z"))
        .send()
        .await
        .unwrap();
    let note: Value = response.json().await.unwrap();
    let mut wrong = action_payload("note", "2026-10-20T09:30:00Z");
    wrong["plant_ids"] = json!([plant["id"]]);
    let response = client
        .put(format!(
            "{base}/api/actions/{}",
            note["id"].as_str().unwrap()
        ))
        .json(&wrong)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
    let response = client
        .post(format!("{base}/api/grows/{id}/actions?local_time=true"))
        .json(&action_payload("note", "2026-10-20T09:30"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::CREATED);
    let note: Value = response.json().await.unwrap();
    assert_eq!(note["occurred_at"], "2026-10-20T09:30:00Z");
}

/// Story pages cover whole local days, omit blank days, keep notes
/// separate from photos, and paginate without repeating a boundary.
#[tokio::test]
async fn story_groups_notes_photos_and_paginates_whole_days() {
    let (base, _root) = spawn().await;
    let client = reqwest::Client::new();
    let grow: Value = client
        .post(format!("{base}/api/grows"))
        .json(&grow_payload("Story run"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = grow["id"].as_str().unwrap();
    for (at, note) in [
        ("2026-10-18T09:00:00Z", "new leaves"),
        ("2026-10-08T09:00:00Z", "older note"),
    ] {
        let mut input = action_payload("note", at);
        input["notes"] = json!(note);
        let response = client
            .post(format!("{base}/api/grows/{id}/actions"))
            .json(&input)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), reqwest::StatusCode::CREATED);
    }
    let form = reqwest::multipart::Form::new()
        .part(
            "file",
            reqwest::multipart::Part::bytes(plain_jpeg(16, 16, [200, 30, 30]))
                .file_name("plant.jpg")
                .mime_str("image/jpeg")
                .unwrap(),
        )
        .text("captured_at", "2026-10-18T10:00");
    let response = client
        .post(format!("{base}/api/grows/{id}/photos"))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::CREATED);
    let first: Value = client
        .get(format!("{base}/api/grows/{id}/story?before=2026-10-19"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(first["timezone"], "UTC");
    assert!(first["days"].is_array(), "story error: {first}");
    let first_days: Vec<_> = first["days"]
        .as_array()
        .unwrap()
        .iter()
        .map(|day| day["date"].as_str().unwrap())
        .collect();
    assert_eq!(first_days, ["2026-10-18"], "story: {first}");
    let day = &first["days"][0];
    assert_eq!(day["actions"].as_array().unwrap().len(), 1);
    assert_eq!(day["photos"].as_array().unwrap().len(), 1);
    assert_eq!(day["has_observations"], false);
    let before = first["before"].as_str().unwrap();
    let second: Value = client
        .get(format!("{base}/api/grows/{id}/story?before={before}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(second["days"].as_array().unwrap().len(), 1);
    assert_eq!(second["days"][0]["date"], "2026-10-08");
    assert_eq!(second["before"], "2026-10-05");
    let third: Value = client
        .get(format!("{base}/api/grows/{id}/story?before=2026-10-05"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(third["days"].as_array().unwrap().len(), 0);
    assert_eq!(third["before"], Value::Null);
    // The photo calendar is window-independent: even a window that
    // holds no photo day reports every photo date across the grow.
    let fourth: Value = client
        .get(format!("{base}/api/grows/{id}/story?before=2026-10-15"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(fourth["days"][0]["date"], "2026-10-08");
    assert_eq!(fourth["photo_dates"], json!(["2026-10-18"]));
    assert_eq!(first["photo_dates"], json!(["2026-10-18"]));
}

/// An entry the grower dated later today still renders on today's
/// card: phase picks stamp noon (D073), so a morning pick's wall
/// clock is ahead of now. Membership is calendar-correct; only the
/// environment summary is clipped. Before the fix the whole day
/// vanished from the page until the stamp had passed.
#[tokio::test]
async fn story_shows_an_entry_dated_later_today() {
    let (base, _root) = spawn().await;
    let client = reqwest::Client::new();
    let grow: Value = client
        .post(format!("{base}/api/grows"))
        .json(&json!({
            "name": "Later today",
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
    let id = grow["id"].as_str().unwrap();
    // One hour ahead, capped at the last minute of the UTC day so the
    // stamp cannot roll onto tomorrow's (unrendered) page.
    let now = time::OffsetDateTime::now_utc();
    let tomorrow = time::Date::from_calendar_date(now.year(), now.month(), now.day())
        .unwrap()
        .next_day()
        .unwrap()
        .midnight();
    let stamp = std::cmp::min(
        now + time::Duration::hours(1),
        tomorrow.assume_utc() - time::Duration::minutes(1),
    );
    let occurred = stamp
        .format(&time::format_description::well_known::Rfc3339)
        .unwrap();
    let mut input = action_payload("note", &occurred);
    input["notes"] = json!("dated later today");
    let response = client
        .post(format!("{base}/api/grows/{id}/actions"))
        .json(&input)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::CREATED);
    let page: Value = client
        .get(format!("{base}/api/grows/{id}/story"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let days = page["days"].as_array().unwrap();
    assert_eq!(days.len(), 1, "story: {page}");
    assert_eq!(days[0]["date"], started_today());
    let actions = days[0]["actions"].as_array().unwrap();
    assert_eq!(actions.len(), 1, "story: {page}");
    assert_eq!(actions[0]["notes"], "dated later today");
}

/// An eight-week flowering schedule with one product, the shape the
/// schedules API accepts.
fn schedule_payload(name: &str) -> Value {
    json!({
        "name": name,
        "source": null,
        "flower_weeks": 8,
        "products": [{
            "id": "",
            "name": "Bloom",
            "unit": "ml_per_l",
            "group": "Feeds",
            "note": null,
            "doses": {
                "propagation": null,
                "veg": { "min": 1000, "max": 2000 },
                "flower": [{ "min": 2000, "max": 4000 }, null, null, null, null, null, null, null],
                "water": null,
            }
        }],
        "created_at": "",
        "updated_at": "",
    })
}

/// A photoperiod grow with no planned veg weeks and the schedule
/// attached: without a logged flip the veg phase is open-ended, so
/// the fit refuses and no plan exists — the shape the edit/delete
/// refit tests need.
fn grow_with_schedule_payload(name: &str, schedule_id: &str) -> Value {
    json!({
        "name": name,
        "photoperiod": "photoperiod",
        "feed_schedule_id": schedule_id,
        "substrate": "soil",
        "started_on": started_today(),
        "status": "active",
        "notes": null,
    })
}

/// Spawns the API like [`spawn`] but with the feed presets seeded, so
/// feeding-schedule tests can create schedules and fit plans.
async fn spawn_seeded() -> (String, PathBuf) {
    let db = Db::open_in_memory().unwrap();
    leaflog_backend::seed::seed_feed_presets(&db).await.unwrap();
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dist");
    let photo_root =
        std::env::temp_dir().join(format!("leaflog-m3-{}", leaflog_backend::db::new_id()));
    let state = AppState::new(
        Assets::Disk(fixture),
        Mode::Dev,
        None,
        db,
        PhotoStore::new(photo_root.clone()),
        offline_bridge(),
    );
    let service = app::service(state);
    let make = service.into_make_service_with_connect_info::<SocketAddr>();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, make).await.unwrap();
    });
    (format!("http://{addr}"), photo_root)
}

/// A small JPEG with a well-formed EXIF APP1 segment
/// (DateTimeOriginal), built with the exif crate's writer.
fn jpeg_with_exif() -> Vec<u8> {
    let img = image::DynamicImage::new_rgb8(16, 16);
    let mut plain = Vec::new();
    image::codecs::jpeg::JpegEncoder::new(&mut plain)
        .encode(
            img.to_rgb8().as_raw(),
            16,
            16,
            image::ExtendedColorType::Rgb8,
        )
        .expect("encoding cannot fail");

    let field = exif::Field {
        tag: exif::Tag::DateTimeOriginal,
        ifd_num: exif::In::PRIMARY,
        value: exif::Value::Ascii(vec![b"2026:10:20 09:30:00".to_vec()]),
    };
    let mut writer = exif::experimental::Writer::new();
    writer.push_field(&field);
    let mut tiff = std::io::Cursor::new(Vec::new());
    writer
        .write(&mut tiff, false)
        .expect("the exif writer produces a valid TIFF");
    let tiff = tiff.into_inner();

    let mut app1 = Vec::with_capacity(tiff.len() + 6);
    app1.extend_from_slice(b"Exif\0\0");
    app1.extend_from_slice(&tiff);

    let mut bytes = Vec::with_capacity(plain.len() + app1.len() + 4);
    bytes.extend_from_slice(&plain[..2]);
    bytes.push(0xff);
    bytes.push(0xe1);
    bytes.extend_from_slice(&u16::try_from(app1.len() + 2).unwrap().to_be_bytes());
    bytes.extend_from_slice(&app1);
    bytes.extend_from_slice(&plain[2..]);
    bytes
}

#[tokio::test]
async fn grow_plant_round_trip() {
    let (base, _root) = spawn().await;
    let client = reqwest::Client::new();

    let grow: Value = client
        .post(format!("{base}/api/grows"))
        .json(&grow_payload("Closet run"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = grow["id"].as_str().unwrap().to_owned();
    assert_eq!(grow["substrate"], "soil");

    let plant: Value = client
        .post(format!("{base}/api/grows/{id}/plants"))
        .json(&json!({ "name": "NL #1" }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let plants: Vec<Value> = client
        .get(format!("{base}/api/grows/{id}/plants"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(plants.len(), 1);
    assert_eq!(plants[0]["name"], "NL #1");

    // Bad start date is rejected.
    let mut bad = grow_payload("Bad");
    bad["started_on"] = json!("01.10.2026");
    let response = client
        .post(format!("{base}/api/grows"))
        .json(&bad)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);

    // Delete plant then grow.
    client
        .delete(format!(
            "{base}/api/plants/{}",
            plant["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap();
    let response = client
        .delete(format!("{base}/api/grows/{id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::NO_CONTENT);
}

/// Create and update round-trip every grow-carried strain and lamp
/// fact (D075 step 01): the payload's values come back on the create
/// response, on the update response and on a fresh read.
#[tokio::test]
async fn grow_round_trips_strain_and_lamp_fields() {
    let (base, _root) = spawn().await;
    let client = reqwest::Client::new();

    let mut payload = grow_payload("Labelled run");
    let fields = strain_lamp_fields("Northern Lights #5", "auto", 113.5, false);
    for (key, value) in fields.as_object().expect("fields is an object") {
        payload[key] = value.clone();
    }
    let created: Value = client
        .post(format!("{base}/api/grows"))
        .json(&payload)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = created["id"].as_str().unwrap().to_owned();
    assert_strain_lamp_fields(&created, &fields);

    // The edit flips every field (autoflower → photoperiod, the
    // tolerance levels each move, watts set and cleared differently).
    let mut update = payload.clone();
    update["name"] = json!("Labelled run v2");
    update["strain_label"] = json!("Moby Dick");
    update["photoperiod"] = json!("photoperiod");
    update["heat_tolerance"] = json!("average");
    update["cold_tolerance"] = json!("sensitive");
    update["light_tolerance"] = json!("average");
    update["real_watts"] = json!(240.0);
    update["dimmable"] = json!(true);
    let updated: Value = client
        .put(format!("{base}/api/grows/{id}"))
        .json(&update)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_strain_lamp_fields(&updated, &update);

    // A fresh read sees the persisted values, not just the echo.
    let fetched: Value = client
        .get(format!("{base}/api/grows/{id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_strain_lamp_fields(&fetched, &update);
}

/// A grow created without the new fields defaults them: label empty,
/// photoperiod photoperiod, tolerances average, watts unset, lamp
/// dimmable (D075 step 01 defaults).
#[tokio::test]
async fn grow_defaults_strain_and_lamp_fields_when_omitted() {
    let (base, _root) = spawn().await;
    let client = reqwest::Client::new();

    let created: Value = client
        .post(format!("{base}/api/grows"))
        .json(&grow_payload("Plain run"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = created["id"].as_str().unwrap().to_owned();
    let defaults = json!({
        "strain_label": "",
        "photoperiod": "photoperiod",
        "heat_tolerance": "average",
        "cold_tolerance": "average",
        "light_tolerance": "average",
        "real_watts": null,
        "dimmable": true,
    });
    assert_strain_lamp_fields(&created, &defaults);

    // A full replace that omits the fields resets to the defaults too
    // (PUT is a full update, like every other grow field).
    let mut update = grow_payload("Plain run v2");
    update["strain_label"] = json!("should be replaced");
    let _ = client
        .put(format!("{base}/api/grows/{id}"))
        .json(&json!({
            "name": update["name"],
            "substrate": update["substrate"],
            "started_on": update["started_on"],
            "status": update["status"],
            "notes": update["notes"],
        }))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let fetched: Value = client
        .get(format!("{base}/api/grows/{id}"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_strain_lamp_fields(&fetched, &defaults);
}

/// A negative watt draw is nonsense; the grow's copy of the field
/// follows the same rule — 400, never a stored row.
#[tokio::test]
async fn grow_rejects_negative_real_watts() {
    let (base, _root) = spawn().await;
    let client = reqwest::Client::new();

    let mut payload = grow_payload("Negative watts");
    payload["real_watts"] = json!(-5.0);
    let response = client
        .post(format!("{base}/api/grows"))
        .json(&payload)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn action_log_derives_phase_and_day() {
    let (base, _root) = spawn().await;
    let client = reqwest::Client::new();

    let grow: Value = client
        .post(format!("{base}/api/grows"))
        .json(&grow_payload("Phase run"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = grow["id"].as_str().unwrap().to_owned();

    // Day 3: seedling.
    let entry: Value = client
        .post(format!("{base}/api/grows/{id}/actions"))
        .json(&action_payload("water", "2026-10-04T10:00:00Z"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(entry["grow_day"], 3);
    assert_eq!(entry["phase"], "seedling");

    // Day 30: still vegetative without a flip.
    let entry: Value = client
        .post(format!("{base}/api/grows/{id}/actions"))
        .json(&action_payload("measurement", "2026-10-31T10:00:00Z"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(entry["grow_day"], 30);
    assert_eq!(entry["phase"], "vegetative");

    // The flip on day 31: its own entry is already "flower".
    let flip: Value = client
        .post(format!("{base}/api/grows/{id}/actions"))
        .json(&action_payload("flip", "2026-11-01T12:00:00Z"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(flip["grow_day"], 31);
    assert_eq!(flip["phase"], "flower");

    // Day 45: flower now that the flip is in the past.
    let mut feed = action_payload("feed", "2026-11-15T10:00:00Z");
    feed["details"] = json!({
        "volume_ml": 1000.0,
        "input_ph": 6.3,
        "input_ec_us": 1200,
        "recipe": "bloom A/B",
    });
    let entry: Value = client
        .post(format!("{base}/api/grows/{id}/actions"))
        .json(&feed)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(entry["grow_day"], 45);
    assert_eq!(entry["phase"], "flower");
    assert_eq!(entry["details"]["input_ec_us"], 1200);

    // Harvest: its own entry flips to drying.
    let harvest: Value = client
        .post(format!("{base}/api/grows/{id}/actions"))
        .json(&action_payload("harvest", "2026-11-20T10:00:00Z"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(harvest["phase"], "drying");

    // Filter by type.
    let waters: Vec<Value> = client
        .get(format!("{base}/api/grows/{id}/actions?kind=water"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(waters.len(), 1);

    // Invalid pH rejected.
    let mut bad = action_payload("water", "2026-11-16T10:00:00Z");
    bad["details"] = json!({ "input_ph": 22.0 });
    let response = client
        .post(format!("{base}/api/grows/{id}/actions"))
        .json(&bad)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
}

/// A `datetime-local` input yields `YYYY-MM-DDTHH:MM` with no seconds
/// and no UTC offset; the backend must normalize that to RFC 3339 UTC
/// so a quick-add action round-trips instead of failing validation.
#[tokio::test]
async fn action_accepts_datetime_local_occurred_at() {
    let (base, _root) = spawn().await;
    let client = reqwest::Client::new();

    let grow: Value = client
        .post(format!("{base}/api/grows"))
        .json(&grow_payload("Quick-add run"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = grow["id"].as_str().unwrap().to_owned();

    let response = client
        .post(format!("{base}/api/grows/{id}/actions"))
        .json(&action_payload("water", "2026-11-09T14:30"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        response.status(),
        reqwest::StatusCode::CREATED,
        "datetime-local occurred_at (no seconds, no offset) must be accepted"
    );
    let entry: Value = response.json().await.unwrap();
    let stored = entry["occurred_at"].as_str().unwrap().to_owned();
    time::OffsetDateTime::parse(&stored, &time::format_description::well_known::Rfc3339)
        .unwrap_or_else(|error| panic!("stored occurred_at {stored:?} is not RFC 3339: {error}"));
}

/// Re-typing an entry to `flip` via `PUT /api/actions/{id}` must
/// re-derive the phase with the incoming change. `create_action`
/// treats the entry being written as already counting (a flip entry is
/// itself "flower"); editing an entry into a flip on the same day
/// must do the same. The stored phase may not silently stay at the
/// pre-flip value.
#[tokio::test]
async fn update_retyping_to_flip_rederives_phase() {
    let (base, _root) = spawn().await;
    let client = reqwest::Client::new();

    let grow: Value = client
        .post(format!("{base}/api/grows"))
        .json(&grow_payload("Retype run"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = grow["id"].as_str().unwrap().to_owned();

    // Day 39 note: vegetative, no flip in the log.
    let note: Value = client
        .post(format!("{base}/api/grows/{id}/actions"))
        .json(&action_payload("note", "2026-11-09T10:00:00Z"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let action_id = note["id"].as_str().unwrap().to_owned();

    // Re-type the same entry into a flip, same occurred_at.
    let response = client
        .put(format!("{base}/api/actions/{action_id}"))
        .json(&action_payload("flip", "2026-11-09T10:00:00Z"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let entries: Vec<Value> = client
        .get(format!("{base}/api/grows/{id}/actions"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let updated = entries
        .iter()
        .find(|entry| entry["id"] == json!(action_id))
        .expect("updated entry still listed");
    assert_eq!(
        updated["action_type"], "flip",
        "sanity: the re-type itself persisted"
    );
    assert_eq!(
        updated["phase"], "flower",
        "the flip being written must count for its own entry, as create_action does"
    );
}

/// Editing an entry into a flip must also re-fit the feeding plan
/// (audit D145): the plan is fit against the phase timeline, so a
/// late-recorded flip that arrives through an edit may not leave the
/// plan on the all-vegetative boundaries.
#[tokio::test]
async fn update_retyping_to_flip_refits_the_feed_plan() {
    let (base, _root) = spawn_seeded().await;
    let client = reqwest::Client::new();

    let schedule: Value = client
        .post(format!("{base}/api/schedules"))
        .json(&schedule_payload("Edit-refit run"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let grow: Value = client
        .post(format!("{base}/api/grows"))
        .json(&grow_with_schedule_payload(
            "Edit-refit grow",
            schedule["id"].as_str().unwrap(),
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = grow["id"].as_str().unwrap().to_owned();

    // A day-39 note: no flip in the log and no planned veg weeks, so
    // the veg phase is open-ended and the fit refuses — no plan.
    let note: Value = client
        .post(format!("{base}/api/grows/{id}/actions"))
        .json(&action_payload("note", "2026-11-09T10:00:00Z"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let action_id = note["id"].as_str().unwrap().to_owned();
    let plan_before: Value = client
        .get(format!("{base}/api/grows/{id}/feed/plan"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        plan_before.get("days"),
        None,
        "no flip and no planned veg weeks: open-ended veg fits nothing"
    );

    // Re-type the note into a flip through the edit path.
    let response = client
        .put(format!("{base}/api/actions/{action_id}"))
        .json(&action_payload("flip", "2026-11-09T10:00:00Z"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::OK);

    let plan_after: Value = client
        .get(format!("{base}/api/grows/{id}/feed/plan"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        plan_after["days"]
            .as_array()
            .is_some_and(|days| !days.is_empty()),
        "the edit re-fit the plan now that a flip bounds the veg phase"
    );

    // Deleting the flip again moves the timeline back to open-ended:
    // the plan must be gone, not left over from the edit (D145).
    let response = client
        .delete(format!("{base}/api/actions/{action_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::NO_CONTENT);
    let plan_cleared: Value = client
        .get(format!("{base}/api/grows/{id}/feed/plan"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        plan_cleared.get("days"),
        None,
        "removing the flip must clear the plan, not keep the fitted one"
    );
}

/// Deleting the flip again must re-fit too: the plan drops back to
/// the open-ended state (no stored plan), not a stale flower-shaped
/// plan (audit D145).
#[tokio::test]
async fn delete_flip_refits_the_feed_plan() {
    let (base, _root) = spawn_seeded().await;
    let client = reqwest::Client::new();

    let schedule: Value = client
        .post(format!("{base}/api/schedules"))
        .json(&schedule_payload("Delete-refit run"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let grow: Value = client
        .post(format!("{base}/api/grows"))
        .json(&grow_with_schedule_payload(
            "Delete-refit grow",
            schedule["id"].as_str().unwrap(),
        ))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = grow["id"].as_str().unwrap().to_owned();

    // Creating the flip through the create path fits a plan.
    let flip: Value = client
        .post(format!("{base}/api/grows/{id}/actions"))
        .json(&action_payload("flip", "2026-11-09T10:00:00Z"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let flip_id = flip["id"].as_str().unwrap().to_owned();
    let plan: Value = client
        .get(format!("{base}/api/grows/{id}/feed/plan"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        plan["days"].as_array().is_some_and(|days| !days.is_empty()),
        "the create path fit a plan for the flip-bounded timeline"
    );

    let response = client
        .delete(format!("{base}/api/actions/{flip_id}"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::NO_CONTENT);

    let plan_after: Value = client
        .get(format!("{base}/api/grows/{id}/feed/plan"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(
        plan_after.get("days"),
        None,
        "without the flip the fit refuses again and the stale plan is gone"
    );
}

#[tokio::test]
async fn photo_upload_strips_exif_and_serves() {
    let (base, root) = spawn().await;
    let client = reqwest::Client::new();

    let grow: Value = client
        .post(format!("{base}/api/grows"))
        .json(&grow_payload("Photo run"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = grow["id"].as_str().unwrap().to_owned();

    let form = reqwest::multipart::Form::new()
        .part(
            "file",
            reqwest::multipart::Part::bytes(jpeg_with_exif())
                .file_name("plant.jpg")
                .mime_str("image/jpeg")
                .unwrap(),
        )
        .text("tags", " canopy, day-30 ");

    let photo: Value = client
        .post(format!("{base}/api/grows/{id}/photos"))
        .multipart(form)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // EXIF capture time (2026-10-20) beats upload time; day 19 from the
    // Oct 1 start; tags trimmed.
    assert_eq!(photo["captured_at"], "2026-10-20T09:30:00Z");
    assert_eq!(photo["grow_day"], 19);
    assert_eq!(photo["tags"], json!(["canopy", "day-30"]));

    // The stored original must carry no Exif/GPS markers.
    let original = client
        .get(format!(
            "{base}/api/photos/{}/original",
            photo["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(original.status(), 200);
    assert_eq!(
        original.headers()[reqwest::header::CONTENT_TYPE],
        "image/jpeg"
    );
    let bytes = original.bytes().await.unwrap();
    assert!(!bytes.windows(6).any(|window| window == b"Exif\0\0"));
    assert!(!bytes.windows(4).any(|window| window == b"GPS\0"));

    // Thumbnail is WebP.
    let thumb = client
        .get(format!(
            "{base}/api/photos/{}/thumbnail",
            photo["id"].as_str().unwrap()
        ))
        .send()
        .await
        .unwrap();
    assert_eq!(thumb.status(), 200);
    assert_eq!(thumb.headers()[reqwest::header::CONTENT_TYPE], "image/webp");
    let thumb_bytes = thumb.bytes().await.unwrap();
    assert_eq!(&thumb_bytes[..4], b"RIFF");
    assert_eq!(&thumb_bytes[8..12], b"WEBP");

    // Layout on disk matches the backup contract: originals/ and
    // thumbnails/ trees under the photo root.
    let original_name = photo["original_name"].as_str().unwrap().to_owned();
    let stored = root.join("originals").join(&id).join(&original_name);
    assert!(stored.exists(), "original stored at {stored:?}");
    assert!(root.join("thumbnails").join(&id).file_name().is_some());

    // Usage endpoint reports the stored bytes.
    let usage: Value = client
        .get(format!("{base}/api/photos/usage"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(usage["total_bytes"], photo["bytes_stored"]);
    assert!(usage["warn_bytes"].as_u64().unwrap() > 0);

    // Deleting the photo removes row and files.
    let photo_id = photo["id"].as_str().unwrap().to_owned();
    client
        .delete(format!("{base}/api/photos/{photo_id}"))
        .send()
        .await
        .unwrap();
    assert!(!stored.exists(), "original file removed with the row");
    let missing = client
        .get(format!("{base}/api/photos/{photo_id}/original"))
        .send()
        .await
        .unwrap();
    assert_eq!(missing.status(), reqwest::StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn photo_upload_rejects_garbage() {
    let (base, _root) = spawn().await;
    let client = reqwest::Client::new();
    let grow: Value = client
        .post(format!("{base}/api/grows"))
        .json(&grow_payload("Reject run"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = grow["id"].as_str().unwrap().to_owned();

    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(b"not an image at all")
            .file_name("x.jpg")
            .mime_str("image/jpeg")
            .unwrap(),
    );
    let response = client
        .post(format!("{base}/api/grows/{id}/photos"))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);

    // Missing file field entirely.
    let response = client
        .post(format!("{base}/api/grows/{id}/photos"))
        .multipart(reqwest::multipart::Form::new().text("tags", "x"))
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
}

/// A small file whose header declares gigantic dimensions must be
/// rejected before the decoder allocates for it (audit D145 /
/// threat-model R1). The dimensions live in a real PNG header, so a
/// 57-byte input would otherwise ask for a ~90 GB allocation.
#[tokio::test]
async fn photo_upload_rejects_huge_declared_dimensions() {
    let (base, _root) = spawn().await;
    let client = reqwest::Client::new();
    let grow: Value = client
        .post(format!("{base}/api/grows"))
        .json(&grow_payload("Bomb run"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = grow["id"].as_str().unwrap().to_owned();

    // A hand-built PNG: signature + one IHDR chunk declaring
    // 100000x100000, RGB8, no interlace. No pixel data — the guard
    // must fire on the header alone.
    let mut ihdr = Vec::new();
    ihdr.extend_from_slice(&100_000_u32.to_be_bytes());
    ihdr.extend_from_slice(&100_000_u32.to_be_bytes());
    ihdr.extend_from_slice(&[8, 2, 0, 0, 0]);
    let mut bomb = Vec::new();
    bomb.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
    bomb.extend_from_slice(&(ihdr.len() as u32).to_be_bytes());
    bomb.extend_from_slice(b"IHDR");
    bomb.extend_from_slice(&ihdr);
    bomb.extend_from_slice(&png_crc32(b"IHDR", &ihdr).to_be_bytes());
    assert_eq!(
        bomb.len(),
        8 + 4 + 4 + 13 + 4,
        "sanity: the PNG is header-only"
    );

    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(bomb)
            .file_name("bomb.png")
            .mime_str("image/png")
            .unwrap(),
    );
    let response = client
        .post(format!("{base}/api/grows/{id}/photos"))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);
}

/// The PNG chunk CRC (ISO 3309 / ITU-T V.42): a table-less bitwise
/// CRC-32 over type + data, enough for the hand-built header fixture.
fn png_crc32(chunk_type: &[u8], data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFF_u32;
    for byte in chunk_type.iter().chain(data.iter()) {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            let mask = (crc & 1).wrapping_neg();
            crc = (crc >> 1) ^ (0xEDB8_8320 & mask);
        }
    }
    !crc
}

/// Photo responses carry private cache rules (threat-model R2):
/// originals never touch a shared disk cache, thumbnails may live in
/// one for a day.
#[tokio::test]
async fn photo_responses_carry_private_cache_control() {
    let (base, _root) = spawn().await;
    let client = reqwest::Client::new();
    let grow: Value = client
        .post(format!("{base}/api/grows"))
        .json(&grow_payload("Cache run"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = grow["id"].as_str().unwrap().to_owned();

    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(plain_jpeg(8, 8, [10, 20, 30]))
            .file_name("plant.jpg")
            .mime_str("image/jpeg")
            .unwrap(),
    );
    let photo: Value = client
        .post(format!("{base}/api/grows/{id}/photos"))
        .multipart(form)
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let photo_id = photo["id"].as_str().unwrap().to_owned();

    let original = client
        .get(format!("{base}/api/photos/{photo_id}/original"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        original.headers().get(reqwest::header::CACHE_CONTROL),
        Some(&reqwest::header::HeaderValue::from_static(
            "private, no-store"
        )),
        "originals must not be cached by shared browsers"
    );
    let thumbnail = client
        .get(format!("{base}/api/photos/{photo_id}/thumbnail"))
        .send()
        .await
        .unwrap();
    assert_eq!(
        thumbnail.headers().get(reqwest::header::CACHE_CONTROL),
        Some(&reqwest::header::HeaderValue::from_static(
            "private, max-age=86400"
        )),
        "thumbnails may be cached privately for a day"
    );
}

/// A plain minimal JPEG (no EXIF) filled with one colour; size and fill
/// vary between calls so the bytes on disk are distinct.
fn plain_jpeg(width: u32, height: u32, fill: [u8; 3]) -> Vec<u8> {
    let img = image::RgbImage::from_pixel(width, height, image::Rgb(fill));
    let mut bytes = Vec::new();
    image::codecs::jpeg::JpegEncoder::new(&mut bytes)
        .encode(img.as_raw(), width, height, image::ExtendedColorType::Rgb8)
        .expect("encoding cannot fail");
    bytes
}

#[tokio::test]
async fn photo_upload_stores_every_file_part() {
    let (base, _root) = spawn().await;
    let client = reqwest::Client::new();
    let grow: Value = client
        .post(format!("{base}/api/grows"))
        .json(&grow_payload("Multi-photo run"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = grow["id"].as_str().unwrap().to_owned();

    // One multipart POST carrying two "file" parts, exactly as the
    // frontend appends every selected photo under the same field name.
    let form = reqwest::multipart::Form::new()
        .part(
            "file",
            reqwest::multipart::Part::bytes(plain_jpeg(16, 16, [200, 30, 30]))
                .file_name("first.jpg")
                .mime_str("image/jpeg")
                .unwrap(),
        )
        .part(
            "file",
            reqwest::multipart::Part::bytes(plain_jpeg(24, 16, [30, 30, 200]))
                .file_name("second.jpg")
                .mime_str("image/jpeg")
                .unwrap(),
        );
    let response = client
        .post(format!("{base}/api/grows/{id}/photos"))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::CREATED);

    // Every uploaded part must be stored: two rows, each with its own
    // file on disk.
    let rows: Vec<Value> = client
        .get(format!("{base}/api/grows/{id}/photos"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(rows.len(), 2, "one row per uploaded file part");
    let names: Vec<&str> = rows
        .iter()
        .map(|row| row["original_name"].as_str().unwrap())
        .collect();
    assert_ne!(names[0], names[1], "each part stored as its own file");
}

#[tokio::test]
async fn failed_multi_part_upload_stores_nothing() {
    let (base, root) = spawn().await;
    let client = reqwest::Client::new();
    let grow: Value = client
        .post(format!("{base}/api/grows"))
        .json(&grow_payload("Partial-failure run"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let id = grow["id"].as_str().unwrap().to_owned();

    // First part valid, second part corrupt — exactly what a multi-file
    // selection with one broken image produces. The whole request must
    // fail and leave neither rows nor files behind, or a retry would
    // duplicate the first photo (there is no idempotency key).
    let form = reqwest::multipart::Form::new()
        .part(
            "file",
            reqwest::multipart::Part::bytes(plain_jpeg(16, 16, [90, 90, 30]))
                .file_name("good.jpg")
                .mime_str("image/jpeg")
                .unwrap(),
        )
        .part(
            "file",
            reqwest::multipart::Part::bytes(b"not an image at all")
                .file_name("bad.jpg")
                .mime_str("image/jpeg")
                .unwrap(),
        );
    let response = client
        .post(format!("{base}/api/grows/{id}/photos"))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::BAD_REQUEST);

    let rows: Vec<Value> = client
        .get(format!("{base}/api/grows/{id}/photos"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        rows.is_empty(),
        "a failed multi-part upload must not persist earlier parts: {rows:?}"
    );

    // Nothing on disk either: no originals, no thumbnails.
    for kind in ["originals", "thumbnails"] {
        let dir = root.join(kind).join(&id);
        let empty = std::fs::read_dir(&dir)
            .map(|entries| entries.count() == 0)
            .unwrap_or(true);
        assert!(empty, "{kind} must hold no files after the failure");
    }

    // The user fixes the selection and retries with the good photo
    // only: exactly one row, no duplicate of the first attempt.
    let form = reqwest::multipart::Form::new().part(
        "file",
        reqwest::multipart::Part::bytes(plain_jpeg(16, 16, [90, 90, 30]))
            .file_name("good.jpg")
            .mime_str("image/jpeg")
            .unwrap(),
    );
    let response = client
        .post(format!("{base}/api/grows/{id}/photos"))
        .multipart(form)
        .send()
        .await
        .unwrap();
    assert_eq!(response.status(), reqwest::StatusCode::CREATED);
    let rows: Vec<Value> = client
        .get(format!("{base}/api/grows/{id}/photos"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(rows.len(), 1, "the retry must not duplicate the photo");
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
