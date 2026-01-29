//! Compact game state for GPU transfer
//!
//! The GPU needs a fixed-size, Copy-able representation of game state
//! that fits efficiently in GPU memory. This module provides:
//! - CompactPiece: 2 bytes per piece
//! - CompactBoard: fixed array of all 61 hexes
//! - CompactGameState: ~256 bytes total, suitable for GPU kernels

use bytemuck::{Pod, Zeroable};
use cudarc::driver::ValidAsZeroBits;
use hexwar_core::{GameResult, GameState, Hex, Player};

/// Board has 61 hexes (radius 4 hex grid)
pub const BOARD_SIZE: usize = 61;

/// Maximum pieces per side
pub const MAX_PIECES_PER_SIDE: usize = 15;

/// Maximum legal moves we track (for random selection)
pub const MAX_LEGAL_MOVES: usize = 128;

/// Compact piece representation (2 bytes)
/// - piece_type: u8 (0-29 = piece type, 255 = empty)
/// - packed: u8 (bits 0-2 = facing, bit 3 = owner, bits 4-7 = reserved)
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct CompactPiece {
    pub piece_type: u8,
    pub packed: u8,
}

impl CompactPiece {
    pub const EMPTY: u8 = 255;

    pub fn new(piece_type: u8, owner: Player, facing: u8) -> Self {
        let owner_bit = match owner {
            Player::White => 0,
            Player::Black => 1,
        };
        Self {
            piece_type,
            packed: (facing & 0x07) | ((owner_bit & 0x01) << 3),
        }
    }

    pub fn empty() -> Self {
        Self {
            piece_type: Self::EMPTY,
            packed: 0,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.piece_type == Self::EMPTY
    }

    pub fn facing(&self) -> u8 {
        self.packed & 0x07
    }

    pub fn owner(&self) -> Player {
        if (self.packed >> 3) & 0x01 == 0 {
            Player::White
        } else {
            Player::Black
        }
    }
}

/// Compact move representation (4 bytes)
/// Encodes all move types in a fixed-size format
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct CompactMove {
    /// Move type: 0=Pass, 1=Movement, 2=Rotate, 3=Swap, 4=Rebirth, 255=Invalid
    pub move_type: u8,
    /// Source hex index (0-60) or 255 for none
    pub from_idx: u8,
    /// Destination hex index (0-60) or 255 for none
    pub to_idx: u8,
    /// New facing (0-5) or other data
    pub facing: u8,
}

impl CompactMove {
    pub const PASS: CompactMove = CompactMove {
        move_type: 0,
        from_idx: 255,
        to_idx: 255,
        facing: 0,
    };

    pub const INVALID: CompactMove = CompactMove {
        move_type: 255,
        from_idx: 255,
        to_idx: 255,
        facing: 0,
    };

    pub fn movement(from_idx: u8, to_idx: u8, facing: u8) -> Self {
        Self {
            move_type: 1,
            from_idx,
            to_idx,
            facing,
        }
    }

    pub fn rotate(pos_idx: u8, facing: u8) -> Self {
        Self {
            move_type: 2,
            from_idx: pos_idx,
            to_idx: pos_idx,
            facing,
        }
    }

    pub fn swap(from_idx: u8, to_idx: u8) -> Self {
        Self {
            move_type: 3,
            from_idx,
            to_idx,
            facing: 0,
        }
    }

    pub fn rebirth(dest_idx: u8, facing: u8) -> Self {
        Self {
            move_type: 4,
            from_idx: 255,
            to_idx: dest_idx,
            facing,
        }
    }

    pub fn is_valid(&self) -> bool {
        self.move_type != 255
    }
}

/// Compact game state for GPU (256 bytes total)
/// Designed to be Copy for efficient GPU transfer
///
/// Note: We manually implement Pod/Zeroable because bytemuck derive
/// doesn't support arrays > 32 elements by default.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CompactGameState {
    /// Board state: 61 hexes * 2 bytes = 122 bytes
    pub board: [CompactPiece; BOARD_SIZE],
    /// Current player (0=White, 1=Black)
    pub current_player: u8,
    /// Game result (0=Ongoing, 1=WhiteWins, 2=BlackWins)
    pub result: u8,
    /// Current round number
    pub round: u16,
    /// White template (0-5 for A-F)
    pub white_template: u8,
    /// Black template (0-5 for A-F)
    pub black_template: u8,
    /// Action phase within turn (template-dependent)
    pub action_phase: u8,
    /// Padding to align and reach 256 bytes
    /// 122 + 1 + 1 + 2 + 1 + 1 + 1 = 129 bytes, need 127 more
    pub _padding: [u8; 127],
}

// SAFETY: CompactGameState is #[repr(C)] with all Pod fields
unsafe impl Pod for CompactGameState {}
unsafe impl Zeroable for CompactGameState {}

impl Default for CompactGameState {
    fn default() -> Self {
        Self::new_empty()
    }
}

impl CompactGameState {
    /// Create an empty game state
    pub fn new_empty() -> Self {
        Self {
            board: [CompactPiece::empty(); BOARD_SIZE],
            current_player: 0,
            result: 0,
            round: 0,
            white_template: 0,
            black_template: 0,
            action_phase: 0,
            _padding: [0; 127],
        }
    }

    /// Convert hex coordinates to board index
    /// Uses a standard mapping for the 61 hexes in a radius-4 grid
    pub fn hex_to_index(hex: Hex) -> Option<usize> {
        if !hex.is_valid() {
            return None;
        }
        // Map (q, r) to linear index
        // q ranges from -4 to 4, r ranges depending on q
        // We use row-major ordering within valid bounds
        let q = hex.q as i32;
        let r = hex.r as i32;

        // Count hexes before this row
        let mut idx: i32 = 0;
        for prev_q in -4..q {
            let r_min = (-4).max(-4 - prev_q);
            let r_max = 4.min(4 - prev_q);
            idx += r_max - r_min + 1;
        }
        // Add offset within row
        let r_min = (-4).max(-4 - q);
        idx += r - r_min;

        Some(idx as usize)
    }

    /// Convert board index back to hex coordinates
    pub fn index_to_hex(idx: usize) -> Option<Hex> {
        if idx >= BOARD_SIZE {
            return None;
        }

        let mut remaining = idx as i32;
        for q in -4i8..=4 {
            let r_min = (-4i8).max(-4 - q);
            let r_max = 4i8.min(4 - q);
            let row_size = (r_max - r_min + 1) as i32;

            if remaining < row_size {
                return Some(Hex::new(q, r_min + remaining as i8));
            }
            remaining -= row_size;
        }
        None
    }

    /// Get piece at board index
    pub fn get(&self, idx: usize) -> CompactPiece {
        if idx < BOARD_SIZE {
            self.board[idx]
        } else {
            CompactPiece::empty()
        }
    }

    /// Set piece at board index
    pub fn set(&mut self, idx: usize, piece: CompactPiece) {
        if idx < BOARD_SIZE {
            self.board[idx] = piece;
        }
    }

    /// Check if game is over
    pub fn is_game_over(&self) -> bool {
        self.result != 0
    }

    /// Get current player
    pub fn get_current_player(&self) -> Player {
        if self.current_player == 0 {
            Player::White
        } else {
            Player::Black
        }
    }

    /// Get game result
    pub fn get_result(&self) -> GameResult {
        match self.result {
            0 => GameResult::Ongoing,
            1 => GameResult::WhiteWins,
            2 => GameResult::BlackWins,
            _ => GameResult::Ongoing,
        }
    }

    /// Convert from hexwar-core GameState
    /// Note: This is a simplified conversion; full state requires Agent 1's implementation
    pub fn from_game_state(state: &GameState) -> Self {
        let mut compact = Self::new_empty();

        compact.current_player = match state.current_player() {
            Player::White => 0,
            Player::Black => 1,
        };

        compact.result = match state.result() {
            GameResult::Ongoing => 0,
            GameResult::WhiteWins => 1,
            GameResult::BlackWins => 2,
        };

        compact.round = state.round;

        // Copy pieces from GameState
        for (hex, piece) in state.pieces() {
            if let Some(idx) = Self::hex_to_index(hex) {
                compact.board[idx] =
                    CompactPiece::new(piece.piece_type, piece.owner, piece.facing);
            }
        }

        compact
    }
}

/// Result of a single simulated game
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Pod, Zeroable)]
pub struct SimulationResult {
    /// Game result (0=Ongoing, 1=WhiteWins, 2=BlackWins)
    pub result: u8,
    /// Number of rounds played
    pub rounds: u8,
    /// Final evaluation score (as fixed-point: value * 100)
    pub final_eval_x100: i16,
    /// Padding for alignment (4 bytes as 2x2 for Pod derive)
    pub _padding1: u16,
    pub _padding2: u16,
}

// SAFETY: SimulationResult is #[repr(C)] with all Pod fields, all zeros is valid
unsafe impl ValidAsZeroBits for SimulationResult {}

impl SimulationResult {
    pub fn get_result(&self) -> GameResult {
        match self.result {
            1 => GameResult::WhiteWins,
            2 => GameResult::BlackWins,
            _ => GameResult::Ongoing,
        }
    }

    pub fn final_eval(&self) -> f32 {
        self.final_eval_x100 as f32 / 100.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compact_piece_roundtrip() {
        let piece = CompactPiece::new(5, Player::Black, 3);
        assert_eq!(piece.piece_type, 5);
        assert_eq!(piece.owner(), Player::Black);
        assert_eq!(piece.facing(), 3);
    }

    #[test]
    fn test_hex_index_roundtrip() {
        // Test all valid hexes
        for q in -4i8..=4 {
            let r_min = (-4i8).max(-4 - q);
            let r_max = 4i8.min(4 - q);
            for r in r_min..=r_max {
                let hex = Hex::new(q, r);
                let idx = CompactGameState::hex_to_index(hex).expect("valid hex should have index");
                let hex2 = CompactGameState::index_to_hex(idx).expect("valid index should have hex");
                assert_eq!(hex, hex2, "Roundtrip failed for ({}, {})", q, r);
            }
        }
    }

    #[test]
    fn test_board_size() {
        // Count valid hexes
        let mut count = 0;
        for q in -4i8..=4 {
            let r_min = (-4i8).max(-4 - q);
            let r_max = 4i8.min(4 - q);
            count += (r_max - r_min + 1) as usize;
        }
        assert_eq!(count, BOARD_SIZE);
    }

    #[test]
    fn test_compact_state_size() {
        // Verify our state fits in expected size
        assert_eq!(std::mem::size_of::<CompactGameState>(), 256);
        assert_eq!(std::mem::size_of::<SimulationResult>(), 8);
    }
}
