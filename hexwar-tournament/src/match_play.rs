//! Match play - multiple games between two rulesets
//!
//! Level 2 - Phase-level implementation

use hexwar_core::RuleSet;
use rayon::prelude::*;

use crate::config::AiConfig;
use crate::game_runner::{GameOutcome, GameRunner};

/// Result of a match (multiple games)
#[derive(Clone, Debug)]
pub struct MatchResult {
    /// Wins for white player
    pub white_wins: u32,
    /// Wins for black player
    pub black_wins: u32,
    /// Draws (games that ended without a winner)
    pub draws: u32,
    /// Average game length in rounds
    pub avg_rounds: f32,
    /// Total games played
    pub games_played: u32,
    /// Individual game outcomes
    pub game_outcomes: Vec<GameOutcome>,
}

impl MatchResult {
    /// Create empty result
    pub fn empty() -> Self {
        Self {
            white_wins: 0,
            black_wins: 0,
            draws: 0,
            avg_rounds: 0.0,
            games_played: 0,
            game_outcomes: Vec::new(),
        }
    }

    /// Combine two results
    pub fn combine(&self, other: &MatchResult) -> MatchResult {
        let total_games = self.games_played + other.games_played;
        let avg_rounds = if total_games > 0 {
            (self.avg_rounds * self.games_played as f32
                + other.avg_rounds * other.games_played as f32)
                / total_games as f32
        } else {
            0.0
        };

        let mut game_outcomes = self.game_outcomes.clone();
        game_outcomes.extend(other.game_outcomes.iter().cloned());

        MatchResult {
            white_wins: self.white_wins + other.white_wins,
            black_wins: self.black_wins + other.black_wins,
            draws: self.draws + other.draws,
            avg_rounds,
            games_played: total_games,
            game_outcomes,
        }
    }

    /// Get win rate for white player
    pub fn white_win_rate(&self) -> f32 {
        if self.games_played == 0 {
            0.0
        } else {
            self.white_wins as f32 / self.games_played as f32
        }
    }

    /// Get win rate for black player
    pub fn black_win_rate(&self) -> f32 {
        if self.games_played == 0 {
            0.0
        } else {
            self.black_wins as f32 / self.games_played as f32
        }
    }

    /// Get draw rate
    pub fn draw_rate(&self) -> f32 {
        if self.games_played == 0 {
            0.0
        } else {
            self.draws as f32 / self.games_played as f32
        }
    }

    /// Get score for a ruleset (from its perspective as white or black)
    /// Wins = 1.0, Draws = 0.5, Losses = 0.0
    pub fn score_for_white(&self) -> f32 {
        self.white_wins as f32 + 0.5 * self.draws as f32
    }

    /// Get score for black
    pub fn score_for_black(&self) -> f32 {
        self.black_wins as f32 + 0.5 * self.draws as f32
    }
}

/// Play a match between two rulesets (Level 2 phase)
///
/// Plays multiple games alternating colors for fairness.
pub fn play_match(
    white_ruleset: &RuleSet,
    black_ruleset: &RuleSet,
    ai_config: AiConfig,
    games_per_pair: usize,
    max_rounds: u32,
) -> MatchResult {
    if games_per_pair == 0 {
        return MatchResult::empty();
    }

    // Play games, alternating colors each game
    let game_configs = prepare_game_configs(games_per_pair);
    let results = execute_games(white_ruleset, black_ruleset, &ai_config, &game_configs, max_rounds);
    aggregate_results(results)
}

/// Play a match with parallel execution (Level 2 phase)
pub fn play_match_parallel(
    white_ruleset: &RuleSet,
    black_ruleset: &RuleSet,
    ai_config: AiConfig,
    games_per_pair: usize,
    max_rounds: u32,
) -> MatchResult {
    if games_per_pair == 0 {
        return MatchResult::empty();
    }

    let game_configs = prepare_game_configs(games_per_pair);
    let results = execute_games_parallel(white_ruleset, black_ruleset, &ai_config, &game_configs, max_rounds);
    aggregate_results(results)
}

// ============================================================================
// Level 3 - Steps
// ============================================================================

/// Configuration for a single game in a match
#[derive(Clone, Copy)]
struct GameConfig {
    /// Which ruleset plays white this game
    white_is_first: bool,
    /// Game index (for seeding)
    game_index: usize,
}

/// Prepare game configurations for a match
fn prepare_game_configs(games_per_pair: usize) -> Vec<GameConfig> {
    (0..games_per_pair)
        .map(|i| GameConfig {
            white_is_first: i % 2 == 0, // Alternate colors
            game_index: i,
        })
        .collect()
}

/// Execute games sequentially
fn execute_games(
    white_ruleset: &RuleSet,
    black_ruleset: &RuleSet,
    ai_config: &AiConfig,
    game_configs: &[GameConfig],
    max_rounds: u32,
) -> Vec<GameOutcomeWithContext> {
    let base_seed = ai_config.seed.unwrap_or(42);

    game_configs
        .iter()
        .map(|gc| {
            let seed = base_seed.wrapping_add(gc.game_index as u64);
            play_single_game(white_ruleset, black_ruleset, ai_config, gc, max_rounds, seed)
        })
        .collect()
}

/// Execute games in parallel using rayon
fn execute_games_parallel(
    white_ruleset: &RuleSet,
    black_ruleset: &RuleSet,
    ai_config: &AiConfig,
    game_configs: &[GameConfig],
    max_rounds: u32,
) -> Vec<GameOutcomeWithContext> {
    let base_seed = ai_config.seed.unwrap_or(42);

    game_configs
        .par_iter()
        .map(|gc| {
            let seed = base_seed.wrapping_add(gc.game_index as u64);
            play_single_game(white_ruleset, black_ruleset, ai_config, gc, max_rounds, seed)
        })
        .collect()
}

/// Outcome with context about which ruleset was playing which color
#[derive(Clone)]
struct GameOutcomeWithContext {
    outcome: GameOutcome,
    /// True if the "white_ruleset" from play_match was playing white
    first_ruleset_was_white: bool,
}

/// Play a single game with the given configuration
fn play_single_game(
    white_ruleset: &RuleSet,
    black_ruleset: &RuleSet,
    ai_config: &AiConfig,
    gc: &GameConfig,
    max_rounds: u32,
    seed: u64,
) -> GameOutcomeWithContext {
    let initial_state = if gc.white_is_first {
        create_game_state(white_ruleset, black_ruleset)
    } else {
        create_game_state(black_ruleset, white_ruleset)
    };

    let mut runner = GameRunner::new(ai_config.clone());
    runner.reset_seed(seed);

    let outcome = runner.play_game(initial_state, max_rounds);

    GameOutcomeWithContext {
        outcome,
        first_ruleset_was_white: gc.white_is_first,
    }
}

/// Create game state from two rulesets
fn create_game_state(white_ruleset: &RuleSet, black_ruleset: &RuleSet) -> hexwar_core::GameState {
    // Combine the rulesets into a single game state
    // White uses white_ruleset's white side, black uses black_ruleset's black side
    use hexwar_core::GameState;

    let mut white_setup: Vec<(u8, hexwar_core::Hex, u8)> = Vec::new();
    let mut black_setup: Vec<(u8, hexwar_core::Hex, u8)> = Vec::new();

    // Add white pieces from white_ruleset
    if !white_ruleset.white_positions.is_empty() {
        white_setup.push((
            white_ruleset.white_king,
            white_ruleset.white_positions[0],
            white_ruleset.white_facings.get(0).copied().unwrap_or(0),
        ));
    }
    for (i, &piece_type) in white_ruleset.white_pieces.iter().enumerate() {
        if i + 1 < white_ruleset.white_positions.len() {
            let pos = white_ruleset.white_positions[i + 1];
            let facing = white_ruleset.white_facings.get(i + 1).copied().unwrap_or(0);
            white_setup.push((piece_type, pos, facing));
        }
    }

    // Add black pieces from black_ruleset
    if !black_ruleset.black_positions.is_empty() {
        black_setup.push((
            black_ruleset.black_king,
            black_ruleset.black_positions[0],
            black_ruleset.black_facings.get(0).copied().unwrap_or(3),
        ));
    }
    for (i, &piece_type) in black_ruleset.black_pieces.iter().enumerate() {
        if i + 1 < black_ruleset.black_positions.len() {
            let pos = black_ruleset.black_positions[i + 1];
            let facing = black_ruleset.black_facings.get(i + 1).copied().unwrap_or(3);
            black_setup.push((piece_type, pos, facing));
        }
    }

    GameState::new(
        &white_setup,
        &black_setup,
        white_ruleset.white_template,
        black_ruleset.black_template,
    )
}

/// Aggregate game outcomes into a match result
fn aggregate_results(outcomes: Vec<GameOutcomeWithContext>) -> MatchResult {
    let mut white_wins = 0u32;
    let mut black_wins = 0u32;
    let mut draws = 0u32;
    let mut total_rounds = 0u32;
    let mut game_outcomes = Vec::with_capacity(outcomes.len());

    for owc in outcomes {
        total_rounds += owc.outcome.rounds;
        game_outcomes.push(owc.outcome.clone());

        // Attribute wins correctly based on who was playing which color
        match owc.outcome.result {
            hexwar_core::GameResult::WhiteWins => {
                if owc.first_ruleset_was_white {
                    white_wins += 1; // First ruleset won
                } else {
                    black_wins += 1; // Second ruleset won (was playing white)
                }
            }
            hexwar_core::GameResult::BlackWins => {
                if owc.first_ruleset_was_white {
                    black_wins += 1; // Second ruleset won
                } else {
                    white_wins += 1; // First ruleset won (was playing black)
                }
            }
            hexwar_core::GameResult::Ongoing => {
                draws += 1;
            }
        }
    }

    let games_played = game_outcomes.len() as u32;
    let avg_rounds = if games_played > 0 {
        total_rounds as f32 / games_played as f32
    } else {
        0.0
    };

    MatchResult {
        white_wins,
        black_wins,
        draws,
        avg_rounds,
        games_played,
        game_outcomes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hexwar_core::game::Template;
    use hexwar_core::board::Hex;

    fn make_test_ruleset(name: &str) -> RuleSet {
        RuleSet {
            name: name.to_string(),
            white_king: 25, // K1
            white_pieces: vec![1, 1, 1, 1], // Guards
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
    fn test_match_result_empty() {
        let result = MatchResult::empty();
        assert_eq!(result.games_played, 0);
        assert_eq!(result.white_win_rate(), 0.0);
    }

    #[test]
    fn test_match_result_combine() {
        let r1 = MatchResult {
            white_wins: 2,
            black_wins: 1,
            draws: 1,
            avg_rounds: 20.0,
            games_played: 4,
            game_outcomes: vec![],
        };
        let r2 = MatchResult {
            white_wins: 1,
            black_wins: 2,
            draws: 1,
            avg_rounds: 30.0,
            games_played: 4,
            game_outcomes: vec![],
        };

        let combined = r1.combine(&r2);
        assert_eq!(combined.white_wins, 3);
        assert_eq!(combined.black_wins, 3);
        assert_eq!(combined.draws, 2);
        assert_eq!(combined.games_played, 8);
        assert!((combined.avg_rounds - 25.0).abs() < 0.01);
    }

    #[test]
    fn test_match_result_win_rates() {
        let result = MatchResult {
            white_wins: 6,
            black_wins: 3,
            draws: 1,
            avg_rounds: 25.0,
            games_played: 10,
            game_outcomes: vec![],
        };

        assert_eq!(result.white_win_rate(), 0.6);
        assert_eq!(result.black_win_rate(), 0.3);
        assert_eq!(result.draw_rate(), 0.1);
    }

    #[test]
    fn test_match_result_scores() {
        let result = MatchResult {
            white_wins: 3,
            black_wins: 2,
            draws: 2,
            avg_rounds: 25.0,
            games_played: 7,
            game_outcomes: vec![],
        };

        // White: 3 wins + 0.5 * 2 draws = 4.0
        assert_eq!(result.score_for_white(), 4.0);
        // Black: 2 wins + 0.5 * 2 draws = 3.0
        assert_eq!(result.score_for_black(), 3.0);
    }

    #[test]
    fn test_play_match_basic() {
        let white_rs = make_test_ruleset("white");
        let black_rs = make_test_ruleset("black");
        let ai_config = AiConfig::alpha_beta(1).with_seed(42);

        let result = play_match(&white_rs, &black_rs, ai_config, 2, 20);

        assert_eq!(result.games_played, 2);
        assert!(result.white_wins + result.black_wins + result.draws == 2);
    }

    #[test]
    fn test_play_match_zero_games() {
        let white_rs = make_test_ruleset("white");
        let black_rs = make_test_ruleset("black");
        let ai_config = AiConfig::alpha_beta(1);

        let result = play_match(&white_rs, &black_rs, ai_config, 0, 20);

        assert_eq!(result.games_played, 0);
    }

    #[test]
    fn test_prepare_game_configs() {
        let configs = prepare_game_configs(4);
        assert_eq!(configs.len(), 4);
        assert!(configs[0].white_is_first);
        assert!(!configs[1].white_is_first);
        assert!(configs[2].white_is_first);
        assert!(!configs[3].white_is_first);
    }
}
