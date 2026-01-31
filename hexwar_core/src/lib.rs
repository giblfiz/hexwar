//! HEXWAR Core - High-performance minimax engine
//!
//! Rust implementation of the HEXWAR game engine with alpha-beta pruning.

use pyo3::prelude::*;
use rustc_hash::FxHashMap;
// use std::cmp::{max, min};  // Removed - not currently used
use rand::prelude::*;
use rand_chacha::ChaCha8Rng;

// ============================================================================
// BOARD GEOMETRY
// ============================================================================

const BOARD_RADIUS: i8 = 4;
const WIN_VALUE: f32 = 100000.0;

/// Direction vectors in axial coordinates (dq, dr)
/// Index: 0=N, 1=NE, 2=SE, 3=S, 4=SW, 5=NW
const DIRECTIONS: [(i8, i8); 6] = [
    (0, -1),  // N
    (1, -1),  // NE
    (1, 0),   // SE
    (0, 1),   // S
    (-1, 1),  // SW
    (-1, 0),  // NW
];

/// Relative directions from facing
const FORWARD: u8 = 0;
const FORWARD_RIGHT: u8 = 1;
const BACK_RIGHT: u8 = 2;
const BACKWARD: u8 = 3;
const BACK_LEFT: u8 = 4;
const FORWARD_LEFT: u8 = 5;

#[inline]
fn is_valid_hex(q: i8, r: i8) -> bool {
    q.abs() <= BOARD_RADIUS && r.abs() <= BOARD_RADIUS && (q + r).abs() <= BOARD_RADIUS
}

#[inline]
fn distance_to_center(q: i8, r: i8) -> i8 {
    (q.abs() + r.abs() + (q + r).abs()) / 2
}

#[inline]
fn get_direction_vector(facing: u8, relative: u8) -> (i8, i8) {
    let absolute = ((facing + relative) % 6) as usize;
    DIRECTIONS[absolute]
}

/// Facing angles in degrees (facing 0-5 corresponds to visual direction)
/// 0=N(270°), 1=NE(330°), 2=SE(30°), 3=S(90°), 4=SW(150°), 5=NW(210°)
const FACING_ANGLES: [f32; 6] = [270.0, 330.0, 30.0, 90.0, 150.0, 210.0];

/// Generate all hex positions at exactly distance N from origin (q, r)
/// This implements the hex ring algorithm
fn iter_hex_ring(q: i8, r: i8, distance: u8) -> impl Iterator<Item = (i8, i8)> {
    let distance = distance as i8;
    // For a hex ring at distance N, there are 6*N positions
    // Walk around the hexagon, starting from each corner and walking along each edge
    (0..6).flat_map(move |side| {
        let dir = DIRECTIONS[side];
        (0..distance).map(move |step| {
            // Start position for this side (corner of hexagon)
            let start_q = q + distance * DIRECTIONS[(side + 4) % 6].0;
            let start_r = r + distance * DIRECTIONS[(side + 4) % 6].1;
            // Walk along the edge
            (start_q + step * dir.0, start_r + step * dir.1)
        })
    })
}

/// Check if a destination hex is within the forward arc (150°, ±75° from forward)
fn in_forward_arc(from_q: i8, from_r: i8, to_q: i8, to_r: i8, facing: u8) -> bool {
    let dq = to_q - from_q;
    let dr = to_r - from_r;

    // Convert axial to cartesian for angle calculation
    // x = 1.5 * q, y = sqrt(3)/2 * q + sqrt(3) * r
    let x = 1.5 * dq as f32;
    let y = 0.8660254 * dq as f32 + 1.7320508 * dr as f32;

    // Calculate angle in degrees
    let mut angle = y.atan2(x).to_degrees();
    if angle < 0.0 {
        angle += 360.0;
    }

    // Get forward angle for this facing
    let forward_angle = FACING_ANGLES[facing as usize];

    // Calculate angular difference
    let mut diff = (angle - forward_angle).abs();
    if diff > 180.0 {
        diff = 360.0 - diff;
    }

    // Forward arc is ±75° from forward (150° total)
    diff <= 75.0
}

// ============================================================================
// PIECE TYPES
// ============================================================================

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
enum MoveType {
    Step,
    Slide,
    Jump,
    None,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
enum Special {
    None,
    SwapMove,
    SwapRotate,
    Rebirth,  // Phoenix can be resurrected from graveyard
    Phased,   // Ghost: can't capture and can't be captured
}

#[derive(Clone, Copy, Debug)]
struct PieceType {
    move_type: MoveType,
    move_range: u8,
    directions: u8,  // Bitmask: bit i = direction i is valid
    special: Special,
    is_king: bool,
}

impl PieceType {
    const fn new(move_type: MoveType, range: u8, dirs: u8, special: Special, is_king: bool) -> Self {
        Self { move_type, move_range: range, directions: dirs, special, is_king }
    }
}

// Direction bitmasks
const DIR_F: u8 = 1 << FORWARD;
const DIR_FR: u8 = 1 << FORWARD_RIGHT;
const DIR_BR: u8 = 1 << BACK_RIGHT;
const DIR_B: u8 = 1 << BACKWARD;
const DIR_BL: u8 = 1 << BACK_LEFT;
const DIR_FL: u8 = 1 << FORWARD_LEFT;

const ALL_DIRS: u8 = DIR_F | DIR_FR | DIR_BR | DIR_B | DIR_BL | DIR_FL;
const FORWARD_ARC: u8 = DIR_F | DIR_FL | DIR_FR;
const DIAGONAL_DIRS: u8 = DIR_FL | DIR_FR | DIR_BL | DIR_BR;
const FORWARD_BACK: u8 = DIR_F | DIR_B;
const TRIDENT_DIRS: u8 = DIR_FL | DIR_FR | DIR_B;  // Three non-adjacent directions

// Piece type indices (matches Python IDs)
const PT_A1: u8 = 0;   // Pawn
const PT_A2: u8 = 1;   // Guard
const PT_A3: u8 = 2;   // Scout
const PT_A4: u8 = 3;   // Crab
const PT_A5: u8 = 4;   // Flanker
const PT_B1: u8 = 5;   // Strider
const PT_B2: u8 = 6;   // Dancer
const PT_B3: u8 = 7;   // Ranger
const PT_B4: u8 = 8;   // Hound
const PT_C1: u8 = 9;   // Lancer
const PT_C2: u8 = 10;  // Dragoon
const PT_C3: u8 = 11;  // Courser
const PT_D1: u8 = 12;  // Pike
const PT_D2: u8 = 13;  // Rook
const PT_D3: u8 = 14;  // Bishop
const PT_D4: u8 = 15;  // Chariot
const PT_D5: u8 = 16;  // Queen
const PT_E1: u8 = 17;  // Knight
const PT_E2: u8 = 18;  // Frog
const PT_F1: u8 = 19;  // Locust
const PT_F2: u8 = 20;  // Cricket
const PT_W1: u8 = 21;  // Warper
const PT_W2: u8 = 22;  // Shifter
const PT_P1: u8 = 23;  // Phoenix
const PT_G1: u8 = 24;  // Ghost
const PT_K1: u8 = 25;  // King Guard
const PT_K2: u8 = 26;  // King Scout
const PT_K3: u8 = 27;  // King Ranger
const PT_K4: u8 = 28;  // King Frog
const PT_K5: u8 = 29;  // King Pike
const PT_B5: u8 = 30;  // Triton (Step-2, Trident)
const PT_D6: u8 = 31;  // Triskelion (Slide, Trident)

const PIECE_TYPES: [PieceType; 32] = [
    // Step-1
    PieceType::new(MoveType::Step, 1, DIR_F, Special::None, false),              // A1 Pawn
    PieceType::new(MoveType::Step, 1, ALL_DIRS, Special::None, false),           // A2 Guard
    PieceType::new(MoveType::Step, 1, FORWARD_ARC, Special::None, false),        // A3 Scout
    PieceType::new(MoveType::Step, 1, DIR_FL | DIR_FR | DIR_B, Special::None, false), // A4 Crab
    PieceType::new(MoveType::Step, 1, DIR_FL | DIR_FR, Special::None, false),    // A5 Flanker
    // Step-2
    PieceType::new(MoveType::Step, 2, DIR_F, Special::None, false),              // B1 Strider
    PieceType::new(MoveType::Step, 2, DIR_FL | DIR_FR, Special::None, false),    // B2 Dancer
    PieceType::new(MoveType::Step, 2, ALL_DIRS, Special::None, false),           // B3 Ranger
    PieceType::new(MoveType::Step, 2, FORWARD_ARC, Special::None, false),        // B4 Hound
    // Step-3
    PieceType::new(MoveType::Step, 3, DIR_F, Special::None, false),              // C1 Lancer
    PieceType::new(MoveType::Step, 3, FORWARD_ARC, Special::None, false),        // C2 Dragoon
    PieceType::new(MoveType::Step, 3, ALL_DIRS, Special::None, false),           // C3 Courser
    // Slide
    PieceType::new(MoveType::Slide, 99, DIR_F, Special::None, false),            // D1 Pike
    PieceType::new(MoveType::Slide, 99, FORWARD_BACK, Special::None, false),     // D2 Rook
    PieceType::new(MoveType::Slide, 99, DIAGONAL_DIRS, Special::None, false),    // D3 Bishop
    PieceType::new(MoveType::Slide, 99, FORWARD_ARC, Special::None, false),      // D4 Chariot
    PieceType::new(MoveType::Slide, 99, ALL_DIRS, Special::None, false),         // D5 Queen
    // Jump
    PieceType::new(MoveType::Jump, 2, FORWARD_ARC, Special::None, false),        // E1 Knight (forward arc, d2)
    PieceType::new(MoveType::Jump, 2, ALL_DIRS, Special::None, false),           // E2 Frog (omni, d2)
    PieceType::new(MoveType::Jump, 3, FORWARD_ARC, Special::None, false),        // F1 Locust (forward arc, d3)
    PieceType::new(MoveType::Jump, 3, ALL_DIRS, Special::None, false),           // F2 Cricket (omni, d3)
    // Special
    PieceType::new(MoveType::None, 0, 0, Special::SwapMove, false),              // W1 Warper
    PieceType::new(MoveType::Step, 1, ALL_DIRS, Special::SwapRotate, false),     // W2 Shifter
    PieceType::new(MoveType::Step, 1, FORWARD_ARC, Special::Rebirth, false),     // P1 Phoenix
    PieceType::new(MoveType::Step, 1, ALL_DIRS, Special::Phased, false),         // G1 Ghost
    // Kings
    PieceType::new(MoveType::Step, 1, ALL_DIRS, Special::None, true),            // K1 Guard
    PieceType::new(MoveType::Step, 1, FORWARD_ARC, Special::None, true),         // K2 Scout
    PieceType::new(MoveType::Step, 2, ALL_DIRS, Special::None, true),            // K3 Ranger
    PieceType::new(MoveType::Jump, 2, ALL_DIRS, Special::None, true),            // K4 Frog
    PieceType::new(MoveType::Slide, 99, DIR_F, Special::None, true),             // K5 Pike
    // Trident pieces
    PieceType::new(MoveType::Step, 2, TRIDENT_DIRS, Special::None, false),       // B5 Triton
    PieceType::new(MoveType::Slide, 99, TRIDENT_DIRS, Special::None, false),     // D6 Triskelion
];

fn piece_id_to_index(id: &str) -> Option<u8> {
    match id {
        "A1" => Some(PT_A1), "A2" => Some(PT_A2), "A3" => Some(PT_A3),
        "A4" => Some(PT_A4), "A5" => Some(PT_A5),
        "B1" => Some(PT_B1), "B2" => Some(PT_B2), "B3" => Some(PT_B3), "B4" => Some(PT_B4),
        "B5" => Some(PT_B5),
        "C1" => Some(PT_C1), "C2" => Some(PT_C2), "C3" => Some(PT_C3),
        "D1" => Some(PT_D1), "D2" => Some(PT_D2), "D3" => Some(PT_D3),
        "D4" => Some(PT_D4), "D5" => Some(PT_D5), "D6" => Some(PT_D6),
        "E1" => Some(PT_E1), "E2" => Some(PT_E2), "F1" => Some(PT_F1), "F2" => Some(PT_F2),
        "W1" => Some(PT_W1), "W2" => Some(PT_W2), "P1" => Some(PT_P1), "G1" => Some(PT_G1),
        "K1" => Some(PT_K1), "K2" => Some(PT_K2), "K3" => Some(PT_K3),
        "K4" => Some(PT_K4), "K5" => Some(PT_K5),
        _ => None,
    }
}

// ============================================================================
// PIECE AND GAME STATE
// ============================================================================

#[derive(Clone, Copy, Debug)]
struct Piece {
    type_idx: u8,
    owner: u8,     // 0=White, 1=Black
    facing: u8,    // 0-5
}

impl Piece {
    fn piece_type(&self) -> &'static PieceType {
        &PIECE_TYPES[self.type_idx as usize]
    }

    fn is_king(&self) -> bool {
        self.piece_type().is_king
    }

    fn special(&self) -> Special {
        self.piece_type().special
    }
}

/// Position encoded as single byte: q+4 in upper nibble, r+4 in lower nibble
type Pos = u8;

#[inline]
fn encode_pos(q: i8, r: i8) -> Pos {
    ((q + 4) as u8) << 4 | ((r + 4) as u8)
}

#[inline]
fn decode_pos(p: Pos) -> (i8, i8) {
    ((p >> 4) as i8 - 4, (p & 0xF) as i8 - 4)
}

// Action templates
#[derive(Clone, Copy, Debug)]
#[repr(u8)]
enum ActionType {
    Move,
    Rotate,
    MoveOrRotate,  // Player chooses: move any piece OR rotate any piece
}

#[derive(Clone, Copy, Debug)]
#[repr(u8)]
enum Constraint {
    Any,
    Same,
    Different,
}

type Template = &'static [(ActionType, Constraint)];

const TEMPLATE_A: Template = &[(ActionType::Rotate, Constraint::Any), (ActionType::Move, Constraint::Same)];
const TEMPLATE_B: Template = &[(ActionType::Move, Constraint::Any), (ActionType::Rotate, Constraint::Any), (ActionType::Rotate, Constraint::Any)];
const TEMPLATE_C: Template = &[(ActionType::Move, Constraint::Any), (ActionType::Move, Constraint::Different), (ActionType::Rotate, Constraint::Any)];
const TEMPLATE_D: Template = &[(ActionType::Move, Constraint::Any), (ActionType::Rotate, Constraint::Different)];
// Simple 1-action templates for faster deep search
const TEMPLATE_E: Template = &[(ActionType::MoveOrRotate, Constraint::Any)];  // Move OR Rotate (chess-like)
const TEMPLATE_F: Template = &[(ActionType::Move, Constraint::Any), (ActionType::Rotate, Constraint::Same)];  // Move then rotate same

fn get_template(id: u8) -> Template {
    match id {
        0 => TEMPLATE_A, // A
        1 => TEMPLATE_B, // B
        2 => TEMPLATE_C, // C
        3 => TEMPLATE_D, // D
        4 => TEMPLATE_E, // E - single move
        _ => TEMPLATE_F, // F - move + rotate same
    }
}

fn template_char_to_id(c: char) -> u8 {
    match c {
        'A' => 0, 'B' => 1, 'C' => 2, 'D' => 3, 'E' => 4, _ => 5
    }
}

#[derive(Clone)]
struct GameState {
    // Board: piece at each position (61 hexes max, but use hashmap for sparse access)
    board: FxHashMap<Pos, Piece>,

    // King positions (encoded)
    white_king_pos: Option<Pos>,
    black_king_pos: Option<Pos>,

    // Current player (0=White, 1=Black)
    current_player: u8,

    // Templates for each player
    white_template: u8,
    black_template: u8,

    // Turn state
    action_index: u8,
    last_piece_pos: Option<Pos>,
    round_number: u16,

    // Winner (None = ongoing, 0 = White, 1 = Black)
    winner: Option<u8>,

    // Phoenix graveyard tracking (for Rebirth mechanic)
    white_phoenix_captured: bool,
    black_phoenix_captured: bool,
}

impl GameState {
    fn current_template(&self) -> Template {
        get_template(if self.current_player == 0 { self.white_template } else { self.black_template })
    }

    fn current_action(&self) -> Option<(ActionType, Constraint)> {
        let template = self.current_template();
        if (self.action_index as usize) < template.len() {
            Some(template[self.action_index as usize])
        } else {
            None
        }
    }

    fn is_turn_complete(&self) -> bool {
        (self.action_index as usize) >= self.current_template().len()
    }
}

// ============================================================================
// MOVE REPRESENTATION
// ============================================================================

#[derive(Clone, Copy, Debug)]
enum Move {
    Pass,
    Surrender,  // Explicitly give up - slightly better than being captured
    Movement { from: Pos, to: Pos, new_facing: u8 },
    Rotate { pos: Pos, new_facing: u8 },
    Swap { from: Pos, target: Pos },
    Rebirth { dest: Pos, new_facing: u8 },  // Phoenix returns from graveyard
}

// ============================================================================
// MOVE GENERATION
// ============================================================================

fn generate_destinations(state: &GameState, pos: Pos, piece: &Piece, moves: &mut Vec<Move>) {
    let pt = piece.piece_type();
    let (q, r) = decode_pos(pos);

    if pt.move_type == MoveType::None {
        return; // Warper has no normal movement
    }

    let is_ghost = piece.special() == Special::Phased;

    // JUMP pieces use ring/arc pattern, not directional movement
    if pt.move_type == MoveType::Jump {
        let is_forward_arc = pt.directions == FORWARD_ARC;

        // Iterate all hexes at exactly jump distance
        for (dest_q, dest_r) in iter_hex_ring(q, r, pt.move_range) {
            if !is_valid_hex(dest_q, dest_r) {
                continue;
            }

            // Filter by forward arc if applicable (150°, ±75° from forward)
            if is_forward_arc && !in_forward_arc(q, r, dest_q, dest_r, piece.facing) {
                continue;
            }

            let dest = encode_pos(dest_q, dest_r);

            // Check landing: empty or capturable enemy
            if let Some(occupant) = state.board.get(&dest) {
                if occupant.owner != piece.owner {
                    // Can capture unless ghost involved
                    if !is_ghost && occupant.special() != Special::Phased {
                        moves.push(Move::Movement { from: pos, to: dest, new_facing: piece.facing });
                    }
                }
                // Can't land on friendly or uncapturable
            } else {
                // Empty hex - can land
                moves.push(Move::Movement { from: pos, to: dest, new_facing: piece.facing });
            }
        }
        return;
    }

    // STEP and SLIDE use directional movement
    for rel_dir in 0..6u8 {
        if (pt.directions & (1 << rel_dir)) == 0 {
            continue;
        }

        let (dq, dr) = get_direction_vector(piece.facing, rel_dir);

        match pt.move_type {
            MoveType::Step => {
                let mut cq = q;
                let mut cr = r;
                for _ in 0..pt.move_range {
                    cq += dq;
                    cr += dr;
                    if !is_valid_hex(cq, cr) {
                        break;
                    }
                    let dest = encode_pos(cq, cr);
                    if let Some(occupant) = state.board.get(&dest) {
                        if occupant.owner != piece.owner {
                            // Can capture unless ghost involved
                            if !is_ghost && occupant.special() != Special::Phased {
                                moves.push(Move::Movement { from: pos, to: dest, new_facing: piece.facing });
                            }
                        }
                        break; // Blocked
                    }
                    moves.push(Move::Movement { from: pos, to: dest, new_facing: piece.facing });
                }
            }
            MoveType::Slide => {
                let mut cq = q;
                let mut cr = r;
                loop {
                    cq += dq;
                    cr += dr;
                    if !is_valid_hex(cq, cr) {
                        break;
                    }
                    let dest = encode_pos(cq, cr);
                    if let Some(occupant) = state.board.get(&dest) {
                        if occupant.owner != piece.owner {
                            if !is_ghost && occupant.special() != Special::Phased {
                                moves.push(Move::Movement { from: pos, to: dest, new_facing: piece.facing });
                            }
                        }
                        break;
                    }
                    moves.push(Move::Movement { from: pos, to: dest, new_facing: piece.facing });
                }
            }
            _ => {}  // Jump and None handled above
        }
    }
}

fn generate_legal_moves(state: &GameState) -> Vec<Move> {
    if state.winner.is_some() {
        return vec![];
    }

    let action = match state.current_action() {
        Some(a) => a,
        None => return vec![],
    };

    let (action_type, constraint) = action;
    let mut moves = vec![Move::Pass, Move::Surrender];

    // Find valid pieces to act
    for (&pos, piece) in &state.board {
        if piece.owner != state.current_player {
            continue;
        }

        match constraint {
            Constraint::Same => {
                if state.last_piece_pos != Some(pos) {
                    continue;
                }
            }
            Constraint::Different => {
                if state.last_piece_pos == Some(pos) {
                    continue;
                }
            }
            Constraint::Any => {}
        }

        match action_type {
            ActionType::Move => {
                generate_destinations(state, pos, piece, &mut moves);

                // Special: Warper swap
                if piece.special() == Special::SwapMove {
                    for (&target_pos, target) in &state.board {
                        if target_pos != pos && target.owner == piece.owner {
                            moves.push(Move::Swap { from: pos, target: target_pos });
                        }
                    }
                }
            }
            ActionType::Rotate => {
                // Skip rotation for omnidirectional pieces (rotating does nothing)
                if piece.piece_type().directions != ALL_DIRS {
                    for new_facing in 0..6u8 {
                        moves.push(Move::Rotate { pos, new_facing });
                    }
                }

                // Special: Shifter swap on rotate
                if piece.special() == Special::SwapRotate {
                    for (&target_pos, target) in &state.board {
                        if target_pos != pos && target.owner == piece.owner {
                            moves.push(Move::Swap { from: pos, target: target_pos });
                        }
                    }
                }
            }
            ActionType::MoveOrRotate => {
                // Generate both move and rotate options
                generate_destinations(state, pos, piece, &mut moves);

                // Special: Warper swap
                if piece.special() == Special::SwapMove {
                    for (&target_pos, target) in &state.board {
                        if target_pos != pos && target.owner == piece.owner {
                            moves.push(Move::Swap { from: pos, target: target_pos });
                        }
                    }
                }

                // Also generate rotations (skip for omnidirectional pieces)
                if piece.piece_type().directions != ALL_DIRS {
                    for new_facing in 0..6u8 {
                        moves.push(Move::Rotate { pos, new_facing });
                    }
                }

                // Special: Shifter swap on rotate
                if piece.special() == Special::SwapRotate {
                    for (&target_pos, target) in &state.board {
                        if target_pos != pos && target.owner == piece.owner {
                            moves.push(Move::Swap { from: pos, target: target_pos });
                        }
                    }
                }
            }
        }
    }

    // Phoenix Rebirth: if Phoenix is in graveyard and action allows moves,
    // player can spend their move to bring Phoenix back adjacent to king
    if matches!(action_type, ActionType::Move | ActionType::MoveOrRotate) {
        let phoenix_in_graveyard = if state.current_player == 0 {
            state.white_phoenix_captured
        } else {
            state.black_phoenix_captured
        };

        if phoenix_in_graveyard {
            let king_pos = if state.current_player == 0 {
                state.white_king_pos
            } else {
                state.black_king_pos
            };

            if let Some(king_encoded) = king_pos {
                let (kq, kr) = decode_pos(king_encoded);

                for &(dq, dr) in &DIRECTIONS {
                    let nq = kq + dq;
                    let nr = kr + dr;
                    if is_valid_hex(nq, nr) {
                        let dest = encode_pos(nq, nr);
                        // Can only place on empty hex
                        if !state.board.contains_key(&dest) {
                            // Generate rebirth for all 6 facings
                            for new_facing in 0..6u8 {
                                moves.push(Move::Rebirth { dest, new_facing });
                            }
                        }
                    }
                }
            }
        }
    }

    moves
}

// ============================================================================
// APPLY MOVE
// ============================================================================

fn apply_move(state: &GameState, mv: Move) -> GameState {
    let mut new_state = state.clone();

    match mv {
        Move::Pass => {}

        Move::Surrender => {
            // Player gives up - opponent wins
            new_state.winner = Some(1 - new_state.current_player);
        }

        Move::Movement { from, to, new_facing } => {
            let mut piece = new_state.board.remove(&from).unwrap();

            // Handle capture
            if let Some(captured) = new_state.board.remove(&to) {
                if captured.is_king() {
                    new_state.winner = Some(new_state.current_player);
                }
                // Track Phoenix capture for Rebirth mechanic
                if captured.type_idx == PT_P1 {
                    if captured.owner == 0 {
                        new_state.white_phoenix_captured = true;
                    } else {
                        new_state.black_phoenix_captured = true;
                    }
                }
            }

            piece.facing = new_facing;
            if piece.is_king() {
                if new_state.current_player == 0 {
                    new_state.white_king_pos = Some(to);
                } else {
                    new_state.black_king_pos = Some(to);
                }
            }
            new_state.board.insert(to, piece);
            new_state.last_piece_pos = Some(to);
        }

        Move::Rotate { pos, new_facing } => {
            if let Some(piece) = new_state.board.get_mut(&pos) {
                piece.facing = new_facing;
            }
            new_state.last_piece_pos = Some(pos);
        }

        Move::Swap { from, target } => {
            let piece1 = new_state.board.remove(&from).unwrap();
            let piece2 = new_state.board.remove(&target).unwrap();
            new_state.board.insert(from, piece2);
            new_state.board.insert(target, piece1);

            // Update king positions if needed
            if new_state.white_king_pos == Some(from) {
                new_state.white_king_pos = Some(target);
            } else if new_state.white_king_pos == Some(target) {
                new_state.white_king_pos = Some(from);
            }
            if new_state.black_king_pos == Some(from) {
                new_state.black_king_pos = Some(target);
            } else if new_state.black_king_pos == Some(target) {
                new_state.black_king_pos = Some(from);
            }

            new_state.last_piece_pos = Some(from);
        }

        Move::Rebirth { dest, new_facing } => {
            // Phoenix returns from graveyard to board adjacent to king
            let phoenix = Piece {
                type_idx: PT_P1,
                owner: new_state.current_player,
                facing: new_facing,
            };
            new_state.board.insert(dest, phoenix);

            // Remove Phoenix from graveyard
            if new_state.current_player == 0 {
                new_state.white_phoenix_captured = false;
            } else {
                new_state.black_phoenix_captured = false;
            }

            new_state.last_piece_pos = Some(dest);
        }
    }

    // Advance action
    new_state.action_index += 1;

    // End turn if complete
    if new_state.is_turn_complete() {
        new_state.current_player = 1 - new_state.current_player;
        new_state.action_index = 0;
        new_state.last_piece_pos = None;

        if new_state.current_player == 0 {
            new_state.round_number += 1;
        }

        // Check round limit
        if new_state.round_number > 50 && new_state.winner.is_none() {
            new_state = resolve_by_proximity(new_state);
        }
    }

    new_state
}

fn resolve_by_proximity(mut state: GameState) -> GameState {
    let white_king = state.white_king_pos;
    let black_king = state.black_king_pos;

    match (white_king, black_king) {
        (None, _) => { state.winner = Some(1); }
        (_, None) => { state.winner = Some(0); }
        (Some(wk), Some(bk)) => {
            let (wq, wr) = decode_pos(wk);
            let (bq, br) = decode_pos(bk);
            let white_dist = distance_to_center(wq, wr);
            let black_dist = distance_to_center(bq, br);

            if white_dist < black_dist {
                state.winner = Some(0);
            } else if black_dist < white_dist {
                state.winner = Some(1);
            } else {
                // Count pieces
                let white_count = state.board.values().filter(|p| p.owner == 0).count();
                let black_count = state.board.values().filter(|p| p.owner == 1).count();
                if white_count > black_count {
                    state.winner = Some(0);
                } else if black_count > white_count {
                    state.winner = Some(1);
                } else {
                    state.winner = Some(0); // White wins ties
                }
            }
        }
    }
    state
}

// ============================================================================
// EVALUATION
// ============================================================================

struct Heuristics {
    white_values: [f32; 32],
    black_values: [f32; 32],
    white_center_weight: f32,
    black_center_weight: f32,
}

impl Heuristics {
    fn get_piece_value(&self, type_idx: u8, owner: u8) -> f32 {
        if PIECE_TYPES[type_idx as usize].is_king {
            return WIN_VALUE;
        }
        if owner == 0 {
            self.white_values[type_idx as usize]
        } else {
            self.black_values[type_idx as usize]
        }
    }

    fn get_center_weight(&self, owner: u8) -> f32 {
        if owner == 0 { self.white_center_weight } else { self.black_center_weight }
    }
}

// Maximum king-of-the-hill bonus at round 50 (equivalent to ~10 pieces advantage)
const KOTH_MAX_URGENCY: f32 = 50.0;
const KOTH_ROUND_LIMIT: f32 = 50.0;

fn evaluate(state: &GameState, heuristics: &Heuristics, rng: &mut ChaCha8Rng, noise_scale: f32) -> f32 {
    if let Some(winner) = state.winner {
        return if winner == state.current_player { WIN_VALUE } else { -WIN_VALUE };
    }

    let mut score = 0.0f32;
    let current = state.current_player;

    for (&pos, piece) in &state.board {
        let pv = heuristics.get_piece_value(piece.type_idx, piece.owner);
        let cw = heuristics.get_center_weight(piece.owner);
        let (q, r) = decode_pos(pos);
        let center_bonus = cw * (4.0 - distance_to_center(q, r) as f32);
        let value = pv + center_bonus;

        if piece.owner == current {
            score += value;
        } else {
            score -= value;
        }
    }

    // King-of-the-hill urgency: accelerates as round 50 approaches
    // Uses cubic curve for aggressive late-game urgency
    let round_progress = (state.round_number as f32 / KOTH_ROUND_LIMIT).min(1.0);
    let urgency = round_progress * round_progress * round_progress * KOTH_MAX_URGENCY;

    if urgency > 0.1 {
        // Get king positions
        let my_king_pos = if current == 0 { state.white_king_pos } else { state.black_king_pos };
        let opp_king_pos = if current == 0 { state.black_king_pos } else { state.white_king_pos };

        match (my_king_pos, opp_king_pos) {
            (Some(my_pos), Some(opp_pos)) => {
                let (mq, mr) = decode_pos(my_pos);
                let (oq, or) = decode_pos(opp_pos);
                let my_dist = distance_to_center(mq, mr) as f32;
                let opp_dist = distance_to_center(oq, or) as f32;
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

    // Add small noise for variety (Gaussian-ish via uniform)
    let noise = (rng.gen::<f32>() - 0.5) * noise_scale;
    score + noise
}

// Simple move ordering score
fn move_score(state: &GameState, mv: &Move, heuristics: &Heuristics) -> f32 {
    match mv {
        Move::Pass => -1000.0,
        Move::Surrender => -50000.0,  // Very low but considered before giving up
        Move::Swap { .. } => 50.0,
        Move::Rotate { .. } => 0.0,
        Move::Rebirth { .. } => 40.0,
        Move::Movement { from, to, .. } => {
            let mut score = 0.0;

            // Capture bonus (MVV)
            if let Some(victim) = state.board.get(to) {
                if victim.owner != state.current_player {
                    score += heuristics.get_piece_value(victim.type_idx, victim.owner) * 10.0;
                }
            }

            // Center proximity
            let (fq, fr) = decode_pos(*from);
            let (tq, tr) = decode_pos(*to);
            score += (distance_to_center(fq, fr) - distance_to_center(tq, tr)) as f32 * 0.5;

            score
        }
    }
}

// ============================================================================
// NEGAMAX WITH ALPHA-BETA, NULL-MOVE PRUNING, AND LMR
// ============================================================================

// Null-move pruning reduction factor (R=2 is standard)
const NULL_MOVE_R: i32 = 2;

// Late Move Reduction thresholds
const LMR_MOVE_THRESHOLD: usize = 3;  // Apply LMR after first 3 moves
const LMR_DEPTH_THRESHOLD: i32 = 2;   // Only apply LMR at depth >= 2

/// Check if a move is a capture (piece moves to occupied enemy square)
fn is_capture_move(state: &GameState, mv: Move) -> bool {
    match mv {
        Move::Movement { to, .. } => {
            if let Some(piece) = state.board.get(&to) {
                piece.owner != state.current_player
            } else {
                false
            }
        }
        _ => false,
    }
}

/// Create a null-move state (skip our turn, give opponent the move)
/// Only valid at the start of a turn (action_index == 0)
fn make_null_move(state: &GameState) -> GameState {
    let mut new_state = state.clone();
    new_state.current_player = 1 - new_state.current_player;
    new_state.action_index = 0;
    new_state.last_piece_pos = None;
    if new_state.current_player == 0 {
        new_state.round_number += 1;
    }
    new_state
}

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
    // Terminal check - add depth bonus so winning sooner / losing later is preferred
    // Higher depth = closer to current position (fewer moves searched)
    // For wins: closer is better (+depth)
    // For losses: farther is better (-depth)
    if state.winner.is_some() {
        return if state.winner == Some(state.current_player) {
            WIN_VALUE + depth as f32  // Win sooner is better
        } else {
            -WIN_VALUE - depth as f32  // Lose later is better (so losing NOW is worst)
        };
    }

    // Depth limit (only at turn boundaries handled by depth decrement)
    if depth <= 0 {
        return evaluate(state, heuristics, rng, noise_scale);
    }

    let mut moves = generate_legal_moves(state);
    if moves.is_empty() {
        return evaluate(state, heuristics, rng, noise_scale);
    }

    // =========================================================================
    // NULL-MOVE PRUNING
    // =========================================================================
    // If we're at the start of our turn (action_index == 0), try "passing"
    // If even after giving opponent a free move we're still >= beta, prune
    if allow_null_move
        && depth >= NULL_MOVE_R + 1
        && state.action_index == 0
        && !is_in_danger(state)  // Don't null-move prune when our king is threatened
    {
        let null_state = make_null_move(state);
        let null_score = -negamax(
            &null_state,
            depth - 1 - NULL_MOVE_R,  // Reduced depth search
            -beta,
            -beta + 0.01,  // Null window
            heuristics,
            max_moves,
            rng,
            noise_scale,
            false,  // Don't allow consecutive null moves
        );

        if null_score >= beta {
            return beta;  // Cutoff - position is so good we can skip and still win
        }
    }

    // Sort moves by score (descending) - good move ordering is crucial for LMR
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
    let original_player = state.current_player;

    for (move_index, mv) in moves.iter().enumerate() {
        // Special handling for surrender - give it a fixed score
        let score = if matches!(mv, Move::Surrender) {
            -WIN_VALUE - depth as f32 + 0.5
        } else {
            let child = apply_move(state, *mv);
            let turn_changed = child.current_player != original_player;

            // =====================================================================
            // LATE MOVE REDUCTIONS (LMR)
            // =====================================================================
            // For moves beyond the first few (which are likely best due to ordering),
            // search at reduced depth. If they look promising, re-search at full depth.
            let is_capture = is_capture_move(state, *mv);
            let use_lmr = turn_changed
                && move_index >= LMR_MOVE_THRESHOLD
                && depth >= LMR_DEPTH_THRESHOLD
                && !is_capture;  // Don't reduce captures

            if turn_changed {
                let search_depth = if use_lmr { depth - 2 } else { depth - 1 };
                let mut s = -negamax(
                    &child, search_depth, -beta, -alpha,
                    heuristics, max_moves, rng, noise_scale, true
                );

                // Re-search at full depth if LMR found a promising move
                if use_lmr && s > alpha {
                    s = -negamax(
                        &child, depth - 1, -beta, -alpha,
                        heuristics, max_moves, rng, noise_scale, true
                    );
                }
                s
            } else {
                // Within same turn, don't reduce
                negamax(&child, depth, alpha, beta, heuristics, max_moves, rng, noise_scale, allow_null_move)
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

/// Check if current player's king is in immediate danger
/// (Used to avoid null-move pruning in dangerous positions)
/// Calculate hex distance between two positions
#[inline]
fn hex_distance(q1: i8, r1: i8, q2: i8, r2: i8) -> i8 {
    let dq = (q1 - q2).abs();
    let dr = (r1 - r2).abs();
    let ds = ((q1 + r1) - (q2 + r2)).abs();
    (dq + dr + ds) / 2
}

fn is_in_danger(state: &GameState) -> bool {
    let king_pos = if state.current_player == 0 {
        state.white_king_pos
    } else {
        state.black_king_pos
    };

    // If king is gone, definitely in danger
    let king_pos = match king_pos {
        Some(p) => p,
        None => return true,
    };

    // Check if any enemy piece can capture the king
    // This is a simplified check - just see if enemy has pieces nearby
    let (kq, kr) = decode_pos(king_pos);

    for (pos, piece) in state.board.iter() {
        if piece.owner != state.current_player {
            let (pq, pr) = decode_pos(*pos);
            // If enemy piece is within 3 hexes of king, consider it dangerous
            let dist = hex_distance(pq, pr, kq, kr);
            if dist <= 3 {
                return true;
            }
        }
    }

    false
}

// Debug flag - set to true to enable debug output
const DEBUG_SEARCH: bool = false;

fn get_best_move(
    state: &GameState,
    depth: i32,
    heuristics: &Heuristics,
    max_moves: usize,
    rng: &mut ChaCha8Rng,
    noise_scale: f32,
) -> Option<Move> {
    let mut moves = generate_legal_moves(state);
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
    let original_player = state.current_player;

    if DEBUG_SEARCH && depth >= 10 {
        eprintln!("DEBUG: get_best_move at depth {} for player {}, {} moves", depth, state.current_player, moves.len());
    }

    for mv in moves {
        // Special handling for surrender: give it a fixed score slightly better than death
        // so the AI explicitly surrenders instead of making a random losing move
        let score = if matches!(mv, Move::Surrender) {
            // Surrender scores slightly better than immediate loss (-WIN_VALUE - depth)
            // This ensures AI surrenders cleanly when all moves lead to immediate death
            -WIN_VALUE - depth as f32 + 0.5
        } else {
            let child = apply_move(state, mv);
            let turn_changed = child.current_player != original_player;

            if turn_changed {
                -negamax(&child, depth - 1, f32::NEG_INFINITY, f32::INFINITY, heuristics, max_moves, rng, noise_scale, true)
            } else {
                negamax(&child, depth, f32::NEG_INFINITY, f32::INFINITY, heuristics, max_moves, rng, noise_scale, true)
            }
        };

        if DEBUG_SEARCH && depth >= 10 {
            // Print ALL move scores at D10
            match mv {
                Move::Movement { from, to, .. } => {
                    let (fq, fr) = decode_pos(from);
                    let (tq, tr) = decode_pos(to);
                    let piece_type = state.board.get(&from).map(|p| p.type_idx).unwrap_or(255);
                    let is_king = state.board.get(&from).map(|p| p.is_king()).unwrap_or(false);
                    let king_mark = if is_king { " [KING]" } else { "" };
                    eprintln!("DEBUG:   MOVE ({},{}) -> ({},{}) type={} score={:.1}{}", fq, fr, tq, tr, piece_type, score, king_mark);
                }
                Move::Rotate { pos, new_facing } => {
                    let (pq, pr) = decode_pos(pos);
                    eprintln!("DEBUG:   ROTATE ({},{}) to facing {} score={:.1}", pq, pr, new_facing, score);
                }
                _ => {
                    eprintln!("DEBUG:   OTHER move score={:.1}", score);
                }
            }
        }

        if score > best_score {
            best_score = score;
            best_move = mv;
        }
    }

    if DEBUG_SEARCH && depth >= 10 {
        eprintln!("DEBUG: Best move score={:.1}", best_score);
    }

    Some(best_move)
}

// ============================================================================
// PLAY AI GAME
// ============================================================================

const NOISE_SCALE: f32 = 0.1;

fn play_ai_game(
    initial_state: GameState,
    white_depth: i32,
    black_depth: i32,
    heuristics: &Heuristics,
    max_total_moves: usize,
    max_moves_per_action: usize,
    rng: &mut ChaCha8Rng,
) -> (GameState, Option<u8>) {
    let mut state = initial_state;
    let mut moves_made = 0;

    while state.winner.is_none() && moves_made < max_total_moves {
        let depth = if state.current_player == 0 { white_depth } else { black_depth };

        if let Some(mv) = get_best_move(&state, depth, heuristics, max_moves_per_action, rng, NOISE_SCALE) {
            state = apply_move(&state, mv);
            moves_made += 1;
        } else {
            break;
        }
    }

    // Resolve if no winner
    if state.winner.is_none() {
        state = resolve_by_proximity(state);
    }

    let winner = state.winner;
    (state, winner)
}

/// Convert a Move to a Python-friendly tuple representation
fn move_to_tuple(mv: Move) -> (String, Option<(i32, i32)>, Option<(i32, i32)>, Option<i32>) {
    match mv {
        Move::Pass => ("PASS".to_string(), None, None, None),
        Move::Surrender => ("SURRENDER".to_string(), None, None, None),
        Move::Movement { from, to, new_facing } => {
            let (fq, fr) = decode_pos(from);
            let (tq, tr) = decode_pos(to);
            ("MOVE".to_string(), Some((fq as i32, fr as i32)), Some((tq as i32, tr as i32)), Some(new_facing as i32))
        }
        Move::Rotate { pos, new_facing } => {
            let (q, r) = decode_pos(pos);
            ("ROTATE".to_string(), Some((q as i32, r as i32)), None, Some(new_facing as i32))
        }
        Move::Swap { from, target } => {
            let (fq, fr) = decode_pos(from);
            let (tq, tr) = decode_pos(target);
            ("SPECIAL".to_string(), Some((fq as i32, fr as i32)), Some((tq as i32, tr as i32)), None)
        }
        Move::Rebirth { dest, new_facing } => {
            let (q, r) = decode_pos(dest);
            ("SPECIAL".to_string(), None, Some((q as i32, r as i32)), Some(new_facing as i32))
        }
    }
}

/// Play a game and record all moves
fn play_ai_game_with_record(
    initial_state: GameState,
    white_depth: i32,
    black_depth: i32,
    heuristics: &Heuristics,
    max_total_moves: usize,
    max_moves_per_action: usize,
    rng: &mut ChaCha8Rng,
) -> (GameState, Option<u8>, Vec<Move>) {
    let mut state = initial_state;
    let mut moves_made = 0;
    let mut move_history = Vec::new();

    while state.winner.is_none() && moves_made < max_total_moves {
        let depth = if state.current_player == 0 { white_depth } else { black_depth };

        if let Some(mv) = get_best_move(&state, depth, heuristics, max_moves_per_action, rng, NOISE_SCALE) {
            move_history.push(mv);
            state = apply_move(&state, mv);
            moves_made += 1;
        } else {
            break;
        }
    }

    // Resolve if no winner
    if state.winner.is_none() {
        state = resolve_by_proximity(state);
    }

    let winner = state.winner;
    (state, winner, move_history)
}

// ============================================================================
// PYTHON BINDINGS
// ============================================================================

/// Play a game and return the winner
#[pyfunction]
#[pyo3(signature = (white_pieces, black_pieces, white_template, black_template, white_depth, black_depth, heuristics_dict, max_moves=500, max_moves_per_action=15, seed=42))]
fn play_game(
    white_pieces: Vec<(String, (i32, i32), i32)>,
    black_pieces: Vec<(String, (i32, i32), i32)>,
    white_template: char,
    black_template: char,
    white_depth: i32,
    black_depth: i32,
    heuristics_dict: &Bound<'_, PyDict>,
    max_moves: usize,
    max_moves_per_action: usize,
    seed: u64,
) -> PyResult<(i32, i32)> {
    // Build initial state
    let mut board = FxHashMap::default();
    let mut white_king_pos = None;
    let mut black_king_pos = None;

    for (type_id, (q, r), facing) in &white_pieces {
        let type_idx = piece_id_to_index(type_id).ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("Unknown piece type: {}", type_id))
        })?;
        let pos = encode_pos(*q as i8, *r as i8);
        let piece = Piece { type_idx, owner: 0, facing: *facing as u8 };
        if piece.is_king() {
            white_king_pos = Some(pos);
        }
        board.insert(pos, piece);
    }

    for (type_id, (q, r), facing) in &black_pieces {
        let type_idx = piece_id_to_index(type_id).ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("Unknown piece type: {}", type_id))
        })?;
        let pos = encode_pos(*q as i8, *r as i8);
        let piece = Piece { type_idx, owner: 1, facing: *facing as u8 };
        if piece.is_king() {
            black_king_pos = Some(pos);
        }
        board.insert(pos, piece);
    }

    // Parse heuristics
    let white_values_binding = heuristics_dict.get_item("white_piece_values")?.unwrap();
    let white_values_dict: &Bound<'_, PyDict> = white_values_binding.downcast()?;
    let black_values_binding = heuristics_dict.get_item("black_piece_values")?.unwrap();
    let black_values_dict: &Bound<'_, PyDict> = black_values_binding.downcast()?;
    let white_center: f32 = heuristics_dict.get_item("white_center_weight")?.unwrap().extract()?;
    let black_center: f32 = heuristics_dict.get_item("black_center_weight")?.unwrap().extract()?;

    let mut white_values = [1.0f32; 32];
    let mut black_values = [1.0f32; 32];

    for (key, value) in white_values_dict.iter() {
        let id: String = key.extract()?;
        let val: f32 = value.extract()?;
        if let Some(idx) = piece_id_to_index(&id) {
            white_values[idx as usize] = val;
        }
    }

    for (key, value) in black_values_dict.iter() {
        let id: String = key.extract()?;
        let val: f32 = value.extract()?;
        if let Some(idx) = piece_id_to_index(&id) {
            black_values[idx as usize] = val;
        }
    }

    let heuristics = Heuristics {
        white_values,
        black_values,
        white_center_weight: white_center,
        black_center_weight: black_center,
    };

    let state = GameState {
        board,
        white_king_pos,
        black_king_pos,
        current_player: 0,
        white_template: template_char_to_id(white_template),
        black_template: template_char_to_id(black_template),
        action_index: 0,
        last_piece_pos: None,
        round_number: 1,
        winner: None,
        white_phoenix_captured: false,
        black_phoenix_captured: false,
    };

    // Create RNG from seed
    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    let (final_state, winner) = play_ai_game(
        state,
        white_depth,
        black_depth,
        &heuristics,
        max_moves,
        max_moves_per_action,
        &mut rng,
    );

    let winner_int = winner.map(|w| w as i32).unwrap_or(-1);
    let rounds = final_state.round_number as i32;

    Ok((winner_int, rounds))
}

/// Play a game and return the winner along with move history
#[pyfunction]
#[pyo3(signature = (white_pieces, black_pieces, white_template, black_template, white_depth, black_depth, heuristics_dict, max_moves=500, max_moves_per_action=15, seed=42))]
fn play_game_with_record(
    white_pieces: Vec<(String, (i32, i32), i32)>,
    black_pieces: Vec<(String, (i32, i32), i32)>,
    white_template: char,
    black_template: char,
    white_depth: i32,
    black_depth: i32,
    heuristics_dict: &Bound<'_, PyDict>,
    max_moves: usize,
    max_moves_per_action: usize,
    seed: u64,
) -> PyResult<(i32, i32, Vec<(String, Option<(i32, i32)>, Option<(i32, i32)>, Option<i32>)>)> {
    // Build initial state (same as play_game)
    let mut board = FxHashMap::default();
    let mut white_king_pos = None;
    let mut black_king_pos = None;

    for (type_id, (q, r), facing) in &white_pieces {
        let type_idx = piece_id_to_index(type_id).ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("Unknown piece type: {}", type_id))
        })?;
        let pos = encode_pos(*q as i8, *r as i8);
        let piece = Piece { type_idx, owner: 0, facing: *facing as u8 };
        if piece.is_king() {
            white_king_pos = Some(pos);
        }
        board.insert(pos, piece);
    }

    for (type_id, (q, r), facing) in &black_pieces {
        let type_idx = piece_id_to_index(type_id).ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("Unknown piece type: {}", type_id))
        })?;
        let pos = encode_pos(*q as i8, *r as i8);
        let piece = Piece { type_idx, owner: 1, facing: *facing as u8 };
        if piece.is_king() {
            black_king_pos = Some(pos);
        }
        board.insert(pos, piece);
    }

    // Parse heuristics
    let white_values_binding = heuristics_dict.get_item("white_piece_values")?.unwrap();
    let white_values_dict: &Bound<'_, PyDict> = white_values_binding.downcast()?;
    let black_values_binding = heuristics_dict.get_item("black_piece_values")?.unwrap();
    let black_values_dict: &Bound<'_, PyDict> = black_values_binding.downcast()?;
    let white_center: f32 = heuristics_dict.get_item("white_center_weight")?.unwrap().extract()?;
    let black_center: f32 = heuristics_dict.get_item("black_center_weight")?.unwrap().extract()?;

    let mut white_values = [1.0f32; 32];
    let mut black_values = [1.0f32; 32];

    for (key, value) in white_values_dict.iter() {
        let id: String = key.extract()?;
        let val: f32 = value.extract()?;
        if let Some(idx) = piece_id_to_index(&id) {
            white_values[idx as usize] = val;
        }
    }

    for (key, value) in black_values_dict.iter() {
        let id: String = key.extract()?;
        let val: f32 = value.extract()?;
        if let Some(idx) = piece_id_to_index(&id) {
            black_values[idx as usize] = val;
        }
    }

    let heuristics = Heuristics {
        white_values,
        black_values,
        white_center_weight: white_center,
        black_center_weight: black_center,
    };

    let state = GameState {
        board,
        white_king_pos,
        black_king_pos,
        current_player: 0,
        white_template: template_char_to_id(white_template),
        black_template: template_char_to_id(black_template),
        action_index: 0,
        last_piece_pos: None,
        round_number: 1,
        winner: None,
        white_phoenix_captured: false,
        black_phoenix_captured: false,
    };

    // Create RNG from seed
    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    let (final_state, winner, moves) = play_ai_game_with_record(
        state,
        white_depth,
        black_depth,
        &heuristics,
        max_moves,
        max_moves_per_action,
        &mut rng,
    );

    let winner_int = winner.map(|w| w as i32).unwrap_or(-1);
    let rounds = final_state.round_number as i32;
    let move_tuples: Vec<_> = moves.into_iter().map(move_to_tuple).collect();

    Ok((winner_int, rounds, move_tuples))
}

/// Get AI move for interactive play
#[pyfunction]
#[pyo3(signature = (pieces, current_player, white_template, black_template, action_index, depth, heuristics_dict, max_moves_per_action=15, seed=42))]
fn get_ai_move(
    pieces: Vec<(String, (i32, i32), i32, i32)>,  // (piece_id, pos, facing, owner)
    current_player: i32,
    white_template: char,
    black_template: char,
    action_index: i32,
    depth: i32,
    heuristics_dict: &Bound<'_, PyDict>,
    max_moves_per_action: usize,
    seed: u64,
) -> PyResult<Option<(String, Option<(i32, i32)>, Option<(i32, i32)>, Option<i32>)>> {
    // Build board
    let mut board = FxHashMap::default();
    let mut white_king_pos = None;
    let mut black_king_pos = None;

    for (type_id, (q, r), facing, owner) in &pieces {
        let type_idx = piece_id_to_index(type_id).ok_or_else(|| {
            PyErr::new::<pyo3::exceptions::PyValueError, _>(format!("Unknown piece type: {}", type_id))
        })?;
        let pos = encode_pos(*q as i8, *r as i8);
        let piece = Piece { type_idx, owner: *owner as u8, facing: *facing as u8 };
        if piece.is_king() {
            if *owner == 0 {
                white_king_pos = Some(pos);
            } else {
                black_king_pos = Some(pos);
            }
        }
        board.insert(pos, piece);
    }

    // Parse heuristics
    let white_values_binding = heuristics_dict.get_item("white_piece_values")?.unwrap();
    let white_values_dict: &Bound<'_, PyDict> = white_values_binding.downcast()?;
    let black_values_binding = heuristics_dict.get_item("black_piece_values")?.unwrap();
    let black_values_dict: &Bound<'_, PyDict> = black_values_binding.downcast()?;
    let white_center: f32 = heuristics_dict.get_item("white_center_weight")?.unwrap().extract()?;
    let black_center: f32 = heuristics_dict.get_item("black_center_weight")?.unwrap().extract()?;

    let mut white_values = [1.0f32; 32];
    let mut black_values = [1.0f32; 32];

    for (key, value) in white_values_dict.iter() {
        let id: String = key.extract()?;
        let val: f32 = value.extract()?;
        if let Some(idx) = piece_id_to_index(&id) {
            white_values[idx as usize] = val;
        }
    }

    for (key, value) in black_values_dict.iter() {
        let id: String = key.extract()?;
        let val: f32 = value.extract()?;
        if let Some(idx) = piece_id_to_index(&id) {
            black_values[idx as usize] = val;
        }
    }

    let heuristics = Heuristics {
        white_values,
        black_values,
        white_center_weight: white_center,
        black_center_weight: black_center,
    };

    let state = GameState {
        board,
        white_king_pos,
        black_king_pos,
        current_player: current_player as u8,
        white_template: template_char_to_id(white_template),
        black_template: template_char_to_id(black_template),
        action_index: action_index as u8,
        last_piece_pos: None,
        round_number: 1,
        winner: None,
        white_phoenix_captured: false,
        black_phoenix_captured: false,
    };

    let mut rng = ChaCha8Rng::seed_from_u64(seed);
    let noise_scale = 0.01;

    let best_move = get_best_move(&state, depth, &heuristics, max_moves_per_action, &mut rng, noise_scale);

    match best_move {
        Some(mv) => Ok(Some(move_to_tuple(mv))),
        None => Ok(None),
    }
}

/// Python module
#[pymodule]
fn hexwar_core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(play_game, m)?)?;
    m.add_function(wrap_pyfunction!(play_game_with_record, m)?)?;
    m.add_function(wrap_pyfunction!(get_ai_move, m)?)?;
    Ok(())
}

use pyo3::types::PyDict;
