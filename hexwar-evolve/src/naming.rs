//! Ruleset naming - Human-readable identifiers for tracking evolution
//!
//! Generates memorable two-word names (e.g., "iron-wolf", "swift-tower")
//! from ruleset compositions. Names are deterministic based on army
//! composition, so the same ruleset always gets the same name.

use hexwar_core::RuleSet;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

// 64 adjectives + 64 nouns = 4096 unique names
const ADJECTIVES: [&str; 64] = [
    "red", "blue", "gold", "dark", "pale", "wild", "calm", "bold",
    "swift", "slow", "warm", "cold", "soft", "hard", "deep", "high",
    "iron", "silk", "jade", "ruby", "onyx", "opal", "amber", "coral",
    "quick", "still", "bright", "dim", "fresh", "old", "new", "lost",
    "stone", "glass", "steel", "brass", "copper", "silver", "bronze", "chrome",
    "sharp", "blunt", "keen", "dull", "pure", "mixed", "raw", "fine",
    "north", "south", "east", "west", "inner", "outer", "upper", "lower",
    "first", "last", "prime", "dual", "twin", "lone", "true", "void",
];

const NOUNS: [&str; 64] = [
    "wolf", "bear", "hawk", "lion", "fox", "owl", "elk", "ram",
    "oak", "pine", "elm", "ash", "fern", "moss", "vine", "root",
    "storm", "flame", "frost", "tide", "wind", "dust", "mist", "haze",
    "crown", "blade", "shield", "helm", "lance", "bow", "staff", "ring",
    "tower", "gate", "wall", "bridge", "path", "road", "trail", "pass",
    "dawn", "dusk", "noon", "night", "moon", "star", "sun", "sky",
    "peak", "vale", "cave", "lake", "river", "shore", "cliff", "ridge",
    "forge", "anvil", "hammer", "arrow", "spear", "axe", "sword", "torch",
];

/// Generate a unique signature for a ruleset based on army composition.
///
/// Two rulesets with identical pieces (regardless of positions) get the same signature.
/// Used for tracking fitness history across generations.
pub fn ruleset_signature(rs: &RuleSet) -> String {
    let mut white_pieces: Vec<u8> = rs.white_pieces.clone();
    white_pieces.sort();
    let mut black_pieces: Vec<u8> = rs.black_pieces.clone();
    black_pieces.sort();

    format!(
        "{}:{:?}|{}:{:?}",
        rs.white_king, white_pieces,
        rs.black_king, black_pieces
    )
}

/// Convert a ruleset signature to a deterministic two-word name.
///
/// Uses hashing to produce a consistent, human-readable name
/// like "iron-wolf" or "swift-tower".
pub fn signature_to_name(sig: &str) -> String {
    let mut hasher = DefaultHasher::new();
    sig.hash(&mut hasher);
    let h = hasher.finish();

    let adj_idx = ((h >> 6) & 0x3F) as usize;  // bits 6-11 -> adjective (0-63)
    let noun_idx = (h & 0x3F) as usize;         // bits 0-5 -> noun (0-63)

    format!("{}-{}", ADJECTIVES[adj_idx], NOUNS[noun_idx])
}

/// Generate a memorable two-word name from a ruleset's composition.
///
/// Hashes the army composition to produce a consistent, human-readable name.
/// Same ruleset always gets the same name.
pub fn ruleset_name(rs: &RuleSet) -> String {
    let sig = ruleset_signature(rs);
    signature_to_name(&sig)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hexwar_core::board::Hex;
    use hexwar_core::game::Template;

    fn make_test_ruleset() -> RuleSet {
        RuleSet {
            name: "test".to_string(),
            white_king: 25,  // K1
            white_pieces: vec![1, 1, 2, 3],  // Guard, Guard, Scout, Crab
            white_positions: vec![
                Hex::new(0, 3),
                Hex::new(-1, 3),
                Hex::new(1, 2),
                Hex::new(-2, 4),
                Hex::new(2, 1),
            ],
            white_facings: vec![0; 5],
            white_template: Template::E,
            black_king: 28,  // K4
            black_pieces: vec![5, 5, 6, 7],  // Strider, Strider, Dancer, Ranger
            black_positions: vec![
                Hex::new(0, -3),
                Hex::new(1, -3),
                Hex::new(-1, -2),
                Hex::new(2, -4),
                Hex::new(-2, -1),
            ],
            black_facings: vec![3; 5],
            black_template: Template::E,
        }
    }

    #[test]
    fn test_signature_deterministic() {
        let rs = make_test_ruleset();
        let sig1 = ruleset_signature(&rs);
        let sig2 = ruleset_signature(&rs);
        assert_eq!(sig1, sig2);
    }

    #[test]
    fn test_signature_ignores_piece_order() {
        let mut rs1 = make_test_ruleset();
        let mut rs2 = make_test_ruleset();

        rs1.white_pieces = vec![1, 2, 1, 3];  // Different order
        rs2.white_pieces = vec![3, 1, 1, 2];  // Same pieces, different order

        assert_eq!(ruleset_signature(&rs1), ruleset_signature(&rs2));
    }

    #[test]
    fn test_name_deterministic() {
        let rs = make_test_ruleset();
        let name1 = ruleset_name(&rs);
        let name2 = ruleset_name(&rs);
        assert_eq!(name1, name2);
    }

    #[test]
    fn test_name_format() {
        let rs = make_test_ruleset();
        let name = ruleset_name(&rs);
        assert!(name.contains('-'), "Name should contain hyphen: {}", name);
        let parts: Vec<&str> = name.split('-').collect();
        assert_eq!(parts.len(), 2, "Name should have two parts: {}", name);
    }

    #[test]
    fn test_different_rulesets_different_names() {
        let rs1 = make_test_ruleset();
        let mut rs2 = make_test_ruleset();
        rs2.white_pieces = vec![16, 16, 16, 16];  // All Queens

        let name1 = ruleset_name(&rs1);
        let name2 = ruleset_name(&rs2);
        // Names could theoretically collide, but with 4096 options unlikely
        assert_ne!(name1, name2, "Different rulesets should have different names");
    }
}
