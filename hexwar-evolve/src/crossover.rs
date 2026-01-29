//! Crossover operators for ruleset evolution
//!
//! Combines two parent rulesets to produce offspring that inherit
//! traits from both parents.

use hexwar_core::RuleSet;
use rand::Rng;

/// Crossover two rulesets by swapping factions.
///
/// Each faction (white army, black army) has a 50% chance of coming
/// from either parent. Positions and facings are preserved with their
/// respective armies.
///
/// # Arguments
/// * `a` - First parent ruleset
/// * `b` - Second parent ruleset
/// * `rng` - Random number generator
///
/// # Returns
/// New ruleset combining traits from both parents
pub fn crossover_rulesets<R: Rng>(a: &RuleSet, b: &RuleSet, rng: &mut R) -> RuleSet {
    // 50% chance to take white army from parent A vs B
    let (white_pieces, white_positions, white_facings, white_king, white_template) =
        if rng.gen_bool(0.5) {
            (
                a.white_pieces.clone(),
                a.white_positions.clone(),
                a.white_facings.clone(),
                a.white_king,
                a.white_template,
            )
        } else {
            (
                b.white_pieces.clone(),
                b.white_positions.clone(),
                b.white_facings.clone(),
                b.white_king,
                b.white_template,
            )
        };

    // 50% chance to take black army from parent A vs B
    let (black_pieces, black_positions, black_facings, black_king, black_template) =
        if rng.gen_bool(0.5) {
            (
                a.black_pieces.clone(),
                a.black_positions.clone(),
                a.black_facings.clone(),
                a.black_king,
                a.black_template,
            )
        } else {
            (
                b.black_pieces.clone(),
                b.black_positions.clone(),
                b.black_facings.clone(),
                b.black_king,
                b.black_template,
            )
        };

    RuleSet {
        name: "crossover".to_string(),
        white_king,
        white_pieces,
        white_positions,
        white_facings,
        white_template,
        black_king,
        black_pieces,
        black_positions,
        black_facings,
        black_template,
    }
}

/// Crossover with piece-level mixing.
///
/// Instead of swapping entire armies, this variant mixes individual
/// pieces from both parents. Each piece position has a chance to
/// come from either parent.
///
/// Note: This can produce invalid rulesets if parents have different
/// army sizes. Use `crossover_rulesets` for safer operation.
///
/// # Arguments
/// * `a` - First parent ruleset
/// * `b` - Second parent ruleset
/// * `mix_rate` - Probability of taking piece from parent B (0.0 to 1.0)
/// * `rng` - Random number generator
///
/// # Returns
/// New ruleset with mixed pieces
pub fn crossover_piece_mix<R: Rng>(
    a: &RuleSet,
    b: &RuleSet,
    mix_rate: f64,
    rng: &mut R,
) -> RuleSet {
    let mut result = a.clone();
    result.name = "crossover-mix".to_string();

    // Mix white pieces (use minimum length to avoid index errors)
    let white_len = a.white_pieces.len().min(b.white_pieces.len());
    for i in 0..white_len {
        if rng.gen_bool(mix_rate) {
            result.white_pieces[i] = b.white_pieces[i];
        }
    }

    // Mix black pieces
    let black_len = a.black_pieces.len().min(b.black_pieces.len());
    for i in 0..black_len {
        if rng.gen_bool(mix_rate) {
            result.black_pieces[i] = b.black_pieces[i];
        }
    }

    // Kings: 50% chance each
    if rng.gen_bool(0.5) {
        result.white_king = b.white_king;
    }
    if rng.gen_bool(0.5) {
        result.black_king = b.black_king;
    }

    result
}

/// Crossover that preserves one side (for fixed-opponent evolution).
///
/// When evolving against a fixed opponent, we only want to crossover
/// the evolving side.
///
/// # Arguments
/// * `a` - First parent ruleset
/// * `b` - Second parent ruleset
/// * `preserve_white` - If true, always use parent A's white army
/// * `rng` - Random number generator
///
/// # Returns
/// New ruleset with constrained crossover
pub fn crossover_one_side<R: Rng>(
    a: &RuleSet,
    b: &RuleSet,
    preserve_white: bool,
    rng: &mut R,
) -> RuleSet {
    if preserve_white {
        // White army always from A, black army from either
        let (black_pieces, black_positions, black_facings, black_king, black_template) =
            if rng.gen_bool(0.5) {
                (
                    a.black_pieces.clone(),
                    a.black_positions.clone(),
                    a.black_facings.clone(),
                    a.black_king,
                    a.black_template,
                )
            } else {
                (
                    b.black_pieces.clone(),
                    b.black_positions.clone(),
                    b.black_facings.clone(),
                    b.black_king,
                    b.black_template,
                )
            };

        RuleSet {
            name: "crossover".to_string(),
            white_king: a.white_king,
            white_pieces: a.white_pieces.clone(),
            white_positions: a.white_positions.clone(),
            white_facings: a.white_facings.clone(),
            white_template: a.white_template,
            black_king,
            black_pieces,
            black_positions,
            black_facings,
            black_template,
        }
    } else {
        // Black army always from A, white army from either
        let (white_pieces, white_positions, white_facings, white_king, white_template) =
            if rng.gen_bool(0.5) {
                (
                    a.white_pieces.clone(),
                    a.white_positions.clone(),
                    a.white_facings.clone(),
                    a.white_king,
                    a.white_template,
                )
            } else {
                (
                    b.white_pieces.clone(),
                    b.white_positions.clone(),
                    b.white_facings.clone(),
                    b.white_king,
                    b.white_template,
                )
            };

        RuleSet {
            name: "crossover".to_string(),
            white_king,
            white_pieces,
            white_positions,
            white_facings,
            white_template,
            black_king: a.black_king,
            black_pieces: a.black_pieces.clone(),
            black_positions: a.black_positions.clone(),
            black_facings: a.black_facings.clone(),
            black_template: a.black_template,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use hexwar_core::board::Hex;
    use hexwar_core::game::Template;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    fn make_parent_a() -> RuleSet {
        RuleSet {
            name: "parent-a".to_string(),
            white_king: 25,  // K1
            white_pieces: vec![1, 1, 1, 1],  // All guards
            white_positions: vec![
                Hex::new(0, 3),
                Hex::new(-1, 3),
                Hex::new(1, 2),
                Hex::new(-2, 4),
                Hex::new(2, 1),
            ],
            white_facings: vec![0; 5],
            white_template: Template::E,
            black_king: 25,
            black_pieces: vec![2, 2, 2, 2],  // All scouts
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

    fn make_parent_b() -> RuleSet {
        RuleSet {
            name: "parent-b".to_string(),
            white_king: 28,  // K4
            white_pieces: vec![16, 16, 16, 16],  // All queens
            white_positions: vec![
                Hex::new(0, 3),
                Hex::new(-1, 2),
                Hex::new(1, 3),
                Hex::new(-2, 3),
                Hex::new(2, 2),
            ],
            white_facings: vec![1; 5],
            white_template: Template::D,
            black_king: 28,
            black_pieces: vec![17, 17, 17, 17],  // All knights
            black_positions: vec![
                Hex::new(0, -3),
                Hex::new(1, -2),
                Hex::new(-1, -3),
                Hex::new(2, -3),
                Hex::new(-2, -2),
            ],
            black_facings: vec![4; 5],
            black_template: Template::D,
        }
    }

    #[test]
    fn test_crossover_produces_valid_child() {
        let a = make_parent_a();
        let b = make_parent_b();
        let mut rng = ChaCha8Rng::seed_from_u64(42);

        for _ in 0..100 {
            let child = crossover_rulesets(&a, &b, &mut rng);

            // Child should have armies from one of the parents
            assert!(
                child.white_pieces == a.white_pieces || child.white_pieces == b.white_pieces,
                "Child white pieces should match one parent"
            );
            assert!(
                child.black_pieces == a.black_pieces || child.black_pieces == b.black_pieces,
                "Child black pieces should match one parent"
            );
        }
    }

    #[test]
    fn test_crossover_mixes_parents() {
        let a = make_parent_a();
        let b = make_parent_b();
        let mut rng = ChaCha8Rng::seed_from_u64(42);

        let mut saw_a_white = false;
        let mut saw_b_white = false;
        let mut saw_a_black = false;
        let mut saw_b_black = false;

        for _ in 0..100 {
            let child = crossover_rulesets(&a, &b, &mut rng);

            if child.white_pieces == a.white_pieces {
                saw_a_white = true;
            }
            if child.white_pieces == b.white_pieces {
                saw_b_white = true;
            }
            if child.black_pieces == a.black_pieces {
                saw_a_black = true;
            }
            if child.black_pieces == b.black_pieces {
                saw_b_black = true;
            }
        }

        assert!(saw_a_white, "Should sometimes get white from parent A");
        assert!(saw_b_white, "Should sometimes get white from parent B");
        assert!(saw_a_black, "Should sometimes get black from parent A");
        assert!(saw_b_black, "Should sometimes get black from parent B");
    }

    #[test]
    fn test_crossover_one_side_preserve_white() {
        let a = make_parent_a();
        let b = make_parent_b();
        let mut rng = ChaCha8Rng::seed_from_u64(42);

        for _ in 0..50 {
            let child = crossover_one_side(&a, &b, true, &mut rng);

            // White should always be from A
            assert_eq!(child.white_pieces, a.white_pieces);
            assert_eq!(child.white_king, a.white_king);
        }
    }

    #[test]
    fn test_crossover_one_side_preserve_black() {
        let a = make_parent_a();
        let b = make_parent_b();
        let mut rng = ChaCha8Rng::seed_from_u64(42);

        for _ in 0..50 {
            let child = crossover_one_side(&a, &b, false, &mut rng);

            // Black should always be from A
            assert_eq!(child.black_pieces, a.black_pieces);
            assert_eq!(child.black_king, a.black_king);
        }
    }

    #[test]
    fn test_crossover_piece_mix() {
        let a = make_parent_a();
        let b = make_parent_b();
        let mut rng = ChaCha8Rng::seed_from_u64(42);

        // With mix_rate = 1.0, should get all pieces from B
        let child = crossover_piece_mix(&a, &b, 1.0, &mut rng);
        assert_eq!(child.white_pieces, b.white_pieces);
        assert_eq!(child.black_pieces, b.black_pieces);

        // With mix_rate = 0.0, should get all pieces from A
        let child = crossover_piece_mix(&a, &b, 0.0, &mut rng);
        assert_eq!(child.white_pieces, a.white_pieces);
        assert_eq!(child.black_pieces, a.black_pieces);
    }
}
