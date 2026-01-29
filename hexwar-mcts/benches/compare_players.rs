//! MCTS vs Alpha-Beta Performance Benchmark
//!
//! Compares:
//! 1. Time to find a move at various depths/simulations
//! 2. Move quality via game play
//! 3. Throughput (moves per second)

use std::time::Instant;
use hexwar_core::{GameState, Heuristics, board::Hex, game::Template, pieces::piece_id_to_index};
use hexwar_core::ai::AlphaBetaAI;
use hexwar_mcts::{MctsPlayer, MctsConfig};

// ============================================================================
// TEST POSITIONS
// ============================================================================

/// Create a simple balanced test position: King + 2 pieces per side
fn test_position_balanced() -> GameState {
    let white = vec![
        (piece_id_to_index("K1").unwrap(), Hex::new(0, 3), 0),
        (piece_id_to_index("A2").unwrap(), Hex::new(-1, 3), 0),
        (piece_id_to_index("B3").unwrap(), Hex::new(1, 3), 0),
    ];
    let black = vec![
        (piece_id_to_index("K1").unwrap(), Hex::new(0, -3), 3),
        (piece_id_to_index("A2").unwrap(), Hex::new(1, -3), 3),
        (piece_id_to_index("B3").unwrap(), Hex::new(-1, -3), 3),
    ];
    GameState::new(&white, &black, Template::E, Template::E)
}

/// Create a mid-game position with more pieces
fn test_position_midgame() -> GameState {
    let white = vec![
        (piece_id_to_index("K1").unwrap(), Hex::new(0, 2), 0),
        (piece_id_to_index("A2").unwrap(), Hex::new(-2, 2), 0),
        (piece_id_to_index("B3").unwrap(), Hex::new(2, 2), 0),
        (piece_id_to_index("C2").unwrap(), Hex::new(-1, 1), 0),
        (piece_id_to_index("P1").unwrap(), Hex::new(1, 1), 0),
    ];
    let black = vec![
        (piece_id_to_index("K1").unwrap(), Hex::new(0, -2), 3),
        (piece_id_to_index("A2").unwrap(), Hex::new(2, -2), 3),
        (piece_id_to_index("B3").unwrap(), Hex::new(-2, -2), 3),
        (piece_id_to_index("C2").unwrap(), Hex::new(1, -1), 3),
        (piece_id_to_index("P1").unwrap(), Hex::new(-1, -1), 3),
    ];
    GameState::new(&white, &black, Template::E, Template::E)
}

// ============================================================================
// BENCHMARK STRUCTURES
// ============================================================================

#[derive(Clone, Debug)]
struct BenchmarkResult {
    player: String,
    config: String,
    avg_move_time_ms: f64,
    moves_per_second: f64,
    total_time_ms: f64,
    move_count: usize,
}

impl BenchmarkResult {
    fn to_table_row(&self) -> String {
        format!(
            "| {} | {} | {:.2}ms | {:.0} | {:.0}ms |",
            self.player,
            self.config,
            self.avg_move_time_ms,
            self.moves_per_second,
            self.total_time_ms
        )
    }
}

// ============================================================================
// BENCHMARK: Time to Find Move
// ============================================================================

fn benchmark_move_time(
    state: &GameState,
    position_name: &str,
) -> Vec<BenchmarkResult> {
    println!("\n=== MOVE TIME BENCHMARK: {} ===", position_name);
    let mut results = Vec::new();

    // Alpha-Beta at various depths
    let ab_depths = [2, 4, 6];
    for depth in &ab_depths {
        print!("  AB depth {} ... ", depth);
        let mut ai = AlphaBetaAI::new(*depth, Heuristics::default());
        let mut total_time = 0.0;
        let iterations = 5;

        for _ in 0..iterations {
            let start = Instant::now();
            let _ = ai.best_move(state);
            total_time += start.elapsed().as_secs_f64() * 1000.0;
        }

        let avg_time = total_time / iterations as f64;
        let moves_per_sec = 1000.0 / avg_time;

        results.push(BenchmarkResult {
            player: "Alpha-Beta".to_string(),
            config: format!("Depth {}", depth),
            avg_move_time_ms: avg_time,
            moves_per_second: moves_per_sec,
            total_time_ms: total_time,
            move_count: iterations,
        });

        println!("{:.2}ms", avg_time);
    }

    // MCTS with various simulation counts
    let mcts_sims = [100, 500, 1000, 5000];
    for sims in &mcts_sims {
        print!("  MCTS {} sims ... ", sims);
        let config = MctsConfig::cpu_only(*sims);
        let player = MctsPlayer::cpu_only(config);
        let mut total_time = 0.0;
        let iterations = 3;

        for _ in 0..iterations {
            let start = Instant::now();
            let _ = player.best_move(state);
            total_time += start.elapsed().as_secs_f64() * 1000.0;
        }

        let avg_time = total_time / iterations as f64;
        let moves_per_sec = 1000.0 / avg_time;

        results.push(BenchmarkResult {
            player: "MCTS".to_string(),
            config: format!("{} sims", sims),
            avg_move_time_ms: avg_time,
            moves_per_second: moves_per_sec,
            total_time_ms: total_time,
            move_count: iterations,
        });

        println!("{:.2}ms", avg_time);
    }

    results
}

// ============================================================================
// BENCHMARK: Throughput (Moves per Second)
// ============================================================================

fn benchmark_throughput(
    initial: &GameState,
    position_name: &str,
) -> Vec<BenchmarkResult> {
    println!("\n=== THROUGHPUT BENCHMARK: {} ===", position_name);
    let mut results = Vec::new();

    // Alpha-Beta depths - play short games
    let ab_depths = [2];
    for depth in &ab_depths {
        print!("  AB depth {} (game) ... ", depth);
        let mut ai = AlphaBetaAI::new(*depth, Heuristics::default());

        let start = Instant::now();
        let (_final_state, history) = ai.play_game(initial.clone(), 10);
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;

        let moves_per_sec = (history.len() as f64 / elapsed) * 1000.0;

        results.push(BenchmarkResult {
            player: "Alpha-Beta".to_string(),
            config: format!("Depth {}", depth),
            avg_move_time_ms: elapsed / history.len() as f64,
            moves_per_second: moves_per_sec,
            total_time_ms: elapsed,
            move_count: history.len(),
        });

        println!("{} moves in {:.0}ms ({:.0}/sec)", history.len(), elapsed, moves_per_sec);
    }

    // MCTS - play short games
    let mcts_sims = [100, 1000];
    for sims in &mcts_sims {
        print!("  MCTS {} sims (game) ... ", sims);
        let config = MctsConfig::cpu_only(*sims);
        let player = MctsPlayer::cpu_only(config);

        let start = Instant::now();
        let (_final_state, history) = player.play_game(initial.clone(), 10);
        let elapsed = start.elapsed().as_secs_f64() * 1000.0;

        let moves_per_sec = (history.len() as f64 / elapsed) * 1000.0;

        results.push(BenchmarkResult {
            player: "MCTS".to_string(),
            config: format!("{} sims", sims),
            avg_move_time_ms: elapsed / history.len() as f64,
            moves_per_second: moves_per_sec,
            total_time_ms: elapsed,
            move_count: history.len(),
        });

        println!("{} moves in {:.0}ms ({:.0}/sec)", history.len(), elapsed, moves_per_sec);
    }

    results
}

// ============================================================================
// BENCHMARK: Move Quality (Head-to-Head Games)
// ============================================================================

#[derive(Debug)]
struct GameResult {
    white_player: String,
    black_player: String,
    winner: String,
    move_count: usize,
}

fn benchmark_move_quality(
    initial: &GameState,
    position_name: &str,
) -> Vec<GameResult> {
    println!("\n=== MOVE QUALITY BENCHMARK: {} ===", position_name);
    let mut results = Vec::new();

    // AB D2 vs MCTS 1000
    print!("  AB D2 vs MCTS 1000 ... ");
    for game_num in 0..3 {
        let mut ab = AlphaBetaAI::new(2, Heuristics::default());
        let mcts_cfg = MctsConfig::cpu_only(1000);
        let mcts = MctsPlayer::cpu_only(mcts_cfg);

        let mut state = initial.clone();
        let mut moves = 0;
        let max_moves = 100;

        while state.result() == hexwar_core::GameResult::Ongoing && moves < max_moves {
            if state.current_player() == hexwar_core::game::Player::White {
                if let Some(mv) = ab.best_move(&state) {
                    state = state.apply_move(mv);
                    moves += 1;
                } else {
                    break;
                }
            } else {
                if let Some(mv) = mcts.best_move(&state) {
                    state = state.apply_move(mv);
                    moves += 1;
                } else {
                    break;
                }
            }
        }

        let winner = match state.result() {
            hexwar_core::GameResult::WhiteWins => "AB D2 (White)".to_string(),
            hexwar_core::GameResult::BlackWins => "MCTS 1000 (Black)".to_string(),
            hexwar_core::GameResult::Ongoing => "Draw".to_string(),
        };

        results.push(GameResult {
            white_player: "AB D2".to_string(),
            black_player: "MCTS 1000".to_string(),
            winner,
            move_count: moves,
        });

        println!("  Game {}: {} ({} moves)", game_num + 1, results[game_num].winner, moves);
    }

    results
}

// ============================================================================
// MAIN
// ============================================================================

fn main() {
    println!("\n╔════════════════════════════════════════════════════════════╗");
    println!("║  HEXWAR: MCTS vs Alpha-Beta Benchmark                     ║");
    println!("║  Comparing search algorithms on various metrics           ║");
    println!("╚════════════════════════════════════════════════════════════╝");

    let balanced = test_position_balanced();
    let midgame = test_position_midgame();

    let mut all_results = Vec::new();

    // === BALANCED POSITION ===
    let balanced_times = benchmark_move_time(&balanced, "Balanced (K + 2 pieces)");
    all_results.extend(balanced_times.clone());

    // === MID-GAME POSITION ===
    let midgame_times = benchmark_move_time(&midgame, "Mid-Game (K + 4 pieces)");
    all_results.extend(midgame_times.clone());

    // === THROUGHPUT ===
    let balanced_throughput = benchmark_throughput(&balanced, "Balanced");
    all_results.extend(balanced_throughput.clone());

    let midgame_throughput = benchmark_throughput(&midgame, "Mid-Game");
    all_results.extend(midgame_throughput.clone());

    // === MOVE QUALITY ===
    let _quality = benchmark_move_quality(&balanced, "Balanced");

    // === RESULTS TABLE ===
    println!("\n\n╔════════════════════════════════════════════════════════════════════╗");
    println!("║                     BENCHMARK RESULTS TABLE                       ║");
    println!("╠════════════════════════════════════════════════════════════════════╣");
    println!("| Player      | Config        | Avg Move Time | Moves/Sec | Total Time |");
    println!("├─────────────┼───────────────┼───────────────┼───────────┼────────────┤");

    for result in &all_results {
        println!("{}", result.to_table_row());
    }

    println!("╚════════════════════════════════════════════════════════════════════╝");

    // === ANALYSIS ===
    println!("\n╔════════════════════════════════════════════════════════════╗");
    println!("║                    ANALYSIS & NOTES                       ║");
    println!("╚════════════════════════════════════════════════════════════╝");

    let ab_d2 = all_results.iter().find(|r| r.player == "Alpha-Beta" && r.config == "Depth 2");
    let mcts_1000 = all_results.iter().find(|r| r.player == "MCTS" && r.config == "1000 sims");

    if let (Some(ab), Some(mcts)) = (ab_d2, mcts_1000) {
        let ratio = ab.avg_move_time_ms / mcts.avg_move_time_ms;
        println!("\n• Alpha-Beta D2 vs MCTS 1000:");
        if ratio > 1.0 {
            println!("  MCTS is {:.1}x faster", ratio);
        } else {
            println!("  AB is {:.1}x faster", 1.0 / ratio);
        }
    }

    println!("\n• Alpha-Beta Depth Scaling:");
    let depths = [2, 4, 6];
    for (i, d) in depths.iter().enumerate() {
        if let Some(result) = all_results.iter().find(|r| r.player == "Alpha-Beta" && r.config == format!("Depth {}", d)) {
            if i > 0 {
                if let Some(prev) = all_results.iter().find(|r| r.player == "Alpha-Beta" && r.config == format!("Depth {}", depths[i-1])) {
                    let scaling = result.avg_move_time_ms / prev.avg_move_time_ms;
                    println!("  D{} → D{}: {:.1}x slower", depths[i-1], d, scaling);
                }
            }
        }
    }

    println!("\n• MCTS Simulation Scaling:");
    let sims = [100, 500, 1000, 5000];
    for (i, s) in sims.iter().enumerate() {
        if let Some(result) = all_results.iter().find(|r| r.player == "MCTS" && r.config == format!("{} sims", s)) {
            if i > 0 {
                if let Some(prev) = all_results.iter().find(|r| r.player == "MCTS" && r.config == format!("{} sims", sims[i-1])) {
                    let scaling = result.avg_move_time_ms / prev.avg_move_time_ms;
                    println!("  {} → {} sims: {:.2}x slower", sims[i-1], s, scaling);
                }
            }
        }
    }

    println!("\n");
}

// ============================================================================
// HELPER: Print section header
// ============================================================================

fn print_header(title: &str) {
    println!("\n╔{}╗", "═".repeat(title.len() + 4));
    println!("║  {}  ║", title);
    println!("╚{}╝", "═".repeat(title.len() + 4));
}
