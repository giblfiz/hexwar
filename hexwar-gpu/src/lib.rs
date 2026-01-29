//! HEXWAR GPU - CUDA-accelerated game simulation
//!
//! This crate provides GPU-parallel game simulation:
//! - Batch game simulation (1000+ games simultaneously)
//! - Random rollout policy for MCTS
//! - Compact game state for GPU memory

// TODO: Agent 2 will implement CUDA integration

use hexwar_core::{GameState, GameResult};

/// Handle to GPU resources
pub struct GpuContext {
    // TODO: CUDA context
}

/// Results of batch simulation
pub struct GpuGameResults {
    outcomes: Vec<GameOutcome>,
}

#[derive(Clone, Debug)]
pub struct GameOutcome {
    pub result: GameResult,
    pub rounds: u32,
    pub final_eval: f32,
}

impl GpuContext {
    pub fn new() -> anyhow::Result<Self> {
        todo!("Agent 2: Initialize CUDA context")
    }

    pub fn simulate_batch(
        &self,
        _states: &[GameState],
        _max_moves: u32,
        _seed: u64,
    ) -> GpuGameResults {
        todo!("Agent 2: GPU batch simulation")
    }
}

impl GpuGameResults {
    pub fn len(&self) -> usize {
        self.outcomes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.outcomes.is_empty()
    }

    pub fn download(&self) -> Vec<GameOutcome> {
        self.outcomes.clone()
    }
}
