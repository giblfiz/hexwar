//! HEXWAR MCTS - Monte Carlo Tree Search with GPU acceleration
//!
//! This crate provides GPU-accelerated MCTS:
//! - Tree policy (UCB1)
//! - GPU-batched rollouts
//! - Backpropagation

// TODO: Agent 5 will implement MCTS

use hexwar_core::{GameState, Move};
use hexwar_gpu::GpuContext;

/// MCTS configuration
#[derive(Clone, Debug)]
pub struct MctsConfig {
    pub simulations: usize,
    pub batch_size: usize,
    pub exploration: f32,
    pub max_rollout_depth: u32,
}

impl Default for MctsConfig {
    fn default() -> Self {
        Self {
            simulations: 1000,
            batch_size: 256,
            exploration: 1.41,  // sqrt(2)
            max_rollout_depth: 50,
        }
    }
}

/// MCTS player using GPU for rollouts
pub struct MctsPlayer {
    config: MctsConfig,
    #[allow(dead_code)]
    gpu: GpuContext,
}

impl MctsPlayer {
    pub fn new(config: MctsConfig, gpu: GpuContext) -> Self {
        Self { config, gpu }
    }

    pub fn config(&self) -> &MctsConfig {
        &self.config
    }

    /// Get best move using MCTS
    pub fn best_move(&self, _state: &GameState) -> Option<Move> {
        todo!("Agent 5: Implement MCTS search")
    }

    /// Play a full game
    pub fn play_game(
        &self,
        _initial: GameState,
        _max_rounds: u32,
    ) -> (GameState, Vec<Move>) {
        todo!("Agent 5: Implement game playing")
    }
}
