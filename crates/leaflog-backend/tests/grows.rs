//! Integration tests for M3: grows, plants, the action log and the
//! photo pipeline (upload, EXIF strip, thumbnails, serving).

use std::net::SocketAddr;
use std::path::PathBuf;

use axum::ServiceExt as _;
use leaflog_backend::photos::PhotoStore;
use leaflog_backend::{AppState, Assets, Db, Mode, app};
use serde_json::{Value, json};

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
        "strain_id": null,
        "substrate": "soil",
        "started_on": "2026-10-01",
        "status": "active",
        "notes": null,
    })
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
