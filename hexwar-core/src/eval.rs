//! Position evaluation

use crate::game::{GameState, GameResult, Player};
use crate::pieces::get_piece_type;
use serde::{Deserialize, Serialize};

/// Heuristic weights for position evaluation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Heuristics {
    /// Value of each piece type (index 0-31)
    pub piece_values: [f32; 32],
    /// Weight for king centrality
    pub center_weight: f32,
    /// Weight for mobility (legal move count)
    pub mobility_weight: f32,
}

impl Default for Heuristics {
    fn default() -> Self {
        // ZENITH heuristics - optimized through tournament evolution
        // Philosophy: High material values, strong center weight, zero mobility
        let mut values = [1.0f32; 32];

        // Basic pieces - inflated values
        values[0] = 4.0;   // Pawn
        values[1] = 10.0;  // Guard (all-direction premium)
        values[2] = 6.0;   // Scout
        values[3] = 6.0;   // Crab
        values[4] = 5.0;   // Flanker

        // Better mobility pieces
        values[5] = 7.0;   // Strider
        values[6] = 8.0;   // Dancer
        values[7] = 14.0;  // Ranger (all-direction premium)
        values[8] = 9.0;   // Hound

        // Strong pieces
        values[9] = 9.0;   // Lancer
        values[10] = 12.0; // Dragoon
        values[11] = 16.0; // Courser (all-direction premium)

        // Sliders - powerful
        values[12] = 10.0; // Pike
        values[13] = 12.0; // Rook
        values[14] = 12.0; // Bishop
        values[15] = 13.0; // Chariot
        values[16] = 19.0; // Queen

        // Jumpers - tactical
        values[17] = 9.0;  // Knight
        values[18] = 10.0; // Frog
        values[19] = 10.0; // Locust
        values[20] = 11.0; // Cricket

        // Special pieces
        values[21] = 8.0;  // Warper
        values[22] = 8.5;  // Shifter
        values[23] = 8.0;  // Phoenix
        values[24] = 6.0;  // Ghost

        // Kings: zero (handled specially in evaluation)
        values[25..30].fill(0.0);

        // Trident pieces (new)
        values[30] = 8.0;   // Triton (step-2, 3 non-adjacent dirs) - like Dancer+
        values[31] = 11.0;  // Triskelion (slider, 3 non-adjacent dirs) - like Bishop-

        Self {
            piece_values: values,
            center_weight: 1.5,      // Strong center preference
            mobility_weight: 0.0,    // Ignore mobility entirely
        }
    }
}

/// Win value (effectively infinite)
pub const WIN_VALUE: f32 = 100000.0;

/// Maximum king-of-the-hill bonus at round 50
const KOTH_MAX_URGENCY: f32 = 50.0;
const KOTH_ROUND_LIMIT: f32 = 50.0;

/// Evaluate position from current player's perspective
pub fn evaluate(state: &GameState, heuristics: &Heuristics) -> f32 {
    // Check terminal states
    match state.result() {
        GameResult::WhiteWins => {
            return if state.current_player() == Player::White {
                WIN_VALUE
            } else {
                -WIN_VALUE
            };
        }
        GameResult::BlackWins => {
            return if state.current_player() == Player::Black {
                WIN_VALUE
            } else {
                -WIN_VALUE
            };
        }
        GameResult::Ongoing => {}
    }

    let current = state.current_player();
    let opponent = current.opponent();

    let mut score = 0.0f32;

    // Material and position evaluation
    for (hex, piece) in state.pieces() {
        let pt = get_piece_type(piece.piece_type);

        // Kings have infinite value (handled at terminal check)
        let piece_value = if pt.is_king {
            0.0
        } else {
            heuristics.piece_values[piece.piece_type as usize]
        };

        let center_bonus = heuristics.center_weight * (4.0 - hex.distance_to_center() as f32);
        let value = piece_value + center_bonus;

        if piece.owner == current {
            score += value;
        } else {
            score -= value;
        }
    }

    // Mobility evaluation
    if heuristics.mobility_weight.abs() > 0.001 {
        let my_mobility = state.mobility(current) as f32;
        let opp_mobility = state.mobility(opponent) as f32;
        score += heuristics.mobility_weight * (my_mobility - opp_mobility);
    }

    // King-of-the-hill urgency: accelerates as round 50 approaches
    // Uses cubic curve for aggressive late-game urgency
    let round_progress = (state.round as f32 / KOTH_ROUND_LIMIT).min(1.0);
    let urgency = round_progress * round_progress * round_progress * KOTH_MAX_URGENCY;

    if urgency > 0.1 {
        let my_king_pos = if current == Player::White {
            state.white_king_pos()
        } else {
            state.black_king_pos()
        };
        let opp_king_pos = if current == Player::White {
            state.black_king_pos()
        } else {
            state.white_king_pos()
        };

        match (my_king_pos, opp_king_pos) {
            (Some(my_pos), Some(opp_pos)) => {
                let my_dist = my_pos.distance_to_center() as f32;
                let opp_dist = opp_pos.distance_to_center() as f32;
                // Positive if I'm closer to center (winning KOTH)
                let koth_advantage = opp_dist - my_dist;
                score += urgency * koth_advantage;
            }
            (Some(_), None) => {
                // Opponent has no king - we're winning anyway
                score += urgency * 4.0;
            }
            (None, Some(_)) => {
                // We have no king - we're losing anyway
                score -= urgency * 4.0;
            }
            (None, None) => {}
        }
    }

    score
}

/// Evaluate with depth bonus for preferring faster wins
pub fn evaluate_with_depth(state: &GameState, heuristics: &Heuristics, depth: i32) -> f32 {
    match state.result() {
        GameResult::WhiteWins => {
            let base = if state.current_player() == Player::White {
                WIN_VALUE
            } else {
                -WIN_VALUE
            };
            // Win sooner is better (higher depth = closer to current position)
            return base + if base > 0.0 { depth as f32 } else { -(depth as f32) };
        }
        GameResult::BlackWins => {
            let base = if state.current_player() == Player::Black {
                WIN_VALUE
            } else {
                -WIN_VALUE
            };
            return base + if base > 0.0 { depth as f32 } else { -(depth as f32) };
        }
        GameResult::Ongoing => evaluate(state, heuristics),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::board::Hex;
    use crate::game::Template;
    use crate::pieces::piece_id_to_index;

    fn simple_game() -> GameState {
        let white = vec![
            (piece_id_to_index("K1").unwrap(), Hex::new(0, 3), 0),
            (piece_id_to_index("A2").unwrap(), Hex::new(-1, 3), 0),
        ];
        let black = vec![
            (piece_id_to_index("K1").unwrap(), Hex::new(0, -3), 3),
            (piece_id_to_index("A2").unwrap(), Hex::new(1, -3), 3),
        ];
        GameState::new(&white, &black, Template::E, Template::E)
    }

    #[test]
    fn test_evaluate_symmetric() {
        let game = simple_game();
        let heuristics = Heuristics::default();
        let score = evaluate(&game, &heuristics);
        // Symmetric position should have score close to 0
        assert!(score.abs() < 1.0, "Score {} should be near 0 for symmetric position", score);
    }

    #[test]
    fn test_default_heuristics() {
        let h = Heuristics::default();
        assert!(h.piece_values[16] > h.piece_values[0]); // Queen > Pawn
        assert!(h.center_weight > 0.0);
        assert!(h.mobility_weight >= 0.0);
    }
}
