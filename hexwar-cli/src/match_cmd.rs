//! Match command - play games between two rulesets
//!
//! ## Architecture (4-layer granularity)
//!
//! - Level 1: run() - orchestration
//! - Level 2: load_rulesets(), play_match(), report_results()
//! - Level 3: play_single_game(), compute_statistics()
//! - Level 4: formatting utilities

use std::path::PathBuf;

use anyhow::{Context, Result};
use clap::Args;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use hexwar_core::{AlphaBetaAI, GameResult, GameState, Heuristics, Move, RuleSet};
use hexwar_mcts::{MctsConfig, MctsPlayer};

// ============================================================================
// COMMAND ARGUMENTS (Level 4 - Configuration)
// ============================================================================

#[derive(Args)]
pub struct MatchArgs {
    /// White ruleset JSON file
    #[arg(long, value_name = "FILE")]
    pub white: PathBuf,

    /// Black ruleset JSON file
    #[arg(long, value_name = "FILE")]
    pub black: PathBuf,

    /// Number of games to play (will alternate colors)
    #[arg(long, default_value = "10")]
    pub games: usize,

    /// AI search depth (for alpha-beta)
    #[arg(long, default_value = "4")]
    pub depth: u32,

    /// Use MCTS instead of alpha-beta
    #[arg(long)]
    pub mcts: bool,

    /// MCTS simulations per move (when using --mcts)
    #[arg(long, default_value = "1000")]
    pub simulations: usize,

    /// Maximum rounds per game
    #[arg(long, default_value = "50")]
    pub max_rounds: u32,

    /// Output results as JSON
    #[arg(long)]
    pub json: bool,
}

/// Result of a single game
#[derive(Clone, Debug)]
struct GameRecord {
    game_number: usize,
    result: GameResult,
    rounds: u32,
    white_ruleset: String,
    black_ruleset: String,
    #[allow(dead_code)] // Used for detailed analysis/replay
    moves: Vec<Move>,
}

/// Aggregated match results
#[derive(Clone, Debug)]
struct MatchResults {
    games: Vec<GameRecord>,
    white_wins: usize,
    black_wins: usize,
    draws: usize,
    avg_rounds: f32,
}

// ============================================================================
// LEVEL 1 - ORCHESTRATION
// ============================================================================

/// Run match command
///
/// This function reads like a table of contents:
/// 1. Load both rulesets
/// 2. Play the match (multiple games)
/// 3. Report results
pub fn run(args: MatchArgs, seed: Option<u64>) -> Result<()> {
    let (white_rs, black_rs) = load_rulesets(&args)?;

    tracing::info!(
        "Starting match: {} vs {} ({} games, depth={})",
        white_rs.name,
        black_rs.name,
        args.games,
        args.depth
    );

    let results = play_match(&white_rs, &black_rs, &args, seed)?;

    report_results(&results, &args);

    Ok(())
}

// ============================================================================
// LEVEL 2 - PHASES
// ============================================================================

/// Load both rulesets from JSON files
fn load_rulesets(args: &MatchArgs) -> Result<(RuleSet, RuleSet)> {
    let white = RuleSet::load(&args.white)
        .with_context(|| format!("Failed to load white ruleset: {}", args.white.display()))?;

    let black = RuleSet::load(&args.black)
        .with_context(|| format!("Failed to load black ruleset: {}", args.black.display()))?;

    Ok((white, black))
}

/// Play all games in the match
fn play_match(
    white_rs: &RuleSet,
    black_rs: &RuleSet,
    args: &MatchArgs,
    seed: Option<u64>,
) -> Result<MatchResults> {
    let mut rng = create_rng(seed);
    let mut games = Vec::with_capacity(args.games);

    for game_num in 0..args.games {
        // Alternate colors for fairness
        let swap_colors = game_num % 2 == 1;

        let record = if swap_colors {
            play_single_game(black_rs, white_rs, game_num + 1, args, &mut rng)?
        } else {
            play_single_game(white_rs, black_rs, game_num + 1, args, &mut rng)?
        };

        tracing::info!(
            "Game {}: {:?} ({} rounds)",
            record.game_number,
            record.result,
            record.rounds
        );

        games.push(record);
    }

    let results = compute_match_statistics(games);
    Ok(results)
}

/// Report match results
fn report_results(results: &MatchResults, args: &MatchArgs) {
    if args.json {
        print_json_results(results);
    } else {
        print_text_results(results);
    }
}

// ============================================================================
// LEVEL 3 - STEPS
// ============================================================================

/// Play a single game between two rulesets
fn play_single_game(
    white_rs: &RuleSet,
    black_rs: &RuleSet,
    game_number: usize,
    args: &MatchArgs,
    rng: &mut ChaCha8Rng,
) -> Result<GameRecord> {
    // Create initial game state from white's army setup
    // In a real match, we'd need to combine both rulesets somehow
    // For now, use white's ruleset for setup (both armies from same ruleset)
    let state = create_game_state(white_rs, black_rs);

    let (final_state, moves) = if args.mcts {
        play_with_mcts(state, args.simulations, args.max_rounds)
    } else {
        play_with_alpha_beta(state, args.depth, args.max_rounds, rng)
    };

    Ok(GameRecord {
        game_number,
        result: final_state.result(),
        rounds: moves.len() as u32 / 2, // Each player moves once per round
        white_ruleset: white_rs.name.clone(),
        black_ruleset: black_rs.name.clone(),
        moves,
    })
}

/// Create game state from two rulesets
fn create_game_state(white_rs: &RuleSet, black_rs: &RuleSet) -> GameState {
    // Build combined setup: white army from white_rs, black army from black_rs
    let mut white_setup: Vec<(u8, hexwar_core::Hex, u8)> = Vec::new();
    let mut black_setup: Vec<(u8, hexwar_core::Hex, u8)> = Vec::new();

    // White army
    if !white_rs.white_positions.is_empty() {
        white_setup.push((
            white_rs.white_king,
            white_rs.white_positions[0],
            white_rs.white_facings.first().copied().unwrap_or(0),
        ));
    }
    for (i, &piece_type) in white_rs.white_pieces.iter().enumerate() {
        if i + 1 < white_rs.white_positions.len() {
            white_setup.push((
                piece_type,
                white_rs.white_positions[i + 1],
                white_rs.white_facings.get(i + 1).copied().unwrap_or(0),
            ));
        }
    }

    // Black army
    if !black_rs.black_positions.is_empty() {
        black_setup.push((
            black_rs.black_king,
            black_rs.black_positions[0],
            black_rs.black_facings.first().copied().unwrap_or(3),
        ));
    }
    for (i, &piece_type) in black_rs.black_pieces.iter().enumerate() {
        if i + 1 < black_rs.black_positions.len() {
            black_setup.push((
                piece_type,
                black_rs.black_positions[i + 1],
                black_rs.black_facings.get(i + 1).copied().unwrap_or(3),
            ));
        }
    }

    GameState::new(
        &white_setup,
        &black_setup,
        white_rs.white_template,
        black_rs.black_template,
    )
}

/// Play game using alpha-beta AI
fn play_with_alpha_beta(
    initial: GameState,
    depth: u32,
    max_rounds: u32,
    _rng: &mut ChaCha8Rng,
) -> (GameState, Vec<Move>) {
    let mut state = initial;
    let mut moves = Vec::new();
    let heuristics = Heuristics::default();
    let mut ai = AlphaBetaAI::new(depth, heuristics);

    let max_moves = max_rounds * 2; // Two moves per round (one per player)

    while state.result() == GameResult::Ongoing && moves.len() < max_moves as usize {
        if let Some(mv) = ai.best_move(&state) {
            state = state.apply_move(mv);
            moves.push(mv);
        } else {
            break;
        }
    }

    (state, moves)
}

/// Play game using MCTS
fn play_with_mcts(initial: GameState, simulations: usize, max_rounds: u32) -> (GameState, Vec<Move>) {
    let config = MctsConfig::cpu_only(simulations);
    let player = MctsPlayer::cpu_only(config);

    let max_moves = max_rounds * 2;
    let mut state = initial;
    let mut moves = Vec::new();

    while state.result() == GameResult::Ongoing && moves.len() < max_moves as usize {
        if let Some(mv) = player.best_move(&state) {
            state = state.apply_move(mv);
            moves.push(mv);
        } else {
            break;
        }
    }

    (state, moves)
}

/// Compute aggregate statistics from game records
fn compute_match_statistics(games: Vec<GameRecord>) -> MatchResults {
    let white_wins = games
        .iter()
        .filter(|g| g.result == GameResult::WhiteWins)
        .count();
    let black_wins = games
        .iter()
        .filter(|g| g.result == GameResult::BlackWins)
        .count();
    let draws = games
        .iter()
        .filter(|g| g.result == GameResult::Ongoing)
        .count();

    let total_rounds: u32 = games.iter().map(|g| g.rounds).sum();
    let avg_rounds = if games.is_empty() {
        0.0
    } else {
        total_rounds as f32 / games.len() as f32
    };

    MatchResults {
        games,
        white_wins,
        black_wins,
        draws,
        avg_rounds,
    }
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

/// Print results as JSON
fn print_json_results(results: &MatchResults) {
    #[derive(serde::Serialize)]
    struct JsonGame {
        game_number: usize,
        result: String,
        rounds: u32,
        white_ruleset: String,
        black_ruleset: String,
    }

    #[derive(serde::Serialize)]
    struct JsonOutput {
        total_games: usize,
        white_wins: usize,
        black_wins: usize,
        draws: usize,
        avg_rounds: f32,
        white_win_rate: f32,
        games: Vec<JsonGame>,
    }

    let total = results.games.len();
    let output = JsonOutput {
        total_games: total,
        white_wins: results.white_wins,
        black_wins: results.black_wins,
        draws: results.draws,
        avg_rounds: results.avg_rounds,
        white_win_rate: if total > 0 {
            results.white_wins as f32 / total as f32
        } else {
            0.0
        },
        games: results
            .games
            .iter()
            .map(|g| JsonGame {
                game_number: g.game_number,
                result: format!("{:?}", g.result),
                rounds: g.rounds,
                white_ruleset: g.white_ruleset.clone(),
                black_ruleset: g.black_ruleset.clone(),
            })
            .collect(),
    };

    if let Ok(json) = serde_json::to_string_pretty(&output) {
        println!("{}", json);
    }
}

/// Print results as text
fn print_text_results(results: &MatchResults) {
    let total = results.games.len();

    println!("\n=== Match Results ===");
    println!("Total games: {}", total);
    println!(
        "White wins:  {} ({:.1}%)",
        results.white_wins,
        if total > 0 {
            results.white_wins as f32 / total as f32 * 100.0
        } else {
            0.0
        }
    );
    println!(
        "Black wins:  {} ({:.1}%)",
        results.black_wins,
        if total > 0 {
            results.black_wins as f32 / total as f32 * 100.0
        } else {
            0.0
        }
    );
    println!(
        "Draws:       {} ({:.1}%)",
        results.draws,
        if total > 0 {
            results.draws as f32 / total as f32 * 100.0
        } else {
            0.0
        }
    );
    println!("Avg rounds:  {:.1}", results.avg_rounds);

    println!("\nGame details:");
    for game in &results.games {
        println!(
            "  Game {}: {:?} in {} rounds",
            game.game_number, game.result, game.rounds
        );
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_match_statistics_empty() {
        let results = compute_match_statistics(vec![]);
        assert_eq!(results.white_wins, 0);
        assert_eq!(results.black_wins, 0);
        assert_eq!(results.draws, 0);
        assert_eq!(results.avg_rounds, 0.0);
    }

    #[test]
    fn test_compute_match_statistics() {
        let games = vec![
            GameRecord {
                game_number: 1,
                result: GameResult::WhiteWins,
                rounds: 10,
                white_ruleset: "w".into(),
                black_ruleset: "b".into(),
                moves: vec![],
            },
            GameRecord {
                game_number: 2,
                result: GameResult::BlackWins,
                rounds: 20,
                white_ruleset: "w".into(),
                black_ruleset: "b".into(),
                moves: vec![],
            },
            GameRecord {
                game_number: 3,
                result: GameResult::WhiteWins,
                rounds: 30,
                white_ruleset: "w".into(),
                black_ruleset: "b".into(),
                moves: vec![],
            },
        ];

        let results = compute_match_statistics(games);
        assert_eq!(results.white_wins, 2);
        assert_eq!(results.black_wins, 1);
        assert_eq!(results.draws, 0);
        assert_eq!(results.avg_rounds, 20.0);
    }

    #[test]
    fn test_create_rng_deterministic() {
        let mut rng1 = create_rng(Some(42));
        let mut rng2 = create_rng(Some(42));

        use rand::Rng;
        assert_eq!(rng1.gen::<u64>(), rng2.gen::<u64>());
    }
}
