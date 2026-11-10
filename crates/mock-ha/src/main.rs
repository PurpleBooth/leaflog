//! Mock Home Assistant Supervisor binary: serves the fake REST + WS
//! surface on `MOCK_HA_PORT` (default `8098`) for `just dev` and tests.
//! The application (and lint block) live in the library crate root.

use std::net::SocketAddr;

use mock_ha::{FakeRoom, MockState};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let port = std::env::var("MOCK_HA_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8098);
    let app = mock_ha::app(MockState {
        notifications: std::sync::Arc::new(tokio::sync::Mutex::new(Vec::new())),
        room: std::sync::Arc::new(tokio::sync::Mutex::new(FakeRoom::new())),
    });
    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("mock-ha (fake Supervisor) listening on {addr}");
    axum::serve(listener, app).await?;
    Ok(())
}
