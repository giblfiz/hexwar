//! HEXWAR Server - HTTP API for visualizer
//!
//! This crate provides the web backend:
//! - REST API for game operations
//! - Static file serving for visualizer
//! - Designer state management
//! - Game playback API

mod routes;
mod state;

use axum::{routing::get, Router};
use std::net::SocketAddr;
use std::sync::Arc;
use tower_http::services::ServeDir;

pub use state::ServerState;

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
            static_dir: "hexwar/visualizer".to_string(),
        }
    }
}

/// Create the router with all routes
pub fn create_router(config: &ServerConfig, state: Arc<ServerState>) -> Router {
    let static_service = ServeDir::new(&config.static_dir);

    Router::new()
        // Status endpoint
        .route("/api/status", get(routes::status::status_handler))
        // Piece types
        .route("/api/pieces", get(routes::pieces::get_pieces))
        .route("/api/piece-types", get(routes::pieces::get_pieces))
        // Board geometry
        .route("/api/board", get(routes::board::get_board))
        // Designer API
        .route(
            "/api/designer",
            get(routes::designer::get_designer_state)
                .post(routes::designer::update_designer_state),
        )
        .route(
            "/api/designer/poll",
            get(routes::designer::poll_designer_state),
        )
        .route(
            "/api/designer/load",
            axum::routing::post(routes::designer::load_into_designer),
        )
        // Champions/Seeds API
        .route("/api/champions", get(routes::champions::get_champions_list))
        .route(
            "/api/champion/{name}",
            get(routes::champions::get_champion_data),
        )
        .route("/api/seeds", get(routes::champions::get_seeds_list))
        .route("/api/seed/{name}", get(routes::champions::get_seed_data))
        // Rulesets API
        .route("/api/rulesets", get(routes::rulesets::get_rulesets))
        // Playback API
        .route(
            "/api/playback/state",
            get(routes::playback::get_playback_state),
        )
        .route(
            "/api/playback/load",
            axum::routing::post(routes::playback::load_game_record),
        )
        .route(
            "/api/playback/forward",
            axum::routing::post(routes::playback::playback_forward),
        )
        .route(
            "/api/playback/backward",
            axum::routing::post(routes::playback::playback_backward),
        )
        .route(
            "/api/playback/goto",
            axum::routing::post(routes::playback::playback_goto),
        )
        .route(
            "/api/playback/stop",
            axum::routing::post(routes::playback::playback_stop),
        )
        // Game API
        .route(
            "/api/game/start",
            axum::routing::post(routes::game::start_game),
        )
        .route(
            "/api/game/move",
            axum::routing::post(routes::game::make_player_move),
        )
        .route(
            "/api/game/ai-move",
            axum::routing::post(routes::game::get_ai_move),
        )
        // Shared state
        .with_state(state)
        // Static file serving (must be last)
        .fallback_service(static_service)
}

/// Start the HTTP server
pub async fn run_server(config: ServerConfig) -> anyhow::Result<()> {
    let addr = SocketAddr::from(([0, 0, 0, 0], config.port));
    let state = Arc::new(ServerState::new());
    let router = create_router(&config, state);

    tracing::info!("HEXWAR Server starting on http://0.0.0.0:{}", config.port);
    tracing::info!("Static files served from: {}", config.static_dir);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, router).await?;

    Ok(())
}
