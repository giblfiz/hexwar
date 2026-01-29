//! HEXWAR Core - Game engine and AI
//!
//! This crate provides the core game logic for HEXWAR:
//! - Board geometry (hex grid with axial coordinates)
//! - Piece types and movement rules
//! - Game state and move generation
//! - Position evaluation with mobility heuristic
//! - CPU-based alpha-beta AI

pub mod board;
pub mod pieces;
pub mod game;
pub mod eval;
pub mod ai;
pub mod ruleset;

// Re-exports for convenient access
pub use board::{Hex, DIRECTIONS, BOARD_RADIUS};
pub use pieces::{PieceType, PieceTypeId, PIECE_TYPES, piece_id_to_index, get_piece_type};
pub use game::{GameState, Move, Player, GameResult, Piece, Template};
pub use eval::{Heuristics, evaluate, WIN_VALUE};
pub use ai::AlphaBetaAI;
pub use ruleset::RuleSet;
