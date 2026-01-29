"""
HEXWAR Piece Definitions

All 30 piece types defined as pure data.
Movement patterns use relative directions from piece facing.

## Movement Types

- **STEP**: Move 1 to `range` hexes in a straight line. Blocked by pieces.
- **SLIDE**: Move any distance in a straight line until blocked.
- **JUMP**: Land on any hex at exactly distance N from the piece.
  Forward arc uses 150° angle-based filtering (±75° from forward).
  Omni uses full ring (all 360°).

  For distance-2 jumpers:
    - Full ring (ALL_DIRS): 12 possible positions
    - Forward arc (FORWARD_ARC): 5 possible positions

  For distance-3 jumpers:
    - Full ring (ALL_DIRS): 18 possible positions
    - Forward arc (FORWARD_ARC): 7 possible positions

- **NONE**: Cannot move normally (Warper uses special swap ability).
"""

from dataclasses import dataclass
from typing import Literal
from hexwar.board import (
    FORWARD, FORWARD_RIGHT, BACK_RIGHT, BACKWARD, BACK_LEFT, FORWARD_LEFT
)

# Movement types
MoveType = Literal['STEP', 'SLIDE', 'JUMP', 'NONE']

# Direction sets for convenience
ALL_DIRS = (FORWARD, FORWARD_RIGHT, BACK_RIGHT, BACKWARD, BACK_LEFT, FORWARD_LEFT)
FORWARD_ARC = (FORWARD, FORWARD_LEFT, FORWARD_RIGHT)
DIAGONAL_DIRS = (FORWARD_LEFT, FORWARD_RIGHT, BACK_LEFT, BACK_RIGHT)
FORWARD_BACK = (FORWARD, BACKWARD)

# Special ability types:
# - SWAP_MOVE: Warper can swap with any friendly piece instead of moving
# - SWAP_ROTATE: Shifter can swap with any friendly piece instead of rotating
# - REBIRTH: When Phoenix is captured, player can spend MOVE to place it adjacent to king
# - PHASED: Ghost cannot capture AND cannot be captured (just occupies space)
SpecialType = Literal['SWAP_MOVE', 'SWAP_ROTATE', 'REBIRTH', 'PHASED', None]


@dataclass(frozen=True)
class PieceType:
    """Definition of a piece type's capabilities."""
    id: str
    name: str
    move_type: MoveType
    move_range: int  # 1, 2, 3, or 999 for SLIDE
    directions: tuple[int, ...]  # Relative directions this piece can move
    special: SpecialType = None
    is_king: bool = False


# Infinity for slide pieces
INF = 999

# ============================================================================
# PIECE CATALOG - All 29 types from spec
# ============================================================================

# Step-1 Pieces (A1-A5)
PAWN = PieceType('A1', 'Pawn', 'STEP', 1, (FORWARD,))
GUARD = PieceType('A2', 'Guard', 'STEP', 1, ALL_DIRS)
SCOUT = PieceType('A3', 'Scout', 'STEP', 1, FORWARD_ARC)
CRAB = PieceType('A4', 'Crab', 'STEP', 1, (FORWARD_LEFT, FORWARD_RIGHT, BACKWARD))
FLANKER = PieceType('A5', 'Flanker', 'STEP', 1, (FORWARD_LEFT, FORWARD_RIGHT))

# Step-2 Pieces (B1-B4)
STRIDER = PieceType('B1', 'Strider', 'STEP', 2, (FORWARD,))
DANCER = PieceType('B2', 'Dancer', 'STEP', 2, (FORWARD_LEFT, FORWARD_RIGHT))
RANGER = PieceType('B3', 'Ranger', 'STEP', 2, ALL_DIRS)
HOUND = PieceType('B4', 'Hound', 'STEP', 2, FORWARD_ARC)

# Step-3 Pieces (C1-C3)
LANCER = PieceType('C1', 'Lancer', 'STEP', 3, (FORWARD,))
DRAGOON = PieceType('C2', 'Dragoon', 'STEP', 3, FORWARD_ARC)
COURSER = PieceType('C3', 'Courser', 'STEP', 3, ALL_DIRS)

# Slide Pieces (D1-D5)
PIKE = PieceType('D1', 'Pike', 'SLIDE', INF, (FORWARD,))
ROOK = PieceType('D2', 'Rook', 'SLIDE', INF, FORWARD_BACK)
BISHOP = PieceType('D3', 'Bishop', 'SLIDE', INF, DIAGONAL_DIRS)
CHARIOT = PieceType('D4', 'Chariot', 'SLIDE', INF, FORWARD_ARC)
QUEEN = PieceType('D5', 'Queen', 'SLIDE', INF, ALL_DIRS)

# Jump Pieces (E1-E2, F1-F2)
KNIGHT = PieceType('E1', 'Knight', 'JUMP', 2, FORWARD_ARC)  # Forward arc, distance 2 (5 positions)
FROG = PieceType('E2', 'Frog', 'JUMP', 2, ALL_DIRS)  # Omni, distance 2 (12 positions)
LOCUST = PieceType('F1', 'Locust', 'JUMP', 3, FORWARD_ARC)  # Forward arc, distance 3 (7 positions)
CRICKET = PieceType('F2', 'Cricket', 'JUMP', 3, ALL_DIRS)  # Omni, distance 3 (18 positions)

# Special Pieces (W1-W2, P1, G1)
WARPER = PieceType('W1', 'Warper', 'NONE', 0, (), special='SWAP_MOVE')  # Swaps with friendlies
SHIFTER = PieceType('W2', 'Shifter', 'STEP', 1, ALL_DIRS, special='SWAP_ROTATE')  # Swaps on rotate
PHOENIX = PieceType('P1', 'Phoenix', 'STEP', 1, FORWARD_ARC, special='REBIRTH')  # Returns from graveyard
GHOST = PieceType('G1', 'Ghost', 'STEP', 1, ALL_DIRS, special='PHASED')  # Can't capture/be captured

# King Variants (K1-K5)
KING_GUARD = PieceType('K1', 'King (Guard)', 'STEP', 1, ALL_DIRS, is_king=True)
KING_SCOUT = PieceType('K2', 'King (Scout)', 'STEP', 1, FORWARD_ARC, is_king=True)
KING_RANGER = PieceType('K3', 'King (Ranger)', 'STEP', 2, ALL_DIRS, is_king=True)
KING_FROG = PieceType('K4', 'King (Frog)', 'JUMP', 2, ALL_DIRS, is_king=True)
KING_PIKE = PieceType('K5', 'King (Pike)', 'SLIDE', INF, (FORWARD,), is_king=True)

# Master lookup table by ID
PIECE_TYPES: dict[str, PieceType] = {
    # Step-1
    'A1': PAWN, 'A2': GUARD, 'A3': SCOUT, 'A4': CRAB, 'A5': FLANKER,
    # Step-2
    'B1': STRIDER, 'B2': DANCER, 'B3': RANGER, 'B4': HOUND,
    # Step-3
    'C1': LANCER, 'C2': DRAGOON, 'C3': COURSER,
    # Slide
    'D1': PIKE, 'D2': ROOK, 'D3': BISHOP, 'D4': CHARIOT, 'D5': QUEEN,
    # Jump
    'E1': KNIGHT, 'E2': FROG, 'F1': LOCUST, 'F2': CRICKET,
    # Special
    'W1': WARPER, 'W2': SHIFTER, 'P1': PHOENIX, 'G1': GHOST,
    # Kings
    'K1': KING_GUARD, 'K2': KING_SCOUT, 'K3': KING_RANGER,
    'K4': KING_FROG, 'K5': KING_PIKE,
}

# Convenience sets
REGULAR_PIECE_IDS = tuple(k for k, v in PIECE_TYPES.items() if not v.is_king)
KING_IDS = tuple(k for k, v in PIECE_TYPES.items() if v.is_king)
SPECIAL_PIECE_IDS = tuple(k for k, v in PIECE_TYPES.items() if v.special is not None)


def get_piece_type(type_id: str) -> PieceType:
    """Get a piece type definition by ID."""
    return PIECE_TYPES[type_id]


def is_king(type_id: str) -> bool:
    """Check if a piece type is a king variant."""
    return PIECE_TYPES[type_id].is_king


def has_special(type_id: str) -> bool:
    """Check if a piece type has a special ability."""
    return PIECE_TYPES[type_id].special is not None


def get_special(type_id: str) -> SpecialType:
    """Get the special ability type for a piece, or None."""
    return PIECE_TYPES[type_id].special


@dataclass
class Piece:
    """A piece instance on the board."""
    type_id: str
    owner: int  # 0 = White, 1 = Black
    facing: int  # 0-5 (N, NE, SE, S, SW, NW)

    @property
    def piece_type(self) -> PieceType:
        """Get the type definition for this piece."""
        return PIECE_TYPES[self.type_id]

    @property
    def is_king(self) -> bool:
        return self.piece_type.is_king

    def __repr__(self) -> str:
        owner_str = 'W' if self.owner == 0 else 'B'
        return f"{owner_str}{self.type_id}"
