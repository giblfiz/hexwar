//! HEXWAR CLI - Command-line interface for HEXWAR game balancer
//!
//! ## Commands
//!
//! - `evolve`: Run evolutionary balancing to find balanced army compositions
//! - `match`: Play a match between two rulesets
//! - `server`: Start the web visualizer server
//! - `benchmark`: Run performance benchmarks (GPU vs CPU)
//!
//! ## Architecture (4-layer granularity)
//!
//! - Level 1: main() - orchestration, command dispatch
//! - Level 2: Command modules (evolve, match_cmd, server, benchmark)
//! - Level 3: Implementation details within each module
//! - Level 4: Utilities and library calls

mod benchmark;
mod evolve;
mod match_cmd;
mod server;

use clap::{Parser, Subcommand};

// ============================================================================
// CLI ARGUMENT STRUCTURES (Level 4 - Configuration)
// ============================================================================

#[derive(Parser)]
#[command(name = "hexwar")]
#[command(version, about = "HEXWAR evolutionary game balancer")]
#[command(long_about = "HEXWAR uses genetic algorithms to evolve balanced army compositions \
    for an asymmetric hex-based strategy game. It supports GPU acceleration for MCTS rollouts \
    and provides tools for visualization and analysis.")]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Enable verbose output
    #[arg(short, long, global = true)]
    verbose: bool,

    /// Random seed for reproducibility
    #[arg(long, global = true)]
    seed: Option<u64>,
}

#[derive(Subcommand)]
enum Commands {
    /// Run evolutionary balancing to find balanced army compositions
    Evolve(evolve::EvolveArgs),

    /// Play a match between two rulesets
    Match(match_cmd::MatchArgs),

    /// Start the web visualizer server
    Server(server::ServerArgs),

    /// Run performance benchmarks (GPU vs CPU)
    Benchmark(benchmark::BenchmarkArgs),
}

// ============================================================================
// MAIN ENTRY POINT (Level 1 - Orchestration)
// ============================================================================

/// Main entry point - dispatches to subcommands
///
/// This function reads like a table of contents:
/// 1. Initialize logging
/// 2. Parse command-line arguments
/// 3. Dispatch to appropriate command handler
fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    initialize_logging(cli.verbose);

    dispatch_command(cli)
}

// ============================================================================
// LEVEL 2 - PHASES
// ============================================================================

/// Initialize tracing/logging based on verbosity
fn initialize_logging(verbose: bool) {
    use tracing_subscriber::EnvFilter;

    let filter = if verbose {
        EnvFilter::new("hexwar=debug,info")
    } else {
        EnvFilter::new("hexwar=info,warn")
    };

    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_target(false)
        .init();
}

/// Dispatch to the appropriate command handler
fn dispatch_command(cli: Cli) -> anyhow::Result<()> {
    let seed = cli.seed;

    match cli.command {
        Commands::Evolve(args) => evolve::run(args, seed),
        Commands::Match(args) => match_cmd::run(args, seed),
        Commands::Server(args) => server::run(args),
        Commands::Benchmark(args) => benchmark::run(args, seed),
    }
}
