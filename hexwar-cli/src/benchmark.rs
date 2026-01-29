//! Benchmark command - compare GPU vs CPU performance
//!
//! ## Architecture (4-layer granularity)
//!
//! - Level 1: run() - orchestration
//! - Level 2: run_cpu_benchmark(), run_gpu_benchmark(), report_results()
//! - Level 3: benchmark_alpha_beta(), benchmark_mcts(), benchmark_gpu_rollouts()
//! - Level 4: timing utilities, formatting

use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::Args;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use hexwar_core::{AlphaBetaAI, GameResult, GameState, Heuristics, RuleSet};
use hexwar_mcts::{MctsConfig, MctsPlayer};

// ============================================================================
// COMMAND ARGUMENTS (Level 4 - Configuration)
// ============================================================================

#[derive(Args)]
pub struct BenchmarkArgs {
    /// Number of games to benchmark
    #[arg(long, default_value = "10")]
    pub games: usize,

    /// Maximum AI depth to test
    #[arg(long, default_value = "6")]
    pub depth: u32,

    /// Maximum moves per game
    #[arg(long, default_value = "50")]
    pub max_moves: u32,

    /// Run GPU benchmarks (requires CUDA)
    #[arg(long)]
    pub gpu: bool,

    /// MCTS simulations for MCTS benchmark
    #[arg(long, default_value = "1000")]
    pub mcts_simulations: usize,

    /// Output results as JSON
    #[arg(long)]
    pub json: bool,
}

/// Results of a single benchmark run
#[derive(Clone, Debug)]
struct BenchmarkResult {
    name: String,
    games: usize,
    total_time: Duration,
    avg_time_per_game: Duration,
    games_per_second: f64,
    notes: String,
}

/// All benchmark results
#[derive(Clone, Debug)]
struct AllResults {
    results: Vec<BenchmarkResult>,
    system_info: String,
}

// ============================================================================
// LEVEL 1 - ORCHESTRATION
// ============================================================================

/// Run benchmark command
///
/// This function reads like a table of contents:
/// 1. Run CPU benchmarks (alpha-beta at various depths)
/// 2. Run MCTS benchmark
/// 3. Optionally run GPU benchmark
/// 4. Report all results
pub fn run(args: BenchmarkArgs, seed: Option<u64>) -> Result<()> {
    tracing::info!("Starting benchmarks: {} games, max depth {}", args.games, args.depth);

    let mut all_results = AllResults {
        results: Vec::new(),
        system_info: get_system_info(),
    };

    run_cpu_benchmarks(&args, seed, &mut all_results)?;
    run_mcts_benchmark(&args, seed, &mut all_results)?;

    if args.gpu {
        run_gpu_benchmark(&args, seed, &mut all_results)?;
    }

    report_results(&all_results, &args);

    Ok(())
}

// ============================================================================
// LEVEL 2 - PHASES
// ============================================================================

/// Run CPU alpha-beta benchmarks at various depths
fn run_cpu_benchmarks(args: &BenchmarkArgs, seed: Option<u64>, results: &mut AllResults) -> Result<()> {
    // Test depths 2, 4, and up to max_depth
    let depths: Vec<u32> = (2..=args.depth).step_by(2).collect();

    for depth in depths {
        tracing::info!("Benchmarking alpha-beta at depth {}...", depth);
        let result = benchmark_alpha_beta(args.games, depth, args.max_moves, seed)?;
        results.results.push(result);
    }

    Ok(())
}

/// Run MCTS benchmark
fn run_mcts_benchmark(args: &BenchmarkArgs, seed: Option<u64>, results: &mut AllResults) -> Result<()> {
    tracing::info!("Benchmarking MCTS ({} simulations)...", args.mcts_simulations);
    let result = benchmark_mcts(args.games, args.mcts_simulations, args.max_moves, seed)?;
    results.results.push(result);
    Ok(())
}

/// Run GPU benchmark (if CUDA available)
fn run_gpu_benchmark(args: &BenchmarkArgs, seed: Option<u64>, results: &mut AllResults) -> Result<()> {
    tracing::info!("Benchmarking GPU rollouts...");

    match benchmark_gpu_rollouts(args.games, args.max_moves, seed) {
        Ok(result) => {
            results.results.push(result);
        }
        Err(e) => {
            tracing::warn!("GPU benchmark failed: {}", e);
            results.results.push(BenchmarkResult {
                name: "GPU Rollouts".to_string(),
                games: 0,
                total_time: Duration::ZERO,
                avg_time_per_game: Duration::ZERO,
                games_per_second: 0.0,
                notes: format!("Failed: {}", e),
            });
        }
    }

    Ok(())
}

/// Report all benchmark results
fn report_results(results: &AllResults, args: &BenchmarkArgs) {
    if args.json {
        print_json_results(results);
    } else {
        print_text_results(results);
    }
}

// ============================================================================
// LEVEL 3 - STEPS
// ============================================================================

/// Benchmark alpha-beta AI at a specific depth
fn benchmark_alpha_beta(
    num_games: usize,
    depth: u32,
    max_moves: u32,
    seed: Option<u64>,
) -> Result<BenchmarkResult> {
    let mut rng = create_rng(seed);
    let heuristics = Heuristics::default();
    let mut ai = AlphaBetaAI::new(depth, heuristics);

    let start = Instant::now();
    let mut total_moves = 0;

    for _ in 0..num_games {
        let state = create_random_game(&mut rng);
        let (_, moves) = play_game_with_ai(&state, &mut ai, max_moves);
        total_moves += moves;
    }

    let total_time = start.elapsed();
    let avg_time = total_time / num_games as u32;

    Ok(BenchmarkResult {
        name: format!("Alpha-Beta D{}", depth),
        games: num_games,
        total_time,
        avg_time_per_game: avg_time,
        games_per_second: num_games as f64 / total_time.as_secs_f64(),
        notes: format!("Avg moves/game: {:.1}", total_moves as f64 / num_games as f64),
    })
}

/// Benchmark MCTS AI
fn benchmark_mcts(
    num_games: usize,
    simulations: usize,
    max_moves: u32,
    seed: Option<u64>,
) -> Result<BenchmarkResult> {
    let mut rng = create_rng(seed);
    let config = MctsConfig::cpu_only(simulations);
    let player = MctsPlayer::cpu_only(config);

    let start = Instant::now();
    let mut total_moves = 0;

    for _ in 0..num_games {
        let state = create_random_game(&mut rng);
        let mut current = state;
        let mut moves = 0;

        while current.result() == GameResult::Ongoing && moves < max_moves * 2 {
            if let Some(mv) = player.best_move(&current) {
                current = current.apply_move(mv);
                moves += 1;
            } else {
                break;
            }
        }

        total_moves += moves;
    }

    let total_time = start.elapsed();
    let avg_time = total_time / num_games as u32;

    Ok(BenchmarkResult {
        name: format!("MCTS {} sims", simulations),
        games: num_games,
        total_time,
        avg_time_per_game: avg_time,
        games_per_second: num_games as f64 / total_time.as_secs_f64(),
        notes: format!("Avg moves/game: {:.1}", total_moves as f64 / num_games as f64 / 2.0),
    })
}

/// Benchmark GPU rollouts
fn benchmark_gpu_rollouts(
    num_games: usize,
    max_moves: u32,
    seed: Option<u64>,
) -> Result<BenchmarkResult> {
    use hexwar_gpu::GpuContext;

    // Initialize GPU context
    let ctx = GpuContext::new().context("Failed to initialize GPU")?;

    let mut rng = create_rng(seed);

    // Create batch of game states
    let states: Vec<GameState> = (0..num_games)
        .map(|_| create_random_game(&mut rng))
        .collect();

    // Benchmark GPU simulation
    let start = Instant::now();
    let results = ctx
        .simulate_batch(&states, max_moves, seed.unwrap_or(42))
        .context("GPU simulation failed")?;
    let total_time = start.elapsed();

    let avg_time = total_time / num_games as u32;

    Ok(BenchmarkResult {
        name: "GPU Rollouts".to_string(),
        games: num_games,
        total_time,
        avg_time_per_game: avg_time,
        games_per_second: num_games as f64 / total_time.as_secs_f64(),
        notes: format!(
            "Avg rounds: {:.1}, White win rate: {:.1}%",
            results.avg_rounds(),
            results.win_rate(hexwar_core::Player::White) * 100.0
        ),
    })
}

/// Play a single game using AI and return final state and move count
fn play_game_with_ai(
    initial: &GameState,
    ai: &mut AlphaBetaAI,
    max_moves: u32,
) -> (GameState, u32) {
    let mut state = initial.clone();
    let mut moves = 0;

    let max_total_moves = max_moves * 2; // Two moves per round

    while state.result() == GameResult::Ongoing && moves < max_total_moves {
        if let Some(mv) = ai.best_move(&state) {
            state = state.apply_move(mv);
            moves += 1;
        } else {
            break;
        }
    }

    (state, moves)
}

/// Create a random starting game state
fn create_random_game(_rng: &mut ChaCha8Rng) -> GameState {
    // Use default ruleset for consistent benchmarking
    let rs = RuleSet::default();
    rs.to_game_state()
}

// ============================================================================
// LEVEL 4 - UTILITIES
// ============================================================================

/// Create RNG from seed or random
fn create_rng(seed: Option<u64>) -> ChaCha8Rng {
    match seed {
        Some(s) => ChaCha8Rng::seed_from_u64(s),
        None => ChaCha8Rng::from_entropy(),
    }
}

/// Get system information string
fn get_system_info() -> String {
    format!(
        "Rust {}, {} CPUs",
        env!("CARGO_PKG_VERSION"),
        std::thread::available_parallelism()
            .map(|p| p.get())
            .unwrap_or(1)
    )
}

/// Format duration for display
fn format_duration(d: Duration) -> String {
    if d.as_secs() >= 60 {
        format!("{}m {:.1}s", d.as_secs() / 60, (d.as_secs() % 60) as f64 + d.subsec_millis() as f64 / 1000.0)
    } else if d.as_secs() >= 1 {
        format!("{:.2}s", d.as_secs_f64())
    } else if d.as_millis() >= 1 {
        format!("{:.1}ms", d.as_secs_f64() * 1000.0)
    } else {
        format!("{:.1}us", d.as_secs_f64() * 1_000_000.0)
    }
}

/// Print results as JSON
fn print_json_results(results: &AllResults) {
    #[derive(serde::Serialize)]
    struct JsonBenchmark {
        name: String,
        games: usize,
        total_time_ms: u64,
        avg_time_ms: f64,
        games_per_second: f64,
        notes: String,
    }

    #[derive(serde::Serialize)]
    struct JsonOutput {
        system_info: String,
        benchmarks: Vec<JsonBenchmark>,
    }

    let output = JsonOutput {
        system_info: results.system_info.clone(),
        benchmarks: results
            .results
            .iter()
            .map(|r| JsonBenchmark {
                name: r.name.clone(),
                games: r.games,
                total_time_ms: r.total_time.as_millis() as u64,
                avg_time_ms: r.avg_time_per_game.as_secs_f64() * 1000.0,
                games_per_second: r.games_per_second,
                notes: r.notes.clone(),
            })
            .collect(),
    };

    if let Ok(json) = serde_json::to_string_pretty(&output) {
        println!("{}", json);
    }
}

/// Print results as text table
fn print_text_results(results: &AllResults) {
    println!("\n=== HEXWAR Benchmark Results ===");
    println!("System: {}\n", results.system_info);

    println!(
        "{:<20} {:>8} {:>12} {:>12} {:>10}  {}",
        "Benchmark", "Games", "Total Time", "Avg/Game", "Games/s", "Notes"
    );
    println!("{}", "-".repeat(90));

    for r in &results.results {
        println!(
            "{:<20} {:>8} {:>12} {:>12} {:>10.2}  {}",
            r.name,
            r.games,
            format_duration(r.total_time),
            format_duration(r.avg_time_per_game),
            r.games_per_second,
            r.notes
        );
    }

    // Print speedup comparison if we have GPU results
    let gpu_result = results.results.iter().find(|r| r.name == "GPU Rollouts");
    let cpu_result = results.results.iter().find(|r| r.name.starts_with("Alpha-Beta D2"));

    if let (Some(gpu), Some(cpu)) = (gpu_result, cpu_result) {
        if gpu.games_per_second > 0.0 && cpu.games_per_second > 0.0 {
            let speedup = gpu.games_per_second / cpu.games_per_second;
            println!("\nGPU speedup vs Alpha-Beta D2: {:.1}x", speedup);
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_duration() {
        assert!(format_duration(Duration::from_millis(500)).contains("ms"));
        assert!(format_duration(Duration::from_secs(5)).contains("s"));
        assert!(format_duration(Duration::from_secs(90)).contains("m"));
    }

    #[test]
    fn test_create_random_game() {
        let mut rng = ChaCha8Rng::seed_from_u64(42);
        let state = create_random_game(&mut rng);
        assert_eq!(state.result(), GameResult::Ongoing);
    }

    #[test]
    fn test_get_system_info() {
        let info = get_system_info();
        assert!(info.contains("Rust"));
        assert!(info.contains("CPUs"));
    }
}
