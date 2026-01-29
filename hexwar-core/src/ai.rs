//! CPU-based Alpha-Beta AI

use crate::eval::{evaluate, evaluate_with_depth, Heuristics, WIN_VALUE};
use crate::game::{GameState, GameResult, Move, Player};
use crate::pieces::get_piece_type;
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;

// ============================================================================
// CONSTANTS
// ============================================================================

/// Null-move pruning reduction factor
const NULL_MOVE_R: i32 = 2;

/// Late Move Reduction thresholds
const LMR_MOVE_THRESHOLD: usize = 3;
const LMR_DEPTH_THRESHOLD: i32 = 2;

/// Noise scale for evaluation variety
const NOISE_SCALE: f32 = 0.1;

// ============================================================================
// ALPHA-BETA AI
// ============================================================================

/// Alpha-Beta AI player
pub struct AlphaBetaAI {
    pub depth: u32,
    pub max_moves_per_action: usize,
    pub heuristics: Heuristics,
    rng: ChaCha8Rng,
}

impl AlphaBetaAI {
    pub fn new(depth: u32, heuristics: Heuristics) -> Self {
        Self {
            depth,
            max_moves_per_action: 15,
            heuristics,
            rng: ChaCha8Rng::seed_from_u64(42),
        }
    }

    pub fn with_seed(depth: u32, heuristics: Heuristics, seed: u64) -> Self {
        Self {
            depth,
            max_moves_per_action: 15,
            heuristics,
            rng: ChaCha8Rng::seed_from_u64(seed),
        }
    }

    /// Get best move for current position
    pub fn best_move(&mut self, state: &GameState) -> Option<Move> {
        get_best_move(
            state,
            self.depth as i32,
            &self.heuristics,
            self.max_moves_per_action,
            &mut self.rng,
            NOISE_SCALE,
        )
    }

    /// Play a complete game
    pub fn play_game(&mut self, initial: GameState, max_rounds: u32) -> (GameState, Vec<Move>) {
        let mut state = initial;
        let mut history = Vec::new();
        let mut moves_made = 0;
        let max_moves = max_rounds as usize * 10; // Rough estimate

        while state.result() == GameResult::Ongoing && moves_made < max_moves {
            if let Some(mv) = self.best_move(&state) {
                history.push(mv);
                state = state.apply_move(mv);
                moves_made += 1;
            } else {
                break;
            }
        }

        (state, history)
    }

    /// Evaluate a position
    pub fn evaluate(&self, state: &GameState) -> f32 {
        evaluate(state, &self.heuristics)
    }
}

// ============================================================================
// MOVE ORDERING
// ============================================================================

/// Score a move for ordering (higher = search first)
fn move_score(state: &GameState, mv: &Move, heuristics: &Heuristics) -> f32 {
    match mv {
        Move::Pass => -1000.0,
        Move::Surrender => -50000.0,
        Move::Swap { .. } => 50.0,
        Move::Rotate { .. } => 0.0,
        Move::Rebirth { .. } => 40.0,
        Move::Movement { from, to, .. } => {
            let mut score = 0.0;

            // Capture bonus (MVV - Most Valuable Victim)
            if let Some(victim) = state.get_piece(*to) {
                if victim.owner != state.current_player() {
                    let pt = get_piece_type(victim.piece_type);
                    let value = if pt.is_king {
                        WIN_VALUE
                    } else {
                        heuristics.piece_values[victim.piece_type as usize]
                    };
                    score += value * 10.0;
                }
            }

            // Center proximity bonus
            let from_dist = from.distance_to_center();
            let to_dist = to.distance_to_center();
            score += (from_dist - to_dist) as f32 * 0.5;

            score
        }
    }
}

/// Check if a move is a capture
fn is_capture_move(state: &GameState, mv: Move) -> bool {
    match mv {
        Move::Movement { to, .. } => {
            if let Some(piece) = state.get_piece(to) {
                piece.owner != state.current_player()
            } else {
                false
            }
        }
        _ => false,
    }
}

// ============================================================================
// DANGER DETECTION (for null-move pruning)
// ============================================================================

/// Check if current player's king is in immediate danger
fn is_in_danger(state: &GameState) -> bool {
    let king_pos = match state.current_player() {
        Player::White => state.white_king_pos(),
        Player::Black => state.black_king_pos(),
    };

    let king_pos = match king_pos {
        Some(p) => p,
        None => return true, // No king = definitely in danger
    };

    // Check if any enemy piece is within 3 hexes of king
    for (pos, piece) in state.pieces() {
        if piece.owner != state.current_player() {
            let dist = pos.distance_to(king_pos);
            if dist <= 3 {
                return true;
            }
        }
    }

    false
}

/// Create a null-move state (skip turn)
fn make_null_move(state: &GameState) -> GameState {
    // Create a state where we've passed all our actions
    let mut new_state = state.clone();

    // Apply Pass moves until turn changes
    loop {
        let moves = new_state.legal_moves();
        if moves.is_empty() || new_state.current_player() != state.current_player() {
            break;
        }
        new_state = new_state.apply_move(Move::Pass);
    }

    new_state
}

// ============================================================================
// NEGAMAX WITH ALPHA-BETA
// ============================================================================

fn negamax(
    state: &GameState,
    depth: i32,
    mut alpha: f32,
    beta: f32,
    heuristics: &Heuristics,
    max_moves: usize,
    rng: &mut ChaCha8Rng,
    noise_scale: f32,
    allow_null_move: bool,
) -> f32 {
    // Terminal check with depth bonus
    if state.result() != GameResult::Ongoing {
        return evaluate_with_depth(state, heuristics, depth);
    }

    // Depth limit
    if depth <= 0 {
        let base = evaluate(state, heuristics);
        let noise = (rng.gen::<f32>() - 0.5) * noise_scale;
        return base + noise;
    }

    let mut moves = state.legal_moves();
    if moves.is_empty() {
        let base = evaluate(state, heuristics);
        let noise = (rng.gen::<f32>() - 0.5) * noise_scale;
        return base + noise;
    }

    // Null-move pruning
    if allow_null_move && depth >= NULL_MOVE_R + 1 && !is_in_danger(state) {
        let null_state = make_null_move(state);
        if null_state.current_player() != state.current_player() {
            let null_score = -negamax(
                &null_state,
                depth - 1 - NULL_MOVE_R,
                -beta,
                -beta + 0.01,
                heuristics,
                max_moves,
                rng,
                noise_scale,
                false,
            );

            if null_score >= beta {
                return beta;
            }
        }
    }

    // Sort moves by score (descending)
    moves.sort_by(|a, b| {
        move_score(state, b, heuristics)
            .partial_cmp(&move_score(state, a, heuristics))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Limit moves
    if moves.len() > max_moves {
        moves.truncate(max_moves);
    }

    let mut best = f32::NEG_INFINITY;
    let original_player = state.current_player();

    for (move_index, mv) in moves.iter().enumerate() {
        let score = if matches!(mv, Move::Surrender) {
            // Surrender scores slightly better than immediate loss
            -WIN_VALUE - depth as f32 + 0.5
        } else {
            let child = state.apply_move(*mv);
            let turn_changed = child.current_player() != original_player;

            // Late Move Reductions
            let is_capture = is_capture_move(state, *mv);
            let use_lmr = turn_changed
                && move_index >= LMR_MOVE_THRESHOLD
                && depth >= LMR_DEPTH_THRESHOLD
                && !is_capture;

            if turn_changed {
                let search_depth = if use_lmr { depth - 2 } else { depth - 1 };
                let mut s = -negamax(
                    &child,
                    search_depth,
                    -beta,
                    -alpha,
                    heuristics,
                    max_moves,
                    rng,
                    noise_scale,
                    true,
                );

                // Re-search at full depth if LMR found promising move
                if use_lmr && s > alpha {
                    s = -negamax(
                        &child,
                        depth - 1,
                        -beta,
                        -alpha,
                        heuristics,
                        max_moves,
                        rng,
                        noise_scale,
                        true,
                    );
                }
                s
            } else {
                // Within same turn, don't reduce depth
                negamax(
                    &child,
                    depth,
                    alpha,
                    beta,
                    heuristics,
                    max_moves,
                    rng,
                    noise_scale,
                    allow_null_move,
                )
            }
        };

        best = best.max(score);
        alpha = alpha.max(score);

        if alpha >= beta {
            break;
        }
    }

    best
}

fn get_best_move(
    state: &GameState,
    depth: i32,
    heuristics: &Heuristics,
    max_moves: usize,
    rng: &mut ChaCha8Rng,
    noise_scale: f32,
) -> Option<Move> {
    let mut moves = state.legal_moves();
    if moves.is_empty() {
        return None;
    }
    if moves.len() == 1 {
        return Some(moves[0]);
    }

    // Sort moves
    moves.sort_by(|a, b| {
        move_score(state, b, heuristics)
            .partial_cmp(&move_score(state, a, heuristics))
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    if moves.len() > max_moves {
        moves.truncate(max_moves);
    }

    let mut best_move = moves[0];
    let mut best_score = f32::NEG_INFINITY;
    let original_player = state.current_player();

    for mv in moves {
        let score = if matches!(mv, Move::Surrender) {
            -WIN_VALUE - depth as f32 + 0.5
        } else {
            let child = state.apply_move(mv);
            let turn_changed = child.current_player() != original_player;

            if turn_changed {
                -negamax(
                    &child,
                    depth - 1,
                    f32::NEG_INFINITY,
                    f32::INFINITY,
                    heuristics,
                    max_moves,
                    rng,
                    noise_scale,
                    true,
                )
            } else {
                negamax(
                    &child,
                    depth,
                    f32::NEG_INFINITY,
                    f32::INFINITY,
                    heuristics,
                    max_moves,
                    rng,
                    noise_scale,
                    true,
                )
            }
        };

        if score > best_score {
            best_score = score;
            best_move = mv;
        }
    }

    Some(best_move)
}

// ============================================================================
// TESTS
// ============================================================================

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
    fn test_ai_returns_move() {
        let game = simple_game();
        let mut ai = AlphaBetaAI::new(2, Heuristics::default());
        let mv = ai.best_move(&game);
        assert!(mv.is_some());
    }

    #[test]
    fn test_ai_captures_king() {
        // Set up a position where white can capture black king
        let white = vec![
            (piece_id_to_index("K1").unwrap(), Hex::new(0, 0), 0),
            (piece_id_to_index("D5").unwrap(), Hex::new(0, 1), 0),
        ];
        let black = vec![
            (piece_id_to_index("K1").unwrap(), Hex::new(0, -1), 3),
        ];
        let game = GameState::new(&white, &black, Template::E, Template::E);

        let mut ai = AlphaBetaAI::new(2, Heuristics::default());
        let mv = ai.best_move(&game);

        // AI should capture the king
        assert!(matches!(mv, Some(Move::Movement { to, .. }) if to == Hex::new(0, -1)));
    }

    #[test]
    fn test_move_ordering() {
        let game = simple_game();
        let heuristics = Heuristics::default();

        // Surrender should have very low score
        let surrender_score = move_score(&game, &Move::Surrender, &heuristics);
        let pass_score = move_score(&game, &Move::Pass, &heuristics);
        assert!(surrender_score < pass_score);
    }

    #[test]
    fn test_play_game() {
        let game = simple_game();
        let mut ai = AlphaBetaAI::new(1, Heuristics::default());
        let (final_state, history) = ai.play_game(game, 10);

        // Game should have progressed
        assert!(!history.is_empty());
        // Either game ended or reached round limit
        assert!(final_state.result() != GameResult::Ongoing || final_state.round > 1);
    }
}
