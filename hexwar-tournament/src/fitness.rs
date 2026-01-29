//! Fitness evaluation for evolution
//!
//! Level 2 - Phase-level implementation

use hexwar_core::RuleSet;
use rayon::prelude::*;

use crate::config::EvalConfig;
use crate::match_play::{play_match, play_match_parallel, MatchResult};

/// Result of fitness evaluation
#[derive(Clone, Debug)]
pub struct FitnessResult {
    /// Total wins
    pub wins: u32,
    /// Total losses
    pub losses: u32,
    /// Total draws
    pub draws: u32,
    /// Average game length
    pub avg_rounds: f32,
    /// Computed fitness score (higher = better)
    pub fitness_score: f32,
    /// Games played per opponent
    pub games_per_opponent: u32,
    /// Number of opponents faced
    pub opponents_faced: u32,
}

impl FitnessResult {
    /// Create empty result
    pub fn empty() -> Self {
        Self {
            wins: 0,
            losses: 0,
            draws: 0,
            avg_rounds: 0.0,
            fitness_score: 0.0,
            games_per_opponent: 0,
            opponents_faced: 0,
        }
    }

    /// Total games played
    pub fn total_games(&self) -> u32 {
        self.wins + self.losses + self.draws
    }

    /// Win rate (wins / total games)
    pub fn win_rate(&self) -> f32 {
        let total = self.total_games();
        if total == 0 {
            0.0
        } else {
            self.wins as f32 / total as f32
        }
    }

    /// Score (wins + 0.5 * draws)
    pub fn score(&self) -> f32 {
        self.wins as f32 + 0.5 * self.draws as f32
    }

    /// Max possible score
    pub fn max_score(&self) -> f32 {
        self.total_games() as f32
    }

    /// Normalized score (0.0 to 1.0)
    pub fn normalized_score(&self) -> f32 {
        let max = self.max_score();
        if max == 0.0 {
            0.0
        } else {
            self.score() / max
        }
    }
}

/// Evaluate fitness of a candidate against a pool of opponents (Level 2 phase)
///
/// # Arguments
/// * `candidate` - The ruleset being evaluated
/// * `opponents` - Pool of opponent rulesets
/// * `config` - Evaluation configuration
///
/// # Returns
/// Fitness result with aggregated statistics
pub fn evaluate_fitness(
    candidate: &RuleSet,
    opponents: &[RuleSet],
    config: &EvalConfig,
) -> FitnessResult {
    if opponents.is_empty() {
        return FitnessResult::empty();
    }

    let match_results = play_against_opponents(candidate, opponents, config);
    aggregate_fitness(match_results, opponents.len())
}

/// Evaluate fitness in parallel across opponents
#[allow(dead_code)]
pub fn evaluate_fitness_parallel(
    candidate: &RuleSet,
    opponents: &[RuleSet],
    config: &EvalConfig,
) -> FitnessResult {
    if opponents.is_empty() {
        return FitnessResult::empty();
    }

    let match_results = play_against_opponents_parallel(candidate, opponents, config);
    aggregate_fitness(match_results, opponents.len())
}

// ============================================================================
// Level 3 - Steps
// ============================================================================

/// Play candidate against all opponents sequentially
fn play_against_opponents(
    candidate: &RuleSet,
    opponents: &[RuleSet],
    config: &EvalConfig,
) -> Vec<MatchResult> {
    opponents
        .iter()
        .map(|opponent| {
            play_matches_vs_opponent(candidate, opponent, config)
        })
        .collect()
}

/// Play candidate against all opponents in parallel
#[allow(dead_code)]
fn play_against_opponents_parallel(
    candidate: &RuleSet,
    opponents: &[RuleSet],
    config: &EvalConfig,
) -> Vec<MatchResult> {
    opponents
        .par_iter()
        .map(|opponent| {
            play_matches_vs_opponent(candidate, opponent, config)
        })
        .collect()
}

/// Play matches against a single opponent
fn play_matches_vs_opponent(
    candidate: &RuleSet,
    opponent: &RuleSet,
    config: &EvalConfig,
) -> MatchResult {
    // Play with candidate as white, then as black
    let as_white = if config.parallel {
        play_match_parallel(
            candidate,
            opponent,
            config.ai_config.clone(),
            config.games_per_opponent / 2,
            config.max_rounds,
        )
    } else {
        play_match(
            candidate,
            opponent,
            config.ai_config.clone(),
            config.games_per_opponent / 2,
            config.max_rounds,
        )
    };

    let as_black = if config.parallel {
        play_match_parallel(
            opponent,
            candidate,
            config.ai_config.clone(),
            config.games_per_opponent - config.games_per_opponent / 2,
            config.max_rounds,
        )
    } else {
        play_match(
            opponent,
            candidate,
            config.ai_config.clone(),
            config.games_per_opponent - config.games_per_opponent / 2,
            config.max_rounds,
        )
    };

    // Combine results from candidate's perspective
    // as_white: candidate was white_ruleset, so white_wins/black_wins are direct
    // as_black: candidate was black_ruleset, so we need to swap
    let from_white = MatchResult {
        white_wins: as_white.white_wins,  // Candidate's wins as white
        black_wins: as_white.black_wins,  // Candidate's losses (opponent wins)
        draws: as_white.draws,
        avg_rounds: as_white.avg_rounds,
        games_played: as_white.games_played,
        game_outcomes: as_white.game_outcomes,
    };

    let from_black = MatchResult {
        white_wins: as_black.black_wins,  // Candidate's wins as black (was labeled black)
        black_wins: as_black.white_wins,  // Candidate's losses (opponent was white)
        draws: as_black.draws,
        avg_rounds: as_black.avg_rounds,
        games_played: as_black.games_played,
        game_outcomes: as_black.game_outcomes,
    };

    from_white.combine(&from_black)
}

/// Aggregate match results into fitness result
fn aggregate_fitness(results: Vec<MatchResult>, opponents_count: usize) -> FitnessResult {
    let mut total_wins = 0u32;
    let mut total_losses = 0u32;
    let mut total_draws = 0u32;
    let mut total_rounds = 0f32;
    let mut total_games = 0u32;

    for result in &results {
        total_wins += result.white_wins;     // Candidate's wins
        total_losses += result.black_wins;   // Candidate's losses
        total_draws += result.draws;
        total_rounds += result.avg_rounds * result.games_played as f32;
        total_games += result.games_played;
    }

    let avg_rounds = if total_games > 0 {
        total_rounds / total_games as f32
    } else {
        0.0
    };

    let games_per_opponent = if opponents_count > 0 {
        total_games / opponents_count as u32
    } else {
        0
    };

    // Compute fitness score
    // Base: win rate scaled to roughly 0-100
    // Bonus for more decisive victories (fewer draws)
    let win_rate = if total_games > 0 {
        total_wins as f32 / total_games as f32
    } else {
        0.0
    };

    let draw_rate = if total_games > 0 {
        total_draws as f32 / total_games as f32
    } else {
        0.0
    };

    // Fitness = win_rate * 100 + draw_penalty (draws are worth half a win)
    // This gives values roughly in range 0-100
    let fitness_score = (win_rate + 0.5 * draw_rate) * 100.0;

    FitnessResult {
        wins: total_wins,
        losses: total_losses,
        draws: total_draws,
        avg_rounds,
        fitness_score,
        games_per_opponent,
        opponents_faced: opponents_count as u32,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AiConfig;
    use hexwar_core::board::Hex;
    use hexwar_core::game::Template;

    fn make_test_ruleset(name: &str) -> RuleSet {
        RuleSet {
            name: name.to_string(),
            white_king: 25,
            white_pieces: vec![1, 1, 1, 1],
            white_positions: vec![
                Hex::new(0, 3),
                Hex::new(-1, 3),
                Hex::new(1, 2),
                Hex::new(-2, 3),
                Hex::new(2, 1),
            ],
            white_facings: vec![0; 5],
            white_template: Template::E,
            black_king: 25,
            black_pieces: vec![1, 1, 1, 1],
            black_positions: vec![
                Hex::new(0, -3),
                Hex::new(1, -3),
                Hex::new(-1, -2),
                Hex::new(2, -3),
                Hex::new(-2, -1),
            ],
            black_facings: vec![3; 5],
            black_template: Template::E,
        }
    }

    #[test]
    fn test_fitness_result_empty() {
        let result = FitnessResult::empty();
        assert_eq!(result.total_games(), 0);
        assert_eq!(result.win_rate(), 0.0);
        assert_eq!(result.normalized_score(), 0.0);
    }

    #[test]
    fn test_fitness_result_calculations() {
        let result = FitnessResult {
            wins: 6,
            losses: 2,
            draws: 2,
            avg_rounds: 25.0,
            fitness_score: 70.0,
            games_per_opponent: 5,
            opponents_faced: 2,
        };

        assert_eq!(result.total_games(), 10);
        assert_eq!(result.win_rate(), 0.6);
        assert_eq!(result.score(), 7.0);  // 6 + 0.5 * 2
        assert_eq!(result.max_score(), 10.0);
        assert_eq!(result.normalized_score(), 0.7);
    }

    #[test]
    fn test_evaluate_fitness_empty_opponents() {
        let candidate = make_test_ruleset("candidate");
        let opponents: Vec<RuleSet> = vec![];
        let config = EvalConfig::default();

        let result = evaluate_fitness(&candidate, &opponents, &config);
        assert_eq!(result.total_games(), 0);
    }

    #[test]
    fn test_evaluate_fitness_basic() {
        let candidate = make_test_ruleset("candidate");
        let opponent = make_test_ruleset("opponent");
        let config = EvalConfig {
            ai_config: AiConfig::alpha_beta(1).with_seed(42),
            games_per_opponent: 2,
            parallel: false,
            max_rounds: 20,
        };

        let result = evaluate_fitness(&candidate, &[opponent], &config);

        assert_eq!(result.opponents_faced, 1);
        assert!(result.total_games() > 0);
        // With symmetric armies and identical AI, results should be roughly even
    }

    #[test]
    fn test_aggregate_fitness() {
        let results = vec![
            MatchResult {
                white_wins: 3,
                black_wins: 1,
                draws: 1,
                avg_rounds: 20.0,
                games_played: 5,
                game_outcomes: vec![],
            },
            MatchResult {
                white_wins: 2,
                black_wins: 2,
                draws: 1,
                avg_rounds: 25.0,
                games_played: 5,
                game_outcomes: vec![],
            },
        ];

        let fitness = aggregate_fitness(results, 2);

        assert_eq!(fitness.wins, 5);
        assert_eq!(fitness.losses, 3);
        assert_eq!(fitness.draws, 2);
        assert_eq!(fitness.opponents_faced, 2);
        assert_eq!(fitness.total_games(), 10);
    }
}
