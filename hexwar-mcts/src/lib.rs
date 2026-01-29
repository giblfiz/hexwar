//! HEXWAR MCTS - Monte Carlo Tree Search with GPU acceleration
//!
//! This crate provides GPU-accelerated MCTS:
//! - Tree policy (UCB1 for selection)
//! - GPU-batched rollouts (with CPU fallback)
//! - Backpropagation of results
//!
//! ## Architecture (4-layer granularity)
//!
//! - Level 1: MctsPlayer (orchestration)
//! - Level 2: search loop, tree operations
//! - Level 3: UCB1 calculation, expansion, backprop
//! - Level 4: utilities, node accessors

mod tree;
mod search;
mod rollout;

pub use tree::{MctsTree, NodeId};
pub use search::SearchResult;

use hexwar_core::{GameState, Move, GameResult};
use hexwar_gpu::GpuContext;

// ============================================================================
// CONFIGURATION
// ============================================================================

/// MCTS configuration
#[derive(Clone, Debug)]
pub struct MctsConfig {
    /// Total simulations per move decision
    pub simulations: usize,
    /// Batch size for GPU rollouts
    pub batch_size: usize,
    /// UCB1 exploration constant (C)
    pub exploration: f32,
    /// Maximum rollout depth
    pub max_rollout_depth: u32,
    /// Whether to use GPU for rollouts (falls back to CPU if false or unavailable)
    pub use_gpu: bool,
}

impl Default for MctsConfig {
    fn default() -> Self {
        Self {
            simulations: 1000,
            batch_size: 256,
            exploration: 1.41421356, // sqrt(2)
            max_rollout_depth: 50,
            use_gpu: true,
        }
    }
}

impl MctsConfig {
    /// Create config for CPU-only operation
    pub fn cpu_only(simulations: usize) -> Self {
        Self {
            simulations,
            batch_size: 1,
            use_gpu: false,
            ..Default::default()
        }
    }

    /// Create config with specific exploration constant
    pub fn with_exploration(mut self, c: f32) -> Self {
        self.exploration = c;
        self
    }
}

// ============================================================================
// MCTS PLAYER (Level 1 - Orchestration)
// ============================================================================

/// MCTS player using GPU for rollouts
pub struct MctsPlayer {
    config: MctsConfig,
    gpu: Option<GpuContext>,
}

impl MctsPlayer {
    /// Create MCTS player with GPU context
    pub fn new(config: MctsConfig, gpu: GpuContext) -> Self {
        Self {
            config,
            gpu: Some(gpu),
        }
    }

    /// Create MCTS player without GPU (CPU fallback only)
    pub fn cpu_only(config: MctsConfig) -> Self {
        Self {
            config,
            gpu: None,
        }
    }

    /// Get configuration
    pub fn config(&self) -> &MctsConfig {
        &self.config
    }

    /// Get best move using MCTS (Level 1 orchestration)
    ///
    /// This function reads like a table of contents:
    /// 1. Initialize search tree
    /// 2. Run MCTS search loop
    /// 3. Extract best move from results
    pub fn best_move(&self, state: &GameState) -> Option<Move> {
        let tree = MctsTree::new(state.clone());
        let result = search::run_search(tree, &self.config, self.gpu.as_ref());
        result.best_move()
    }

    /// Play a full game using MCTS for both sides
    ///
    /// Level 1 orchestration:
    /// 1. Initialize game state
    /// 2. Alternate moves until game ends
    /// 3. Collect move history
    pub fn play_game(
        &self,
        initial: GameState,
        max_rounds: u32,
    ) -> (GameState, Vec<Move>) {
        let mut state = initial;
        let mut moves = Vec::new();
        let mut rounds = 0;

        while state.result() == GameResult::Ongoing && rounds < max_rounds {
            if let Some(mv) = self.best_move(&state) {
                state = state.apply_move(mv);
                moves.push(mv);
            } else {
                // No legal moves - this shouldn't happen in normal play
                break;
            }
            rounds += 1;
        }

        (state, moves)
    }

    /// Run MCTS and return detailed search statistics
    pub fn search_with_stats(&self, state: &GameState) -> SearchResult {
        let tree = MctsTree::new(state.clone());
        search::run_search(tree, &self.config, self.gpu.as_ref())
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_defaults() {
        let config = MctsConfig::default();
        assert_eq!(config.simulations, 1000);
        assert_eq!(config.batch_size, 256);
        assert!((config.exploration - 1.41421356).abs() < 0.0001);
    }

    #[test]
    fn test_config_cpu_only() {
        let config = MctsConfig::cpu_only(500);
        assert_eq!(config.simulations, 500);
        assert!(!config.use_gpu);
    }

    #[test]
    fn test_config_with_exploration() {
        let config = MctsConfig::default().with_exploration(2.0);
        assert_eq!(config.exploration, 2.0);
    }
}
