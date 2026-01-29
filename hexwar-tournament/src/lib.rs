//! HEXWAR Tournament - Fitness evaluation through game playing
//!
//! This crate provides tournament infrastructure:
//! - Game running (CPU or GPU)
//! - Fitness calculation
//! - Matchup management

// TODO: Agent 4 will port from hexwar/tournament.py

use hexwar_core::RuleSet;

/// Tournament configuration
#[derive(Clone, Debug)]
pub struct TournamentConfig {
    pub games_per_matchup: usize,
    pub depth: u32,
    pub use_gpu: bool,
    pub workers: usize,
}

impl Default for TournamentConfig {
    fn default() -> Self {
        Self {
            games_per_matchup: 10,
            depth: 4,
            use_gpu: true,
            workers: 8,
        }
    }
}

/// Result of fitness evaluation
#[derive(Clone, Debug)]
pub struct FitnessResult {
    pub wins: u32,
    pub losses: u32,
    pub draws: u32,
    pub avg_rounds: f32,
    pub fitness_score: f32,
}

/// Tournament runner
pub struct Tournament {
    config: TournamentConfig,
}

impl Tournament {
    pub fn new(config: TournamentConfig) -> Self {
        Self { config }
    }

    /// Evaluate fitness of candidate against fixed opponent
    pub fn evaluate_vs_fixed(
        &self,
        _candidate: &RuleSet,
        _opponent: &RuleSet,
    ) -> FitnessResult {
        todo!("Agent 4: Implement fitness evaluation")
    }

    /// Round-robin tournament
    pub fn round_robin(&self, _population: &[RuleSet]) -> Vec<FitnessResult> {
        todo!("Agent 4: Implement round-robin")
    }
}
