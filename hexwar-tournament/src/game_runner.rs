//! Game runner - executes single games
//!
//! Level 3 - Step-level implementation

use hexwar_core::{AlphaBetaAI, GameResult, GameState, Move, Player};
use hexwar_mcts::{MctsConfig, MctsPlayer};

use crate::config::{AiConfig, PlayerType};

/// Outcome of a single game
#[derive(Clone, Debug)]
pub struct GameOutcome {
    /// Final game result
    pub result: GameResult,
    /// Number of rounds played
    pub rounds: u32,
    /// Move history
    pub moves: Vec<Move>,
}

impl GameOutcome {
    /// Check if white won
    pub fn white_wins(&self) -> bool {
        self.result == GameResult::WhiteWins
    }

    /// Check if black won
    pub fn black_wins(&self) -> bool {
        self.result == GameResult::BlackWins
    }

    /// Check if game is a draw (ongoing at round limit)
    pub fn is_draw(&self) -> bool {
        self.result == GameResult::Ongoing
    }

    /// Get winner (None for draw)
    pub fn winner(&self) -> Option<Player> {
        match self.result {
            GameResult::WhiteWins => Some(Player::White),
            GameResult::BlackWins => Some(Player::Black),
            GameResult::Ongoing => None,
        }
    }
}

/// Game runner that plays games with AI
pub struct GameRunner {
    /// AI configuration
    config: AiConfig,
    /// Random seed counter
    seed_counter: u64,
}

impl GameRunner {
    /// Create a new game runner
    pub fn new(config: AiConfig) -> Self {
        let seed_counter = config.seed.unwrap_or(42);
        Self {
            config,
            seed_counter,
        }
    }

    /// Play a single game, returning the outcome
    pub fn play_game(&mut self, initial_state: GameState, max_rounds: u32) -> GameOutcome {
        match self.config.player_type {
            PlayerType::AlphaBeta => self.play_alpha_beta(initial_state, max_rounds),
            PlayerType::MCTS => self.play_mcts(initial_state, max_rounds),
        }
    }

    /// Play game using alpha-beta AI
    fn play_alpha_beta(&mut self, initial_state: GameState, max_rounds: u32) -> GameOutcome {
        let seed = self.next_seed();
        let mut ai = AlphaBetaAI::with_seed(
            self.config.depth,
            self.config.heuristics.clone(),
            seed,
        );

        let (final_state, moves) = ai.play_game(initial_state, max_rounds);

        GameOutcome {
            result: final_state.result(),
            rounds: final_state.round as u32,
            moves,
        }
    }

    /// Play game using MCTS
    fn play_mcts(&mut self, initial_state: GameState, max_rounds: u32) -> GameOutcome {
        let mcts_config = MctsConfig::cpu_only(self.config.simulations as usize);
        let player = MctsPlayer::cpu_only(mcts_config);

        let (final_state, moves) = player.play_game(initial_state, max_rounds);

        GameOutcome {
            result: final_state.result(),
            rounds: final_state.round as u32,
            moves,
        }
    }

    /// Get next seed and increment counter
    fn next_seed(&mut self) -> u64 {
        let seed = self.seed_counter;
        self.seed_counter = self.seed_counter.wrapping_add(1);
        seed
    }

    /// Reset seed counter
    pub fn reset_seed(&mut self, seed: u64) {
        self.seed_counter = seed;
    }

    /// Get configuration
    pub fn config(&self) -> &AiConfig {
        &self.config
    }
}

/// Play a game with separate AI configs for white and black
#[allow(dead_code)]
pub fn play_game_asymmetric(
    initial_state: GameState,
    white_config: &AiConfig,
    black_config: &AiConfig,
    max_rounds: u32,
    seed: u64,
) -> GameOutcome {
    let mut state = initial_state;
    let mut moves = Vec::new();

    // Create AIs for each side
    let mut white_ai = create_ai(white_config, seed);
    let mut black_ai = create_ai(black_config, seed.wrapping_add(1));

    while state.result() == GameResult::Ongoing && state.round as u32 <= max_rounds {
        let mv = match state.current_player() {
            Player::White => get_ai_move(&mut white_ai, &state),
            Player::Black => get_ai_move(&mut black_ai, &state),
        };

        match mv {
            Some(m) => {
                state = state.apply_move(m);
                moves.push(m);
            }
            None => break,
        }
    }

    GameOutcome {
        result: state.result(),
        rounds: state.round as u32,
        moves,
    }
}

/// AI wrapper enum for asymmetric play
#[allow(dead_code)]
enum AiPlayer {
    AlphaBeta(AlphaBetaAI),
    Mcts(MctsPlayer),
}

/// Create an AI player from config
#[allow(dead_code)]
fn create_ai(config: &AiConfig, seed: u64) -> AiPlayer {
    match config.player_type {
        PlayerType::AlphaBeta => {
            AiPlayer::AlphaBeta(AlphaBetaAI::with_seed(
                config.depth,
                config.heuristics.clone(),
                seed,
            ))
        }
        PlayerType::MCTS => {
            let mcts_config = MctsConfig::cpu_only(config.simulations as usize);
            AiPlayer::Mcts(MctsPlayer::cpu_only(mcts_config))
        }
    }
}

/// Get move from AI player
#[allow(dead_code)]
fn get_ai_move(ai: &mut AiPlayer, state: &GameState) -> Option<Move> {
    match ai {
        AiPlayer::AlphaBeta(ref mut ab) => ab.best_move(state),
        AiPlayer::Mcts(ref mcts) => mcts.best_move(state),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hexwar_core::board::Hex;
    use hexwar_core::game::Template;
    use hexwar_core::pieces::piece_id_to_index;

    fn simple_game() -> GameState {
        let white = vec![
            (piece_id_to_index("K1").unwrap(), Hex::new(0, 3), 0),
            (piece_id_to_index("A2").unwrap(), Hex::new(-1, 3), 0),
        ];
        let black = vec![
            (piece_id_to_index("K1").unwrap(), Hex::new(0, -3), 3),
            (piece_id_to_index("A2").unwrap(), Hex::new(1, -3), 3),
        ];
        GameState::new(&white, &black, Template::E, Template::E)
    }

    #[test]
    fn test_game_runner_creation() {
        let config = AiConfig::alpha_beta(2);
        let runner = GameRunner::new(config);
        assert_eq!(runner.config().depth, 2);
    }

    #[test]
    fn test_play_game_alpha_beta() {
        let config = AiConfig::alpha_beta(1).with_seed(42);
        let mut runner = GameRunner::new(config);
        let state = simple_game();

        let outcome = runner.play_game(state, 20);

        // Game should have progressed
        assert!(outcome.rounds > 0);
        // Should have some moves
        assert!(!outcome.moves.is_empty());
    }

    #[test]
    fn test_game_outcome_winner() {
        let outcome = GameOutcome {
            result: GameResult::WhiteWins,
            rounds: 10,
            moves: vec![],
        };
        assert_eq!(outcome.winner(), Some(Player::White));
        assert!(outcome.white_wins());
        assert!(!outcome.black_wins());

        let draw_outcome = GameOutcome {
            result: GameResult::Ongoing,
            rounds: 50,
            moves: vec![],
        };
        assert_eq!(draw_outcome.winner(), None);
        assert!(draw_outcome.is_draw());
    }

    #[test]
    fn test_seed_counter() {
        let config = AiConfig::alpha_beta(1).with_seed(100);
        let mut runner = GameRunner::new(config);

        let state = simple_game();
        let _ = runner.play_game(state.clone(), 5);
        let _ = runner.play_game(state, 5);

        // Seed counter should have advanced
        runner.reset_seed(100);
        // Now it's reset
    }
}
