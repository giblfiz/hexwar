//! HEXWAR Tournament - Fitness evaluation through game playing
//!
//! This crate provides tournament infrastructure:
//! - Match play between rulesets
//! - Fitness evaluation against opponent pools
//! - Tournament formats (round-robin, Swiss)
//!
//! ## Architecture (4-layer granularity)
//!
//! - Level 1: run_tournament (orchestration)
//! - Level 2: evaluate_fitness, play_match (phases)
//! - Level 3: play_game, compute_fitness (steps)
//! - Level 4: utilities, configuration

mod config;
mod fitness;
mod game_runner;
mod match_play;
mod tournament;

pub use config::{AiConfig, EvalConfig, PlayerType, TournamentConfig, TournamentFormat};
pub use fitness::{evaluate_fitness, FitnessResult};
pub use game_runner::GameRunner;
pub use match_play::{play_match, MatchResult};
pub use tournament::{run_tournament, TournamentResult, Standing};
