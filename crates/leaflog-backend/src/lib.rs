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

pub mod app;
pub mod assets;
pub mod ingress;

use std::path::PathBuf;

pub use app::{AppState, Mode, build_router};
pub use assets::Assets;

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
    let state = AppState::new(config.assets(), mode, config.dev_prefix.clone());
    let app = build_router(state).into_make_service_with_connect_info::<std::net::SocketAddr>();
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
