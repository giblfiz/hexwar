//! Example to run the HEXWAR server standalone
//!
//! Run with: cargo run -p hexwar-server --example run_server

use hexwar_server::{run_server, ServerConfig};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let config = ServerConfig {
        port: 8002,
        static_dir: "hexwar/visualizer".to_string(),
    };

    println!("Starting HEXWAR server on port {}", config.port);
    println!("Static files from: {}", config.static_dir);
    println!("Open http://localhost:{}/designer.html", config.port);

    run_server(config).await
}
