"""
HEXWAR Game State and Move Generation

Core game logic including:
- GameState representation
- Move generation for all piece types
- Action templates and turn structure
- Victory condition checking
"""

from __future__ import annotations
from dataclasses import dataclass, field
from typing import Iterator, NamedTuple, Literal
from copy import deepcopy

from hexwar.board import (
    BOARD_RADIUS, ALL_HEXES, NUM_HEXES,
    WHITE_HOME_ZONE, BLACK_HOME_ZONE,
    is_valid_hex, hex_distance, distance_to_center,
    get_direction_vector, get_neighbor, get_neighbors, get_valid_neighbors,
    DIRECTIONS, default_facing, hex_to_sector, iter_hex_ring,
)
from hexwar.pieces import (
    PIECE_TYPES, Piece, PieceType, get_piece_type,
    is_king, has_special, get_special, INF,
)


# ============================================================================
# ACTION TEMPLATES
# ============================================================================

# Template definitions: list of action types in order
# Each entry is ('MOVE' or 'ROTATE', constraint)
# Constraint: 'ANY' = any piece, 'SAME' = same as previous, 'DIFFERENT' = different from previous
ActionTemplate = tuple[tuple[str, str], ...]

TEMPLATE_A: ActionTemplate = (('ROTATE', 'ANY'), ('MOVE', 'SAME'))  # Rotate-Move Same
TEMPLATE_B: ActionTemplate = (('MOVE', 'ANY'), ('ROTATE', 'ANY'), ('ROTATE', 'ANY'))  # Move-Rotate-Rotate
TEMPLATE_C: ActionTemplate = (('MOVE', 'ANY'), ('MOVE', 'DIFFERENT'), ('ROTATE', 'ANY'))  # Move-Move-Rotate
TEMPLATE_D: ActionTemplate = (('MOVE', 'ANY'), ('ROTATE', 'DIFFERENT'))  # Move-Rotate Different
# Simple 1-2 action templates for faster deep search
TEMPLATE_E: ActionTemplate = (('MOVE_OR_ROTATE', 'ANY'),)  # Move OR Rotate (chess-like, 1 action)
TEMPLATE_F: ActionTemplate = (('MOVE', 'ANY'), ('ROTATE', 'SAME'))  # Move-Rotate Same (2 actions)

TEMPLATES = {
    'A': TEMPLATE_A,
    'B': TEMPLATE_B,
    'C': TEMPLATE_C,
    'D': TEMPLATE_D,
    'E': TEMPLATE_E,
    'F': TEMPLATE_F,
}


# ============================================================================
# MOVE REPRESENTATION
# ============================================================================

class Move(NamedTuple):
    """Represents a single action (move or rotate)."""
    action_type: Literal['MOVE', 'ROTATE', 'SPECIAL', 'PASS', 'SURRENDER']
    from_pos: tuple[int, int] | None  # Source position (None for PASS/SURRENDER)
    to_pos: tuple[int, int] | None  # Destination (None for ROTATE/PASS/SURRENDER/some specials)
    new_facing: int | None  # New facing after move (for ROTATE/MOVE)
    special_data: dict | None = None  # Extra data for specials (swap target, etc.)


# ============================================================================
# GAME STATE
# ============================================================================

@dataclass
class GameState:
    """Complete state of a HEXWAR game."""

    # Board: dict from (q, r) to Piece
    board: dict[tuple[int, int], Piece]

    # Graveyards: captured pieces by type_id (index 0=White's graveyard, 1=Black's)
    graveyards: tuple[list[str], list[str]]

    # Whose turn is it (0=White, 1=Black)
    current_player: int

    # Turn counters
    turn_number: int  # Increments each turn (player action sequence)
    round_number: int  # Increments after Black's turn

    # Action template for each player
    templates: tuple[str, str]  # ('A', 'D') etc.

    # Current action sequence state
    action_index: int  # Which action in template we're on
    last_piece_pos: tuple[int, int] | None  # Position of piece that acted last (for SAME/DIFFERENT)

    # King positions for quick lookup
    king_positions: tuple[tuple[int, int] | None, tuple[int, int] | None]

    # Game over state
    winner: int | None = None  # 0=White wins, 1=Black wins, None=ongoing

    @classmethod
    def create_initial(
        cls,
        white_pieces: list[tuple[str, tuple[int, int], int]],  # (type_id, (q,r), facing)
        black_pieces: list[tuple[str, tuple[int, int], int]],
        white_template: str = 'D',
        black_template: str = 'A',
    ) -> GameState:
        """Create a new game with specified piece positions."""
        board: dict[tuple[int, int], Piece] = {}
        white_king_pos = None
        black_king_pos = None

        for type_id, pos, facing in white_pieces:
            board[pos] = Piece(type_id, owner=0, facing=facing)
            if is_king(type_id):
                white_king_pos = pos

        for type_id, pos, facing in black_pieces:
            board[pos] = Piece(type_id, owner=1, facing=facing)
            if is_king(type_id):
                black_king_pos = pos

        return cls(
            board=board,
            graveyards=([], []),
            current_player=0,  # White moves first
            turn_number=1,
            round_number=1,
            templates=(white_template, black_template),
            action_index=0,
            last_piece_pos=None,
            king_positions=(white_king_pos, black_king_pos),
        )

    @property
    def current_template(self) -> ActionTemplate:
        """Get the action template for the current player."""
        template_id = self.templates[self.current_player]
        return TEMPLATES[template_id]

    @property
    def current_action(self) -> tuple[str, str] | None:
        """Get the current action type and constraint, or None if turn is over."""
        template = self.current_template
        if self.action_index >= len(template):
            return None
        return template[self.action_index]

    @property
    def is_turn_complete(self) -> bool:
        """Check if current player has completed their turn."""
        return self.action_index >= len(self.current_template)

    def copy(self) -> GameState:
        """Create a deep copy of this state."""
        return GameState(
            board={pos: Piece(p.type_id, p.owner, p.facing) for pos, p in self.board.items()},
            graveyards=(list(self.graveyards[0]), list(self.graveyards[1])),
            current_player=self.current_player,
            turn_number=self.turn_number,
            round_number=self.round_number,
            templates=self.templates,
            action_index=self.action_index,
            last_piece_pos=self.last_piece_pos,
            king_positions=self.king_positions,
            winner=self.winner,
        )


# ============================================================================
# MOVE GENERATION
# ============================================================================

def generate_destinations(
    state: GameState,
    pos: tuple[int, int],
    piece: Piece,
) -> Iterator[tuple[int, int]]:
    """Generate all valid destination hexes for a piece's normal movement."""
    ptype = piece.piece_type
    owner = piece.owner
    facing = piece.facing

    if ptype.move_type == 'NONE':
        return  # Warper has no normal movement

    if ptype.move_type == 'JUMP':
        # JUMP: Land on any hex at exactly distance N from current position.
        # For omni (ALL_DIRS), full ring (12 hexes at d2, 18 at d3).
        # For FORWARD_ARC, 150° arc centered on forward (5 at d2, 7 at d3).
        import math
        jump_distance = ptype.move_range

        # Check if this is a forward-arc piece (uses angle-based filtering)
        # vs omni or other patterns (uses sector-based filtering)
        is_forward_arc = len(ptype.directions) == 3 and set(ptype.directions) == {0, 1, 5}

        if is_forward_arc:
            # Forward arc: 150° centered on facing direction (±75°)
            FACING_ANGLES = [270, 330, 30, 90, 150, 210]
            forward_angle = FACING_ANGLES[facing]

            for dest in iter_hex_ring(pos[0], pos[1], jump_distance):
                if not is_valid_hex(*dest):
                    continue

                # Calculate angle of destination relative to piece
                dq = dest[0] - pos[0]
                dr = dest[1] - pos[1]
                x = 1.5 * dq
                y = 0.8660254 * dq + 1.7320508 * dr
                angle = math.degrees(math.atan2(y, x))
                if angle < 0:
                    angle += 360

                # Check if within ±75° of forward direction
                diff = abs(angle - forward_angle)
                if diff > 180:
                    diff = 360 - diff
                if diff > 75:
                    continue

                # Check if we can land there
                occupant = state.board.get(dest)
                if occupant is None:
                    yield dest
                elif occupant.owner != owner:
                    if (get_special(piece.type_id) != 'PHASED' and
                        get_special(occupant.type_id) != 'PHASED'):
                        yield dest
        else:
            # Omni or other patterns: use sector-based filtering
            allowed_sectors = set()
            for rel_dir in ptype.directions:
                absolute_dir = (facing + rel_dir) % 6
                allowed_sectors.add(absolute_dir)

            for dest in iter_hex_ring(pos[0], pos[1], jump_distance):
                if not is_valid_hex(*dest):
                    continue

                dq = dest[0] - pos[0]
                dr = dest[1] - pos[1]
                sector = hex_to_sector(dq, dr)

                if sector not in allowed_sectors:
                    continue

                occupant = state.board.get(dest)
                if occupant is None:
                    yield dest
                elif occupant.owner != owner:
                    if (get_special(piece.type_id) != 'PHASED' and
                        get_special(occupant.type_id) != 'PHASED'):
                        yield dest
        return  # JUMP handled, don't fall through to STEP/SLIDE loop

    # STEP and SLIDE loop over each direction
    for rel_dir in ptype.directions:
        dq, dr = get_direction_vector(facing, rel_dir)

        if ptype.move_type == 'STEP':
            # Can move 1 to range hexes in this direction
            cq, cr = pos
            for dist in range(1, ptype.move_range + 1):
                cq, cr = cq + dq, cr + dr
                if not is_valid_hex(cq, cr):
                    break  # Off board
                occupant = state.board.get((cq, cr))
                if occupant is not None:
                    if occupant.owner != owner:
                        # Can capture enemy unless:
                        # - Moving piece is PHASED (Ghost can't capture)
                        # - Target piece is PHASED (Ghost can't be captured)
                        if (get_special(piece.type_id) != 'PHASED' and
                            get_special(occupant.type_id) != 'PHASED'):
                            yield (cq, cr)
                    break  # Blocked by any piece
                yield (cq, cr)

        elif ptype.move_type == 'SLIDE':
            # Move any distance until blocked
            cq, cr = pos
            while True:
                cq, cr = cq + dq, cr + dr
                if not is_valid_hex(cq, cr):
                    break
                occupant = state.board.get((cq, cr))
                if occupant is not None:
                    if occupant.owner != owner:
                        # Can capture unless Ghost involved
                        if (get_special(piece.type_id) != 'PHASED' and
                            get_special(occupant.type_id) != 'PHASED'):
                            yield (cq, cr)  # Capture
                    break  # Blocked
                yield (cq, cr)


def generate_moves_for_piece(
    state: GameState,
    pos: tuple[int, int],
) -> Iterator[Move]:
    """Generate all MOVE actions for a piece at given position."""
    piece = state.board.get(pos)
    if piece is None or piece.owner != state.current_player:
        return

    # Ghost cannot capture
    is_ghost = get_special(piece.type_id) == 'PHASED'

    # Normal movement
    for dest in generate_destinations(state, pos, piece):
        occupant = state.board.get(dest)
        if is_ghost and occupant is not None:
            continue  # Ghost can't capture

        # Piece keeps its facing when moving (can rotate separately)
        yield Move('MOVE', pos, dest, piece.facing, None)


def generate_rotates_for_piece(
    state: GameState,
    pos: tuple[int, int],
) -> Iterator[Move]:
    """Generate all ROTATE actions for a piece at given position."""
    piece = state.board.get(pos)
    if piece is None or piece.owner != state.current_player:
        return

    # Skip rotation for omnidirectional pieces (rotating does nothing)
    pt = PIECE_TYPES.get(piece.type_id)
    if pt and len(pt.directions) == 6:
        return

    # Can rotate to any of 6 facings (including current = no-op rotate)
    for new_facing in range(6):
        yield Move('ROTATE', pos, None, new_facing, None)


def generate_special_moves(
    state: GameState,
    pos: tuple[int, int],
    action_type: str,
) -> Iterator[Move]:
    """Generate special ability moves for a piece."""
    piece = state.board.get(pos)
    if piece is None or piece.owner != state.current_player:
        return

    special = get_special(piece.type_id)
    if special is None:
        return

    if special == 'SWAP_MOVE' and action_type == 'MOVE':
        # Warper: swap with any friendly piece
        for target_pos, target_piece in state.board.items():
            if target_pos != pos and target_piece.owner == piece.owner:
                yield Move(
                    'SPECIAL', pos, None, piece.facing,
                    {'type': 'SWAP', 'target': target_pos}
                )

    elif special == 'SWAP_ROTATE' and action_type == 'ROTATE':
        # Shifter: swap with any friendly piece (consumes rotate)
        for target_pos, target_piece in state.board.items():
            if target_pos != pos and target_piece.owner == piece.owner:
                yield Move(
                    'SPECIAL', pos, None, piece.facing,
                    {'type': 'SWAP', 'target': target_pos}
                )

    # Note: REBIRTH (Phoenix) special is handled separately in generate_phoenix_rebirth()
    # because it's triggered when Phoenix is in graveyard, not when on board


def direction_from_to(from_pos: tuple[int, int], to_pos: tuple[int, int]) -> int:
    """Get the direction index (0-5) from one hex to an adjacent hex."""
    dq = to_pos[0] - from_pos[0]
    dr = to_pos[1] - from_pos[1]
    for i, (dir_q, dir_r) in enumerate(DIRECTIONS):
        if dq == dir_q and dr == dir_r:
            return i
    # Fallback (shouldn't happen for adjacent hexes)
    return 0


def generate_phoenix_rebirth(state: GameState) -> Iterator[Move]:
    """Generate Phoenix rebirth moves if Phoenix is in current player's graveyard.

    Phoenix REBIRTH: When the Phoenix is captured and in the graveyard, the player
    may spend their MOVE action to bring the Phoenix back onto the board, placing
    it in any empty hex adjacent to their king. The resurrected piece faces toward the king.
    """
    player = state.current_player
    graveyard = state.graveyards[player]

    # Check if Phoenix is in graveyard
    if 'P1' not in graveyard:
        return

    king_pos = state.king_positions[player]
    if king_pos is None:
        return

    # Find empty hexes adjacent to king
    for neighbor in get_valid_neighbors(*king_pos):
        if neighbor not in state.board:
            # Phoenix faces toward the king (per rules)
            facing = direction_from_to(neighbor, king_pos)
            yield Move(
                'SPECIAL', None, neighbor, facing,
                {'type': 'REBIRTH'}
            )


def generate_legal_actions(state: GameState) -> list[Move]:
    """Generate all legal actions for the current action step."""
    if state.winner is not None:
        return []  # Game is over

    action = state.current_action
    if action is None:
        return []  # Turn is complete

    action_type, constraint = action
    actions = []

    # Always can pass or surrender
    actions.append(Move('PASS', None, None, None, None))
    actions.append(Move('SURRENDER', None, None, None, None))

    # Determine which pieces can act based on constraint
    valid_positions = []
    for pos, piece in state.board.items():
        if piece.owner != state.current_player:
            continue

        if constraint == 'SAME':
            if state.last_piece_pos is None or pos != state.last_piece_pos:
                continue
        elif constraint == 'DIFFERENT':
            if state.last_piece_pos is not None and pos == state.last_piece_pos:
                continue
        # 'ANY' has no constraint

        valid_positions.append(pos)

    # Generate actions based on type
    for pos in valid_positions:
        if action_type == 'MOVE':
            actions.extend(generate_moves_for_piece(state, pos))
            actions.extend(generate_special_moves(state, pos, 'MOVE'))
        elif action_type == 'ROTATE':
            actions.extend(generate_rotates_for_piece(state, pos))
            actions.extend(generate_special_moves(state, pos, 'ROTATE'))
        elif action_type == 'MOVE_OR_ROTATE':
            # Generate both move and rotate options
            actions.extend(generate_moves_for_piece(state, pos))
            actions.extend(generate_special_moves(state, pos, 'MOVE'))
            actions.extend(generate_rotates_for_piece(state, pos))
            actions.extend(generate_special_moves(state, pos, 'ROTATE'))

    # Phoenix rebirth: available when Phoenix is in graveyard (uses MOVE action)
    if action_type in ('MOVE', 'MOVE_OR_ROTATE'):
        actions.extend(generate_phoenix_rebirth(state))

    return actions


# ============================================================================
# APPLYING MOVES
# ============================================================================

def apply_move(state: GameState, move: Move) -> GameState:
    """Apply a move to a game state, returning a new state."""
    new_state = state.copy()

    if move.action_type == 'PASS':
        pass  # No board changes

    elif move.action_type == 'SURRENDER':
        # Player gives up - opponent wins
        new_state.winner = 1 - new_state.current_player

    elif move.action_type == 'MOVE':
        from_pos = move.from_pos
        to_pos = move.to_pos
        piece = new_state.board.pop(from_pos)

        # Handle capture - piece goes to OWNER's graveyard (for rebirth)
        if to_pos in new_state.board:
            captured = new_state.board[to_pos]
            new_state.graveyards[captured.owner].append(captured.type_id)

            # Check if king was captured
            if captured.is_king:
                new_state.winner = new_state.current_player

        # Move piece
        piece.facing = move.new_facing if move.new_facing is not None else piece.facing
        new_state.board[to_pos] = piece

        # Update king position if king moved
        if piece.is_king:
            if new_state.current_player == 0:
                new_state.king_positions = (to_pos, new_state.king_positions[1])
            else:
                new_state.king_positions = (new_state.king_positions[0], to_pos)

        new_state.last_piece_pos = to_pos

    elif move.action_type == 'ROTATE':
        pos = move.from_pos
        piece = new_state.board[pos]
        new_state.board[pos] = Piece(piece.type_id, piece.owner, move.new_facing)
        new_state.last_piece_pos = pos

    elif move.action_type == 'SPECIAL':
        special_data = move.special_data

        if special_data['type'] == 'SWAP':
            # Swap positions
            pos1 = move.from_pos
            pos2 = special_data['target']
            piece1 = new_state.board[pos1]
            piece2 = new_state.board[pos2]
            new_state.board[pos1] = piece2
            new_state.board[pos2] = piece1

            # Update king position if a king was involved
            for owner in (0, 1):
                if new_state.king_positions[owner] == pos1:
                    new_state.king_positions = (
                        pos2 if owner == 0 else new_state.king_positions[0],
                        pos2 if owner == 1 else new_state.king_positions[1],
                    )
                elif new_state.king_positions[owner] == pos2:
                    new_state.king_positions = (
                        pos1 if owner == 0 else new_state.king_positions[0],
                        pos1 if owner == 1 else new_state.king_positions[1],
                    )

            new_state.last_piece_pos = pos1

        elif special_data['type'] == 'REBIRTH':
            # Phoenix rebirth: bring Phoenix back from graveyard
            dest = move.to_pos
            owner = new_state.current_player

            # Remove Phoenix from graveyard
            new_state.graveyards[owner].remove('P1')

            # Place Phoenix facing toward center (default facing for owner)
            facing = move.new_facing if move.new_facing is not None else default_facing(owner)
            new_state.board[dest] = Piece('P1', owner, facing)

            new_state.last_piece_pos = dest

    # Advance action index
    new_state.action_index += 1

    # Check if turn is complete
    if new_state.is_turn_complete:
        new_state = _end_turn(new_state)

    return new_state


def _end_turn(state: GameState) -> GameState:
    """Handle end of turn: switch players, update counters, check turn limit."""
    # Switch player
    state.current_player = 1 - state.current_player
    state.action_index = 0
    state.last_piece_pos = None
    state.turn_number += 1

    # Increment round after Black's turn
    if state.current_player == 0:  # Just switched to White
        state.round_number += 1

    # Check turn limit (50 rounds)
    if state.round_number > 50 and state.winner is None:
        state = _resolve_by_proximity(state)

    return state


def _resolve_by_proximity(state: GameState) -> GameState:
    """Resolve game by proximity to center when turn limit is reached."""
    white_king = state.king_positions[0]
    black_king = state.king_positions[1]

    if white_king is None:
        state.winner = 1  # Black wins (White king captured somehow)
    elif black_king is None:
        state.winner = 0  # White wins
    else:
        white_dist = distance_to_center(*white_king)
        black_dist = distance_to_center(*black_king)

        if white_dist < black_dist:
            state.winner = 0  # White closer to center
        elif black_dist < white_dist:
            state.winner = 1  # Black closer to center
        else:
            # Tie on distance: winner has more pieces
            white_count = sum(1 for p in state.board.values() if p.owner == 0)
            black_count = sum(1 for p in state.board.values() if p.owner == 1)
            if white_count > black_count:
                state.winner = 0
            elif black_count > white_count:
                state.winner = 1
            else:
                # Still tied: White wins (slight urgency for Black per spec)
                state.winner = 0

    return state


# ============================================================================
# UTILITY FUNCTIONS
# ============================================================================

def is_game_over(state: GameState) -> bool:
    """Check if the game has ended."""
    return state.winner is not None


def get_winner(state: GameState) -> int | None:
    """Get the winner (0=White, 1=Black) or None if ongoing."""
    return state.winner


def print_board(state: GameState) -> str:
    """Generate a simple text representation of the board."""
    lines = []
    lines.append(f"Turn {state.turn_number} (Round {state.round_number})")
    lines.append(f"Current player: {'White' if state.current_player == 0 else 'Black'}")
    lines.append(f"Action: {state.action_index + 1}/{len(state.current_template)}")
    lines.append("")

    # Simple grid display
    for r in range(-BOARD_RADIUS, BOARD_RADIUS + 1):
        indent = "  " * (BOARD_RADIUS + r)
        row = []
        for q in range(-BOARD_RADIUS, BOARD_RADIUS + 1):
            if is_valid_hex(q, r):
                piece = state.board.get((q, r))
                if piece:
                    row.append(repr(piece))
                else:
                    row.append("...")
        if row:
            lines.append(indent + " ".join(row))

    return "\n".join(lines)
