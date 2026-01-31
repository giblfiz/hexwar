//! Piece type definitions

use serde::{Deserialize, Serialize};

/// Piece type identifier (index into PIECE_TYPES)
pub type PieceTypeId = u8;

/// Movement type
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum MoveType {
    Step,   // Move up to N hexes, blocked by pieces
    Slide,  // Move any distance, blocked by pieces
    Jump,   // Jump exactly N hexes, ignores blocking
    None,   // Cannot move normally (Warper)
}

/// Special abilities
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Special {
    None,
    SwapMove,    // Warper: swap with ally instead of moving
    SwapRotate,  // Shifter: swap with ally on rotate action
    Rebirth,     // Phoenix: can return from graveyard
    Phased,      // Ghost: cannot capture or be captured
}

/// Direction bitmasks
pub const DIR_F: u8 = 1 << 0;   // Forward
pub const DIR_FR: u8 = 1 << 1;  // Forward-Right
pub const DIR_BR: u8 = 1 << 2;  // Back-Right
pub const DIR_B: u8 = 1 << 3;   // Backward
pub const DIR_BL: u8 = 1 << 4;  // Back-Left
pub const DIR_FL: u8 = 1 << 5;  // Forward-Left

pub const ALL_DIRS: u8 = DIR_F | DIR_FR | DIR_BR | DIR_B | DIR_BL | DIR_FL;
pub const FORWARD_ARC: u8 = DIR_F | DIR_FL | DIR_FR;
pub const DIAGONAL_DIRS: u8 = DIR_FL | DIR_FR | DIR_BL | DIR_BR;
pub const FORWARD_BACK: u8 = DIR_F | DIR_B;
pub const TRIDENT_DIRS: u8 = DIR_FL | DIR_FR | DIR_B;  // Three non-adjacent directions

/// Piece type definition
#[derive(Clone, Debug)]
pub struct PieceType {
    pub id: &'static str,
    pub name: &'static str,
    pub move_type: MoveType,
    pub move_range: u8,
    pub directions: u8,  // Bitmask of allowed directions
    pub special: Special,
    pub is_king: bool,
}

impl PieceType {
    const fn new(
        id: &'static str,
        name: &'static str,
        move_type: MoveType,
        range: u8,
        dirs: u8,
        special: Special,
        is_king: bool,
    ) -> Self {
        Self {
            id,
            name,
            move_type,
            move_range: range,
            directions: dirs,
            special,
            is_king,
        }
    }
}

/// All 32 piece types
pub static PIECE_TYPES: [PieceType; 32] = [
    // Step-1
    PieceType::new("A1", "Pawn", MoveType::Step, 1, DIR_F, Special::None, false),
    PieceType::new("A2", "Guard", MoveType::Step, 1, ALL_DIRS, Special::None, false),
    PieceType::new("A3", "Scout", MoveType::Step, 1, FORWARD_ARC, Special::None, false),
    PieceType::new("A4", "Crab", MoveType::Step, 1, DIR_FL | DIR_FR | DIR_B, Special::None, false),
    PieceType::new("A5", "Flanker", MoveType::Step, 1, DIR_FL | DIR_FR, Special::None, false),
    // Step-2
    PieceType::new("B1", "Strider", MoveType::Step, 2, DIR_F, Special::None, false),
    PieceType::new("B2", "Dancer", MoveType::Step, 2, DIR_FL | DIR_FR, Special::None, false),
    PieceType::new("B3", "Ranger", MoveType::Step, 2, ALL_DIRS, Special::None, false),
    PieceType::new("B4", "Hound", MoveType::Step, 2, FORWARD_ARC, Special::None, false),
    // Step-3
    PieceType::new("C1", "Lancer", MoveType::Step, 3, DIR_F, Special::None, false),
    PieceType::new("C2", "Dragoon", MoveType::Step, 3, FORWARD_ARC, Special::None, false),
    PieceType::new("C3", "Courser", MoveType::Step, 3, ALL_DIRS, Special::None, false),
    // Slide
    PieceType::new("D1", "Pike", MoveType::Slide, 99, DIR_F, Special::None, false),
    PieceType::new("D2", "Rook", MoveType::Slide, 99, FORWARD_BACK, Special::None, false),
    PieceType::new("D3", "Bishop", MoveType::Slide, 99, DIAGONAL_DIRS, Special::None, false),
    PieceType::new("D4", "Chariot", MoveType::Slide, 99, FORWARD_ARC, Special::None, false),
    PieceType::new("D5", "Queen", MoveType::Slide, 99, ALL_DIRS, Special::None, false),
    // Jump
    PieceType::new("E1", "Knight", MoveType::Jump, 2, FORWARD_ARC, Special::None, false),
    PieceType::new("E2", "Frog", MoveType::Jump, 2, ALL_DIRS, Special::None, false),
    PieceType::new("F1", "Locust", MoveType::Jump, 3, FORWARD_ARC, Special::None, false),
    PieceType::new("F2", "Cricket", MoveType::Jump, 3, ALL_DIRS, Special::None, false),
    // Special
    PieceType::new("W1", "Warper", MoveType::None, 0, 0, Special::SwapMove, false),
    PieceType::new("W2", "Shifter", MoveType::Step, 1, ALL_DIRS, Special::SwapRotate, false),
    PieceType::new("P1", "Phoenix", MoveType::Step, 1, FORWARD_ARC, Special::Rebirth, false),
    PieceType::new("G1", "Ghost", MoveType::Step, 1, ALL_DIRS, Special::Phased, false),
    // Kings
    PieceType::new("K1", "King Guard", MoveType::Step, 1, ALL_DIRS, Special::None, true),
    PieceType::new("K2", "King Scout", MoveType::Step, 1, FORWARD_ARC, Special::None, true),
    PieceType::new("K3", "King Ranger", MoveType::Step, 2, ALL_DIRS, Special::None, true),
    PieceType::new("K4", "King Frog", MoveType::Jump, 2, ALL_DIRS, Special::None, true),
    PieceType::new("K5", "King Pike", MoveType::Slide, 99, DIR_F, Special::None, true),
    // Trident pieces (3 non-adjacent directions: FL, FR, B)
    PieceType::new("B5", "Triton", MoveType::Step, 2, TRIDENT_DIRS, Special::None, false),
    PieceType::new("D6", "Triskelion", MoveType::Slide, 99, TRIDENT_DIRS, Special::None, false),
];

/// Get piece type index from string ID
pub fn piece_id_to_index(id: &str) -> Option<PieceTypeId> {
    PIECE_TYPES.iter().position(|pt| pt.id == id).map(|i| i as u8)
}

/// Get piece type from index
pub fn get_piece_type(idx: PieceTypeId) -> &'static PieceType {
    &PIECE_TYPES[idx as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_piece_lookup() {
        assert_eq!(piece_id_to_index("A1"), Some(0));
        assert_eq!(piece_id_to_index("K5"), Some(29));
        assert_eq!(piece_id_to_index("XX"), None);
    }

    #[test]
    fn test_kings() {
        for pt in &PIECE_TYPES {
            if pt.id.starts_with('K') {
                assert!(pt.is_king, "{} should be a king", pt.id);
            } else {
                assert!(!pt.is_king, "{} should not be a king", pt.id);
            }
        }
    }
}
