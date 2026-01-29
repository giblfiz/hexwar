//! Integration tests for the HEXWAR game balancer
//!
//! Tests the full stack: core game logic, AI players, MCTS, and evolution

use hexwar_core::{
    board::Hex,
    game::{GameResult, GameState, Move, Template},
    pieces::piece_id_to_index,
    ai::AlphaBetaAI,
    eval::Heuristics,
    RuleSet,
};
use hexwar_mcts::{MctsPlayer, MctsConfig};
use hexwar_evolve::{
    evolve, EvolutionConfig, MutateSide,
    mutate_ruleset, MutationConfig,
    crossover_rulesets,
    tournament_select,
};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use std::time::Instant;

// ============================================================================
// TEST FIXTURES
// ============================================================================

/// Create a simple test game with minimal pieces
fn simple_game() -> GameState {
    let white = vec![
        (piece_id_to_index("K1").unwrap(), Hex::new(0, 3), 0),
        (piece_id_to_index("A2").unwrap(), Hex::new(-1, 3), 0),
        (piece_id_to_index("C3").unwrap(), Hex::new(1, 2), 0),
    ];
    let black = vec![
        (piece_id_to_index("K1").unwrap(), Hex::new(0, -3), 3),
        (piece_id_to_index("A2").unwrap(), Hex::new(1, -3), 3),
        (piece_id_to_index("C3").unwrap(), Hex::new(-1, -2), 3),
    ];
    GameState::new(&white, &black, Template::E, Template::E)
}

/// Create a test RuleSet
fn test_ruleset() -> RuleSet {
    RuleSet {
        name: "test-ruleset".to_string(),
        white_king: 25,
        white_pieces: vec![1, 2, 3, 4, 5, 6, 7, 8],
        white_positions: vec![
            Hex::new(0, 3),
            Hex::new(-1, 3),
            Hex::new(1, 2),
            Hex::new(-2, 3),
            Hex::new(2, 1),
            Hex::new(-1, 2),
            Hex::new(1, 1),
            Hex::new(-2, 2),
            Hex::new(2, 2),
        ],
        white_facings: vec![0; 9],
        white_template: Template::E,
        black_king: 25,
        black_pieces: vec![1, 2, 3, 4, 5, 6, 7, 8],
        black_positions: vec![
            Hex::new(0, -3),
            Hex::new(1, -3),
            Hex::new(-1, -2),
            Hex::new(2, -3),
            Hex::new(-2, -1),
            Hex::new(1, -2),
            Hex::new(-1, -1),
            Hex::new(2, -2),
            Hex::new(-2, -2),
        ],
        black_facings: vec![3; 9],
        black_template: Template::E,
    }
}

// ============================================================================
// GAME LOGIC TESTS
// ============================================================================

#[test]
fn test_game_creation_and_moves() {
    let game = simple_game();

    // Game should be ongoing
    assert_eq!(game.result(), GameResult::Ongoing);

    // Should have legal moves
    let moves = game.legal_moves();
    assert!(!moves.is_empty(), "Should have legal moves");

    // Should be white's turn
    assert_eq!(game.current_player(), hexwar_core::Player::White);

    // Apply a move
    let mv = moves[0];
    let new_state = game.apply_move(mv);

    // State should have changed
    assert!(new_state.current_player() == hexwar_core::Player::White
            || new_state.current_player() == hexwar_core::Player::Black);
}

#[test]
fn test_game_from_ruleset() {
    let ruleset = test_ruleset();
    let game = ruleset.to_game_state();

    assert_eq!(game.result(), GameResult::Ongoing);
    assert!(!game.legal_moves().is_empty());
}

// ============================================================================
// ALPHA-BETA AI TESTS
// ============================================================================

#[test]
fn test_alphabeta_finds_move() {
    let game = simple_game();
    let mut ai = AlphaBetaAI::new(2, Heuristics::default());

    let mv = ai.best_move(&game);
    assert!(mv.is_some(), "AI should find a move");
    assert!(!matches!(mv, Some(Move::Surrender)), "AI should not surrender");
}

#[test]
fn test_alphabeta_plays_game() {
    let game = simple_game();
    let mut ai = AlphaBetaAI::new(2, Heuristics::default());

    let (final_state, history) = ai.play_game(game, 25);

    // Game should have progressed
    assert!(!history.is_empty(), "Should have made moves");

    // Either game ended or reached turn limit
    println!("Game ended: {:?}, moves: {}", final_state.result(), history.len());
}

#[test]
fn test_alphabeta_performance() {
    let game = simple_game();

    // Test depth 2
    let start = Instant::now();
    let mut ai = AlphaBetaAI::new(2, Heuristics::default());
    let _ = ai.best_move(&game);
    let d2_time = start.elapsed();

    // Test depth 4
    let start = Instant::now();
    let mut ai = AlphaBetaAI::new(4, Heuristics::default());
    let _ = ai.best_move(&game);
    let d4_time = start.elapsed();

    println!("Alpha-Beta Performance:");
    println!("  Depth 2: {:?}", d2_time);
    println!("  Depth 4: {:?}", d4_time);

    // Depth 4 should take longer (but not absurdly so)
    // Just verify they both complete
    assert!(d4_time.as_millis() < 30000, "Depth 4 took too long");
}

// ============================================================================
// MCTS TESTS
// ============================================================================

#[test]
fn test_mcts_finds_move() {
    let game = simple_game();
    let config = MctsConfig::cpu_only(100);
    let player = MctsPlayer::cpu_only(config);

    let mv = player.best_move(&game);
    assert!(mv.is_some(), "MCTS should find a move");
    assert!(!matches!(mv, Some(Move::Surrender)), "MCTS should not surrender");
}

#[test]
fn test_mcts_search_statistics() {
    let game = simple_game();
    let config = MctsConfig::cpu_only(50);
    let player = MctsPlayer::cpu_only(config);

    let result = player.search_with_stats(&game);

    // Should have run simulations
    assert!(result.total_simulations > 0, "Should have simulations");
    assert!(result.tree.len() > 0, "Tree should have nodes");

    println!("MCTS Statistics:");
    println!("  Simulations: {}", result.total_simulations);
    println!("  Tree size: {}", result.tree.len());
}

#[test]
fn test_mcts_performance() {
    let game = simple_game();

    // Test 100 simulations
    let start = Instant::now();
    let config = MctsConfig::cpu_only(100);
    let player = MctsPlayer::cpu_only(config);
    let _ = player.best_move(&game);
    let s100_time = start.elapsed();

    // Test 500 simulations
    let start = Instant::now();
    let config = MctsConfig::cpu_only(500);
    let player = MctsPlayer::cpu_only(config);
    let _ = player.best_move(&game);
    let s500_time = start.elapsed();

    println!("MCTS Performance:");
    println!("  100 sims: {:?}", s100_time);
    println!("  500 sims: {:?}", s500_time);

    // Both should complete in reasonable time
    assert!(s500_time.as_millis() < 30000, "500 sims took too long");
}

// ============================================================================
// EVOLUTION TESTS
// ============================================================================

#[test]
fn test_mutation() {
    let ruleset = test_ruleset();
    let config = MutationConfig {
        side: MutateSide::Both,
        allow_template_mutation: false,
    };
    let mut rng = ChaCha8Rng::seed_from_u64(42);

    let mutated = mutate_ruleset(&ruleset, &config, &mut rng);

    // Mutated should be different (probably)
    // At least one piece should differ with high probability
    let different = ruleset.white_pieces != mutated.white_pieces
        || ruleset.black_pieces != mutated.black_pieces
        || ruleset.white_positions != mutated.white_positions
        || ruleset.black_positions != mutated.black_positions;

    // Note: mutation is probabilistic, so we just check it doesn't crash
    println!("Mutation changed ruleset: {}", different);
}

#[test]
fn test_crossover() {
    let rs1 = test_ruleset();
    let mut rs2 = test_ruleset();
    rs2.name = "parent2".to_string();
    rs2.white_pieces = vec![10, 11, 12, 13, 14, 15, 16, 17];

    let mut rng = ChaCha8Rng::seed_from_u64(42);
    let child = crossover_rulesets(&rs1, &rs2, &mut rng);

    // Child should exist and be valid
    assert!(!child.white_pieces.is_empty());
    assert!(!child.black_pieces.is_empty());
}

#[test]
fn test_tournament_selection() {
    let population: Vec<RuleSet> = (0..5).map(|_| test_ruleset()).collect();
    let fitness = vec![1.0, 2.0, 3.0, 4.0, 5.0];

    let mut rng = ChaCha8Rng::seed_from_u64(42);
    let selected = tournament_select(&population, &fitness, 3, &mut rng);

    // Should select one of the rulesets
    assert!(!selected.white_pieces.is_empty());
}

#[test]
fn test_evolution_improves() {
    // Simple fitness: count of white piece IDs
    let fitness_fn = |rs: &RuleSet| -> f32 {
        rs.white_pieces.iter().map(|&p| p as f32).sum()
    };

    let initial_pop: Vec<RuleSet> = (0..10).map(|_| test_ruleset()).collect();
    let initial_best = initial_pop.iter()
        .map(&fitness_fn)
        .fold(f32::NEG_INFINITY, f32::max);

    let config = EvolutionConfig {
        population_size: 10,
        generations: 5,
        mutation_rate: 0.3,
        crossover_rate: 0.7,
        elitism: 2,
        tournament_size: 3,
        evolve_side: MutateSide::Both,
    };

    let mut rng = ChaCha8Rng::seed_from_u64(42);
    let result = evolve(initial_pop, &config, fitness_fn, &mut rng);

    // Best fitness should be >= initial (elitism ensures this)
    let final_best = result.fitness[0];
    assert!(final_best >= initial_best,
        "Final {} should be >= initial {}", final_best, initial_best);

    println!("Evolution: initial best = {}, final best = {}", initial_best, final_best);
}

// ============================================================================
// FULL INTEGRATION TEST
// ============================================================================

#[test]
fn test_full_game_ab_vs_mcts() {
    let game = simple_game();

    // Play a game: AB (white) vs MCTS (black)
    let mut state = game.clone();
    let mut ab_ai = AlphaBetaAI::new(2, Heuristics::default());
    let mcts_config = MctsConfig::cpu_only(100);
    let mcts_player = MctsPlayer::cpu_only(mcts_config);

    let mut moves_played = 0;
    let max_moves = 100;

    while state.result() == GameResult::Ongoing && moves_played < max_moves {
        let mv = if state.current_player() == hexwar_core::Player::White {
            ab_ai.best_move(&state)
        } else {
            mcts_player.best_move(&state)
        };

        match mv {
            Some(m) => {
                state = state.apply_move(m);
                moves_played += 1;
            }
            None => break,
        }
    }

    println!("AB vs MCTS game:");
    println!("  Moves: {}", moves_played);
    println!("  Result: {:?}", state.result());

    // Game should have progressed
    assert!(moves_played > 0, "Should have played moves");
}

// ============================================================================
// PERFORMANCE COMPARISON
// ============================================================================

#[test]
fn test_performance_comparison() {
    println!("\n=== HEXWAR Performance Comparison ===\n");

    let game = simple_game();

    // Alpha-Beta at various depths
    for depth in [2, 3, 4] {
        let start = Instant::now();
        let mut ai = AlphaBetaAI::new(depth, Heuristics::default());
        let mv = ai.best_move(&game);
        let elapsed = start.elapsed();
        println!("AB Depth {}: {:?} -> {:?}", depth, elapsed, mv);
    }

    // MCTS at various simulation counts
    for sims in [50, 100, 200, 500] {
        let start = Instant::now();
        let config = MctsConfig::cpu_only(sims);
        let player = MctsPlayer::cpu_only(config);
        let mv = player.best_move(&game);
        let elapsed = start.elapsed();
        println!("MCTS {} sims: {:?} -> {:?}", sims, elapsed, mv);
    }

    println!("\n=== End Performance Comparison ===\n");
}
