//! HEXWAR Server - HTTP API for visualizer
//!
//! This crate provides the web backend:
//! - REST API for game operations
//! - Static file serving for visualizer
//! - WebSocket for real-time updates (optional)

// TODO: Agent 7 will implement HTTP server

use std::net::SocketAddr;

/// Server configuration
#[derive(Clone, Debug)]
pub struct ServerConfig {
    pub port: u16,
    pub static_dir: String,
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: 8002,
            static_dir: "visualizer".to_string(),
        }
    }
}

/// Start the HTTP server
pub async fn run_server(config: ServerConfig) -> anyhow::Result<()> {
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    tracing::info!("Starting server on {}", addr);

    todo!("Agent 7: Implement HTTP server with axum")
}
