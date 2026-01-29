//! HEXWAR Core - Game engine and AI
//!
//! This crate provides the core game logic for HEXWAR:
//! - Board geometry (hex grid with axial coordinates)
//! - Piece types and movement rules
//! - Game state and move generation
//! - Position evaluation with mobility heuristic
//! - CPU-based alpha-beta AI

// TODO: Agent 1 will refactor from hexwar_core/src/lib.rs

pub mod board;
pub mod pieces;
pub mod game;
pub mod eval;
pub mod ai;
pub mod ruleset;

// Re-exports
pub use board::Hex;
pub use pieces::{PieceType, PieceTypeId, PIECE_TYPES};
pub use game::{GameState, Move, Player, GameResult};
pub use eval::Heuristics;
pub use ai::AlphaBetaAI;
pub use ruleset::RuleSet;
