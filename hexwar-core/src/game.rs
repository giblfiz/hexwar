//! Game state and move generation

use crate::board::{Hex, DIRECTIONS, direction_vector};
use crate::pieces::{
    PieceTypeId, MoveType, Special, ALL_DIRS, FORWARD_ARC,
    get_piece_type,
};
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};

// ============================================================================
// CONSTANTS
// ============================================================================

/// Maximum rounds before proximity rule triggers
const MAX_ROUNDS: u16 = 50;

/// Phoenix piece type index
const PT_P1: u8 = 23;

/// Facing angles in degrees for arc calculations
/// 0=N(270), 1=NE(330), 2=SE(30), 3=S(90), 4=SW(150), 5=NW(210)
const FACING_ANGLES: [f32; 6] = [270.0, 330.0, 30.0, 90.0, 150.0, 210.0];

// ============================================================================
// CORE TYPES
// ============================================================================

/// Player color
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Player {
    White = 0,
    Black = 1,
}

impl Player {
    pub fn opponent(self) -> Self {
        match self {
            Player::White => Player::Black,
            Player::Black => Player::White,
        }
    }

}

/// Game result
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum GameResult {
    Ongoing,
    WhiteWins,
    BlackWins,
}

/// Action template
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Template {
    A,  // Rotate, Move (same)
    B,  // Move, Rotate, Rotate
    C,  // Move, Move (different), Rotate
    D,  // Move, Rotate (different)
    E,  // Move OR Rotate (chess-like)
    F,  // Move, Rotate (same)
}

/// Action type within a template
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ActionType {
    Move,
    Rotate,
    MoveOrRotate,
}

/// Constraint on which piece can act
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Constraint {
    Any,
    Same,
    Different,
}

type TemplateActions = &'static [(ActionType, Constraint)];

const TEMPLATE_A: TemplateActions = &[
    (ActionType::Rotate, Constraint::Any),
    (ActionType::Move, Constraint::Same),
];
const TEMPLATE_B: TemplateActions = &[
    (ActionType::Move, Constraint::Any),
    (ActionType::Rotate, Constraint::Any),
    (ActionType::Rotate, Constraint::Any),
];
const TEMPLATE_C: TemplateActions = &[
    (ActionType::Move, Constraint::Any),
    (ActionType::Move, Constraint::Different),
    (ActionType::Rotate, Constraint::Any),
];
const TEMPLATE_D: TemplateActions = &[
    (ActionType::Move, Constraint::Any),
    (ActionType::Rotate, Constraint::Different),
];
const TEMPLATE_E: TemplateActions = &[
    (ActionType::MoveOrRotate, Constraint::Any),
];
const TEMPLATE_F: TemplateActions = &[
    (ActionType::Move, Constraint::Any),
    (ActionType::Rotate, Constraint::Same),
];

fn get_template_actions(template: Template) -> TemplateActions {
    match template {
        Template::A => TEMPLATE_A,
        Template::B => TEMPLATE_B,
        Template::C => TEMPLATE_C,
        Template::D => TEMPLATE_D,
        Template::E => TEMPLATE_E,
        Template::F => TEMPLATE_F,
    }
}

/// A piece on the board
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct Piece {
    pub piece_type: PieceTypeId,
    pub owner: Player,
    pub facing: u8,
}

impl Piece {
    fn is_king(&self) -> bool {
        get_piece_type(self.piece_type).is_king
    }

    fn special(&self) -> Special {
        get_piece_type(self.piece_type).special
    }
}

/// A legal move
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Move {
    Pass,
    Surrender,
    Movement { from: Hex, to: Hex, new_facing: u8 },
    Rotate { pos: Hex, new_facing: u8 },
    Swap { from: Hex, target: Hex },
    Rebirth { dest: Hex, new_facing: u8 },
}

// ============================================================================
// GAME STATE
// ============================================================================

/// Game state (clone to mutate)
#[derive(Clone, Debug)]
pub struct GameState {
    /// Board: hex -> piece (sparse representation)
    board: FxHashMap<Hex, Piece>,

    /// King positions for quick access
    white_king_pos: Option<Hex>,
    black_king_pos: Option<Hex>,

    /// Current player
    current_player: Player,

    /// Templates for each player
    white_template: Template,
    black_template: Template,

    /// Turn state
    action_index: u8,
    last_piece_pos: Option<Hex>,

    /// Round number (increments after black's turn)
    pub round: u16,

    /// Game result
    pub result: GameResult,

    /// Phoenix graveyard tracking
    white_phoenix_captured: bool,
    black_phoenix_captured: bool,
}

impl GameState {
    // ========================================================================
    // CONSTRUCTORS
    // ========================================================================

    /// Create new game from piece placements
    pub fn new(
        white_pieces: &[(PieceTypeId, Hex, u8)],
        black_pieces: &[(PieceTypeId, Hex, u8)],
        white_template: Template,
        black_template: Template,
    ) -> Self {
        let mut board = FxHashMap::default();
        let mut white_king_pos = None;
        let mut black_king_pos = None;

        for &(piece_type, hex, facing) in white_pieces {
            let piece = Piece {
                piece_type,
                owner: Player::White,
                facing,
            };
            if piece.is_king() {
                white_king_pos = Some(hex);
            }
            board.insert(hex, piece);
        }

        for &(piece_type, hex, facing) in black_pieces {
            let piece = Piece {
                piece_type,
                owner: Player::Black,
                facing,
            };
            if piece.is_king() {
                black_king_pos = Some(hex);
            }
            board.insert(hex, piece);
        }

        Self {
            board,
            white_king_pos,
            black_king_pos,
            current_player: Player::White,
            white_template,
            black_template,
            action_index: 0,
            last_piece_pos: None,
            round: 1,
            result: GameResult::Ongoing,
            white_phoenix_captured: false,
            black_phoenix_captured: false,
        }
    }

    // ========================================================================
    // ACCESSORS
    // ========================================================================

    /// Current player
    pub fn current_player(&self) -> Player {
        self.current_player
    }

    /// Game result
    pub fn result(&self) -> GameResult {
        self.result
    }

    /// Get piece at hex
    pub fn get_piece(&self, hex: Hex) -> Option<&Piece> {
        self.board.get(&hex)
    }

    /// Iterate pieces on board
    pub fn pieces(&self) -> impl Iterator<Item = (Hex, Piece)> + '_ {
        self.board.iter().map(|(&hex, &piece)| (hex, piece))
    }

    /// Get white king position
    pub fn white_king_pos(&self) -> Option<Hex> {
        self.white_king_pos
    }

    /// Get black king position
    pub fn black_king_pos(&self) -> Option<Hex> {
        self.black_king_pos
    }

    // ========================================================================
    // TEMPLATE HELPERS
    // ========================================================================

    fn current_template(&self) -> Template {
        match self.current_player {
            Player::White => self.white_template,
            Player::Black => self.black_template,
        }
    }

    fn current_action(&self) -> Option<(ActionType, Constraint)> {
        let actions = get_template_actions(self.current_template());
        actions.get(self.action_index as usize).copied()
    }

    fn is_turn_complete(&self) -> bool {
        let actions = get_template_actions(self.current_template());
        self.action_index as usize >= actions.len()
    }

    // ========================================================================
    // MOVE GENERATION
    // ========================================================================

    /// Generate all legal moves for current action
    pub fn legal_moves(&self) -> Vec<Move> {
        if self.result != GameResult::Ongoing {
            return vec![];
        }

        let (action_type, constraint) = match self.current_action() {
            Some(a) => a,
            None => return vec![],
        };

        let mut moves = vec![Move::Pass, Move::Surrender];

        // Generate moves for each piece
        for (&pos, piece) in &self.board {
            if piece.owner != self.current_player {
                continue;
            }

            // Check constraint
            if !self.satisfies_constraint(pos, constraint) {
                continue;
            }

            match action_type {
                ActionType::Move => {
                    self.generate_movement_moves(pos, piece, &mut moves);
                }
                ActionType::Rotate => {
                    self.generate_rotation_moves(pos, piece, &mut moves);
                }
                ActionType::MoveOrRotate => {
                    self.generate_movement_moves(pos, piece, &mut moves);
                    self.generate_rotation_moves(pos, piece, &mut moves);
                }
            }
        }

        // Phoenix Rebirth
        if matches!(action_type, ActionType::Move | ActionType::MoveOrRotate) {
            self.generate_rebirth_moves(&mut moves);
        }

        moves
    }

    fn satisfies_constraint(&self, pos: Hex, constraint: Constraint) -> bool {
        match constraint {
            Constraint::Any => true,
            Constraint::Same => self.last_piece_pos == Some(pos),
            Constraint::Different => self.last_piece_pos != Some(pos),
        }
    }

    fn generate_movement_moves(&self, pos: Hex, piece: &Piece, moves: &mut Vec<Move>) {
        let pt = get_piece_type(piece.piece_type);

        // Generate normal destinations
        self.generate_destinations(pos, piece, moves);

        // Special: Warper swap on move
        if pt.special == Special::SwapMove {
            self.generate_swap_moves(pos, moves);
        }
    }

    fn generate_rotation_moves(&self, pos: Hex, piece: &Piece, moves: &mut Vec<Move>) {
        let pt = get_piece_type(piece.piece_type);

        // Skip rotation for omnidirectional pieces (rotating does nothing)
        if pt.directions != ALL_DIRS {
            for new_facing in 0..6u8 {
                moves.push(Move::Rotate { pos, new_facing });
            }
        }

        // Special: Shifter swap on rotate
        if pt.special == Special::SwapRotate {
            self.generate_swap_moves(pos, moves);
        }
    }

    fn generate_swap_moves(&self, from: Hex, moves: &mut Vec<Move>) {
        let from_piece = match self.board.get(&from) {
            Some(p) => p,
            None => return,
        };

        for (&target, target_piece) in &self.board {
            if target != from && target_piece.owner == from_piece.owner {
                moves.push(Move::Swap { from, target });
            }
        }
    }

    fn generate_destinations(&self, pos: Hex, piece: &Piece, moves: &mut Vec<Move>) {
        let pt = get_piece_type(piece.piece_type);

        if pt.move_type == MoveType::None {
            return; // Warper has no normal movement
        }

        let is_ghost = piece.special() == Special::Phased;

        match pt.move_type {
            MoveType::Jump => {
                self.generate_jump_moves(pos, piece, is_ghost, moves);
            }
            MoveType::Step => {
                self.generate_step_moves(pos, piece, is_ghost, moves);
            }
            MoveType::Slide => {
                self.generate_slide_moves(pos, piece, is_ghost, moves);
            }
            MoveType::None => {}
        }
    }

    fn generate_jump_moves(&self, pos: Hex, piece: &Piece, is_ghost: bool, moves: &mut Vec<Move>) {
        let pt = get_piece_type(piece.piece_type);
        let is_forward_arc = pt.directions == FORWARD_ARC;

        // Iterate all hexes at exactly jump distance
        for dest in iter_hex_ring(pos, pt.move_range) {
            if !dest.is_valid() {
                continue;
            }

            // Filter by forward arc if applicable
            if is_forward_arc && !in_forward_arc(pos, dest, piece.facing) {
                continue;
            }

            // Check landing
            if let Some(occupant) = self.board.get(&dest) {
                if occupant.owner != piece.owner {
                    // Can capture unless ghost involved
                    if !is_ghost && occupant.special() != Special::Phased {
                        moves.push(Move::Movement { from: pos, to: dest, new_facing: piece.facing });
                    }
                }
            } else {
                // Empty hex - can land
                moves.push(Move::Movement { from: pos, to: dest, new_facing: piece.facing });
            }
        }
    }

    fn generate_step_moves(&self, pos: Hex, piece: &Piece, is_ghost: bool, moves: &mut Vec<Move>) {
        let pt = get_piece_type(piece.piece_type);

        for rel_dir in 0..6u8 {
            if (pt.directions & (1 << rel_dir)) == 0 {
                continue;
            }

            let (dq, dr) = direction_vector(piece.facing, rel_dir);
            let mut current = pos;

            for _ in 0..pt.move_range {
                current = Hex::new(current.q + dq, current.r + dr);
                if !current.is_valid() {
                    break;
                }

                if let Some(occupant) = self.board.get(&current) {
                    if occupant.owner != piece.owner {
                        if !is_ghost && occupant.special() != Special::Phased {
                            moves.push(Move::Movement { from: pos, to: current, new_facing: piece.facing });
                        }
                    }
                    break; // Blocked
                }

                moves.push(Move::Movement { from: pos, to: current, new_facing: piece.facing });
            }
        }
    }

    fn generate_slide_moves(&self, pos: Hex, piece: &Piece, is_ghost: bool, moves: &mut Vec<Move>) {
        let pt = get_piece_type(piece.piece_type);

        for rel_dir in 0..6u8 {
            if (pt.directions & (1 << rel_dir)) == 0 {
                continue;
            }

            let (dq, dr) = direction_vector(piece.facing, rel_dir);
            let mut current = pos;

            loop {
                current = Hex::new(current.q + dq, current.r + dr);
                if !current.is_valid() {
                    break;
                }

                if let Some(occupant) = self.board.get(&current) {
                    if occupant.owner != piece.owner {
                        if !is_ghost && occupant.special() != Special::Phased {
                            moves.push(Move::Movement { from: pos, to: current, new_facing: piece.facing });
                        }
                    }
                    break; // Blocked
                }

                moves.push(Move::Movement { from: pos, to: current, new_facing: piece.facing });
            }
        }
    }

    fn generate_rebirth_moves(&self, moves: &mut Vec<Move>) {
        let phoenix_captured = match self.current_player {
            Player::White => self.white_phoenix_captured,
            Player::Black => self.black_phoenix_captured,
        };

        if !phoenix_captured {
            return;
        }

        let king_pos = match self.current_player {
            Player::White => self.white_king_pos,
            Player::Black => self.black_king_pos,
        };

        let king_pos = match king_pos {
            Some(p) => p,
            None => return,
        };

        // Generate rebirth moves to all empty hexes adjacent to king
        for &(dq, dr) in &DIRECTIONS {
            let dest = Hex::new(king_pos.q + dq, king_pos.r + dr);
            if dest.is_valid() && !self.board.contains_key(&dest) {
                for new_facing in 0..6u8 {
                    moves.push(Move::Rebirth { dest, new_facing });
                }
            }
        }
    }

    // ========================================================================
    // APPLY MOVE
    // ========================================================================

    /// Apply move, return new state
    pub fn apply_move(&self, mv: Move) -> Self {
        let mut new_state = self.clone();
        new_state.apply_move_internal(mv);
        new_state
    }

    fn apply_move_internal(&mut self, mv: Move) {
        match mv {
            Move::Pass => {}

            Move::Surrender => {
                self.result = match self.current_player {
                    Player::White => GameResult::BlackWins,
                    Player::Black => GameResult::WhiteWins,
                };
            }

            Move::Movement { from, to, new_facing } => {
                self.apply_movement(from, to, new_facing);
            }

            Move::Rotate { pos, new_facing } => {
                if let Some(piece) = self.board.get_mut(&pos) {
                    piece.facing = new_facing;
                }
                self.last_piece_pos = Some(pos);
            }

            Move::Swap { from, target } => {
                self.apply_swap(from, target);
            }

            Move::Rebirth { dest, new_facing } => {
                self.apply_rebirth(dest, new_facing);
            }
        }

        // Advance action
        self.action_index += 1;

        // End turn if complete
        if self.is_turn_complete() {
            self.end_turn();
        }
    }

    fn apply_movement(&mut self, from: Hex, to: Hex, new_facing: u8) {
        let mut piece = self.board.remove(&from).expect("No piece at from position");

        // Handle capture
        if let Some(captured) = self.board.remove(&to) {
            if captured.is_king() {
                self.result = match self.current_player {
                    Player::White => GameResult::WhiteWins,
                    Player::Black => GameResult::BlackWins,
                };
            }

            // Track Phoenix capture
            if captured.piece_type == PT_P1 {
                match captured.owner {
                    Player::White => self.white_phoenix_captured = true,
                    Player::Black => self.black_phoenix_captured = true,
                }
            }
        }

        piece.facing = new_facing;

        // Update king position
        if piece.is_king() {
            match self.current_player {
                Player::White => self.white_king_pos = Some(to),
                Player::Black => self.black_king_pos = Some(to),
            }
        }

        self.board.insert(to, piece);
        self.last_piece_pos = Some(to);
    }

    fn apply_swap(&mut self, from: Hex, target: Hex) {
        let piece1 = self.board.remove(&from).expect("No piece at from");
        let piece2 = self.board.remove(&target).expect("No piece at target");

        self.board.insert(from, piece2);
        self.board.insert(target, piece1);

        // Update king positions
        if self.white_king_pos == Some(from) {
            self.white_king_pos = Some(target);
        } else if self.white_king_pos == Some(target) {
            self.white_king_pos = Some(from);
        }

        if self.black_king_pos == Some(from) {
            self.black_king_pos = Some(target);
        } else if self.black_king_pos == Some(target) {
            self.black_king_pos = Some(from);
        }

        self.last_piece_pos = Some(from);
    }

    fn apply_rebirth(&mut self, dest: Hex, new_facing: u8) {
        let phoenix = Piece {
            piece_type: PT_P1,
            owner: self.current_player,
            facing: new_facing,
        };

        self.board.insert(dest, phoenix);

        match self.current_player {
            Player::White => self.white_phoenix_captured = false,
            Player::Black => self.black_phoenix_captured = false,
        }

        self.last_piece_pos = Some(dest);
    }

    fn end_turn(&mut self) {
        self.current_player = self.current_player.opponent();
        self.action_index = 0;
        self.last_piece_pos = None;

        // Increment round after black's turn
        if self.current_player == Player::White {
            self.round += 1;
        }

        // Check round limit
        if self.round > MAX_ROUNDS && self.result == GameResult::Ongoing {
            self.resolve_by_proximity();
        }
    }

    fn resolve_by_proximity(&mut self) {
        match (self.white_king_pos, self.black_king_pos) {
            (None, _) => self.result = GameResult::BlackWins,
            (_, None) => self.result = GameResult::WhiteWins,
            (Some(wk), Some(bk)) => {
                let white_dist = wk.distance_to_center();
                let black_dist = bk.distance_to_center();

                if white_dist < black_dist {
                    self.result = GameResult::WhiteWins;
                } else if black_dist < white_dist {
                    self.result = GameResult::BlackWins;
                } else {
                    // Count pieces as tiebreaker
                    let white_count = self.board.values().filter(|p| p.owner == Player::White).count();
                    let black_count = self.board.values().filter(|p| p.owner == Player::Black).count();

                    if white_count > black_count {
                        self.result = GameResult::WhiteWins;
                    } else if black_count > white_count {
                        self.result = GameResult::BlackWins;
                    } else {
                        // White wins ties
                        self.result = GameResult::WhiteWins;
                    }
                }
            }
        }
    }

    // ========================================================================
    // MOBILITY
    // ========================================================================

    /// Count legal moves for a player (mobility heuristic)
    /// This temporarily switches perspective to count moves
    pub fn mobility(&self, player: Player) -> usize {
        // Count movement destinations for all pieces of the given player
        let mut count = 0;

        for (&pos, piece) in &self.board {
            if piece.owner != player {
                continue;
            }

            count += self.count_piece_mobility(pos, piece);
        }

        count
    }

    fn count_piece_mobility(&self, pos: Hex, piece: &Piece) -> usize {
        let pt = get_piece_type(piece.piece_type);
        let is_ghost = piece.special() == Special::Phased;
        let mut count = 0;

        match pt.move_type {
            MoveType::Jump => {
                let is_forward_arc = pt.directions == FORWARD_ARC;
                for dest in iter_hex_ring(pos, pt.move_range) {
                    if !dest.is_valid() {
                        continue;
                    }
                    if is_forward_arc && !in_forward_arc(pos, dest, piece.facing) {
                        continue;
                    }
                    if let Some(occupant) = self.board.get(&dest) {
                        if occupant.owner != piece.owner
                            && !is_ghost
                            && occupant.special() != Special::Phased
                        {
                            count += 1;
                        }
                    } else {
                        count += 1;
                    }
                }
            }
            MoveType::Step => {
                for rel_dir in 0..6u8 {
                    if (pt.directions & (1 << rel_dir)) == 0 {
                        continue;
                    }
                    let (dq, dr) = direction_vector(piece.facing, rel_dir);
                    let mut current = pos;
                    for _ in 0..pt.move_range {
                        current = Hex::new(current.q + dq, current.r + dr);
                        if !current.is_valid() {
                            break;
                        }
                        if let Some(occupant) = self.board.get(&current) {
                            if occupant.owner != piece.owner
                                && !is_ghost
                                && occupant.special() != Special::Phased
                            {
                                count += 1;
                            }
                            break;
                        }
                        count += 1;
                    }
                }
            }
            MoveType::Slide => {
                for rel_dir in 0..6u8 {
                    if (pt.directions & (1 << rel_dir)) == 0 {
                        continue;
                    }
                    let (dq, dr) = direction_vector(piece.facing, rel_dir);
                    let mut current = pos;
                    loop {
                        current = Hex::new(current.q + dq, current.r + dr);
                        if !current.is_valid() {
                            break;
                        }
                        if let Some(occupant) = self.board.get(&current) {
                            if occupant.owner != piece.owner
                                && !is_ghost
                                && occupant.special() != Special::Phased
                            {
                                count += 1;
                            }
                            break;
                        }
                        count += 1;
                    }
                }
            }
            MoveType::None => {}
        }

        count
    }
}

// ============================================================================
// HEX GEOMETRY HELPERS
// ============================================================================

/// Generate all hex positions at exactly distance N from origin
fn iter_hex_ring(center: Hex, distance: u8) -> impl Iterator<Item = Hex> {
    let distance = distance as i8;
    (0..6).flat_map(move |side| {
        let dir = DIRECTIONS[side];
        (0..distance).map(move |step| {
            // Start position for this side (corner of hexagon)
            let start_q = center.q + distance * DIRECTIONS[(side + 4) % 6].0;
            let start_r = center.r + distance * DIRECTIONS[(side + 4) % 6].1;
            // Walk along the edge
            Hex::new(start_q + step * dir.0, start_r + step * dir.1)
        })
    })
}

/// Check if destination is within forward arc (150, +-75 from forward)
fn in_forward_arc(from: Hex, to: Hex, facing: u8) -> bool {
    let dq = to.q - from.q;
    let dr = to.r - from.r;

    // Convert axial to cartesian for angle calculation
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

    // Forward arc is +-75 from forward (150 total)
    diff <= 75.0
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
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
    fn test_game_creation() {
        let game = simple_game();
        assert_eq!(game.current_player(), Player::White);
        assert_eq!(game.result(), GameResult::Ongoing);
        assert_eq!(game.round, 1);
        assert!(game.white_king_pos.is_some());
        assert!(game.black_king_pos.is_some());
    }

    #[test]
    fn test_legal_moves() {
        let game = simple_game();
        let moves = game.legal_moves();
        // Should have Pass, Surrender, and movement/rotation moves
        assert!(moves.len() > 2);
        assert!(moves.contains(&Move::Pass));
        assert!(moves.contains(&Move::Surrender));
    }

    #[test]
    fn test_apply_pass() {
        let game = simple_game();
        let new_game = game.apply_move(Move::Pass);
        // With Template::E (single action), turn should change
        assert_eq!(new_game.current_player(), Player::Black);
    }

    #[test]
    fn test_mobility() {
        let game = simple_game();
        let white_mob = game.mobility(Player::White);
        let black_mob = game.mobility(Player::Black);
        // Both sides have same pieces, mobility should be similar
        assert!(white_mob > 0);
        assert!(black_mob > 0);
    }

    #[test]
    fn test_surrender() {
        let game = simple_game();
        let new_game = game.apply_move(Move::Surrender);
        assert_eq!(new_game.result(), GameResult::BlackWins);
    }

    #[test]
    fn test_hex_ring() {
        let center = Hex::new(0, 0);
        let ring: Vec<_> = iter_hex_ring(center, 1).collect();
        assert_eq!(ring.len(), 6); // 6 neighbors
    }

    #[test]
    fn test_king_capture() {
        // Set up a position where white can capture black king
        let white = vec![
            (piece_id_to_index("K1").unwrap(), Hex::new(0, 0), 0),
            (piece_id_to_index("D5").unwrap(), Hex::new(0, 1), 0), // Queen adjacent to black king
        ];
        let black = vec![
            (piece_id_to_index("K1").unwrap(), Hex::new(0, -1), 3), // King can be captured
        ];
        let game = GameState::new(&white, &black, Template::E, Template::E);

        // Queen captures king
        let mv = Move::Movement {
            from: Hex::new(0, 1),
            to: Hex::new(0, -1),
            new_facing: 0,
        };
        let new_game = game.apply_move(mv);
        assert_eq!(new_game.result(), GameResult::WhiteWins);
    }
}
