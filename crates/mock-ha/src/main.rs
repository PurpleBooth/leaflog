//! Mock Home Assistant Supervisor for Leaflog development.
//!
//! `just dev` points the Leaflog backend here instead of a real
//! Supervisor. Milestone M1 ships the REST surface only
//! (`GET /core/api/` and `GET /core/api/states`); the fake WebSocket
//! (`subscribe_entities`) protocol lands with the real client in M4
//! (see `docs/DECISIONS.md` D006).
//!
//! Configuration: `MOCK_HA_PORT` (default `8098`).

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

use std::net::SocketAddr;

use axum::{Json, Router, routing::get};
use serde_json::{Value, json};

/// Default TCP port for the mock Supervisor.
const DEFAULT_PORT: u16 = 8098;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port = std::env::var("MOCK_HA_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(DEFAULT_PORT);
    let app = Router::new()
        .route("/", get(root))
        .route("/core/api/", get(ping))
        .route("/core/api/states", get(states));
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("mock-ha (fake Supervisor) listening on {addr}");
    axum::serve(listener, app).await?;
    Ok(())
}

/// Index describing the mock.
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn root() -> Json<Value> {
    json!({
        "app": "mock-ha",
        "endpoints": ["/core/api/", "/core/api/states"],
        "note": "WebSocket surface arrives in milestone M4",
    })
    .into()
}

/// Mirrors `GET http://supervisor/core/api/`.
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn ping() -> Json<Value> {
    json!({ "message": "API running." }).into()
}

/// Mirrors `GET http://supervisor/core/api/states` with canned grow-room
/// entities the entity pickers can list.
#[allow(clippy::unused_async)] // Axum handlers must be async fns; see docs/DECISIONS.md
async fn states() -> Json<Value> {
    json!([
        fake_state(
            "sensor.grow_temperature",
            "24.3",
            "°C",
            "Grow tent temperature"
        ),
        fake_state("sensor.grow_humidity", "58", "%", "Grow tent humidity"),
        fake_state("sensor.grow_soil_moisture", "31", "%", "Soil moisture"),
        fake_state("sensor.grow_co2", "820", "ppm", "Grow tent CO₂"),
        fake_state("light.grow_led", "on", "", "Grow light"),
    ])
    .into()
}

/// Builds one Home-Assistant-shaped state object.
fn fake_state(entity_id: &str, state: &str, unit: &str, name: &str) -> Value {
    let mut attributes = json!({ "friendly_name": name });
    if !unit.is_empty() {
        attributes["unit_of_measurement"] = json!(unit);
    }
    json!({
        "entity_id": entity_id,
        "state": state,
        "attributes": attributes,
        "last_changed": "2026-11-08T12:00:00.000000+00:00",
        "last_updated": "2026-11-08T12:00:00.000000+00:00",
    })
}
