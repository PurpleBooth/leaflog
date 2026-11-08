//! Backup→restore integration test (spec §13, "Done means").
//!
//! Creates data (DB rows + photo files), simulates a HA backup by
//! copying the `/data` tree (DB + photos/originals; thumbnails are
//! excluded by `backup_exclude`), then simulates a restore by
//! rebuilding from the backup and proving data + photos survive while
//! thumbnails regenerate.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};

use axum::ServiceExt as _;
use leaflog_backend::photos::PhotoStore;
use leaflog_backend::{AppState, Assets, Db, Mode, app};
use serde_json::{Value, json};

/// The app's `/data` tree layout.
#[derive(Debug)]
struct DataTree {
    root: PathBuf,
}

impl DataTree {
    fn create(tag: &str) -> DataTree {
        let root = std::env::temp_dir().join(format!("leaflog-backup-{tag}"));
        let _ = std::fs::remove_dir_all(&root);
        DataTree { root }
    }

    fn db_path(&self) -> PathBuf {
        self.root.join("leaflog.db")
    }

    fn photos_dir(&self) -> PathBuf {
        self.root.join("photos")
    }
}

/// Spawns the API with data in the given tree.
async fn spawn(tree: &DataTree) -> String {
    let fixture = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/dist");
    let db = Db::open(&tree.db_path()).unwrap();
    let photos = PhotoStore::new(tree.photos_dir());
    let bridge = std::sync::Arc::new(leaflog_backend::ha::HaBridge::spawn(
        "http://127.0.0.1:1".to_owned(),
        leaflog_backend::ha::SupervisorToken::new("test"),
        Db::open_in_memory().unwrap(),
    ));
    let state = AppState::new(Assets::Disk(fixture), Mode::Dev, None, db, photos, bridge);
    let make = app::service(state).into_make_service_with_connect_info::<SocketAddr>();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, make).await.unwrap();
    });
    format!("http://{addr}")
}

/// A valid PNG for photo upload (64×64 gradient).
fn valid_png() -> Vec<u8> {
    use std::io::Write;
    let mut png = Vec::new();
    // Minimal PNG header + IHDR + IDAT + IEND (solid color)
    png.extend_from_slice(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]);
    // IHDR: 64x64, 8-bit RGB
    png.extend_from_slice(&[0, 0, 0, 13]); // length
    png.extend_from_slice(b"IHDR");
    png.extend_from_slice(&64u32.to_be_bytes());
    png.extend_from_slice(&64u32.to_be_bytes());
    png.extend_from_slice(&[8, 2, 0, 0, 0]); // 8-bit, color type 2
    let crc = crc32(&png[12..]);
    png.extend_from_slice(&crc.to_be_bytes());
    // IDAT: compressed pixel data (solid blue)
    let mut raw = Vec::new();
    for _y in 0..64 {
        raw.push(0); // filter none
        for _x in 0..64 {
            raw.extend_from_slice(&[50, 150, 200]);
        }
    }
    let compressed = compress_zlib(&raw);
    png.extend_from_slice(&(compressed.len() as u32 + 6).to_be_bytes()[..4]);
    // Actually IDAT length is the compressed data length
    png.truncate(png.len() - 4); // remove the wrong length
    png.extend_from_slice(&(compressed.len() as u32).to_be_bytes());
    png.extend_from_slice(b"IDAT");
    png.extend_from_slice(&compressed);
    let crc2 = crc32(&png[png.len() - compressed.len() - 4..]);
    png.extend_from_slice(&crc2.to_be_bytes());
    // IEND
    png.extend_from_slice(&[0, 0, 0, 0]);
    png.extend_from_slice(b"IEND");
    let crc3 = crc32(&png[png.len() - 4..]);
    png.extend_from_slice(&crc3.to_be_bytes());
    let _ = std::io::stdout().flush();
    png
}

/// Minimal CRC32.
fn crc32(data: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFF_u32;
    for byte in data {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xEDB8_8320;
            } else {
                crc >>= 1;
            }
        }
    }
    !crc
}

/// Minimal zlib (stored blocks).
fn compress_zlib(data: &[u8]) -> Vec<u8> {
    let mut out = vec![0x78, 0x01]; // zlib header
    // Stored block (uncompressed)
    out.push(0x01); // final block, stored
    out.extend_from_slice(&(data.len() as u16).to_be_bytes());
    out.extend_from_slice(&(!(data.len() as u16)).to_be_bytes());
    out.extend_from_slice(data);
    // Adler-32
    let mut a = 1u32;
    let mut b = 0u32;
    for byte in data {
        a = (a + u32::from(*byte)) % 65521;
        b = (b + a) % 65521;
    }
    out.extend_from_slice(&((b << 16) | a).to_be_bytes());
    out
}

#[tokio::test]
async fn backup_restore_keeps_db_and_photos() {
    let original = DataTree::create("original");
    let backup = DataTree::create("backup");
    let restored = DataTree::create("restored");

    // --- 1. Create data in the original tree ---
    let base = spawn(&original).await;
    let client = reqwest::Client::new();

    // Create a grow.
    let grow: Value = client
        .post(format!("{base}/api/grows"))
        .json(&json!({
            "name": "Backup test", "strain_id": null, "substrate": "soil",
            "started_on": "2026-10-01", "status": "active", "notes": null,
        }))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let grow_id = grow["id"].as_str().unwrap().to_owned();

    // Log an action.
    client
        .post(format!("{base}/api/grows/{grow_id}/actions"))
        .json(&json!({
            "plant_ids": [], "occurred_at": "2026-11-08T10:00:00Z",
            "action_type": "water", "details": {"volume_ml": 500},
            "notes": "pre-backup", "appearance_tags": [],
        }))
        .send()
        .await
        .unwrap();

    // Upload a photo.
    let photo: Value = client
        .post(format!("{base}/api/grows/{grow_id}/photos"))
        .multipart(
            reqwest::multipart::Form::new().part(
                "file",
                reqwest::multipart::Part::bytes(valid_png())
                    .file_name("test.png")
                    .mime_str("image/png")
                    .unwrap(),
            ),
        )
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    let photo_id = photo["id"].as_str().unwrap().to_owned();
    let original_name = photo["original_name"].as_str().unwrap().to_owned();

    // Verify the photo file exists.
    let original_file = original
        .photos_dir()
        .join("originals")
        .join(&grow_id)
        .join(&original_name);
    assert!(original_file.exists(), "original photo file must exist");

    // --- 2. Simulate a HA backup ---
    // HA backs up /data; `backup_exclude: photos/thumbnails` skips thumbs.
    std::fs::create_dir_all(&backup.root).unwrap();
    std::fs::copy(original.db_path(), backup.db_path()).unwrap();
    // The WAL/SHM files also need copying for a consistent snapshot.
    for ext in ["-wal", "-shm"] {
        let src = original.db_path().with_extension(format!("db{ext}"));
        if src.exists() {
            std::fs::copy(&src, backup.db_path().with_extension(format!("db{ext}"))).unwrap();
        }
    }
    // Copy photos/originals (thumbnails excluded by backup_exclude).
    copy_dir(
        &original.photos_dir().join("originals"),
        &backup.photos_dir().join("originals"),
    );

    // --- 3. Simulate a restore ---
    std::fs::create_dir_all(&restored.root).unwrap();
    std::fs::copy(backup.db_path(), restored.db_path()).unwrap();
    for ext in ["-wal", "-shm"] {
        let src = backup.db_path().with_extension(format!("db{ext}"));
        if src.exists() {
            std::fs::copy(&src, restored.db_path().with_extension(format!("db{ext}"))).unwrap();
        }
    }
    copy_dir(
        &backup.photos_dir().join("originals"),
        &restored.photos_dir().join("originals"),
    );

    // --- 4. Prove the restored tree works ---
    let restored_base = spawn(&restored).await;
    let restored_client = reqwest::Client::new();

    // DB data survived.
    let grows: Vec<Value> = restored_client
        .get(format!("{restored_base}/api/grows"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(grows.len(), 1, "grow must survive backup→restore");
    assert_eq!(grows[0]["name"], "Backup test");

    // The action log survived.
    let actions: Vec<Value> = restored_client
        .get(format!("{restored_base}/api/grows/{grow_id}/actions"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert!(!actions.is_empty(), "actions must survive");
    assert_eq!(actions[0]["notes"], "pre-backup");

    // The photo row and file survived.
    let photos: Vec<Value> = restored_client
        .get(format!("{restored_base}/api/grows/{grow_id}/photos"))
        .send()
        .await
        .unwrap()
        .json()
        .await
        .unwrap();
    assert_eq!(photos.len(), 1, "photo row must survive");
    assert_eq!(photos[0]["id"], photo_id);

    let restored_file = restored
        .photos_dir()
        .join("originals")
        .join(&grow_id)
        .join(&original_name);
    assert!(restored_file.exists(), "photo file must survive restore");

    // The original serves correctly from the restored tree.
    let image = restored_client
        .get(format!("{restored_base}/api/photos/{photo_id}/original"))
        .send()
        .await
        .unwrap();
    assert_eq!(image.status(), 200);
    assert_eq!(image.headers()[reqwest::header::CONTENT_TYPE], "image/jpeg");

    // The thumbnail regenerates on demand (it was excluded from backup).
    let thumb = restored_client
        .get(format!("{restored_base}/api/photos/{photo_id}/thumbnail"))
        .send()
        .await
        .unwrap();
    assert_eq!(thumb.status(), 200, "thumbnail must regenerate lazily");
    assert_eq!(thumb.headers()[reqwest::header::CONTENT_TYPE], "image/webp");

    // Cleanup.
    for tree in [&original, &backup, &restored] {
        let _ = std::fs::remove_dir_all(&tree.root);
    }
}

/// Recursively copies a directory.
fn copy_dir(src: &Path, dst: &Path) {
    std::fs::create_dir_all(dst).unwrap();
    for entry in std::fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let target = dst.join(entry.file_name());
        if entry.path().is_dir() {
            copy_dir(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).unwrap();
        }
    }
}
