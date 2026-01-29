//! HEXWAR CLI - Command-line interface
//!
//! Commands:
//! - evolve: Run evolutionary balancing
//! - play: Play a single game
//! - serve: Start visualizer server
//! - benchmark: Compare GPU vs CPU performance

// TODO: Agent 6 will implement CLI

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "hexwar")]
#[command(about = "HEXWAR evolutionary game balancer")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Run evolutionary balancing
    Evolve {
        #[arg(long, default_value = "50")]
        population: usize,
        #[arg(long, default_value = "100")]
        generations: usize,
        #[arg(long, default_value = "4")]
        depth: u32,
        #[arg(long)]
        output: String,
    },
    /// Play a single game
    Play {
        #[arg(long)]
        white: String,
        #[arg(long)]
        black: String,
        #[arg(long, default_value = "4")]
        depth: u32,
    },
    /// Start visualizer server
    Serve {
        #[arg(long, default_value = "8002")]
        port: u16,
    },
    /// Benchmark GPU vs CPU
    Benchmark {
        #[arg(long, default_value = "100")]
        games: usize,
        #[arg(long, default_value = "4")]
        depth: u32,
    },
}

fn main() -> anyhow::Result<()> {
    // Initialize logging
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();

    match cli.command {
        Commands::Evolve { population, generations, depth, output } => {
            println!("Evolution: pop={}, gen={}, depth={}, output={}",
                     population, generations, depth, output);
            todo!("Agent 6: Wire up evolution")
        }
        Commands::Play { white, black, depth } => {
            println!("Play: white={}, black={}, depth={}", white, black, depth);
            todo!("Agent 6: Wire up game playing")
        }
        Commands::Serve { port } => {
            println!("Serve: port={}", port);
            todo!("Agent 6: Wire up server")
        }
        Commands::Benchmark { games, depth } => {
            println!("Benchmark: games={}, depth={}", games, depth);
            todo!("Agent 6: Wire up benchmark")
        }
    }
}
