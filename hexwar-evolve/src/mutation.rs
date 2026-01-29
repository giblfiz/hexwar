//! Mutation operators for ruleset evolution
//!
//! Provides various mutation operations:
//! - Add/remove pieces
//! - Swap piece types
//! - Change king type
//! - Shuffle positions
//! - Rotate pieces

use hexwar_core::board::Hex;
use hexwar_core::RuleSet;
use rand::Rng;

// ============================================================================
// Constants
// ============================================================================

/// Regular piece type IDs (A1-G1, W1-W2, P1)
/// Excludes kings (K1-K5)
pub const REGULAR_PIECE_IDS: [u8; 25] = [
    0, 1, 2, 3, 4,      // A1-A5 (pawns/guards)
    5, 6, 7, 8,         // B1-B4 (step-2)
    9, 10, 11,          // C1-C3 (step-3)
    12, 13, 14, 15, 16, // D1-D5 (sliders)
    17, 18, 19, 20,     // E1-F2 (jumpers)
    21, 22, 23, 24,     // W1, W2, P1, G1 (specials)
];

/// King piece type IDs
pub const KING_IDS: [u8; 5] = [25, 26, 27, 28, 29]; // K1-K5

/// Warper ID (W1) - cannot be on same team as Shifter
pub const WARPER_ID: u8 = 21;

/// Shifter ID (W2) - cannot be on same team as Warper
pub const SHIFTER_ID: u8 = 22;

/// Minimum pieces per side
pub const MIN_PIECES: usize = 8;

/// Maximum pieces per side
pub const MAX_PIECES: usize = 15;

// ============================================================================
// Board Zones (simplified - actual positions would come from hexwar-core)
// ============================================================================

/// White piece starting zone (bottom of board, r > 0)
pub fn white_piece_zone() -> Vec<Hex> {
    let mut zone = Vec::new();
    // Ring at r = 1, 2, 3 (excluding wing hexes and king position)
    for r in 1..=3 {
        for q in -4..=4 {
            let hex = Hex::new(q, r);
            if hex.is_valid() && (q, r) != (0, 3) {  // Exclude king position
                zone.push(hex);
            }
        }
    }
    zone
}

/// Black piece starting zone (top of board, r < 0)
pub fn black_piece_zone() -> Vec<Hex> {
    let mut zone = Vec::new();
    // Ring at r = -1, -2, -3 (excluding wing hexes and king position)
    for r in -3..=-1 {
        for q in -4..=4 {
            let hex = Hex::new(q, r);
            if hex.is_valid() && (q, r) != (0, -3) {  // Exclude king position
                zone.push(hex);
            }
        }
    }
    zone
}

/// White king starting position
pub const WHITE_KING_POS: Hex = Hex { q: 0, r: 3 };

/// Black king starting position
pub const BLACK_KING_POS: Hex = Hex { q: 0, r: -3 };

// ============================================================================
// Piece Tiers (for smart mutation)
// ============================================================================

/// Get the tier of a piece (0 = weakest, 6 = strongest)
pub fn piece_tier(piece_id: u8) -> u8 {
    match piece_id {
        // Tier 0: Pawns
        0 | 2 | 3 | 4 => 0,       // A1, A3, A4, A5
        // Tier 1: Guards and basic steppers
        1 | 5 | 6 => 1,          // A2, B1, B2
        // Tier 2: Ranged steppers
        7 | 8 | 9 => 2,          // B3, B4, C1
        // Tier 3: Long steppers and short sliders
        10 | 11 | 12 => 3,       // C2, C3, D1
        // Tier 4: Jumpers and mid sliders
        13 | 17 | 18 | 19 => 4,  // D2, E1, E2, F1
        // Tier 5: Power sliders and specials
        14 | 15 | 20 | 21 | 22 | 23 | 24 => 5,  // D3, D4, F2, W1, W2, P1, G1
        // Tier 6: Queen
        16 => 6,                 // D5
        _ => 3,                  // Unknown -> middle tier
    }
}

/// Get all piece IDs at a given tier
pub fn pieces_by_tier(tier: u8) -> Vec<u8> {
    REGULAR_PIECE_IDS
        .iter()
        .filter(|&&id| piece_tier(id) == tier)
        .copied()
        .collect()
}

// ============================================================================
// Mutation Types
// ============================================================================

/// Which side to mutate
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MutateSide {
    White,
    Black,
    Both,
}

/// Configuration for mutation
#[derive(Clone, Debug)]
pub struct MutationConfig {
    /// Which side(s) to mutate
    pub side: MutateSide,
    /// Whether template mutation is allowed
    pub allow_template_mutation: bool,
}

impl Default for MutationConfig {
    fn default() -> Self {
        Self {
            side: MutateSide::Both,
            allow_template_mutation: false,  // Template E is preferred for D5+
        }
    }
}

// ============================================================================
// Core Mutation Function
// ============================================================================

/// Apply a single random mutation to a ruleset.
///
/// Mutations are chosen with weighted probabilities to bias toward:
/// - Adding pieces (3x more likely than removing)
/// - Duplicating existing piece types (for themed armies)
///
/// # Arguments
/// * `rs` - Ruleset to mutate
/// * `config` - Mutation configuration
/// * `rng` - Random number generator
///
/// # Returns
/// New ruleset with one mutation applied
pub fn mutate_ruleset<R: Rng>(rs: &RuleSet, config: &MutationConfig, rng: &mut R) -> RuleSet {
    let mut result = rs.clone();

    // Build mutation weights based on config
    let mut mutations: Vec<(&str, f32)> = Vec::new();

    match config.side {
        MutateSide::White => {
            mutations.extend(white_only_mutations());
        }
        MutateSide::Black => {
            mutations.extend(black_only_mutations());
        }
        MutateSide::Both => {
            mutations.extend(both_side_mutations());
        }
    }

    // Choose mutation type
    let total_weight: f32 = mutations.iter().map(|(_, w)| w).sum();
    let mut threshold = rng.gen_range(0.0..total_weight);
    let mut selected_mutation = "";

    for (name, weight) in &mutations {
        threshold -= weight;
        if threshold <= 0.0 {
            selected_mutation = name;
            break;
        }
    }

    // Apply the mutation
    apply_mutation(&mut result, selected_mutation, rng);

    // Enforce W1/W2 constraint (Warper and Shifter can't be on same team)
    enforce_warper_shifter_constraint(&mut result);

    result
}

// ============================================================================
// Mutation Weights
// ============================================================================

fn white_only_mutations() -> Vec<(&'static str, f32)> {
    vec![
        ("add_white", 2.0),
        ("add_copy_white", 2.0),
        ("remove_white", 1.0),
        ("swap_white", 1.0),
        ("swap_existing_white", 2.0),
        ("change_white_king", 1.0),
        ("shuffle_white_positions", 1.0),
        ("swap_two_white_positions", 1.0),
        ("rotate_white", 1.0),
    ]
}

fn black_only_mutations() -> Vec<(&'static str, f32)> {
    vec![
        ("add_black", 2.0),
        ("add_copy_black", 2.0),
        ("remove_black", 1.0),
        ("swap_black", 1.0),
        ("swap_existing_black", 2.0),
        ("change_black_king", 1.0),
        ("shuffle_black_positions", 1.0),
        ("swap_two_black_positions", 1.0),
        ("rotate_black", 1.0),
    ]
}

fn both_side_mutations() -> Vec<(&'static str, f32)> {
    let mut mutations = Vec::new();
    mutations.extend(white_only_mutations());
    mutations.extend(black_only_mutations());
    mutations
}

// ============================================================================
// Mutation Implementation
// ============================================================================

fn apply_mutation<R: Rng>(rs: &mut RuleSet, mutation: &str, rng: &mut R) {
    match mutation {
        // === White mutations ===
        "add_white" => {
            if rs.white_pieces.len() < MAX_PIECES {
                add_piece_white(rs, rng, false);
            }
        }
        "add_copy_white" => {
            if rs.white_pieces.len() < MAX_PIECES && !rs.white_pieces.is_empty() {
                add_piece_white(rs, rng, true);
            }
        }
        "remove_white" => {
            if rs.white_pieces.len() > MIN_PIECES {
                remove_piece_white(rs, rng);
            }
        }
        "swap_white" => {
            if !rs.white_pieces.is_empty() {
                swap_piece_white(rs, rng, false);
            }
        }
        "swap_existing_white" => {
            if rs.white_pieces.len() >= 2 {
                swap_piece_white(rs, rng, true);
            }
        }
        "change_white_king" => {
            rs.white_king = KING_IDS[rng.gen_range(0..KING_IDS.len())];
        }
        "shuffle_white_positions" => {
            shuffle_positions(&mut rs.white_positions, &mut rs.white_facings, rng);
        }
        "swap_two_white_positions" => {
            swap_two_positions(&mut rs.white_positions, &mut rs.white_facings, rng);
        }
        "rotate_white" => {
            rotate_piece(&mut rs.white_facings, rng);
        }

        // === Black mutations ===
        "add_black" => {
            if rs.black_pieces.len() < MAX_PIECES {
                add_piece_black(rs, rng, false);
            }
        }
        "add_copy_black" => {
            if rs.black_pieces.len() < MAX_PIECES && !rs.black_pieces.is_empty() {
                add_piece_black(rs, rng, true);
            }
        }
        "remove_black" => {
            if rs.black_pieces.len() > MIN_PIECES {
                remove_piece_black(rs, rng);
            }
        }
        "swap_black" => {
            if !rs.black_pieces.is_empty() {
                swap_piece_black(rs, rng, false);
            }
        }
        "swap_existing_black" => {
            if rs.black_pieces.len() >= 2 {
                swap_piece_black(rs, rng, true);
            }
        }
        "change_black_king" => {
            rs.black_king = KING_IDS[rng.gen_range(0..KING_IDS.len())];
        }
        "shuffle_black_positions" => {
            shuffle_positions(&mut rs.black_positions, &mut rs.black_facings, rng);
        }
        "swap_two_black_positions" => {
            swap_two_positions(&mut rs.black_positions, &mut rs.black_facings, rng);
        }
        "rotate_black" => {
            rotate_piece(&mut rs.black_facings, rng);
        }

        _ => {} // Unknown mutation, do nothing
    }
}

// ============================================================================
// Helper Functions
// ============================================================================

fn add_piece_white<R: Rng>(rs: &mut RuleSet, rng: &mut R, copy_existing: bool) {
    let zone = white_piece_zone();
    let available: Vec<Hex> = zone
        .into_iter()
        .filter(|h| !rs.white_positions.contains(h))
        .collect();

    if available.is_empty() {
        return;
    }

    let piece = if copy_existing && !rs.white_pieces.is_empty() {
        rs.white_pieces[rng.gen_range(0..rs.white_pieces.len())]
    } else {
        REGULAR_PIECE_IDS[rng.gen_range(0..REGULAR_PIECE_IDS.len())]
    };

    let pos = available[rng.gen_range(0..available.len())];

    rs.white_pieces.push(piece);
    rs.white_positions.push(pos);
    rs.white_facings.push(0);  // Default facing for white
}

fn add_piece_black<R: Rng>(rs: &mut RuleSet, rng: &mut R, copy_existing: bool) {
    let zone = black_piece_zone();
    let available: Vec<Hex> = zone
        .into_iter()
        .filter(|h| !rs.black_positions.contains(h))
        .collect();

    if available.is_empty() {
        return;
    }

    let piece = if copy_existing && !rs.black_pieces.is_empty() {
        rs.black_pieces[rng.gen_range(0..rs.black_pieces.len())]
    } else {
        REGULAR_PIECE_IDS[rng.gen_range(0..REGULAR_PIECE_IDS.len())]
    };

    let pos = available[rng.gen_range(0..available.len())];

    rs.black_pieces.push(piece);
    rs.black_positions.push(pos);
    rs.black_facings.push(3);  // Default facing for black
}

fn remove_piece_white<R: Rng>(rs: &mut RuleSet, rng: &mut R) {
    if rs.white_pieces.is_empty() {
        return;
    }

    let idx = rng.gen_range(0..rs.white_pieces.len());
    rs.white_pieces.remove(idx);

    // Positions/facings: king is at index 0, pieces start at index 1
    if idx + 1 < rs.white_positions.len() {
        rs.white_positions.remove(idx + 1);
    }
    if idx + 1 < rs.white_facings.len() {
        rs.white_facings.remove(idx + 1);
    }
}

fn remove_piece_black<R: Rng>(rs: &mut RuleSet, rng: &mut R) {
    if rs.black_pieces.is_empty() {
        return;
    }

    let idx = rng.gen_range(0..rs.black_pieces.len());
    rs.black_pieces.remove(idx);

    // Positions/facings: king is at index 0, pieces start at index 1
    if idx + 1 < rs.black_positions.len() {
        rs.black_positions.remove(idx + 1);
    }
    if idx + 1 < rs.black_facings.len() {
        rs.black_facings.remove(idx + 1);
    }
}

fn swap_piece_white<R: Rng>(rs: &mut RuleSet, rng: &mut R, use_existing: bool) {
    if rs.white_pieces.is_empty() {
        return;
    }

    let idx = rng.gen_range(0..rs.white_pieces.len());

    let new_piece = if use_existing && rs.white_pieces.len() >= 2 {
        // Pick from existing types (excluding the one we're replacing)
        let other_idx = (idx + 1 + rng.gen_range(0..rs.white_pieces.len() - 1)) % rs.white_pieces.len();
        rs.white_pieces[other_idx]
    } else {
        REGULAR_PIECE_IDS[rng.gen_range(0..REGULAR_PIECE_IDS.len())]
    };

    rs.white_pieces[idx] = new_piece;
}

fn swap_piece_black<R: Rng>(rs: &mut RuleSet, rng: &mut R, use_existing: bool) {
    if rs.black_pieces.is_empty() {
        return;
    }

    let idx = rng.gen_range(0..rs.black_pieces.len());

    let new_piece = if use_existing && rs.black_pieces.len() >= 2 {
        let other_idx = (idx + 1 + rng.gen_range(0..rs.black_pieces.len() - 1)) % rs.black_pieces.len();
        rs.black_pieces[other_idx]
    } else {
        REGULAR_PIECE_IDS[rng.gen_range(0..REGULAR_PIECE_IDS.len())]
    };

    rs.black_pieces[idx] = new_piece;
}

fn shuffle_positions<R: Rng>(positions: &mut Vec<Hex>, facings: &mut Vec<u8>, rng: &mut R) {
    if positions.len() <= 2 {
        return;  // Nothing to shuffle (just king + maybe one piece)
    }

    // Shuffle piece positions (index 1+), keep king at index 0
    let n = positions.len();
    for i in (2..n).rev() {
        let j = rng.gen_range(1..=i);
        positions.swap(i, j);
        if i < facings.len() && j < facings.len() {
            facings.swap(i, j);
        }
    }
}

fn swap_two_positions<R: Rng>(positions: &mut Vec<Hex>, facings: &mut Vec<u8>, rng: &mut R) {
    if positions.len() < 3 {
        return;  // Need king + at least 2 pieces
    }

    let n = positions.len();
    let i = rng.gen_range(1..n);
    let j = (i + 1 + rng.gen_range(0..n - 2)) % (n - 1) + 1;

    positions.swap(i, j);
    if i < facings.len() && j < facings.len() {
        facings.swap(i, j);
    }
}

fn rotate_piece<R: Rng>(facings: &mut Vec<u8>, rng: &mut R) {
    if facings.len() <= 1 {
        return;  // Nothing to rotate (just king)
    }

    let idx = rng.gen_range(1..facings.len());
    let rotation: i8 = match rng.gen_range(0..4) {
        0 => 1,
        1 => 2,
        2 => -1,
        _ => -2,
    };

    facings[idx] = ((facings[idx] as i8 + rotation).rem_euclid(6)) as u8;
}

fn enforce_warper_shifter_constraint(rs: &mut RuleSet) {
    // W1 and W2 can't be on the same team
    if rs.white_pieces.contains(&WARPER_ID) && rs.white_pieces.contains(&SHIFTER_ID) {
        if let Some(pos) = rs.white_pieces.iter().position(|&p| p == SHIFTER_ID) {
            rs.white_pieces.remove(pos);
            if pos + 1 < rs.white_positions.len() {
                rs.white_positions.remove(pos + 1);
            }
            if pos + 1 < rs.white_facings.len() {
                rs.white_facings.remove(pos + 1);
            }
        }
    }

    if rs.black_pieces.contains(&WARPER_ID) && rs.black_pieces.contains(&SHIFTER_ID) {
        if let Some(pos) = rs.black_pieces.iter().position(|&p| p == SHIFTER_ID) {
            rs.black_pieces.remove(pos);
            if pos + 1 < rs.black_positions.len() {
                rs.black_positions.remove(pos + 1);
            }
            if pos + 1 < rs.black_facings.len() {
                rs.black_facings.remove(pos + 1);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hexwar_core::game::Template;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    fn make_test_ruleset() -> RuleSet {
        RuleSet {
            name: "test".to_string(),
            white_king: 25,
            white_pieces: vec![1, 1, 2, 3, 4, 5, 6, 7, 8, 9],  // 10 pieces
            white_positions: vec![
                Hex::new(0, 3),   // King
                Hex::new(-1, 3),
                Hex::new(1, 2),
                Hex::new(-2, 3),
                Hex::new(2, 1),
                Hex::new(-1, 2),
                Hex::new(1, 1),
                Hex::new(-2, 2),
                Hex::new(2, 2),
                Hex::new(0, 2),
                Hex::new(0, 1),
            ],
            white_facings: vec![0; 11],
            white_template: Template::E,
            black_king: 28,
            black_pieces: vec![1, 1, 2, 3, 4, 5, 6, 7, 8, 9],  // 10 pieces
            black_positions: vec![
                Hex::new(0, -3),  // King
                Hex::new(1, -3),
                Hex::new(-1, -2),
                Hex::new(2, -3),
                Hex::new(-2, -1),
                Hex::new(1, -2),
                Hex::new(-1, -1),
                Hex::new(2, -2),
                Hex::new(-2, -2),
                Hex::new(0, -2),
                Hex::new(0, -1),
            ],
            black_facings: vec![3; 11],
            black_template: Template::E,
        }
    }

    #[test]
    fn test_mutate_produces_valid_ruleset() {
        let rs = make_test_ruleset();
        let config = MutationConfig::default();
        let mut rng = ChaCha8Rng::seed_from_u64(42);

        for _ in 0..100 {
            let mutated = mutate_ruleset(&rs, &config, &mut rng);

            // Check piece count constraints
            assert!(mutated.white_pieces.len() >= MIN_PIECES || rs.white_pieces.len() <= MIN_PIECES);
            assert!(mutated.white_pieces.len() <= MAX_PIECES);
            assert!(mutated.black_pieces.len() >= MIN_PIECES || rs.black_pieces.len() <= MIN_PIECES);
            assert!(mutated.black_pieces.len() <= MAX_PIECES);

            // Check warper/shifter constraint
            assert!(
                !(mutated.white_pieces.contains(&WARPER_ID) && mutated.white_pieces.contains(&SHIFTER_ID)),
                "White has both Warper and Shifter"
            );
            assert!(
                !(mutated.black_pieces.contains(&WARPER_ID) && mutated.black_pieces.contains(&SHIFTER_ID)),
                "Black has both Warper and Shifter"
            );
        }
    }

    #[test]
    fn test_mutate_white_only() {
        let rs = make_test_ruleset();
        let config = MutationConfig {
            side: MutateSide::White,
            ..Default::default()
        };
        let mut rng = ChaCha8Rng::seed_from_u64(42);

        for _ in 0..50 {
            let mutated = mutate_ruleset(&rs, &config, &mut rng);

            // Black side should be unchanged
            assert_eq!(mutated.black_pieces, rs.black_pieces);
            assert_eq!(mutated.black_king, rs.black_king);
        }
    }

    #[test]
    fn test_mutate_black_only() {
        let rs = make_test_ruleset();
        let config = MutationConfig {
            side: MutateSide::Black,
            ..Default::default()
        };
        let mut rng = ChaCha8Rng::seed_from_u64(42);

        for _ in 0..50 {
            let mutated = mutate_ruleset(&rs, &config, &mut rng);

            // White side should be unchanged
            assert_eq!(mutated.white_pieces, rs.white_pieces);
            assert_eq!(mutated.white_king, rs.white_king);
        }
    }

    #[test]
    fn test_piece_tier() {
        // Check a few known tiers
        assert_eq!(piece_tier(0), 0);  // A1 = pawn
        assert_eq!(piece_tier(1), 1);  // A2 = guard
        assert_eq!(piece_tier(16), 6); // D5 = queen
        assert_eq!(piece_tier(21), 5); // W1 = warper
    }

    #[test]
    fn test_pieces_by_tier() {
        let tier0 = pieces_by_tier(0);
        assert!(tier0.contains(&0));  // A1
        assert!(tier0.contains(&2));  // A3

        let tier6 = pieces_by_tier(6);
        assert_eq!(tier6, vec![16]);  // Only queen
    }

    #[test]
    fn test_rotate_piece() {
        let mut facings = vec![0, 2, 4, 1];
        let mut rng = ChaCha8Rng::seed_from_u64(42);

        for _ in 0..10 {
            rotate_piece(&mut facings, &mut rng);
        }

        // King facing (index 0) should be unchanged
        assert_eq!(facings[0], 0);

        // All facings should be valid (0-5)
        for &f in &facings {
            assert!(f < 6, "Facing {} out of range", f);
        }
    }
}
