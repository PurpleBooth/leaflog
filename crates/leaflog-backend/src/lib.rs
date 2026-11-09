//! Leaflog backend: the Axum server that hosts the ingress-correct SPA,
//! the JSON API, and (from milestone M4) the Home Assistant Supervisor
//! link.
//!
//! # Ingress
//!
//! Home Assistant serves this app under
//! `/api/hassio_ingress/<token>/`. The backend therefore never bakes a
//! base path into anything: it reads the `X-Ingress-Path` header per
//! request and injects a `<base href>` plus a `window.__LEAFLOG_BASE__`
//! global into the served `index.html` at runtime (see [`ingress`]). All
//! built asset references are relative.
//!
//! # Runtime configuration (environment variables)
//!
//! - `LEAFLOG_PORT` — TCP port (default `8099`, the HA `ingress_port`).
//! - `LEAFLOG_ENV` — `dev` or `prod`. Defaults to production when
//!   `SUPERVISOR_TOKEN` is present, development otherwise.
//! - `LEAFLOG_DEV_PREFIX` — simulated ingress prefix for development
//!   (served paths under it are stripped, and it doubles as the base
//!   when no `X-Ingress-Path` header is sent).
//! - `LEAFLOG_STATIC_DIR` — serve the frontend from this directory
//!   (development) instead of the embedded bundle.

#![warn(clippy::nursery)]
#![deny(
    unused,
    nonstandard_style,
    future_incompatible,
    missing_copy_implementations,
    missing_debug_implementations,
    missing_docs,
    clippy::cargo,
    clippy::complexity,
    clippy::correctness,
    clippy::perf,
    clippy::style,
    clippy::suspicious,
    clippy::pedantic,
    non_fmt_panics
)]
#![allow(clippy::multiple_crate_versions)]

pub mod api;
pub mod api_env;
pub mod api_feeds;
pub mod api_grows;
pub mod api_ha;
pub mod app;
pub mod assets;
pub mod db;
pub mod ha;
pub mod ingress;
pub mod photos;
pub mod seed;
pub mod store;

use std::path::PathBuf;

pub use app::{AppState, Mode, build_router};
pub use assets::Assets;
use axum::ServiceExt as _;
pub use db::Db;

/// Default TCP port; matches `ingress_port` in the HA app `config.yaml`.
pub const DEFAULT_PORT: u16 = 8099;

/// Runtime configuration resolved from the environment.
#[derive(Debug, Clone)]
pub struct Config {
    /// TCP port to bind.
    pub port: u16,
    /// Development or production mode (see [`Mode`]).
    pub mode: Mode,
    /// Simulated ingress prefix, development only (for example
    /// `/api/hassio_ingress/dev-1234`).
    pub dev_prefix: Option<String>,
    /// Directory to serve the frontend from (development). When `None`,
    /// the embedded bundle compiled behind the `embed-assets` feature is
    /// used.
    pub static_dir: Option<PathBuf>,
}

impl Config {
    /// Reads the configuration from environment variables, falling back
    /// to the documented defaults. Invalid values (for example a
    /// non-numeric port) fall back to the default rather than failing to
    /// start.
    #[must_use]
    pub fn from_env() -> Self {
        let port = std::env::var("LEAFLOG_PORT")
            .ok()
            .and_then(|p| p.parse().ok())
            .unwrap_or(DEFAULT_PORT);
        let mode = match std::env::var("LEAFLOG_ENV")
            .unwrap_or_default()
            .to_ascii_lowercase()
            .as_str()
        {
            "prod" | "production" => Mode::Production,
            "dev" | "development" => Mode::Dev,
            _ => {
                if std::env::var_os("SUPERVISOR_TOKEN").is_some() {
                    Mode::Production
                } else {
                    Mode::Dev
                }
            }
        };
        Self {
            port,
            mode,
            dev_prefix: std::env::var("LEAFLOG_DEV_PREFIX")
                .ok()
                .filter(|p| !p.is_empty()),
            static_dir: match std::env::var("LEAFLOG_STATIC_DIR")
                .ok()
                .filter(|d| !d.is_empty())
            {
                Some(dir) => Some(PathBuf::from(dir)),
                // Without the embed-assets feature there is no embedded
                // bundle; default to the conventional Trunk output path
                // relative to the workspace root.
                None if cfg!(feature = "embed-assets") => None,
                None => Some(PathBuf::from("crates/leaflog-frontend/dist")),
            },
        }
    }

    /// Resolves where frontend assets come from: a directory when
    /// configured (development), the embedded bundle otherwise.
    ///
    /// # Panics
    ///
    /// Panics when no static directory is configured and the
    /// `embed-assets` feature is disabled (there is nothing to serve).
    #[must_use]
    pub fn assets(&self) -> assets::Assets {
        match &self.static_dir {
            Some(dir) => assets::Assets::Disk(dir.clone()),
            None => {
                #[cfg(feature = "embed-assets")]
                return assets::Assets::embedded();
                #[cfg(not(feature = "embed-assets"))]
                panic!("LEAFLOG_STATIC_DIR is required when embed-assets is disabled");
            }
        }
    }

    /// Root of the photo store: `LEAFLOG_PHOTOS_DIR` overrides;
    /// otherwise `/data/photos` in production and `data/photos` in
    /// development.
    #[must_use]
    pub fn photos_dir(&self) -> PathBuf {
        std::env::var("LEAFLOG_PHOTOS_DIR")
            .ok()
            .filter(|path| !path.is_empty())
            .map_or_else(
                || match self.mode {
                    Mode::Production => PathBuf::from("/data/photos"),
                    Mode::Dev => PathBuf::from("data/photos"),
                },
                PathBuf::from,
            )
    }

    /// Supervisor base URL: `LEAFLOG_SUPERVISOR_URL` overrides;
    /// otherwise `http://supervisor` in production and the local mock
    /// (`http://127.0.0.1:8098`) in development.
    #[must_use]
    pub fn supervisor_url(&self) -> String {
        std::env::var("LEAFLOG_SUPERVISOR_URL")
            .ok()
            .filter(|url| !url.is_empty())
            .unwrap_or_else(|| match self.mode {
                Mode::Production => "http://supervisor".to_owned(),
                Mode::Dev => "http://127.0.0.1:8098".to_owned(),
            })
    }

    /// The Supervisor token (dev default is a harmless placeholder).
    #[must_use]
    pub fn supervisor_token(&self) -> ha::SupervisorToken {
        let token = std::env::var("SUPERVISOR_TOKEN").unwrap_or_else(|_| "dev-token".to_owned());
        ha::SupervisorToken::new(&token)
    }

    /// Path of the SQLite database: `LEAFLOG_DB_PATH` overrides;
    /// otherwise `/data/leaflog.db` in production (the Supervisor maps
    /// the add-on `data` volume there) and `data/leaflog.db` relative to
    /// the working directory in development.
    #[must_use]
    pub fn db_path(&self) -> PathBuf {
        std::env::var("LEAFLOG_DB_PATH")
            .ok()
            .filter(|path| !path.is_empty())
            .map_or_else(
                || match self.mode {
                    Mode::Production => PathBuf::from("/data/leaflog.db"),
                    Mode::Dev => PathBuf::from("data/leaflog.db"),
                },
                PathBuf::from,
            )
    }
}

/// Runs the backend server until shutdown (Ctrl-C or SIGTERM).
///
/// # Errors
///
/// Returns an error when the async runtime cannot be created, the TCP
/// listener cannot be bound, or installing the SIGTERM handler fails.
pub fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = Config::from_env();
    tokio::runtime::Runtime::new()?.block_on(serve(config))
}

/// Binds the listener and serves until a shutdown signal arrives.
///
/// # Errors
///
/// Returns an error if the listener cannot be bound or the server loop
/// fails.
async fn serve(config: Config) -> Result<(), Box<dyn std::error::Error>> {
    let mode = config.mode;
    let db = Db::open(&config.db_path())?;
    match seed::seed_strains_if_empty(&db).await {
        Ok(seeded) if seeded > 0 => {
            println!("Seeded {seeded} bundled strain records");
        }
        Ok(_) => {}
        Err(error) => eprintln!("strain seeding skipped: {error}"),
    }
    match seed::seed_equipment_if_empty(&db).await {
        Ok(seeded) if seeded > 0 => {
            println!("Seeded {seeded} predefined equipment items");
        }
        Ok(_) => {}
        Err(error) => eprintln!("equipment seeding skipped: {error}"),
    }
    let photos = crate::photos::PhotoStore::new(config.photos_dir());
    let ha_bridge = std::sync::Arc::new(ha::HaBridge::spawn(
        config.supervisor_url(),
        config.supervisor_token(),
        db.clone(),
    ));
    // Track exactly what any grow has bound.
    if let Ok(entities) = db.with(crate::store::sensors::all_bound_entities).await {
        ha_bridge.track_entities(entities);
    }
    let state = AppState::new(
        config.assets(),
        mode,
        config.dev_prefix.clone(),
        db,
        photos,
        ha_bridge,
    );
    let app = app::service(state).into_make_service_with_connect_info::<std::net::SocketAddr>();
    let listener = tokio::net::TcpListener::bind(("0.0.0.0", config.port)).await?;
    println!(
        "Leaflog backend listening on 0.0.0.0:{} ({mode:?} mode)",
        config.port
    );

    let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())?;
    let shutdown = async move {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {},
            _ = sigterm.recv() => {},
        }
    };
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown)
        .await?;
    Ok(())
}
