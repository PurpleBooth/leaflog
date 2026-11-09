//! Photo storage pipeline (spec §4.3).
//!
//! Originals are re-encoded without EXIF/GPS under
//! `photos/originals/<grow>/`, WebP thumbnails under
//! `photos/thumbnails/<grow>/` (the thumbnail tree is excluded from
//! backups and regenerated on demand). `originals/` and `thumbnails/`
//! are top-level so the app `backup_exclude` entry `photos/thumbnails`
//! covers every grow at once.

use std::io::Cursor;
use std::path::{Path, PathBuf};

use image::{DynamicImage, ImageEncoder as _, ImageFormat};

/// Thumbnail edge length in pixels (aspect preserved).
const THUMBNAIL_EDGE: u32 = 512;

/// JPEG re-encode quality for stripped originals.
const ORIGINAL_QUALITY: u8 = 95;

/// A photo-pipeline failure.
#[derive(Debug, thiserror::Error)]
pub enum PhotoError {
    /// The bytes are not a decodable image (or an unsupported format).
    #[error("unsupported or corrupt image: {0}")]
    Decode(String),
    /// An I/O failure while reading or writing files.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// What [`PhotoStore::store`] produced.
#[derive(Debug, Clone)]
pub struct StoredPhoto {
    /// File name under `photos/originals/<grow>/` (always `.jpg`: the
    /// re-encode normalizes every input to JPEG).
    pub original_name: String,
    /// File name under `photos/thumbnails/<grow>/`.
    pub thumbnail_name: String,
    /// Capture time from EXIF `DateTimeOriginal` when present,
    /// RFC 3339 UTC; `None` when the file carried no usable timestamp.
    pub captured_at: Option<String>,
    /// Size of the stored (stripped) original in bytes.
    pub bytes_stored: u64,
}

/// The on-disk photo store rooted at a configurable directory
/// (`photos_dir` in [`crate::Config`]; `/data/photos` in production).
#[derive(Debug, Clone)]
pub struct PhotoStore {
    root: PathBuf,
}

impl PhotoStore {
    /// Creates the store handle. Directories are created on demand by
    /// [`Self::store`], so a missing tree (for example right after a
    /// restore) is not an error.
    #[must_use]
    pub const fn new(root: PathBuf) -> Self {
        Self { root }
    }

    /// Ensures the `originals/` and `thumbnails/` subtrees exist for a
    /// grow (empty grow id = the top level).
    fn ensure_dirs(&self, grow_id: &str) -> Result<(), PhotoError> {
        std::fs::create_dir_all(self.originals_dir(grow_id))?;
        std::fs::create_dir_all(self.thumbnails_dir(grow_id))?;
        Ok(())
    }

    /// `photos/originals[/<grow>]`.
    #[must_use]
    pub fn originals_dir(&self, grow_id: &str) -> PathBuf {
        joined(&self.root, "originals", grow_id)
    }

    /// `photos/thumbnails[/<grow>]`.
    #[must_use]
    pub fn thumbnails_dir(&self, grow_id: &str) -> PathBuf {
        joined(&self.root, "thumbnails", grow_id)
    }

    /// Full path of a stored original.
    #[must_use]
    pub fn original_path(&self, grow_id: &str, name: &str) -> PathBuf {
        safe_join(&self.originals_dir(grow_id), name)
    }

    /// Full path of a stored thumbnail.
    #[must_use]
    pub fn thumbnail_path(&self, grow_id: &str, name: &str) -> PathBuf {
        safe_join(&self.thumbnails_dir(grow_id), name)
    }

    /// Strips metadata, writes the original and a WebP thumbnail, and
    /// returns the storage facts. All-or-nothing at the file level: if
    /// the thumbnail cannot be written, the already-written original is
    /// removed again, so a failed `store` leaves no files behind.
    ///
    /// # Errors
    ///
    /// Returns a [`PhotoError`] for undecodable input or I/O failures.
    pub fn store(&self, grow_id: &str, bytes: &[u8]) -> Result<StoredPhoto, PhotoError> {
        self.ensure_dirs(grow_id)?;
        let captured_at = exif_timestamp(bytes);
        // Normalize every input (RGBA/grayscale/…) to plain 8-bit RGB up
        // front: the re-encode through the pixel buffer guarantees no
        // EXIF/GPS or other metadata survives, whatever the input was,
        // and every later step works on one known color type.
        let rgb = decode(bytes)?.to_rgb8();
        let mut original = Vec::new();
        let mut encoder =
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut original, ORIGINAL_QUALITY);
        encoder
            .encode(
                rgb.as_raw(),
                rgb.width(),
                rgb.height(),
                image::ExtendedColorType::Rgb8,
            )
            .map_err(|error| PhotoError::Decode(error.to_string()))?;

        let id = crate::db::new_id();
        let original_name = format!("{id}.jpg");
        std::fs::write(self.original_path(grow_id, &original_name), &original)?;

        let thumbnail_name = format!("{id}.webp");
        if let Err(error) = self.write_thumbnail(&rgb, grow_id, &thumbnail_name) {
            // Never leave a half-written pair behind.
            let _ = std::fs::remove_file(self.original_path(grow_id, &original_name));
            return Err(error);
        }

        Ok(StoredPhoto {
            bytes_stored: original.len() as u64,
            original_name,
            thumbnail_name,
            captured_at,
        })
    }

    /// Writes the 512-px WebP thumbnail for an RGB image.
    fn write_thumbnail(
        &self,
        image: &image::RgbImage,
        grow_id: &str,
        name: &str,
    ) -> Result<(), PhotoError> {
        let thumb = image::imageops::thumbnail(image, THUMBNAIL_EDGE, THUMBNAIL_EDGE);
        let mut out = Vec::new();
        let encoder = image::codecs::webp::WebPEncoder::new_lossless(&mut out);
        encoder
            .write_image(
                thumb.as_raw(),
                thumb.width(),
                thumb.height(),
                image::ExtendedColorType::Rgb8,
            )
            .map_err(|error| PhotoError::Decode(error.to_string()))?;
        std::fs::write(self.thumbnail_path(grow_id, name), &out)?;
        Ok(())
    }

    /// Reads a stored original.
    ///
    /// # Errors
    ///
    /// Returns a [`PhotoError`] when the file cannot be read.
    pub fn read_original(&self, grow_id: &str, name: &str) -> Result<Vec<u8>, PhotoError> {
        Ok(std::fs::read(self.original_path(grow_id, name))?)
    }

    /// Reads a thumbnail, regenerating it from the original when the
    /// file is missing (backup restores skip thumbnails; spec §4.3).
    ///
    /// # Errors
    ///
    /// Returns a [`PhotoError`] when neither file can be produced.
    pub fn read_thumbnail(&self, grow_id: &str, name: &str) -> Result<Vec<u8>, PhotoError> {
        let path = self.thumbnail_path(grow_id, name);
        if path.exists() {
            return Ok(std::fs::read(path)?);
        }
        // Regenerate: ensure the thumbnails directory exists (it may be
        // missing after a backup restore).
        std::fs::create_dir_all(self.thumbnails_dir(grow_id))?;
        let original_name = format!("{}.jpg", name.strip_suffix(".webp").unwrap_or(name));
        let bytes = self.read_original(grow_id, &original_name)?;
        let rgb = decode(&bytes)?.to_rgb8();
        self.write_thumbnail(&rgb, grow_id, name)?;
        Ok(std::fs::read(path)?)
    }

    /// Removes a photo's files (best effort; row deletion is the
    /// caller's job).
    pub fn remove(&self, grow_id: &str, original: &str, thumbnail: Option<&str>) {
        let _ = std::fs::remove_file(self.original_path(grow_id, original));
        if let Some(thumbnail) = thumbnail {
            let _ = std::fs::remove_file(self.thumbnail_path(grow_id, thumbnail));
        }
    }

    /// Removes a grow's whole photo tree (grow deletion).
    pub fn remove_grow(&self, grow_id: &str) {
        let _ = std::fs::remove_dir_all(self.originals_dir(grow_id));
        let _ = std::fs::remove_dir_all(self.thumbnails_dir(grow_id));
    }
}

/// Joins non-empty path components.
fn joined(root: &Path, kind: &str, grow_id: &str) -> PathBuf {
    if grow_id.is_empty() {
        root.join(kind)
    } else {
        root.join(kind).join(grow_id)
    }
}

/// Joins a file name, refusing traversal (`..`, absolute names).
fn safe_join(dir: &Path, name: &str) -> PathBuf {
    if name.contains("..") || name.starts_with('/') {
        return dir.join("invalid");
    }
    dir.join(name)
}

/// Decodes and auto-orients an uploaded image.
fn decode(bytes: &[u8]) -> Result<DynamicImage, PhotoError> {
    let format =
        image::guess_format(bytes).map_err(|error| PhotoError::Decode(error.to_string()))?;
    // Accept the formats we can safely re-encode; animated inputs take
    // their first frame.
    if !matches!(
        format,
        ImageFormat::Jpeg | ImageFormat::Png | ImageFormat::WebP
    ) {
        return Err(PhotoError::Decode(format!(
            "unsupported format {format:?}; use JPEG, PNG or WebP"
        )));
    }
    image::load_from_memory_with_format(bytes, format)
        .map_err(|error| PhotoError::Decode(error.to_string()))
}

/// Extracts EXIF `DateTimeOriginal` as RFC 3339, if present and
/// parseable. GPS and everything else is deliberately ignored (and
/// dropped by the re-encode).
fn exif_timestamp(bytes: &[u8]) -> Option<String> {
    let exif = exif::Reader::new()
        .read_from_container(&mut Cursor::new(bytes))
        .ok()?;
    let field = exif.get_field(exif::Tag::DateTimeOriginal, exif::In::PRIMARY)?;
    let exif::Value::Ascii(ascii) = &field.value else {
        return None;
    };
    let value = std::str::from_utf8(ascii.first()?).ok()?.trim();
    // EXIF format: "YYYY:MM:DD HH:MM:SS" (local camera time; no zone
    // in DateTimeOriginal - stored as naive UTC, documented in D018).
    let mut parts = value.split_whitespace();
    let date = parts.next()?;
    let time = parts.next()?;
    let mut date = date.split(':');
    let mut time = time.split(':');
    let (year, month, day) = (date.next()?, date.next()?, date.next()?);
    let (hour, minute, second) = (time.next()?, time.next()?, time.next()?);
    Some(format!("{year}-{month}-{day}T{hour}:{minute}:{second}Z"))
}

#[cfg(test)]
mod tests {
    use super::{PhotoStore, exif_timestamp};

    /// A tiny valid JPEG (8x8) with a well-formed EXIF APP1 segment
    /// carrying `DateTimeOriginal`, built with the exif crate's own writer.
    fn jpeg_with_exif() -> Vec<u8> {
        let img = image::DynamicImage::new_rgb8(8, 8);
        let mut plain = Vec::new();
        image::codecs::jpeg::JpegEncoder::new(&mut plain)
            .encode(img.to_rgb8().as_raw(), 8, 8, image::ExtendedColorType::Rgb8)
            .expect("encoding a solid image cannot fail");

        let field = exif::Field {
            tag: exif::Tag::DateTimeOriginal,
            ifd_num: exif::In::PRIMARY,
            value: exif::Value::Ascii(vec![b"2026:11:08 10:20:30".to_vec()]),
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

    #[test]
    fn exif_timestamp_is_parsed() {
        assert_eq!(
            exif_timestamp(&jpeg_with_exif()).as_deref(),
            Some("2026-11-08T10:20:30Z")
        );
        // No EXIF in a plain JPEG.
        let mut plain = Vec::new();
        let img = image::DynamicImage::new_rgb8(4, 4);
        image::codecs::jpeg::JpegEncoder::new(&mut plain)
            .encode(img.to_rgb8().as_raw(), 4, 4, image::ExtendedColorType::Rgb8)
            .expect("encoding cannot fail");
        assert_eq!(exif_timestamp(&plain), None);
    }

    #[test]
    fn store_strips_metadata_and_writes_webp_thumbnail() {
        let dir = std::env::temp_dir().join(format!("leaflog-photos-{}", crate::db::new_id()));
        let store = PhotoStore::new(dir.clone());
        let stored = store
            .store("grow1", &jpeg_with_exif())
            .expect("valid jpeg stores");

        assert_eq!(
            std::path::Path::new(&stored.original_name).extension(),
            Some(std::ffi::OsStr::new("jpg"))
        );
        assert_eq!(
            std::path::Path::new(&stored.thumbnail_name).extension(),
            Some(std::ffi::OsStr::new("webp"))
        );
        assert_eq!(stored.captured_at.as_deref(), Some("2026-11-08T10:20:30Z"));
        assert!(stored.bytes_stored > 0);

        let original = store.read_original("grow1", &stored.original_name).unwrap();
        assert!(!contains_exif(&original), "EXIF must be stripped");
        assert!(
            !original.windows(4).any(|window| window == b"GPS\0"),
            "GPS block must be gone"
        );

        let thumb = store
            .read_thumbnail("grow1", &stored.thumbnail_name)
            .unwrap();
        assert_eq!(&thumb[..4], b"RIFF", "WebP container magic");
        assert_eq!(&thumb[8..12], b"WEBP");

        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn thumbnail_regenerates_after_loss() {
        let dir = std::env::temp_dir().join(format!("leaflog-photos-{}", crate::db::new_id()));
        let store = PhotoStore::new(dir.clone());
        let stored = store.store("grow1", &jpeg_with_exif()).unwrap();
        std::fs::remove_file(store.thumbnail_path("grow1", &stored.thumbnail_name)).unwrap();
        let thumb = store
            .read_thumbnail("grow1", &stored.thumbnail_name)
            .expect("thumbnail regenerates on demand");
        assert_eq!(&thumb[..4], b"RIFF");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn stores_rgba_png_and_grayscale_jpeg_without_panic() {
        for (label, bytes) in [
            ("rgba-png", png_with_alpha()),
            ("gray-jpeg", grayscale_jpeg()),
        ] {
            let dir = std::env::temp_dir()
                .join(format!("leaflog-photos-{}-{label}", crate::db::new_id()));
            let store = PhotoStore::new(dir.clone());
            let stored = store
                .store("grow1", &bytes)
                .unwrap_or_else(|error| panic!("{label} must store, got {error}"));
            let original = store.read_original("grow1", &stored.original_name).unwrap();
            assert!(!contains_exif(&original), "{label}: EXIF must be stripped");
            let thumb = store
                .read_thumbnail("grow1", &stored.thumbnail_name)
                .unwrap();
            assert_eq!(&thumb[..4], b"RIFF", "{label}: WebP thumbnail");
            std::fs::remove_dir_all(&dir).ok();
        }
    }

    /// An 8-bit RGBA PNG (decodes to `ImageRgba8`).
    fn png_with_alpha() -> Vec<u8> {
        use image::ImageEncoder as _;
        let img = image::DynamicImage::new_rgba8(8, 8);
        let mut out = Vec::new();
        image::codecs::png::PngEncoder::new(&mut out)
            .write_image(
                img.to_rgba8().as_raw(),
                8,
                8,
                image::ExtendedColorType::Rgba8,
            )
            .expect("encoding cannot fail");
        out
    }

    /// An 8-bit grayscale JPEG (decodes to `ImageLuma8`).
    fn grayscale_jpeg() -> Vec<u8> {
        let img = image::DynamicImage::new_luma8(8, 8);
        let mut out = Vec::new();
        image::codecs::jpeg::JpegEncoder::new(&mut out)
            .encode(img.to_luma8().as_raw(), 8, 8, image::ExtendedColorType::L8)
            .expect("encoding cannot fail");
        out
    }

    #[test]
    fn rejects_non_images() {
        let dir = std::env::temp_dir().join(format!("leaflog-photos-{}", crate::db::new_id()));
        let store = PhotoStore::new(dir.clone());
        let error = store
            .store("grow1", b"definitely not an image")
            .unwrap_err();
        assert!(error.to_string().contains("unsupported"));
        std::fs::remove_dir_all(&dir).ok();
    }

    /// True when the bytes carry an Exif APP1 marker.
    fn contains_exif(jpeg: &[u8]) -> bool {
        jpeg.windows(6).any(|window| window == b"Exif\0\0")
    }
}
