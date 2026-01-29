//! Rollout (simulation) strategies for MCTS
//!
//! Provides both CPU and GPU rollout implementations.
//!
//! ## Architecture
//! - Level 2: Batch rollout coordination
//! - Level 3: Single rollout implementation
//! - Level 4: Random move selection

use hexwar_core::{GameState, GameResult, Move};
use hexwar_gpu::{GpuContext, GameOutcome};
use rand::prelude::*;

// ============================================================================
// ROLLOUT RESULT
// ============================================================================

/// Result of a rollout simulation
#[derive(Clone, Debug)]
pub struct RolloutResult {
    /// Final game result
    pub result: GameResult,
    /// Number of moves played
    pub moves_played: u32,
}

impl RolloutResult {
    pub fn from_game_outcome(outcome: &GameOutcome) -> Self {
        Self {
            result: outcome.result,
            moves_played: outcome.rounds,
        }
    }
}

// ============================================================================
// CPU ROLLOUT (Level 3 - Single Rollout)
// ============================================================================

/// Perform a single random rollout on CPU
///
/// Plays random legal moves until the game ends or max_depth is reached.
pub fn cpu_rollout<R: Rng>(state: &GameState, max_depth: u32, rng: &mut R) -> RolloutResult {
    let mut current = state.clone();
    let mut moves_played = 0;

    while current.result() == GameResult::Ongoing && moves_played < max_depth {
        let legal_moves = current.legal_moves();

        if legal_moves.is_empty() {
            // No legal moves - game should have ended, break
            break;
        }

        // Random move selection (uniform)
        let mv = select_random_move(&legal_moves, rng);
        current = current.apply_move(mv);
        moves_played += 1;
    }

    RolloutResult {
        result: current.result(),
        moves_played,
    }
}

/// Select a random move uniformly from the list
fn select_random_move<R: Rng>(moves: &[Move], rng: &mut R) -> Move {
    let idx = rng.gen_range(0..moves.len());
    moves[idx]
}

// ============================================================================
// BATCH ROLLOUT (Level 2 - Batch Coordination)
// ============================================================================

/// Rollout engine that can use GPU or CPU
pub struct RolloutEngine<'a> {
    gpu: Option<&'a GpuContext>,
    max_depth: u32,
    seed: u64,
}

impl<'a> RolloutEngine<'a> {
    /// Create a new rollout engine
    pub fn new(gpu: Option<&'a GpuContext>, max_depth: u32, seed: u64) -> Self {
        Self {
            gpu,
            max_depth,
            seed,
        }
    }

    /// Perform batch rollouts
    ///
    /// Uses GPU if available, otherwise falls back to CPU.
    pub fn rollout_batch(&self, states: &[GameState]) -> Vec<RolloutResult> {
        if let Some(gpu) = self.gpu {
            self.gpu_rollout_batch(gpu, states)
        } else {
            self.cpu_rollout_batch(states)
        }
    }

    /// GPU batch rollout
    fn gpu_rollout_batch(&self, gpu: &GpuContext, states: &[GameState]) -> Vec<RolloutResult> {
        match gpu.simulate_batch(states, self.max_depth, self.seed) {
            Ok(gpu_results) => {
                gpu_results
                    .download()
                    .iter()
                    .map(RolloutResult::from_game_outcome)
                    .collect()
            }
            Err(_) => {
                // GPU failed, fall back to CPU
                self.cpu_rollout_batch(states)
            }
        }
    }

    /// CPU batch rollout (sequential)
    fn cpu_rollout_batch(&self, states: &[GameState]) -> Vec<RolloutResult> {
        // Use different seed for each state to get variety
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(self.seed);

        states
            .iter()
            .map(|state| cpu_rollout(state, self.max_depth, &mut rng))
            .collect()
    }
}

// ============================================================================
// PARALLEL CPU ROLLOUT
// ============================================================================

/// Parallel CPU rollouts using rayon
#[cfg(feature = "parallel")]
pub fn parallel_cpu_rollouts(
    states: &[GameState],
    max_depth: u32,
    seed: u64,
) -> Vec<RolloutResult> {
    use rayon::prelude::*;

    states
        .par_iter()
        .enumerate()
        .map(|(i, state)| {
            // Each thread gets a unique seed based on index
            let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(seed.wrapping_add(i as u64));
            cpu_rollout(state, max_depth, &mut rng)
        })
        .collect()
}

// ============================================================================
// IMPROVED ROLLOUT POLICIES (Future Enhancement)
// ============================================================================
// These are placeholders for future enhancement.
// Suppressing dead_code warnings since they will be used later.

/// A rollout policy determines how to select moves during simulation
#[allow(dead_code)]
pub trait RolloutPolicy: Send + Sync {
    /// Select a move from the list of legal moves
    fn select_move<R: Rng>(&self, state: &GameState, moves: &[Move], rng: &mut R) -> Move;
}

/// Uniform random policy - all moves equally likely
#[allow(dead_code)]
pub struct UniformPolicy;

#[allow(dead_code)]
impl RolloutPolicy for UniformPolicy {
    fn select_move<R: Rng>(&self, _state: &GameState, moves: &[Move], rng: &mut R) -> Move {
        select_random_move(moves, rng)
    }
}

/// Heavy playout policy - biased toward captures and good moves
///
/// This is a placeholder for future enhancement. In practice,
/// heavy playouts can significantly improve MCTS quality.
#[allow(dead_code)]
pub struct HeavyPolicy {
    /// Weight for capture moves
    pub capture_weight: f32,
    /// Weight for central moves
    pub center_weight: f32,
}

#[allow(dead_code)]
impl Default for HeavyPolicy {
    fn default() -> Self {
        Self {
            capture_weight: 3.0,
            center_weight: 1.5,
        }
    }
}

#[allow(dead_code)]
impl RolloutPolicy for HeavyPolicy {
    fn select_move<R: Rng>(&self, _state: &GameState, moves: &[Move], rng: &mut R) -> Move {
        // TODO: Implement weighted selection based on move type
        // For now, falls back to uniform
        select_random_move(moves, rng)
    }
}

/// Perform a rollout with a custom policy
#[allow(dead_code)]
pub fn cpu_rollout_with_policy<R: Rng, P: RolloutPolicy>(
    state: &GameState,
    max_depth: u32,
    policy: &P,
    rng: &mut R,
) -> RolloutResult {
    let mut current = state.clone();
    let mut moves_played = 0;

    while current.result() == GameResult::Ongoing && moves_played < max_depth {
        let legal_moves = current.legal_moves();

        if legal_moves.is_empty() {
            break;
        }

        let mv = policy.select_move(&current, &legal_moves, rng);
        current = current.apply_move(mv);
        moves_played += 1;
    }

    RolloutResult {
        result: current.result(),
        moves_played,
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use hexwar_core::board::Hex;
    use hexwar_core::game::Template;
    use hexwar_core::pieces::piece_id_to_index;

    fn mock_state() -> GameState {
        let white = vec![
            (piece_id_to_index("K1").unwrap(), Hex::new(0, 3), 0),
        ];
        let black = vec![
            (piece_id_to_index("K1").unwrap(), Hex::new(0, -3), 3),
        ];
        GameState::new(&white, &black, Template::E, Template::E)
    }

    #[test]
    fn test_rollout_result_from_outcome() {
        let outcome = GameOutcome {
            result: GameResult::WhiteWins,
            rounds: 42,
            final_eval: 1.0,
        };

        let result = RolloutResult::from_game_outcome(&outcome);
        assert_eq!(result.result, GameResult::WhiteWins);
        assert_eq!(result.moves_played, 42);
    }

    #[test]
    fn test_rollout_engine_creation() {
        let engine = RolloutEngine::new(None, 50, 12345);
        assert_eq!(engine.max_depth, 50);
        assert!(engine.gpu.is_none());
    }

    #[test]
    fn test_uniform_policy() {
        let policy = UniformPolicy;
        let state = mock_state();
        let moves = vec![Move::Pass, Move::Surrender];

        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(42);

        // Just verify it returns a valid move
        let selected = policy.select_move(&state, &moves, &mut rng);
        assert!(moves.contains(&selected));
    }

    #[test]
    fn test_heavy_policy_default() {
        let policy = HeavyPolicy::default();
        assert_eq!(policy.capture_weight, 3.0);
        assert_eq!(policy.center_weight, 1.5);
    }

    #[test]
    fn test_cpu_rollout() {
        let state = mock_state();
        let mut rng = rand_chacha::ChaCha8Rng::seed_from_u64(42);

        // Run a rollout - should complete without panicking
        let result = cpu_rollout(&state, 10, &mut rng);

        // Result should be valid
        assert!(result.moves_played <= 10);
    }

    #[test]
    fn test_rollout_batch() {
        let states = vec![mock_state(), mock_state()];
        let engine = RolloutEngine::new(None, 10, 42);

        let results = engine.rollout_batch(&states);
        assert_eq!(results.len(), 2);
    }
}
