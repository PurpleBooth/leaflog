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
pub mod api_history;
pub mod api_seedbank;
pub mod api_spaces;
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

/// Seven days, in the only unit std's `Duration` constructors speak
/// on stable.
const fn weeks(count: u64) -> std::time::Duration {
    std::time::Duration::from_secs(count * 7 * 24 * 60 * 60)
}

/// Runs the `migrate` subcommand (spec §3).
///
/// `apply` applies pending migrations to the configured database
/// (boot-time apply stays the normal path; this exists for standalone
/// runs); `status` prints the applied and pending set without changing
/// anything.
///
/// # Errors
///
/// Returns an error when the subcommand is unknown or the database
/// cannot be opened or migrated — including a refusal when the applied
/// history diverges from the embedded migration set.
pub fn run_migrate(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let path = Config::from_env().db_path();
    match args {
        [command] if command == "apply" => {
            let applied = db::migrate_apply(&path)?;
            if applied.is_empty() {
                println!("{}: already at the latest schema version", path.display());
            } else {
                println!(
                    "{}: applied {} migration(s):",
                    path.display(),
                    applied.len()
                );
                for name in applied {
                    println!("  - {name}");
                }
            }
            Ok(())
        }
        [command] if command == "status" => {
            let status = db::migrate_status(&path)?;
            println!("{}:", path.display());
            if let Some(legacy) = status.legacy_version {
                println!("  pre-refinery history at version {legacy}; it will be adopted on apply");
            }
            if status.applied.is_empty() && status.legacy_version.is_none() {
                println!("  no migrations applied yet");
            }
            for migration in &status.applied {
                let when = migration.applied_on.as_deref().unwrap_or("unknown date");
                println!(
                    "  applied V{:0>3} {} ({when})",
                    migration.version, migration.name
                );
            }
            if status.pending.is_empty() {
                println!("  no pending migrations");
            } else {
                println!("  pending (applies on `migrate apply` or next boot):");
                for migration in &status.pending {
                    println!("  pending V{:0>3} {}", migration.version, migration.name);
                }
            }
            Ok(())
        }
        _ => Err("usage: leaflog-backend migrate <apply|status>".into()),
    }
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
    match seed::seed_strains(&db).await {
        Ok(seeded) if seeded > 0 => {
            println!("Seeded {seeded} bundled strain records");
        }
        Ok(_) => {}
        Err(error) => eprintln!("strain seeding skipped: {error}"),
    }
    match seed::seed_equipment(&db).await {
        Ok(seeded) if seeded > 0 => {
            println!("Seeded {seeded} predefined equipment items");
        }
        Ok(_) => {}
        Err(error) => eprintln!("equipment seeding skipped: {error}"),
    }
    // Snapshot retention (spec §4.2): prune at startup and weekly.
    {
        let retention_db = db.clone();
        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(weeks(1));
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                ticker.tick().await;
                let pruned = retention_db
                    .with(|conn| -> Result<u64, crate::db::StoreError> {
                        let months = crate::store::retention::retention_months(conn)?;
                        // A retention window is a coarse horizon, not a
                        // calendar promise: 31-day months.
                        let cutoff = time::OffsetDateTime::now_utc()
                            - time::Duration::seconds(i64::from(months) * 31 * 24 * 60 * 60);
                        let cutoff = cutoff
                            .format(&time::format_description::well_known::Rfc3339)
                            .unwrap_or_default();
                        crate::store::retention::prune(conn, &cutoff)
                    })
                    .await;
                match pruned {
                    Ok(0) => {}
                    Ok(count) => println!("Pruned {count} old sensor snapshots"),
                    Err(error) => eprintln!("snapshot retention skipped: {error}"),
                }
            }
        });
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
