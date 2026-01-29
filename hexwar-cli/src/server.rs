//! Server command - start the web visualizer server
//!
//! ## Architecture (4-layer granularity)
//!
//! - Level 1: run() - orchestration
//! - Level 2: configure_server(), start_server()
//! - Level 3: (delegated to hexwar-server crate)
//! - Level 4: configuration validation

use std::path::PathBuf;

use anyhow::Result;
use clap::Args;

use hexwar_server::{run_server, ServerConfig};

// ============================================================================
// COMMAND ARGUMENTS (Level 4 - Configuration)
// ============================================================================

#[derive(Args)]
pub struct ServerArgs {
    /// Port number to listen on
    #[arg(long, default_value = "8002")]
    pub port: u16,

    /// Directory containing static files for the visualizer
    #[arg(long, default_value = "hexwar/visualizer")]
    pub static_dir: PathBuf,
}

// ============================================================================
// LEVEL 1 - ORCHESTRATION
// ============================================================================

/// Run server command
///
/// This function reads like a table of contents:
/// 1. Configure server
/// 2. Start server (blocking)
pub fn run(args: ServerArgs) -> Result<()> {
    let config = configure_server(&args)?;

    tracing::info!(
        "Starting HEXWAR visualizer server on port {}",
        config.port
    );

    start_server(config)
}

// ============================================================================
// LEVEL 2 - PHASES
// ============================================================================

/// Configure server from command arguments
fn configure_server(args: &ServerArgs) -> Result<ServerConfig> {
    validate_static_dir(&args.static_dir)?;

    Ok(ServerConfig {
        port: args.port,
        static_dir: args.static_dir.to_string_lossy().to_string(),
    })
}

/// Start the server (blocking)
fn start_server(config: ServerConfig) -> Result<()> {
    // Create tokio runtime for async server
    let runtime = tokio::runtime::Runtime::new()?;

    runtime.block_on(async {
        run_server(config).await
    })
}

// ============================================================================
// LEVEL 3 - STEPS
// ============================================================================

/// Validate that static directory exists
fn validate_static_dir(path: &PathBuf) -> Result<()> {
    if !path.exists() {
        tracing::warn!(
            "Static directory does not exist: {}. Server will start but may not serve files.",
            path.display()
        );
    } else if !path.is_dir() {
        anyhow::bail!(
            "Static path exists but is not a directory: {}",
            path.display()
        );
    }

    Ok(())
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_configure_server_defaults() {
        let args = ServerArgs {
            port: 8002,
            static_dir: PathBuf::from("test_static"),
        };

        let config = configure_server(&args).unwrap();
        assert_eq!(config.port, 8002);
        assert_eq!(config.static_dir, "test_static");
    }

    #[test]
    fn test_validate_static_dir_nonexistent() {
        // Should not error, just warn
        let result = validate_static_dir(&PathBuf::from("/nonexistent/path"));
        assert!(result.is_ok());
    }
}
