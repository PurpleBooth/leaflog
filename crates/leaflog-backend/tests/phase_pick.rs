//! Integration tests for the Germination phase pick: the pick is the
//! grow's start date (ticket 04, spec 95b2e66). One pick writes the
//! anchor and re-dates every stored grow day in the same transaction.

use std::net::SocketAddr;
use std::path::PathBuf;

use axum::ServiceExt as _;
use leaflog_backend::photos::PhotoStore;
use leaflog_backend::{AppState, Assets, Db, Mode, app};
use serde_json::{Value, json};

mod common;

use common::started_today;

/// Spawns the API against an offline Supervisor; returns the base URL
/// and the shared [`Db`] handle (trigger installation needs it).
async fn spawn() -> (String, Db, PathBuf) {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dist");
    let photo_root = std::env::temp_dir().join(format!(
        "leaflog-phase-pick-{}",
        leaflog_backend::db::new_id()
    ));
    let db = Db::open_in_memory().expect("in-memory db");
    let bridge = std::sync::Arc::new(leaflog_backend::ha::HaBridge::spawn(
        "http://127.0.0.1:1".to_owned(),
        leaflog_backend::ha::SupervisorToken::new("test-token"),
        db.clone(),
    ));
    let state = AppState::new(
        Assets::Disk(fixture),
        Mode::Dev,
        None,
        db.clone(),
        PhotoStore::new(photo_root.clone()),
        bridge,
    );
    let service = app::service(state);
    let make = service.into_make_service_with_connect_info::<SocketAddr>();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, make).await.unwrap();
    });
    (format!("http://{addr}"), db, photo_root)
}

/// Creates a grow started on `started_on` and returns its id.
async fn create_grow(base: &str, started_on: &str) -> String {
    let grow: Value = reqwest::Client::new()
        .post(format!("{base}/api/grows"))
        .json(&json!({
            "name": "Pick run",
            "substrate": "soil",
            "started_on": started_on,
            "status": "active",
            "notes": null
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    grow["id"].as_str().unwrap().to_owned()
}

/// A phase pick (phase_change) targeting `label` at `occurred`.
fn pick_payload(label: &str, occurred: &str) -> Value {
    json!({
        "plant_ids": [],
        "action_type": "phase_change",
        "occurred_at": occurred,
        "details": { "target_phase": label },
        "notes": null,
        "appearance_tags": [],
    })
}

/// A plain log action of `kind` at `occurred`.
fn action_payload(kind: &str, occurred: &str) -> Value {
    json!({
        "plant_ids": [],
        "action_type": kind,
        "occurred_at": occurred,
        "details": {},
        "notes": null,
        "appearance_tags": [],
    })
}

/// Reads the grow's `started_on` through the API.
async fn started_on(base: &str, id: &str) -> String {
    reqwest::Client::new()
        .get(format!("{base}/api/grows/{id}"))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap()["started_on"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// Maps the action list by (action_type, occurred_at) → grow_day.
async fn action_days(base: &str, id: &str) -> Vec<(String, String, Value)> {
    let entries: Value = reqwest::Client::new()
        .get(format!("{base}/api/grows/{id}/actions"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    entries
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| {
            (
                entry["action_type"].as_str().unwrap().to_owned(),
                entry["occurred_at"].as_str().unwrap().to_owned(),
                entry["grow_day"].clone(),
            )
        })
        .collect()
}

#[tokio::test]
async fn non_germination_picks_and_plain_actions_leave_the_start_date_alone() {
    let (base, _db, _root) = spawn().await;
    let client = reqwest::Client::new();
    let id = create_grow(&base, "2026-10-01").await;

    for payload in [
        pick_payload("seedling", "2026-10-10T09:00:00Z"),
        pick_payload("curing", "2026-11-20T09:00:00Z"),
        action_payload("water", "2026-10-05T10:00:00Z"),
    ] {
        client
            .post(format!("{base}/api/grows/{id}/actions"))
            .json(&payload)
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap();
    }

    // Only the Germination pick owns the anchor; every other pick
    // and plain action leaves started_on untouched.
    assert_eq!(started_on(&base, &id).await, "2026-10-01");
    let days = action_days(&base, &id).await;
    let pick_day = days
        .iter()
        .find(|(kind, occurred, _)| kind == "phase_change" && occurred == "2026-10-10T09:00:00Z")
        .unwrap()
        .2
        .clone();
    assert_eq!(pick_day, 9, "seedling pick days from the original anchor");
}

#[tokio::test]
async fn germination_pick_rolls_back_whole_when_a_redate_step_fails() {
    let (base, db, _root) = spawn().await;
    let client = reqwest::Client::new();
    let id = create_grow(&base, "2026-10-01").await;
    client
        .post(format!("{base}/api/grows/{id}/actions"))
        .json(&action_payload("water", "2026-10-04T10:00:00Z"))
        .send()
        .await
        .unwrap()
        .json::<Value>()
        .await
        .unwrap();
    let photo: Value = client
        .post(format!("{base}/api/grows/{id}/photos"))
        .multipart(
            reqwest::multipart::Form::new().part(
                "file",
                reqwest::multipart::Part::bytes(jpeg_with_exif())
                    .file_name("plant.jpg")
                    .mime_str("image/jpeg")
                    .unwrap(),
            ),
        )
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();

    // The photo re-dating step aborts: the pick must fail and leave
    // anchor, entries and photos exactly as they were.
    db.with(|conn| -> Result<(), leaflog_backend::db::StoreError> {
        conn.execute_batch(
            "CREATE TRIGGER abort_photo_redate BEFORE UPDATE ON photo
                 BEGIN SELECT RAISE(ABORT, 'boom'); END",
        )?;
        Ok(())
    })
    .await
    .unwrap();
    let failed = client
        .post(format!("{base}/api/grows/{id}/actions"))
        .json(&pick_payload("germination", "2026-10-03T08:00:00Z"))
        .send()
        .await
        .unwrap();
    assert_eq!(failed.status(), 500);
    db.with(|conn| -> Result<(), leaflog_backend::db::StoreError> {
        conn.execute_batch("DROP TRIGGER abort_photo_redate")?;
        Ok(())
    })
    .await
    .unwrap();

    assert_eq!(started_on(&base, &id).await, "2026-10-01");
    let days = action_days(&base, &id).await;
    assert_eq!(days.len(), 1, "the failed pick left no phase_change row");
    assert_eq!(days[0].2, 3, "water keeps its day under the old anchor");
    let photos: Value = client
        .get(format!("{base}/api/grows/{id}/photos"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(photos[0]["id"], photo["id"]);
    assert_eq!(photos[0]["grow_day"], 19, "photo keeps its day");
}

#[tokio::test]
async fn backdated_germination_pick_reanchors_the_forecast_and_today_marker() {
    let (base, _db, _root) = spawn().await;
    let client = reqwest::Client::new();
    let today = started_today();
    let id = create_grow(&base, "2026-10-01").await;

    let before: Value = client
        .get(format!("{base}/api/grows/{id}/forecast"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(
        before["today_day"].as_u64().unwrap() > 30,
        "day count from Oct 1"
    );

    // The pick lands today (backdated to this morning): the anchor
    // moves, so the today marker restarts and the forecast pins the
    // germination window from the pick.
    let pick: Value = client
        .post(format!("{base}/api/grows/{id}/actions"))
        .json(&pick_payload("germination", &format!("{today}T08:00:00Z")))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(pick["grow_day"], 0);
    assert_eq!(started_on(&base, &id).await, today);

    let forecast: Value = client
        .get(format!("{base}/api/grows/{id}/forecast"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(forecast["today_day"], 0, "today restarts at the new anchor");
    let germination = forecast["windows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|window| window["phase"] == "germination")
        .expect("the forecast keeps the germination window");
    assert_eq!(germination["actual_start"], true, "the pick pins day 0");
}
/// A minimal JPEG with DateTimeOriginal 2026:10:20 09:30:00.
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
async fn germination_pick_sets_the_start_date_and_redays_the_grow() {
    let (base, _db, _root) = spawn().await;
    let client = reqwest::Client::new();
    let id = create_grow(&base, "2026-10-01").await;

    for payload in [
        action_payload("water", "2026-10-04T10:00:00Z"),
        action_payload("flip", "2026-11-01T12:00:00Z"),
        action_payload("feed", "2026-11-15T10:00:00Z"),
    ] {
        client
            .post(format!("{base}/api/grows/{id}/actions"))
            .json(&payload)
            .send()
            .await
            .unwrap()
            .json::<Value>()
            .await
            .unwrap();
    }
    let photo: Value = client
        .post(format!("{base}/api/grows/{id}/photos"))
        .multipart(
            reqwest::multipart::Form::new().part(
                "file",
                reqwest::multipart::Part::bytes(jpeg_with_exif())
                    .file_name("plant.jpg")
                    .mime_str("image/jpeg")
                    .unwrap(),
            ),
        )
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(photo["grow_day"], 19, "day 19 under the Oct 1 anchor");

    // The pick is backdated to Oct 3: the anchor moves and every
    // stored grow day is recomputed from its timestamp.
    let pick: Value = client
        .post(format!("{base}/api/grows/{id}/actions"))
        .json(&pick_payload("germination", "2026-10-03T08:00:00Z"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(pick["grow_day"], 0, "the pick is the new day 0");
    assert_eq!(pick["phase"], "germination");

    assert_eq!(started_on(&base, &id).await, "2026-10-03");

    let days = action_days(&base, &id).await;
    let day_of = |kind: &str, occurred: &str| {
        days.iter()
            .find(|(k, o, _)| k == kind && o == occurred)
            .unwrap_or_else(|| panic!("no {kind} row at {occurred}"))
            .2
            .clone()
    };
    assert_eq!(day_of("water", "2026-10-04T10:00:00Z"), 1);
    assert_eq!(day_of("flip", "2026-11-01T12:00:00Z"), 29);
    assert_eq!(day_of("feed", "2026-11-15T10:00:00Z"), 43);
    assert_eq!(day_of("phase_change", "2026-10-03T08:00:00Z"), 0);

    let photos: Value = client
        .get(format!("{base}/api/grows/{id}/photos"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(photos[0]["grow_day"], 17, "photo re-dated to Oct 17");
}
