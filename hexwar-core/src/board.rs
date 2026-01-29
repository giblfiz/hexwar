//! Hex board geometry with axial coordinates

use serde::{Deserialize, Serialize};

/// Board radius (distance from center to edge)
pub const BOARD_RADIUS: i8 = 4;

/// Axial hex coordinates
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Hex {
    pub q: i8,
    pub r: i8,
}

impl Hex {
    pub const fn new(q: i8, r: i8) -> Self {
        Self { q, r }
    }

    /// Check if this hex is on the board
    pub fn is_valid(&self) -> bool {
        self.q.abs() <= BOARD_RADIUS
            && self.r.abs() <= BOARD_RADIUS
            && (self.q + self.r).abs() <= BOARD_RADIUS
    }

    /// Distance from center (0,0)
    pub fn distance_to_center(&self) -> i8 {
        (self.q.abs() + self.r.abs() + (self.q + self.r).abs()) / 2
    }

    /// Distance between two hexes
    pub fn distance_to(&self, other: Hex) -> i8 {
        let dq = (self.q - other.q).abs();
        let dr = (self.r - other.r).abs();
        let ds = ((self.q + self.r) - (other.q + other.r)).abs();
        (dq + dr + ds) / 2
    }

    /// Get neighbor in direction (0-5)
    pub fn neighbor(&self, direction: u8) -> Hex {
        let (dq, dr) = DIRECTIONS[direction as usize % 6];
        Hex::new(self.q + dq, self.r + dr)
    }
}

/// Direction vectors in axial coordinates (dq, dr)
/// Index: 0=N, 1=NE, 2=SE, 3=S, 4=SW, 5=NW
pub const DIRECTIONS: [(i8, i8); 6] = [
    (0, -1),  // N
    (1, -1),  // NE
    (1, 0),   // SE
    (0, 1),   // S
    (-1, 1),  // SW
    (-1, 0),  // NW
];

/// Relative directions from facing
pub const FORWARD: u8 = 0;
pub const FORWARD_RIGHT: u8 = 1;
pub const BACK_RIGHT: u8 = 2;
pub const BACKWARD: u8 = 3;
pub const BACK_LEFT: u8 = 4;
pub const FORWARD_LEFT: u8 = 5;

/// Get absolute direction from facing + relative direction
pub fn absolute_direction(facing: u8, relative: u8) -> u8 {
    (facing + relative) % 6
}

/// Get direction vector for a facing + relative direction
pub fn direction_vector(facing: u8, relative: u8) -> (i8, i8) {
    DIRECTIONS[absolute_direction(facing, relative) as usize]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_validity() {
        assert!(Hex::new(0, 0).is_valid());
        assert!(Hex::new(4, 0).is_valid());
        assert!(Hex::new(0, 4).is_valid());
        assert!(Hex::new(-4, 0).is_valid());
        assert!(!Hex::new(5, 0).is_valid());
        assert!(!Hex::new(3, 3).is_valid()); // q + r = 6 > 4
    }

    #[test]
    fn test_distance() {
        assert_eq!(Hex::new(0, 0).distance_to_center(), 0);
        assert_eq!(Hex::new(1, 0).distance_to_center(), 1);
        assert_eq!(Hex::new(2, 2).distance_to_center(), 4);
    }
}
