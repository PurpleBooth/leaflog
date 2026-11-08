//! Serving the compiled frontend (SPA) from an embedded bundle or disk.
//!
//! Release builds embed the Trunk output directory into the binary (see
//! the `embed-assets` feature and `docs/DECISIONS.md` D003), so the HA
//! image ships one static artifact. Development builds serve the same
//! layout from disk so `trunk watch` rebuilds appear without recompiling
//! the backend.

use std::path::PathBuf;

#[cfg(feature = "embed-assets")]
use include_dir::{Dir, include_dir};

/// The embedded frontend bundle (empty until `trunk build` has run; see
/// `build.rs`).
#[cfg(feature = "embed-assets")]
static DIST: Dir<'_> = include_dir!("$CARGO_MANIFEST_DIR/../leaflog-frontend/dist");

/// Where frontend assets are served from.
#[derive(Debug, Clone)]
pub enum Assets {
    /// The bundle embedded at compile time (release images).
    #[cfg(feature = "embed-assets")]
    Embedded,
    /// A directory on disk (development and tests).
    Disk(PathBuf),
}

impl Assets {
    /// The embedded bundle; the disk variant is used in development.
    #[cfg(feature = "embed-assets")]
    #[must_use]
    pub const fn embedded() -> Self {
        Self::Embedded
    }

    /// Reads a file by path relative to the frontend root, for example
    /// `pkg/app.js`. Returns `None` for missing or unsafe paths.
    ///
    /// # Errors
    ///
    /// This is an `async fn` for disk-backed sources; missing files are
    /// reported as `None` rather than as errors.
    pub async fn get(&self, path: &str) -> Option<Vec<u8>> {
        let clean = sanitize(path)?;
        match self {
            #[cfg(feature = "embed-assets")]
            Self::Embedded => DIST.get_file(&clean).map(|f| f.contents().to_vec()),
            Self::Disk(root) => tokio::fs::read(root.join(&clean)).await.ok(),
        }
    }

    /// The `index.html` template, when the frontend has been built.
    pub async fn index(&self) -> Option<String> {
        let bytes = self.get("index.html").await?;
        String::from_utf8(bytes).ok()
    }
}

/// Accepts relative paths of safe characters only (no traversal).
fn sanitize(path: &str) -> Option<String> {
    if path.is_empty() || path.starts_with('/') || path.contains("..") {
        return None;
    }
    if path
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '.' | '-' | '_'))
    {
        Some(path.to_owned())
    } else {
        None
    }
}

/// Maps a filename extension to its `Content-Type`.
#[must_use]
pub fn content_type(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or_default();
    match ext {
        "html" | "htm" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json",
        "wasm" => "application/wasm",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "svg" => "image/svg+xml",
        "webp" => "image/webp",
        "ico" => "image/x-icon",
        "woff" => "font/woff",
        "woff2" => "font/woff2",
        "txt" => "text/plain; charset=utf-8",
        _ => "application/octet-stream",
    }
}

#[cfg(test)]
mod tests {
    use super::{content_type, sanitize};

    #[test]
    fn sanitize_rejects_traversal_and_absolute() {
        assert_eq!(sanitize("pkg/app.js").as_deref(), Some("pkg/app.js"));
        assert_eq!(sanitize("index.html").as_deref(), Some("index.html"));
        assert!(sanitize("/etc/passwd").is_none());
        assert!(sanitize("../secret").is_none());
        assert!(sanitize("a/b%20c").is_none());
        assert!(sanitize("").is_none());
    }

    #[test]
    fn content_types_by_extension() {
        assert_eq!(content_type("a/b/app.wasm"), "application/wasm");
        assert_eq!(content_type("styles.css"), "text/css; charset=utf-8");
        assert_eq!(content_type("unknown.blob"), "application/octet-stream");
        assert_eq!(content_type("noext"), "application/octet-stream");
    }
}
