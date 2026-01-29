"""
HEXWAR Board Geometry

Axial coordinate system (q, r) with center at (0, 0).
8-edge hexagonal board with 61 total hexes.

Valid coordinates satisfy: |q| <= 7, |r| <= 7, |q + r| <= 7
"""

from typing import Iterator

# Board configuration
# Spec says 61 hexes total - that requires radius 4
# (Spec text mentions "8 hexes per edge" and "|q| <= 7" but those
# give 169 hexes, not 61. Using radius 4 to match the 61 hex count.)
BOARD_RADIUS = 4

# Direction vectors in axial coordinates (delta_q, delta_r)
# Index corresponds to facing: 0=N, 1=NE, 2=SE, 3=S, 4=SW, 5=NW
DIRECTIONS = (
    (0, -1),   # N  (0)
    (1, -1),   # NE (1)
    (1, 0),    # SE (2)
    (0, 1),    # S  (3)
    (-1, 1),   # SW (4)
    (-1, 0),   # NW (5)
)

DIRECTION_NAMES = ('N', 'NE', 'SE', 'S', 'SW', 'NW')

# Relative direction offsets from facing
# e.g., FORWARD = 0 means same as facing
FORWARD = 0
FORWARD_RIGHT = 1
BACK_RIGHT = 2
BACKWARD = 3
BACK_LEFT = 4
FORWARD_LEFT = 5


def is_valid_hex(q: int, r: int) -> bool:
    """Check if (q, r) is a valid hex on the board."""
    return abs(q) <= BOARD_RADIUS and abs(r) <= BOARD_RADIUS and abs(q + r) <= BOARD_RADIUS


def hex_distance(q1: int, r1: int, q2: int, r2: int) -> int:
    """Calculate distance between two hexes in axial coordinates."""
    return (abs(q1 - q2) + abs(r1 - r2) + abs((q1 + r1) - (q2 + r2))) // 2


def distance_to_center(q: int, r: int) -> int:
    """Calculate distance from a hex to the center (0, 0)."""
    return (abs(q) + abs(r) + abs(q + r)) // 2


def get_direction_vector(facing: int, relative: int) -> tuple[int, int]:
    """Get the (dq, dr) vector for a relative direction from a facing.

    Args:
        facing: Absolute facing (0-5)
        relative: Relative direction (FORWARD, FORWARD_RIGHT, etc.)

    Returns:
        (delta_q, delta_r) tuple
    """
    absolute_dir = (facing + relative) % 6
    return DIRECTIONS[absolute_dir]


def get_neighbor(q: int, r: int, direction: int) -> tuple[int, int]:
    """Get the neighboring hex in the given direction (0-5)."""
    dq, dr = DIRECTIONS[direction]
    return (q + dq, r + dr)


def iter_all_hexes() -> Iterator[tuple[int, int]]:
    """Iterate over all valid hexes on the board."""
    for q in range(-BOARD_RADIUS, BOARD_RADIUS + 1):
        for r in range(-BOARD_RADIUS, BOARD_RADIUS + 1):
            if is_valid_hex(q, r):
                yield (q, r)


# Precompute all valid hexes
ALL_HEXES = tuple(iter_all_hexes())
NUM_HEXES = len(ALL_HEXES)  # Should be 61

# Precompute home zones (3 rows from each edge)
# For radius 4: White south edge (r >= 2), Black north edge (r <= -2)
WHITE_HOME_ZONE = frozenset((q, r) for q, r in ALL_HEXES if r >= 2)
BLACK_HOME_ZONE = frozenset((q, r) for q, r in ALL_HEXES if r <= -2)

# Fixed king positions (new placement rules)
WHITE_KING_POS = (-2, 4)
BLACK_KING_POS = (2, -4)

# Excluded wing positions (corners of first 3 rows)
# These hexes cannot have pieces at game start
WHITE_EXCLUDED_WINGS = frozenset({(-4, 3), (-4, 2), (-3, 2), (2, 2), (1, 2), (1, 3)})
BLACK_EXCLUDED_WINGS = frozenset({(4, -3), (4, -2), (3, -2), (-2, -2), (-1, -2), (-1, -3)})

# Legal piece positions (home zone minus wings minus king position)
WHITE_PIECE_ZONE = WHITE_HOME_ZONE - WHITE_EXCLUDED_WINGS - {WHITE_KING_POS}
BLACK_PIECE_ZONE = BLACK_HOME_ZONE - BLACK_EXCLUDED_WINGS - {BLACK_KING_POS}


def get_home_zone(owner: int) -> frozenset[tuple[int, int]]:
    """Get the home zone for a player (0=White, 1=Black)."""
    return WHITE_HOME_ZONE if owner == 0 else BLACK_HOME_ZONE


def get_piece_zone(owner: int) -> frozenset[tuple[int, int]]:
    """Get the legal piece placement zone for a player (0=White, 1=Black)."""
    return WHITE_PIECE_ZONE if owner == 0 else BLACK_PIECE_ZONE


def get_king_pos(owner: int) -> tuple[int, int]:
    """Get the fixed king position for a player (0=White, 1=Black)."""
    return WHITE_KING_POS if owner == 0 else BLACK_KING_POS


# Precompute neighbors for each hex
_NEIGHBOR_CACHE: dict[tuple[int, int], tuple[tuple[int, int] | None, ...]] = {}

def _build_neighbor_cache() -> None:
    """Build the neighbor lookup cache."""
    for q, r in ALL_HEXES:
        neighbors = []
        for d in range(6):
            nq, nr = get_neighbor(q, r, d)
            if is_valid_hex(nq, nr):
                neighbors.append((nq, nr))
            else:
                neighbors.append(None)
        _NEIGHBOR_CACHE[(q, r)] = tuple(neighbors)

_build_neighbor_cache()


def get_neighbors(q: int, r: int) -> tuple[tuple[int, int] | None, ...]:
    """Get all neighbors of a hex. None for invalid neighbors (edge of board).

    Returns a tuple indexed by direction (0-5).
    """
    return _NEIGHBOR_CACHE[(q, r)]


def get_valid_neighbors(q: int, r: int) -> list[tuple[int, int]]:
    """Get only the valid neighbors of a hex."""
    return [n for n in _NEIGHBOR_CACHE[(q, r)] if n is not None]


def opposite_direction(direction: int) -> int:
    """Get the opposite direction (180 degrees)."""
    return (direction + 3) % 6


def hex_to_sector(dq: int, dr: int) -> int:
    """Determine which of the 6 direction sectors a relative hex position is in.

    For a hex at (dq, dr) relative to origin, returns the direction (0-5)
    that best describes which sector it's in.

    Uses angle-based calculation for accuracy. Sectors are 60° wedges centered
    on each direction: N=270°, NE=330°, SE=30°, S=90°, SW=150°, NW=210°.
    """
    import math

    if dq == 0 and dr == 0:
        return 0  # At origin, arbitrary

    # Convert axial to pixel coordinates (pointy-top orientation)
    # x = 3/2 * q, y = sqrt(3)/2 * q + sqrt(3) * r
    x = 1.5 * dq
    y = 0.8660254 * dq + 1.7320508 * dr  # sqrt(3)/2, sqrt(3)

    # Calculate angle in degrees (0-360)
    angle = math.degrees(math.atan2(y, x))
    if angle < 0:
        angle += 360

    # Sector boundaries at 0°, 60°, 120°, 180°, 240°, 300°
    # Sector 2 (SE): 0° to 60°
    # Sector 3 (S): 60° to 120°
    # Sector 4 (SW): 120° to 180°
    # Sector 5 (NW): 180° to 240°
    # Sector 0 (N): 240° to 300°
    # Sector 1 (NE): 300° to 360°
    if angle < 60:
        return 2  # SE
    elif angle < 120:
        return 3  # S
    elif angle < 180:
        return 4  # SW
    elif angle < 240:
        return 5  # NW
    elif angle < 300:
        return 0  # N
    else:
        return 1  # NE


def iter_hex_ring(center_q: int, center_r: int, radius: int) -> Iterator[tuple[int, int]]:
    """Iterate over all hexes at exactly `radius` distance from center.

    Yields hexes in order around the ring, starting from the "south" corner
    and going counter-clockwise.
    """
    if radius == 0:
        yield (center_q, center_r)
        return

    # Start at the SW corner (direction 4 from center, radius steps)
    q = center_q + DIRECTIONS[4][0] * radius
    r = center_r + DIRECTIONS[4][1] * radius

    # Walk around the ring: for each of 6 directions, take `radius` steps
    for direction in range(6):
        for _ in range(radius):
            yield (q, r)
            # Move to next hex in current direction
            dq, dr = DIRECTIONS[direction]
            q, r = q + dq, r + dr


# White faces North (0), Black faces South (3)
def default_facing(owner: int) -> int:
    """Get the default facing for a player's pieces."""
    return 0 if owner == 0 else 3
