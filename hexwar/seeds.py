"""
Human-coherent seed configurations for HEXWAR evolution.

These provide sensible starting points that evolution can refine,
rather than starting from random chaos.
"""

from hexwar.evolution import RuleSet
from hexwar.board import WHITE_HOME_ZONE, BLACK_HOME_ZONE, BOARD_RADIUS


def _layout_army(home_zone: frozenset, king_back: bool = True) -> list[tuple[int, int]]:
    """
    Generate positions with human-coherent layout:
    - King in the back row (furthest from center)
    - Stronger pieces in middle rows
    - Pawns/weak pieces in front row (closest to center)

    Returns list of positions, first position is for king.
    Piece positions are ordered FRONT to BACK so that:
    - First pieces in army list (pawns) get front positions
    - Last pieces in army list (queens etc) get back positions
    """
    # Sort hexes by distance from center (r value for home zones)
    hexes = sorted(home_zone, key=lambda h: (abs(h[1]), h[0]))

    if king_back:
        # King goes in back row (highest |r| value)
        back_row = [h for h in hexes if abs(h[1]) == BOARD_RADIUS]
        mid_rows = [h for h in hexes if abs(h[1]) == BOARD_RADIUS - 1]
        front_rows = [h for h in hexes if abs(h[1]) < BOARD_RADIUS - 1]

        # Sort each row by q to spread pieces across
        back_row.sort(key=lambda h: abs(h[0]))
        mid_rows.sort(key=lambda h: abs(h[0]))
        front_rows.sort(key=lambda h: abs(h[0]))

        # King in center of back row
        king_pos = back_row[0] if back_row else hexes[0]

        # Arrange remaining positions: FRONT -> MID -> BACK
        # So first pieces (pawns) go to front, last pieces (queens) go to back
        remaining = front_rows + mid_rows + [h for h in back_row if h != king_pos]
    else:
        king_pos = hexes[len(hexes) // 2]
        remaining = [h for h in hexes if h != king_pos]

    return [king_pos] + remaining


# Pre-computed layouts
WHITE_POSITIONS = _layout_army(WHITE_HOME_ZONE, king_back=True)
BLACK_POSITIONS = _layout_army(BLACK_HOME_ZONE, king_back=True)


def create_chess_like_seed() -> RuleSet:
    """
    Chess-inspired symmetric configuration:
    - Standard king (K1 Guard)
    - 4 pawns in front
    - 2 rooks, 2 bishops, 1 queen
    - 2 knights for mobility

    Pieces ordered weakest-to-strongest so weak go to front positions.
    """
    pieces = [
        # FRONT LINE (expendable)
        'A1', 'A1', 'A1', 'A1',  # 4 Pawns
        'E1', 'E1',              # 2 Knights (mobile but short range)
        # MID/BACK (valuable)
        'D3', 'D3',              # 2 Bishops
        'D2', 'D2',              # 2 Rooks
        'D5',                    # 1 Queen (most valuable, stays back)
    ]

    return RuleSet(
        white_pieces=list(pieces),
        black_pieces=list(pieces),
        white_template='E',
        black_template='E',
        white_king='K1',
        black_king='K1',
        white_positions=WHITE_POSITIONS[:len(pieces)+1],
        black_positions=BLACK_POSITIONS[:len(pieces)+1],
    )


def create_defensive_seed() -> RuleSet:
    """
    Defensive configuration with guards and ranged pieces:
    - Ranger king (K3) for escape options
    - Guards for protection
    - Lancers and Rooks for reach
    - Ghost for disruption
    """
    white_pieces = [
        # FRONT (expendable defenders)
        'A2', 'A2', 'A2',        # 3 Guards (defensive)
        'G1',                    # 1 Ghost (can't be captured - good scout)
        # MID/BACK (ranged firepower)
        'C1', 'C1',              # 2 Lancers (forward reach)
        'B3',                    # 1 Ranger
        'D2',                    # 1 Rook
        'D4',                    # 1 Chariot
    ]

    black_pieces = [
        # FRONT
        'A2', 'A2', 'A2',        # 3 Guards
        'E2',                    # 1 Frog (jumper - mobile scout)
        'B4',                    # 1 Hound
        # BACK
        'C2', 'C2',              # 2 Dragoons
        'D2',                    # 1 Rook
        'D5',                    # 1 Queen
    ]

    return RuleSet(
        white_pieces=white_pieces,
        black_pieces=black_pieces,
        white_template='E',
        black_template='E',
        white_king='K3',  # Ranger king - mobile
        black_king='K1',  # Guard king - defensive
        white_positions=WHITE_POSITIONS[:len(white_pieces)+1],
        black_positions=BLACK_POSITIONS[:len(black_pieces)+1],
    )


def create_aggressive_seed() -> RuleSet:
    """
    Aggressive configuration with mobile strikers:
    - Scout king (K2) - forward-facing
    - Lots of forward-moving pieces
    - Queens and Chariots for power
    """
    white_pieces = [
        # FRONT (fast attackers)
        'A1', 'A1', 'A1',        # 3 Pawns
        'B1', 'B1',              # 2 Striders (2-step forward)
        'F1',                    # 1 Locust (3-jump - strikes deep)
        # BACK (heavy hitters)
        'C2', 'C2',              # 2 Dragoons (3-step arc)
        'D4',                    # 1 Chariot (forward slider)
        'D5',                    # 1 Queen
    ]

    black_pieces = [
        # FRONT (fast attackers)
        'A3', 'A3', 'A3',        # 3 Scouts (forward arc)
        'B4', 'B4',              # 2 Hounds (2-step arc)
        'E1', 'E1',              # 2 Knights
        # BACK (heavy hitters)
        'C1', 'C1',              # 2 Lancers
        'D5',                    # 1 Queen
    ]

    return RuleSet(
        white_pieces=white_pieces,
        black_pieces=black_pieces,
        white_template='E',
        black_template='E',
        white_king='K2',  # Scout king
        black_king='K2',  # Scout king
        white_positions=WHITE_POSITIONS[:len(white_pieces)+1],
        black_positions=BLACK_POSITIONS[:len(black_pieces)+1],
    )


def create_special_seed() -> RuleSet:
    """
    Configuration featuring special pieces:
    - Warper, Ghost, Phoenix
    - Tests how specials affect balance
    """
    white_pieces = [
        # FRONT (scouts and specials that benefit from forward position)
        'A2', 'A2',              # 2 Guards
        'G1',                    # 1 Ghost (phased - good scout)
        'P1',                    # 1 Phoenix (rebirth - can afford to die)
        # BACK (valuable pieces)
        'B3',                    # 1 Ranger
        'W1',                    # 1 Warper (teleport - stays safe)
        'D2',                    # 1 Rook
        'D5',                    # 1 Queen
    ]

    black_pieces = [
        # FRONT
        'A2', 'A2',              # 2 Guards
        'E2',                    # 1 Frog (mobile scout)
        'W2',                    # 1 Shifter (swap rotate)
        # BACK
        'B3',                    # 1 Ranger
        'C3',                    # 1 Courser
        'D2',                    # 1 Rook
        'D5',                    # 1 Queen
    ]

    return RuleSet(
        white_pieces=white_pieces,
        black_pieces=black_pieces,
        white_template='E',
        black_template='E',
        white_king='K4',  # Frog king (jumper)
        black_king='K3',  # Ranger king
        white_positions=WHITE_POSITIONS[:len(white_pieces)+1],
        black_positions=BLACK_POSITIONS[:len(black_pieces)+1],
    )


def create_asymmetric_classic_seed() -> RuleSet:
    """
    Asymmetric armies with NO piece overlap between sides.
    Human-coherent layout: cheap pieces front, valuable back.
    Kings in defensive corner positions.
    Mixed facings for tactical variety.
    """
    # White: Pawn-based army with sliders
    white_pieces = [
        # FRONT - cheap forward-movers (facing N=0)
        'A1', 'A1', 'A1', 'A1', 'A1',  # 5 Pawns
        'B1', 'B1',                     # 2 Striders
        # BACK - sliders and power pieces
        'D2', 'D2',                     # 2 Rooks
        'D3',                           # 1 Bishop
        'D5',                           # 1 Queen
    ]

    # Black: Guard-based army with jumpers (completely different pieces)
    black_pieces = [
        # FRONT - defensive line
        'A2', 'A2', 'A2', 'A2',        # 4 Guards
        'A3', 'A3',                     # 2 Scouts
        # BACK - jumpers and mobile pieces
        'E1', 'E1',                     # 2 Knights
        'E2',                           # 1 Frog
        'C3',                           # 1 Courser
        'D4',                           # 1 Chariot
    ]

    # Custom positions: King in defensive corner, pieces arranged sensibly
    # White king at (-2, 4) - back left corner
    white_positions = [
        (-2, 4),   # King - back corner (defended)
        # Front row (r=2) - pawns and striders
        (0, 2), (-1, 2), (1, 2), (-2, 2), (2, 2),  # 5 Pawns
        (-3, 2), (-4, 2),                           # 2 Striders on flank
        # Back positions (r=3,4) - valuable pieces
        (0, 3), (-1, 3),                            # 2 Rooks (mid)
        (1, 3),                                     # Bishop (mid)
        (-1, 4),                                    # Queen (back, near king)
    ]

    # Black king at (2, -4) - back right corner (mirrored)
    black_positions = [
        (2, -4),   # King - back corner (defended)
        # Front row (r=-2) - guards and scouts
        (0, -2), (1, -2), (-1, -2), (2, -2),       # 4 Guards
        (3, -2), (4, -2),                          # 2 Scouts on flank
        # Back positions - jumpers
        (0, -3), (1, -3),                          # 2 Knights (mid)
        (-1, -3),                                  # Frog (mid)
        (2, -3),                                   # Courser (mid)
        (1, -4),                                   # Chariot (back, near king)
    ]

    return RuleSet(
        white_pieces=white_pieces,
        black_pieces=black_pieces,
        white_template='E',
        black_template='E',
        white_king='K1',   # Guard king - defensive
        black_king='K4',   # Frog king - can jump to escape
        white_positions=[tuple(p) for p in white_positions],
        black_positions=[tuple(p) for p in black_positions],
    )


def create_asymmetric_mobile_seed() -> RuleSet:
    """
    Asymmetric armies focused on mobility.
    White: Ranged steppers and sliders
    Black: Jumpers and arc-movers
    """
    white_pieces = [
        # FRONT - ranged steppers
        'B3', 'B3', 'B3',              # 3 Rangers (2-step all dirs)
        'B4', 'B4',                     # 2 Hounds (2-step arc)
        # BACK - sliders
        'D1', 'D1',                     # 2 Pikes (forward slider)
        'D2',                           # 1 Rook
        'C1', 'C1',                     # 2 Lancers
    ]

    black_pieces = [
        # FRONT - short movers and jumpers
        'A4', 'A4', 'A4',              # 3 Crabs (quirky movement)
        'A5', 'A5',                     # 2 Flankers (diagonal only)
        # BACK - power jumpers
        'F1', 'F1',                     # 2 Locusts (3-jump arc)
        'E2', 'E2',                     # 2 Frogs (2-jump all)
        'C2',                           # 1 Dragoon
    ]

    # White king at (-2, 4), Black king at (2, -4)
    white_positions = [
        (-2, 4),   # King
        (0, 2), (-1, 2), (1, 2),        # 3 Rangers front-center
        (-2, 2), (2, 2),                # 2 Hounds on flanks
        (0, 3), (-1, 3),                # 2 Pikes mid
        (1, 3),                         # Rook mid
        (-2, 3), (0, 4),                # 2 Lancers back
    ]

    black_positions = [
        (2, -4),   # King
        (0, -2), (1, -2), (-1, -2),     # 3 Crabs front-center
        (2, -2), (-2, -2),              # 2 Flankers on flanks
        (0, -3), (1, -3),               # 2 Locusts mid
        (-1, -3), (2, -3),              # 2 Frogs mid
        (1, -4),                        # Dragoon back
    ]

    return RuleSet(
        white_pieces=white_pieces,
        black_pieces=black_pieces,
        white_template='E',
        black_template='E',
        white_king='K2',   # Scout king - forward arc
        black_king='K3',   # Ranger king - all directions
        white_positions=[tuple(p) for p in white_positions],
        black_positions=[tuple(p) for p in black_positions],
    )


def create_asymmetric_special_seed() -> RuleSet:
    """
    Asymmetric armies featuring special pieces.
    White: Ghost, Warper, Phoenix
    Black: Shifter, heavy sliders
    """
    white_pieces = [
        # FRONT - expendables and specials that benefit from forward position
        'A1', 'A1', 'A1',              # 3 Pawns
        'G1',                           # Ghost (phased scout)
        'P1',                           # Phoenix (can rebirth)
        # BACK - teleporter and sliders
        'W1',                           # Warper (teleport)
        'D3', 'D3',                     # 2 Bishops
        'B3',                           # Ranger
    ]

    black_pieces = [
        # FRONT - guards and shifter
        'A2', 'A2', 'A2',              # 3 Guards
        'W2',                           # Shifter (swap on rotate)
        'B2',                           # Dancer
        # BACK - heavy sliders
        'D4',                           # Chariot
        'D5',                           # Queen
        'C2', 'C2',                     # 2 Dragoons
    ]

    white_positions = [
        (-2, 4),   # King
        (0, 2), (-1, 2), (1, 2),        # 3 Pawns
        (-2, 2),                        # Ghost (flank scout)
        (2, 2),                         # Phoenix (other flank)
        (-1, 4),                        # Warper (back, safe)
        (0, 3), (1, 3),                 # 2 Bishops
        (-1, 3),                        # Ranger
    ]

    black_positions = [
        (2, -4),   # King
        (0, -2), (1, -2), (-1, -2),     # 3 Guards
        (2, -2),                        # Shifter
        (-2, -2),                       # Dancer
        (1, -4),                        # Chariot (back)
        (0, -3),                        # Queen
        (1, -3), (-1, -3),              # 2 Dragoons
    ]

    return RuleSet(
        white_pieces=white_pieces,
        black_pieces=black_pieces,
        white_template='E',
        black_template='E',
        white_king='K5',   # Pike king - forward slider (aggressive)
        black_king='K1',   # Guard king - defensive
        white_positions=[tuple(p) for p in white_positions],
        black_positions=[tuple(p) for p in black_positions],
    )


# Registry of available seeds
SEED_CONFIGS = {
    'chess-like': create_chess_like_seed,
    'defensive': create_defensive_seed,
    'aggressive': create_aggressive_seed,
    'special': create_special_seed,
    # Asymmetric seeds (no piece overlap, defensive king positions)
    'asym-classic': create_asymmetric_classic_seed,
    'asym-mobile': create_asymmetric_mobile_seed,
    'asym-special': create_asymmetric_special_seed,
}


def get_seed(name: str) -> RuleSet:
    """Get a seed configuration by name."""
    if name not in SEED_CONFIGS:
        raise ValueError(f"Unknown seed: {name}. Available: {list(SEED_CONFIGS.keys())}")
    return SEED_CONFIGS[name]()


def list_seeds() -> list[str]:
    """List available seed names."""
    return list(SEED_CONFIGS.keys())
