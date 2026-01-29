//! Game state and move generation

use crate::board::Hex;
use crate::pieces::PieceTypeId;
use serde::{Deserialize, Serialize};

/// Player color
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Player {
    White = 0,
    Black = 1,
}

impl Player {
    pub fn opponent(self) -> Self {
        match self {
            Player::White => Player::Black,
            Player::Black => Player::White,
        }
    }
}

/// Game result
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameResult {
    Ongoing,
    WhiteWins,
    BlackWins,
}

/// Action template
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Template {
    A,  // Rotate, Move (same)
    B,  // Move, Rotate, Rotate
    C,  // Move, Move, Rotate
    D,  // Move, Rotate (different)
    E,  // Move OR Rotate (chess-like)
    F,  // Move, Rotate (same)
}

/// A piece on the board
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Piece {
    pub piece_type: PieceTypeId,
    pub owner: Player,
    pub facing: u8,
}

/// A legal move
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Move {
    Pass,
    Surrender,
    Movement { from: Hex, to: Hex, new_facing: u8 },
    Rotate { pos: Hex, new_facing: u8 },
    Swap { from: Hex, target: Hex },
    Rebirth { dest: Hex, new_facing: u8 },
}

/// Game state (clone to mutate)
#[derive(Clone, Debug)]
pub struct GameState {
    // TODO: Agent 1 will implement full state
    // For now, just stubs
    pub current_player: Player,
    pub result: GameResult,
    pub round: u16,
}

impl GameState {
    /// Create new game from piece placements
    pub fn new(
        _white_pieces: &[(PieceTypeId, Hex, u8)],
        _black_pieces: &[(PieceTypeId, Hex, u8)],
        _white_template: Template,
        _black_template: Template,
    ) -> Self {
        todo!("Agent 1: Implement GameState::new")
    }

    /// Current player
    pub fn current_player(&self) -> Player {
        self.current_player
    }

    /// Game result
    pub fn result(&self) -> GameResult {
        self.result
    }

    /// Generate legal moves
    pub fn legal_moves(&self) -> Vec<Move> {
        todo!("Agent 1: Implement move generation")
    }

    /// Apply move, return new state
    pub fn apply_move(&self, _mv: Move) -> Self {
        todo!("Agent 1: Implement apply_move")
    }

    /// Count legal moves for mobility heuristic
    pub fn mobility(&self, _player: Player) -> usize {
        todo!("Agent 1: Implement mobility counting")
    }

    /// Iterate pieces on board
    pub fn pieces(&self) -> impl Iterator<Item = (Hex, Piece)> {
        // TODO: Implement properly
        std::iter::empty()
    }
}
