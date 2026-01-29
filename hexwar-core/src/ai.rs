//! CPU-based Alpha-Beta AI

use crate::eval::{evaluate, Heuristics};
use crate::game::{GameState, Move};

/// Alpha-Beta AI player
pub struct AlphaBetaAI {
    pub depth: u32,
    pub max_moves_per_action: usize,
    pub heuristics: Heuristics,
}

impl AlphaBetaAI {
    pub fn new(depth: u32, heuristics: Heuristics) -> Self {
        Self {
            depth,
            max_moves_per_action: 15,
            heuristics,
        }
    }

    /// Get best move for current position
    pub fn best_move(&self, state: &GameState) -> Option<Move> {
        // TODO: Agent 1 will implement full alpha-beta search
        // For now, just return first legal move
        let moves = state.legal_moves();
        moves.into_iter().next()
    }

    /// Play a complete game
    pub fn play_game(&self, initial: GameState, max_rounds: u32) -> (GameState, Vec<Move>) {
        let mut state = initial;
        let mut history = Vec::new();
        let mut rounds = 0;

        while state.result() == crate::game::GameResult::Ongoing && rounds < max_rounds {
            if let Some(mv) = self.best_move(&state) {
                history.push(mv);
                state = state.apply_move(mv);
            } else {
                break;
            }
            rounds += 1;
        }

        (state, history)
    }

    /// Evaluate a position
    pub fn evaluate(&self, state: &GameState) -> f32 {
        evaluate(state, &self.heuristics)
    }
}
