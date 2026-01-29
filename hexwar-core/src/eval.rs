//! Position evaluation

use crate::game::{GameState, GameResult, Player};
use serde::{Deserialize, Serialize};

/// Heuristic weights for position evaluation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Heuristics {
    /// Value of each piece type (index 0-29)
    pub piece_values: [f32; 30],
    /// Weight for king centrality
    pub center_weight: f32,
    /// Weight for mobility (legal move count)
    pub mobility_weight: f32,
}

impl Default for Heuristics {
    fn default() -> Self {
        // Default piece values based on mobility/capability
        let mut values = [1.0f32; 30];
        // Step-1: basic pieces
        values[0] = 1.0;   // Pawn
        values[1] = 3.0;   // Guard
        values[2] = 2.0;   // Scout
        values[3] = 2.0;   // Crab
        values[4] = 1.5;   // Flanker
        // Step-2: better mobility
        values[5] = 2.5;   // Strider
        values[6] = 3.0;   // Dancer
        values[7] = 5.0;   // Ranger
        values[8] = 4.0;   // Hound
        // Step-3: even better
        values[9] = 3.5;   // Lancer
        values[10] = 5.5;  // Dragoon
        values[11] = 7.0;  // Courser
        // Sliders: powerful
        values[12] = 4.0;  // Pike
        values[13] = 5.0;  // Rook
        values[14] = 5.0;  // Bishop
        values[15] = 6.0;  // Chariot
        values[16] = 9.0;  // Queen
        // Jumpers: tactical
        values[17] = 4.0;  // Knight
        values[18] = 5.0;  // Frog
        values[19] = 5.0;  // Locust
        values[20] = 6.0;  // Cricket
        // Special: unique value
        values[21] = 4.0;  // Warper
        values[22] = 4.0;  // Shifter
        values[23] = 3.5;  // Phoenix
        values[24] = 2.0;  // Ghost
        // Kings: infinite (handled specially)
        values[25..30].fill(0.0);

        Self {
            piece_values: values,
            center_weight: 0.5,
            mobility_weight: 0.1,  // NEW: weight for legal move count
        }
    }
}

/// Win value (effectively infinite)
pub const WIN_VALUE: f32 = 100000.0;

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

    // Material evaluation
    for (hex, piece) in state.pieces() {
        let piece_value = heuristics.piece_values[piece.piece_type as usize];
        let center_bonus = heuristics.center_weight * (4.0 - hex.distance_to_center() as f32);
        let value = piece_value + center_bonus;

        if piece.owner == current {
            score += value;
        } else {
            score -= value;
        }
    }

    // Mobility evaluation (NEW)
    let my_mobility = state.mobility(current) as f32;
    let opp_mobility = state.mobility(opponent) as f32;
    score += heuristics.mobility_weight * (my_mobility - opp_mobility);

    score
}
