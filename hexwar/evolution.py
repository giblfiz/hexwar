"""
HEXWAR Evolutionary Algorithms

Phase 4: Heuristic evolution (per-color piece values)
Phase 5: Rule set evolution (army compositions)
"""

from __future__ import annotations
from dataclasses import dataclass, field
from typing import Optional
import random
import json
import hashlib
from pathlib import Path

from hexwar.ai import Heuristics
from hexwar.pieces import REGULAR_PIECE_IDS, PIECE_TYPES
from hexwar.tournament import run_matchup, MatchupStats


# ============================================================================
# RULESET NAMING - Human-readable identifiers for tracking evolution
# ============================================================================

# 64 adjectives + 64 nouns = 4096 unique names (enough to track rulesets)
_ADJECTIVES = [
    "red", "blue", "gold", "dark", "pale", "wild", "calm", "bold",
    "swift", "slow", "warm", "cold", "soft", "hard", "deep", "high",
    "iron", "silk", "jade", "ruby", "onyx", "opal", "amber", "coral",
    "quick", "still", "bright", "dim", "fresh", "old", "new", "lost",
    "stone", "glass", "steel", "brass", "copper", "silver", "bronze", "chrome",
    "sharp", "blunt", "keen", "dull", "pure", "mixed", "raw", "fine",
    "north", "south", "east", "west", "inner", "outer", "upper", "lower",
    "first", "last", "prime", "dual", "twin", "lone", "true", "void",
]

_NOUNS = [
    "wolf", "bear", "hawk", "lion", "fox", "owl", "elk", "ram",
    "oak", "pine", "elm", "ash", "fern", "moss", "vine", "root",
    "storm", "flame", "frost", "tide", "wind", "dust", "mist", "haze",
    "crown", "blade", "shield", "helm", "lance", "bow", "staff", "ring",
    "tower", "gate", "wall", "bridge", "path", "road", "trail", "pass",
    "dawn", "dusk", "noon", "night", "moon", "star", "sun", "sky",
    "peak", "vale", "cave", "lake", "river", "shore", "cliff", "ridge",
    "forge", "anvil", "hammer", "arrow", "spear", "axe", "sword", "torch",
]


def ruleset_signature(rs: 'RuleSet') -> str:
    """Generate a unique signature for a ruleset based on army composition.

    Two rulesets with identical pieces (regardless of positions) get the same signature.
    Used for tracking fitness history across generations.
    """
    return (
        rs.white_king + ":" + ",".join(sorted(rs.white_pieces)) + "|" +
        rs.black_king + ":" + ",".join(sorted(rs.black_pieces))
    )


def ruleset_name(rs: 'RuleSet') -> str:
    """Generate a memorable two-word name from a ruleset's composition.

    Hashes the army composition to produce a consistent, human-readable name
    like 'iron-wolf' or 'swift-tower'. Same ruleset always gets same name.
    Uses MD5 for deterministic hashing across Python runs.
    """
    sig = ruleset_signature(rs)
    return signature_to_name(sig)


def signature_to_name(sig: str) -> str:
    """Convert a ruleset signature to a deterministic two-word name.

    Uses MD5 hash for consistency across Python runs (Python's hash() is randomized).
    """
    # MD5 gives deterministic hash across runs
    h = int(hashlib.md5(sig.encode()).hexdigest()[:8], 16)

    adj_idx = (h >> 6) & 0x3F  # bits 6-11 -> adjective (0-63)
    noun_idx = h & 0x3F        # bits 0-5 -> noun (0-63)

    return f"{_ADJECTIVES[adj_idx]}-{_NOUNS[noun_idx]}"


# ============================================================================
# FITNESS TRACKING - UCB-style selection with historical data
# ============================================================================

class FitnessTracker:
    """Tracks historical fitness scores per ruleset configuration.

    Uses Upper Confidence Bound (UCB) style scoring to balance exploitation
    (configs with high mean) vs uncertainty (configs with few evaluations).

    UCB score = mean - c * sqrt(1/n_evals)

    The subtraction (rather than addition) makes us CONSERVATIVE - we penalize
    uncertainty rather than exploring it. Configs must prove themselves.
    """

    def __init__(self, c: float = 0.3, min_evals_for_confidence: int = 8):
        """
        Args:
            c: UCB exploration constant. Higher = more penalty for uncertainty.
               With σ≈0.14, c=0.3 means 1-eval config gets ~0.3 penalty.
            min_evals_for_confidence: Minimum evaluations before we trust a config
                                      enough to declare it the winner.
        """
        self.c = c
        self.min_evals_for_confidence = min_evals_for_confidence
        self.history: dict[str, list[float]] = {}  # sig -> list of fitness scores
        self.rulesets: dict[str, 'RuleSet'] = {}  # sig -> RuleSet object (for recovery)
        self.last_results: dict[str, dict] = {}  # sig -> last full result dict

    def record(self, rs: 'RuleSet', fitness: float, result: dict = None) -> None:
        """Record a fitness evaluation for a ruleset.

        Args:
            rs: The ruleset being evaluated
            fitness: The fitness score
            result: Optional full result dict (matchups, stats, etc.) to cache
        """
        sig = ruleset_signature(rs)
        if sig not in self.history:
            self.history[sig] = []
            self.rulesets[sig] = rs  # Store the RuleSet for later recovery
        self.history[sig].append(fitness)
        if result is not None:
            self.last_results[sig] = result

    def get_last_result(self, rs: 'RuleSet') -> dict | None:
        """Get the last full result dict for a ruleset, if available."""
        sig = ruleset_signature(rs)
        return self.last_results.get(sig)

    def get_ucb_score(self, rs: 'RuleSet', current_fitness: float = None) -> float:
        """Get UCB score for a ruleset.

        If current_fitness is provided and this is a new config, uses that.
        Otherwise uses historical mean with uncertainty penalty.
        """
        sig = ruleset_signature(rs)

        if sig not in self.history:
            # New config - use current fitness with max penalty
            if current_fitness is not None:
                return current_fitness - self.c
            return 0.0  # No data at all

        scores = self.history[sig]
        n = len(scores)
        mean = sum(scores) / n

        # UCB penalty decreases as sqrt(1/n)
        uncertainty_penalty = self.c * (1.0 / n) ** 0.5

        return mean - uncertainty_penalty

    def get_stats(self, rs: 'RuleSet') -> dict:
        """Get statistics for a ruleset."""
        sig = ruleset_signature(rs)
        if sig not in self.history:
            return {'n_evals': 0, 'mean': None, 'min': None, 'max': None}

        scores = self.history[sig]
        return {
            'n_evals': len(scores),
            'mean': sum(scores) / len(scores),
            'min': min(scores),
            'max': max(scores),
        }

    def has_enough_evals(self, rs: 'RuleSet') -> bool:
        """Check if ruleset has enough evaluations to be trusted."""
        sig = ruleset_signature(rs)
        return sig in self.history and len(self.history[sig]) >= self.min_evals_for_confidence

    def get_best_confident(self) -> tuple[str, float] | None:
        """Get the signature with best UCB score among configs with enough evals.

        Returns (signature, ucb_score) or None if no config has enough evals.
        """
        best_sig = None
        best_score = float('-inf')

        for sig, scores in self.history.items():
            if len(scores) >= self.min_evals_for_confidence:
                n = len(scores)
                mean = sum(scores) / n
                ucb = mean - self.c * (1.0 / n) ** 0.5
                if ucb > best_score:
                    best_score = ucb
                    best_sig = sig

        if best_sig is not None:
            return (best_sig, best_score)
        return None


# ============================================================================
# SEED HEURISTICS - Calculated from piece capabilities
# ============================================================================

def _calculate_reachable_squares(piece_type) -> int:
    """Calculate how many squares a piece can reach in one move.

    For STEP: range * num_directions (can step 1 to range in each direction)
    For SLIDE: ~5 * num_directions (average slide distance on hex board)
    For JUMP: num_directions (lands at exactly range distance)
    For NONE: 1 (special pieces like Warper)
    """
    n_dirs = len(piece_type.directions)
    if piece_type.move_type == 'STEP':
        return piece_type.move_range * n_dirs
    elif piece_type.move_type == 'SLIDE':
        # On a radius-4 hex board, average slide is ~5 hexes
        return 5 * n_dirs
    elif piece_type.move_type == 'JUMP':
        return n_dirs  # Lands at exactly one spot per direction
    else:  # NONE (Warper)
        return 1  # Can swap with one piece


def _calculate_max_distance(piece_type) -> int:
    """Calculate maximum move distance for a piece.

    For SLIDE pieces, cap at 8 (board diameter).
    """
    if piece_type.move_type == 'NONE':
        return 1  # Warper special
    if piece_type.move_range == 999:  # INF for slides
        return 8  # Board diameter
    return piece_type.move_range


def _template_direction_multiplier(template: str, n_directions: int) -> float:
    """Calculate how much a template boosts/penalizes directional pieces.

    Template A (Rotate, Move SAME): Can rotate first, so direction doesn't limit you.
        A 1-direction piece effectively has access to all 6 directions.
    Template B (Move, Rotate, Rotate): Move first, but double rotate helps reposition.
    Template C (Move, Move, Rotate): Two moves, aggressive but direction matters initially.
    Template D (Move, Rotate DIFFERENT): Move first, can't rotate that piece - stuck with facing.

    Args:
        template: 'A', 'B', 'C', or 'D'
        n_directions: Number of directions the piece can move (1-6)

    Returns:
        Multiplier to apply to base piece value (1.0 = no change)
    """
    if n_directions >= 6:
        # Omnidirectional pieces (Guard, Frog, Queen, etc.) aren't affected
        return 1.0

    # How "directional" is this piece? 1 direction = very directional, 6 = not at all
    directionality = (6 - n_directions) / 5  # 0.0 to 1.0

    if template == 'A':
        # Rotate first - directional pieces gain huge benefit
        # A Pawn (1 dir) becomes Guard-like, a Pike becomes Queen-like
        boost = 1.0 + directionality * 1.5  # Up to 2.5x for 1-direction pieces
        return boost

    elif template == 'B':
        # Move first, but double rotate helps next turn positioning
        # Slight penalty for very directional pieces
        return 1.0 - directionality * 0.15  # Down to 0.85x

    elif template == 'C':
        # Move-Move-Rotate: direction matters at start, but two moves is powerful
        # Moderate penalty for directional pieces
        return 1.0 - directionality * 0.25  # Down to 0.75x

    elif template == 'D':
        # Move first, rotate different piece - directional pieces are stuck
        # Significant penalty
        return 1.0 - directionality * 0.4  # Down to 0.6x for 1-direction pieces

    return 1.0


def create_template_aware_heuristics(white_template: str, black_template: str) -> Heuristics:
    """Create heuristics that account for how templates affect piece values.

    A Lancer with Template A (rotate-then-move) is nearly a queen.
    A Lancer with Template D (move-then-rotate-different) is just a forward poker.

    This creates "honest" heuristics where the AI correctly values pieces
    based on how useful they actually are with the given template.

    Args:
        white_template: Template letter for White ('A', 'B', 'C', or 'D')
        black_template: Template letter for Black ('A', 'B', 'C', or 'D')

    Returns:
        Heuristics with per-color piece values adjusted for templates
    """
    white_values = {}
    black_values = {}

    for pid in REGULAR_PIECE_IDS:
        pt = PIECE_TYPES[pid]

        # Base value from reachable squares
        base_value = _calculate_reachable_squares(pt)

        # Add bonus for special abilities
        if pt.special == 'SWAP_MOVE':
            base_value += 4  # Warper teleport
        elif pt.special == 'SWAP_ROTATE':
            base_value += 3  # Shifter swap
        elif pt.special == 'RESURRECT':
            base_value += 5  # Phoenix resurrect is very powerful
        elif pt.special == 'PHASED':
            base_value += 3  # Ghost can't be captured

        # Normalize base to reasonable range
        base_value = base_value / 6.0

        # Apply template-specific multipliers
        n_dirs = len(pt.directions) if pt.directions else 6  # Warper has no dirs

        white_mult = _template_direction_multiplier(white_template, n_dirs)
        black_mult = _template_direction_multiplier(black_template, n_dirs)

        white_values[pid] = max(0.5, min(6.0, base_value * white_mult))
        black_values[pid] = max(0.5, min(6.0, base_value * black_mult))

    return Heuristics(
        white_piece_values=white_values,
        black_piece_values=black_values,
        white_center_weight=0.5,
        black_center_weight=0.5,
        white_king_center_weight=1.0,
        black_king_center_weight=1.0,
    )


# NOTE: Heuristic evolution code was removed (Jan 2026).
# We now always use template-aware heuristics computed by create_template_aware_heuristics().
# If heuristic evolution is needed in the future, rebuild from scratch using the
# 4-layer granularity approach documented in CLAUDE.md.


def heuristics_to_genome(h: Heuristics) -> dict:
    """Convert heuristics to a genome dict."""
    return {
        'white_piece_values': dict(h.white_piece_values),
        'black_piece_values': dict(h.black_piece_values),
        'white_center_weight': h.white_center_weight,
        'black_center_weight': h.black_center_weight,
        'white_king_center_weight': getattr(h, 'white_king_center_weight', 1.0),
        'black_king_center_weight': getattr(h, 'black_king_center_weight', 1.0),
    }


def genome_to_heuristics(genome: dict) -> Heuristics:
    """Convert genome dict back to Heuristics."""
    return Heuristics(
        white_piece_values=genome['white_piece_values'],
        black_piece_values=genome['black_piece_values'],
        white_center_weight=genome['white_center_weight'],
        black_center_weight=genome['black_center_weight'],
        white_king_center_weight=genome.get('white_king_center_weight', 1.0),
        black_king_center_weight=genome.get('black_king_center_weight', 1.0),
    )


# ============================================================================
# RULE SET EVOLUTION (Phase 5)
# ============================================================================

@dataclass
class RuleSet:
    """A rule set defining army composition and placement."""
    white_pieces: list[str]  # List of piece type IDs (not including king)
    black_pieces: list[str]
    white_template: str
    black_template: str
    white_king: str
    black_king: str
    # Fixed positions: list of (q, r) tuples, king first, then pieces in order
    # If None, positions are assigned randomly at game creation
    white_positions: list[tuple[int, int]] = None
    black_positions: list[tuple[int, int]] = None
    # Facings: list of integers (0-5), king first, then pieces in order
    # If None, facings default to 0 for white, 3 for black
    white_facings: list[int] = None
    black_facings: list[int] = None


def ruleset_to_genome(rs: RuleSet) -> dict:
    """Convert RuleSet to genome dict for serialization."""
    genome = {
        'white_pieces': list(rs.white_pieces),
        'black_pieces': list(rs.black_pieces),
        'white_template': rs.white_template,
        'black_template': rs.black_template,
        'white_king': rs.white_king,
        'black_king': rs.black_king,
    }
    if rs.white_positions is not None:
        genome['white_positions'] = [list(p) for p in rs.white_positions]
    if rs.black_positions is not None:
        genome['black_positions'] = [list(p) for p in rs.black_positions]
    if rs.white_facings is not None:
        genome['white_facings'] = list(rs.white_facings)
    if rs.black_facings is not None:
        genome['black_facings'] = list(rs.black_facings)
    return genome


def _normalize_position(p) -> tuple[int, int]:
    """Convert a position to (q, r) tuple.

    Handles both:
    - Dict format: {'q': 0, 'r': 3}
    - Tuple/list format: [0, 3] or (0, 3)
    """
    if isinstance(p, dict):
        return (p['q'], p['r'])
    return tuple(p)


def genome_to_ruleset(genome: dict) -> RuleSet:
    """Convert genome dict back to RuleSet."""
    white_pos = None
    black_pos = None
    white_facings = None
    black_facings = None
    if 'white_positions' in genome:
        white_pos = [_normalize_position(p) for p in genome['white_positions']]
    if 'black_positions' in genome:
        black_pos = [_normalize_position(p) for p in genome['black_positions']]
    if 'white_facings' in genome:
        white_facings = list(genome['white_facings'])
    if 'black_facings' in genome:
        black_facings = list(genome['black_facings'])

    return RuleSet(
        white_pieces=list(genome['white_pieces']),
        black_pieces=list(genome['black_pieces']),
        white_template=genome['white_template'],
        black_template=genome['black_template'],
        white_king=genome['white_king'],
        black_king=genome['black_king'],
        white_positions=white_pos,
        black_positions=black_pos,
        white_facings=white_facings,
        black_facings=black_facings,
    )


def _generate_positions(piece_count: int, piece_zone: frozenset, king_pos: tuple[int, int], rng: random.Random) -> list[tuple[int, int]]:
    """Generate random positions for king + pieces.

    King is placed at the fixed king_pos, pieces are randomly placed in piece_zone.
    Returns list of (q, r) positions, king position first.
    """
    available = list(piece_zone)
    rng.shuffle(available)
    # Limit to available positions
    needed = min(piece_count, len(available))
    piece_positions = available[:needed]
    # King position first, then piece positions
    return [king_pos] + piece_positions


def create_random_ruleset(rng: random.Random, forced_template: str = None) -> RuleSet:
    """Create a random rule set with asymmetric armies and fixed positions."""
    from hexwar.pieces import REGULAR_PIECE_IDS, KING_IDS
    from hexwar.board import WHITE_PIECE_ZONE, BLACK_PIECE_ZONE, WHITE_KING_POS, BLACK_KING_POS

    # Random piece counts (8-12 regular pieces per side for richer games)
    white_count = rng.randint(8, 12)
    black_count = rng.randint(8, 12)

    white_pieces = [rng.choice(REGULAR_PIECE_IDS) for _ in range(white_count)]
    black_pieces = [rng.choice(REGULAR_PIECE_IDS) for _ in range(black_count)]

    # Constraint: Warper (W1) and Shifter (W2) not on same team
    if 'W1' in white_pieces and 'W2' in white_pieces:
        white_pieces.remove('W2')
    if 'W1' in black_pieces and 'W2' in black_pieces:
        black_pieces.remove('W2')

    # Handle forced template or random selection
    if forced_template:
        white_template = forced_template
        black_template = forced_template
    else:
        # IMPORTANT: Only template E is viable for D5+ evolution
        # 2-action templates (A, D, F) are 50-100x slower due to tree depth doubling
        # 3-action templates (B, C) are completely impractical
        # Use template E (single MoveOrRotate) for all evolution runs
        white_template = 'E'
        black_template = 'E'

    # Prefer different kings for asymmetry
    king_list = list(KING_IDS)
    white_king = rng.choice(king_list)
    # 80% chance of different king for Black
    if rng.random() < 0.8:
        other_kings = [k for k in king_list if k != white_king]
        black_king = rng.choice(other_kings)
    else:
        black_king = rng.choice(king_list)

    # Generate fixed positions (king at fixed pos, pieces in piece zone)
    white_positions = _generate_positions(len(white_pieces), WHITE_PIECE_ZONE, WHITE_KING_POS, rng)
    black_positions = _generate_positions(len(black_pieces), BLACK_PIECE_ZONE, BLACK_KING_POS, rng)

    return RuleSet(
        white_pieces=white_pieces,
        black_pieces=black_pieces,
        white_template=white_template,
        black_template=black_template,
        white_king=white_king,
        black_king=black_king,
        white_positions=white_positions,
        black_positions=black_positions,
    )


def create_bootstrap_ruleset(rng: random.Random = None) -> RuleSet:
    """Create the bootstrap rule set from the spec with fixed positions."""
    from hexwar.board import WHITE_PIECE_ZONE, BLACK_PIECE_ZONE, WHITE_KING_POS, BLACK_KING_POS

    if rng is None:
        rng = random.Random(42)  # Deterministic default

    white_pieces = ['A1', 'A1', 'A1', 'A1', 'A3', 'A3', 'B1', 'B1', 'C1', 'D2', 'E1']
    black_pieces = ['A2', 'A2', 'A2', 'G1', 'W2', 'P1', 'D3', 'D3', 'D4', 'E2']

    white_positions = _generate_positions(len(white_pieces), WHITE_PIECE_ZONE, WHITE_KING_POS, rng)
    black_positions = _generate_positions(len(black_pieces), BLACK_PIECE_ZONE, BLACK_KING_POS, rng)

    return RuleSet(
        white_pieces=white_pieces,
        black_pieces=black_pieces,
        white_template='E',  # Only E is viable for D5+ (multi-action templates cause exponential slowdown)
        black_template='E',
        white_king='K1',
        black_king='K4',
        white_positions=white_positions,
        black_positions=black_positions,
    )


def mutate_ruleset(rs: RuleSet, rng: random.Random, forced_template: str = None, mutate_black_only: bool = False, mutate_white_only: bool = False) -> RuleSet:
    """Mutate a rule set. One random mutation per call.

    Args:
        rs: RuleSet to mutate
        rng: Random number generator
        forced_template: If set, don't mutate templates
        mutate_black_only: If True, only mutate Black's army (for fixed-white evolution)
        mutate_white_only: If True, only mutate White's army (for fixed-black evolution)
    """
    from hexwar.pieces import REGULAR_PIECE_IDS, KING_IDS
    from hexwar.board import WHITE_PIECE_ZONE, BLACK_PIECE_ZONE

    # Copy lists
    white_pieces = list(rs.white_pieces)
    black_pieces = list(rs.black_pieces)
    white_template = rs.white_template
    black_template = rs.black_template
    white_king = rs.white_king
    black_king = rs.black_king
    white_positions = list(rs.white_positions) if rs.white_positions else None
    black_positions = list(rs.black_positions) if rs.black_positions else None
    white_facings = list(rs.white_facings) if rs.white_facings else None
    black_facings = list(rs.black_facings) if rs.black_facings else None

    # Choose mutation type with weighted probabilities
    # Bias toward MORE pieces (add is 3x more likely than remove)
    # Bias toward duplicating existing pieces (for player learnability)
    if mutate_black_only:
        # Only black mutations when white is fixed
        mutation_weights = {
            'add_black': 2.0,
            'add_copy_black': 2.0,
            'remove_black': 1.0,
            'swap_black': 1.0,
            'swap_existing_black': 2.0,
            'change_black_king': 1.0,
            'shuffle_black_positions': 1.0,
            'swap_two_black_positions': 1.0,
            'rotate_black': 1.0,  # Rotate a piece
        }
        if not forced_template:
            mutation_weights['change_black_template'] = 1.0
    elif mutate_white_only:
        # Only white mutations when black is fixed
        mutation_weights = {
            'add_white': 2.0,
            'add_copy_white': 2.0,
            'remove_white': 1.0,
            'swap_white': 1.0,
            'swap_existing_white': 2.0,
            'change_white_king': 1.0,
            'shuffle_white_positions': 1.0,
            'swap_two_white_positions': 1.0,
            'rotate_white': 1.0,  # Rotate a piece
        }
        if not forced_template:
            mutation_weights['change_white_template'] = 1.0
    else:
        mutation_weights = {
            'add_white': 2.0, 'add_black': 2.0,           # Add random piece
            'add_copy_white': 2.0, 'add_copy_black': 2.0, # Add copy of existing (themed armies)
            'remove_white': 1.0, 'remove_black': 1.0,     # Rarely remove
            'swap_white': 1.0, 'swap_black': 1.0,         # Swap for random piece
            'swap_existing_white': 2.0, 'swap_existing_black': 2.0,  # Swap for existing type
            'change_white_king': 1.0, 'change_black_king': 1.0,
            'shuffle_white_positions': 1.0, 'shuffle_black_positions': 1.0,
            'swap_two_white_positions': 1.0, 'swap_two_black_positions': 1.0,
            'rotate_white': 1.0, 'rotate_black': 1.0,  # Rotate pieces
        }
        if not forced_template:
            mutation_weights['change_white_template'] = 1.0
            mutation_weights['change_black_template'] = 1.0

    mutations = list(mutation_weights.keys())
    weights = [mutation_weights[m] for m in mutations]
    mutation = rng.choices(mutations, weights=weights, k=1)[0]

    # For add mutations: only add piece if there's an available position
    if mutation == 'add_white' and len(white_pieces) < 15:
        if white_positions:
            available = [p for p in WHITE_PIECE_ZONE if p not in white_positions]
            if available:
                white_pieces.append(rng.choice(REGULAR_PIECE_IDS))
                white_positions.append(rng.choice(available))
        else:
            white_pieces.append(rng.choice(REGULAR_PIECE_IDS))
    elif mutation == 'add_black' and len(black_pieces) < 15:
        if black_positions:
            available = [p for p in BLACK_PIECE_ZONE if p not in black_positions]
            if available:
                black_pieces.append(rng.choice(REGULAR_PIECE_IDS))
                black_positions.append(rng.choice(available))
        else:
            black_pieces.append(rng.choice(REGULAR_PIECE_IDS))
    elif mutation == 'add_copy_white' and len(white_pieces) < 15 and white_pieces:
        # Add a copy of an existing piece type (themed armies)
        if white_positions:
            available = [p for p in WHITE_PIECE_ZONE if p not in white_positions]
            if available:
                white_pieces.append(rng.choice(white_pieces))
                white_positions.append(rng.choice(available))
        else:
            white_pieces.append(rng.choice(white_pieces))
    elif mutation == 'add_copy_black' and len(black_pieces) < 15 and black_pieces:
        # Add a copy of an existing piece type (themed armies)
        if black_positions:
            available = [p for p in BLACK_PIECE_ZONE if p not in black_positions]
            if available:
                black_pieces.append(rng.choice(black_pieces))
                black_positions.append(rng.choice(available))
        else:
            black_pieces.append(rng.choice(black_pieces))
    elif mutation == 'remove_white' and len(white_pieces) > 8:
        idx = rng.randrange(len(white_pieces))
        white_pieces.pop(idx)
        if white_positions and len(white_positions) > idx + 1:
            white_positions.pop(idx + 1)  # +1 because king is at index 0
    elif mutation == 'remove_black' and len(black_pieces) > 8:
        idx = rng.randrange(len(black_pieces))
        black_pieces.pop(idx)
        if black_positions and len(black_positions) > idx + 1:
            black_positions.pop(idx + 1)
    elif mutation == 'swap_white' and white_pieces:
        idx = rng.randrange(len(white_pieces))
        white_pieces[idx] = rng.choice(REGULAR_PIECE_IDS)
    elif mutation == 'swap_black' and black_pieces:
        idx = rng.randrange(len(black_pieces))
        black_pieces[idx] = rng.choice(REGULAR_PIECE_IDS)
    elif mutation == 'swap_existing_white' and len(white_pieces) >= 2:
        # Swap a piece for another type already in the army (consolidate themes)
        idx = rng.randrange(len(white_pieces))
        # Pick from existing types (excluding the one we're replacing)
        other_types = [p for i, p in enumerate(white_pieces) if i != idx]
        if other_types:
            white_pieces[idx] = rng.choice(other_types)
    elif mutation == 'swap_existing_black' and len(black_pieces) >= 2:
        # Swap a piece for another type already in the army (consolidate themes)
        idx = rng.randrange(len(black_pieces))
        other_types = [p for i, p in enumerate(black_pieces) if i != idx]
        if other_types:
            black_pieces[idx] = rng.choice(other_types)
    elif mutation == 'change_white_template':
        # Template mutation disabled - only E is viable for D5+ evolution
        # All other templates cause 50-100x slowdown due to multi-action turns
        pass  # Keep current template (E)
    elif mutation == 'change_black_template':
        # Template mutation disabled - only E is viable for D5+ evolution
        pass  # Keep current template (E)
    elif mutation == 'change_white_king':
        white_king = rng.choice(list(KING_IDS))
    elif mutation == 'change_black_king':
        black_king = rng.choice(list(KING_IDS))
    elif mutation == 'shuffle_white_positions' and white_positions and len(white_positions) > 1:
        # Shuffle only piece positions (index 1+), keep king at index 0 fixed
        piece_pos = white_positions[1:]
        rng.shuffle(piece_pos)
        white_positions = [white_positions[0]] + piece_pos
    elif mutation == 'shuffle_black_positions' and black_positions and len(black_positions) > 1:
        # Shuffle only piece positions (index 1+), keep king at index 0 fixed
        piece_pos = black_positions[1:]
        rng.shuffle(piece_pos)
        black_positions = [black_positions[0]] + piece_pos
    elif mutation == 'swap_two_white_positions' and white_positions and len(white_positions) >= 3:
        # Swap two piece positions (indices 1+), don't touch king at index 0
        i, j = rng.sample(range(1, len(white_positions)), 2)
        white_positions[i], white_positions[j] = white_positions[j], white_positions[i]
        # Also swap facings if present
        if white_facings and len(white_facings) >= 3:
            white_facings[i], white_facings[j] = white_facings[j], white_facings[i]
    elif mutation == 'swap_two_black_positions' and black_positions and len(black_positions) >= 3:
        # Swap two piece positions (indices 1+), don't touch king at index 0
        i, j = rng.sample(range(1, len(black_positions)), 2)
        black_positions[i], black_positions[j] = black_positions[j], black_positions[i]
        # Also swap facings if present
        if black_facings and len(black_facings) >= 3:
            black_facings[i], black_facings[j] = black_facings[j], black_facings[i]
    elif mutation == 'rotate_white' and white_facings and len(white_facings) > 1:
        # Rotate a random piece (not king at index 0) by 1-3 steps
        idx = rng.randrange(1, len(white_facings))
        rotation = rng.choice([1, 2, -1, -2])  # CW or CCW by 1-2 steps
        white_facings[idx] = (white_facings[idx] + rotation) % 6
    elif mutation == 'rotate_black' and black_facings and len(black_facings) > 1:
        # Rotate a random piece (not king at index 0) by 1-3 steps
        idx = rng.randrange(1, len(black_facings))
        rotation = rng.choice([1, 2, -1, -2])  # CW or CCW by 1-2 steps
        black_facings[idx] = (black_facings[idx] + rotation) % 6

    # Enforce W1/W2 constraint
    if 'W1' in white_pieces and 'W2' in white_pieces:
        white_pieces.remove('W2')
    if 'W1' in black_pieces and 'W2' in black_pieces:
        black_pieces.remove('W2')

    return RuleSet(
        white_pieces=white_pieces,
        black_pieces=black_pieces,
        white_template=white_template,
        black_template=black_template,
        white_king=white_king,
        black_king=black_king,
        white_positions=white_positions,
        black_positions=black_positions,
        white_facings=white_facings,
        black_facings=black_facings,
    )


# Piece tiers for smart mutation (low value to high value)
PIECE_TIERS = {
    # Tier 0: Pawns (lowest value)
    'A1': 0, 'A3': 0, 'A4': 0, 'A5': 0,
    # Tier 1: Guards and basic steppers
    'A2': 1, 'B1': 1, 'B2': 1,
    # Tier 2: Ranged steppers
    'B3': 2, 'B4': 2, 'C1': 2,
    # Tier 3: Long steppers and short sliders
    'C2': 3, 'C3': 3, 'D1': 3,
    # Tier 4: Jumpers and mid sliders
    'E1': 4, 'E2': 4, 'F1': 4, 'D2': 4,
    # Tier 5: Power sliders and specials
    'D3': 5, 'D4': 5, 'G1': 5, 'P1': 5, 'W1': 5, 'W2': 5,
    # Tier 6: Queen (highest value)
    'D5': 6,
}


def get_pieces_by_tier(tier: int) -> list[str]:
    """Get all piece IDs at a given tier."""
    return [pid for pid, t in PIECE_TIERS.items() if t == tier]


def smart_mutate_ruleset(
    rs: RuleSet,
    white_win_rate: float,
    rng: random.Random,
    forced_template: str = None,
    mutate_black_only: bool = False,
    mutate_white_only: bool = False,
) -> RuleSet:
    """Mutate a ruleset based on color win imbalance.

    Args:
        rs: Ruleset to mutate
        white_win_rate: Fraction of games won by White (0.0-1.0)
        rng: Random number generator
        forced_template: If set, don't mutate templates
        mutate_black_only: If True, only mutate Black's army (for fixed-white evolution)
        mutate_white_only: If True, only mutate White's army (for fixed-black evolution)

    Mutation intensity scales with imbalance:
    - 0.45-0.55: Minor position shuffles
    - 0.35-0.45 or 0.55-0.65: Add/remove pawn-tier piece
    - 0.25-0.35 or 0.65-0.75: Upgrade/downgrade a piece
    - <0.25 or >0.75: Add high-value piece to losing side
    """
    from hexwar.pieces import REGULAR_PIECE_IDS
    from hexwar.board import WHITE_PIECE_ZONE, BLACK_PIECE_ZONE

    # Copy ruleset data
    white_pieces = list(rs.white_pieces)
    black_pieces = list(rs.black_pieces)
    white_template = rs.white_template
    black_template = rs.black_template
    white_king = rs.white_king
    black_king = rs.black_king
    white_positions = list(rs.white_positions) if rs.white_positions else None
    black_positions = list(rs.black_positions) if rs.black_positions else None
    white_facings = list(rs.white_facings) if rs.white_facings else None
    black_facings = list(rs.black_facings) if rs.black_facings else None

    # Determine which side is losing and by how much
    imbalance = abs(white_win_rate - 0.5)
    white_losing = white_win_rate < 0.5

    # =========================================================================
    # FIXED-WHITE MODE: Only mutate black army
    # =========================================================================
    if mutate_black_only:
        # If white is winning (black losing), buff black
        # If white is losing (black winning), nerf black
        needs_buff = not white_losing  # Black needs buff when white is winning

        if imbalance < 0.05:
            # Balanced: small random mutation
            action = rng.choice(['swap_piece', 'add_piece', 'shuffle'])
            if action == 'swap_piece' and black_pieces:
                idx = rng.randrange(len(black_pieces))
                current_tier = PIECE_TIERS.get(black_pieces[idx], 3)
                target_tier = current_tier + rng.choice([-1, 0, 0, 1])
                target_tier = max(0, min(5, target_tier))
                candidates = get_pieces_by_tier(target_tier)
                if candidates:
                    black_pieces[idx] = rng.choice(candidates)
            elif action == 'add_piece' and len(black_pieces) < 15:
                if black_positions:
                    available = [p for p in BLACK_PIECE_ZONE if p not in black_positions]
                    if available:
                        pawn_tier = get_pieces_by_tier(0) + get_pieces_by_tier(1)
                        black_pieces.append(rng.choice(pawn_tier))
                        black_positions.append(rng.choice(available))
            else:  # shuffle + swap
                if black_positions and len(black_positions) >= 3:
                    i, j = rng.sample(range(1, len(black_positions)), 2)
                    black_positions[i], black_positions[j] = black_positions[j], black_positions[i]
                if black_pieces:
                    idx = rng.randrange(len(black_pieces))
                    current_tier = PIECE_TIERS.get(black_pieces[idx], 3)
                    candidates = get_pieces_by_tier(current_tier)
                    if candidates:
                        black_pieces[idx] = rng.choice(candidates)

        elif imbalance < 0.15:
            # Slight imbalance
            if needs_buff and len(black_pieces) < 15:
                # Add pawn to buff black
                if black_positions:
                    available = [p for p in BLACK_PIECE_ZONE if p not in black_positions]
                    if available:
                        pawn_tier = get_pieces_by_tier(0) + get_pieces_by_tier(1)
                        black_pieces.append(rng.choice(pawn_tier))
                        black_positions.append(rng.choice(available))
            elif not needs_buff and len(black_pieces) > 8:
                # Remove pawn to nerf black
                piece_tiers = [(i, PIECE_TIERS.get(p, 3)) for i, p in enumerate(black_pieces)]
                piece_tiers.sort(key=lambda x: x[1])
                idx = piece_tiers[0][0]
                black_pieces.pop(idx)
                if black_positions and len(black_positions) > idx + 1:
                    black_positions.pop(idx + 1)
            else:
                # Shuffle
                if black_positions and len(black_positions) >= 3:
                    i, j = rng.sample(range(1, len(black_positions)), 2)
                    black_positions[i], black_positions[j] = black_positions[j], black_positions[i]

        elif imbalance < 0.25:
            # Moderate imbalance
            if needs_buff and black_pieces:
                # Upgrade a low-tier piece
                piece_tiers = [(i, PIECE_TIERS.get(p, 3)) for i, p in enumerate(black_pieces)]
                piece_tiers.sort(key=lambda x: x[1])
                idx, current_tier = piece_tiers[0]
                new_tier = min(current_tier + rng.randint(1, 2), 5)
                candidates = get_pieces_by_tier(new_tier)
                if candidates:
                    black_pieces[idx] = rng.choice(candidates)
            elif not needs_buff and black_pieces:
                # Downgrade a high-tier piece
                piece_tiers = [(i, PIECE_TIERS.get(p, 3)) for i, p in enumerate(black_pieces)]
                piece_tiers.sort(key=lambda x: -x[1])
                idx, current_tier = piece_tiers[0]
                new_tier = max(current_tier - rng.randint(1, 2), 0)
                candidates = get_pieces_by_tier(new_tier)
                if candidates:
                    black_pieces[idx] = rng.choice(candidates)

        else:
            # Severe imbalance
            if needs_buff:
                # Add high-tier piece or upgrade existing
                can_add = False
                if len(black_pieces) < 15:
                    if black_positions:
                        available = [p for p in BLACK_PIECE_ZONE if p not in black_positions]
                        if available:
                            high_tier = get_pieces_by_tier(5) + get_pieces_by_tier(4)
                            black_pieces.append(rng.choice(high_tier))
                            black_positions.append(rng.choice(available))
                            can_add = True
                if not can_add and black_pieces:
                    piece_tiers = [(i, PIECE_TIERS.get(p, 3)) for i, p in enumerate(black_pieces)]
                    piece_tiers.sort(key=lambda x: x[1])
                    idx, _ = piece_tiers[0]
                    high_tier = get_pieces_by_tier(5) + get_pieces_by_tier(4)
                    black_pieces[idx] = rng.choice(high_tier)
            else:
                # Remove high-tier piece or downgrade existing
                if len(black_pieces) > 8:
                    piece_tiers = [(i, PIECE_TIERS.get(p, 3)) for i, p in enumerate(black_pieces)]
                    piece_tiers.sort(key=lambda x: -x[1])
                    idx = piece_tiers[0][0]
                    black_pieces.pop(idx)
                    if black_positions and len(black_positions) > idx + 1:
                        black_positions.pop(idx + 1)
                elif black_pieces:
                    piece_tiers = [(i, PIECE_TIERS.get(p, 3)) for i, p in enumerate(black_pieces)]
                    piece_tiers.sort(key=lambda x: -x[1])
                    idx, current_tier = piece_tiers[0]
                    low_tier = get_pieces_by_tier(0) + get_pieces_by_tier(1)
                    black_pieces[idx] = rng.choice(low_tier)

        # Enforce W1/W2 constraint for black only
        if 'W1' in black_pieces and 'W2' in black_pieces:
            black_pieces.remove('W2')

        return RuleSet(
            white_pieces=white_pieces,
            black_pieces=black_pieces,
            white_template=white_template,
            black_template=black_template,
            white_king=white_king,
            black_king=black_king,
            white_positions=white_positions,
            black_positions=black_positions,
            white_facings=white_facings,
            black_facings=black_facings,
        )

    # =========================================================================
    # FIXED-BLACK MODE: Only mutate white army
    # =========================================================================
    if mutate_white_only:
        # If white is losing, buff white
        # If white is winning, nerf white
        needs_buff = white_losing  # White needs buff when white is losing

        if imbalance < 0.05:
            # Balanced: small random mutation
            action = rng.choice(['swap_piece', 'add_piece', 'shuffle'])
            if action == 'swap_piece' and white_pieces:
                idx = rng.randrange(len(white_pieces))
                current_tier = PIECE_TIERS.get(white_pieces[idx], 3)
                target_tier = current_tier + rng.choice([-1, 0, 0, 1])
                target_tier = max(0, min(5, target_tier))
                candidates = get_pieces_by_tier(target_tier)
                if candidates:
                    white_pieces[idx] = rng.choice(candidates)
            elif action == 'add_piece' and len(white_pieces) < 15:
                if white_positions:
                    available = [p for p in WHITE_PIECE_ZONE if p not in white_positions]
                    if available:
                        pawn_tier = get_pieces_by_tier(0) + get_pieces_by_tier(1)
                        white_pieces.append(rng.choice(pawn_tier))
                        white_positions.append(rng.choice(available))
            else:  # shuffle + swap
                if white_positions and len(white_positions) >= 3:
                    i, j = rng.sample(range(1, len(white_positions)), 2)
                    white_positions[i], white_positions[j] = white_positions[j], white_positions[i]
                if white_pieces:
                    idx = rng.randrange(len(white_pieces))
                    current_tier = PIECE_TIERS.get(white_pieces[idx], 3)
                    candidates = get_pieces_by_tier(current_tier)
                    if candidates:
                        white_pieces[idx] = rng.choice(candidates)

        elif imbalance < 0.15:
            # Slight imbalance
            if needs_buff and len(white_pieces) < 15:
                # Add pawn to buff white
                if white_positions:
                    available = [p for p in WHITE_PIECE_ZONE if p not in white_positions]
                    if available:
                        pawn_tier = get_pieces_by_tier(0) + get_pieces_by_tier(1)
                        white_pieces.append(rng.choice(pawn_tier))
                        white_positions.append(rng.choice(available))
            elif not needs_buff and len(white_pieces) > 8:
                # Remove pawn to nerf white
                piece_tiers = [(i, PIECE_TIERS.get(p, 3)) for i, p in enumerate(white_pieces)]
                piece_tiers.sort(key=lambda x: x[1])
                idx = piece_tiers[0][0]
                white_pieces.pop(idx)
                if white_positions and len(white_positions) > idx + 1:
                    white_positions.pop(idx + 1)
            else:
                # Shuffle
                if white_positions and len(white_positions) >= 3:
                    i, j = rng.sample(range(1, len(white_positions)), 2)
                    white_positions[i], white_positions[j] = white_positions[j], white_positions[i]

        elif imbalance < 0.25:
            # Moderate imbalance
            if needs_buff and white_pieces:
                # Upgrade a low-tier piece
                piece_tiers = [(i, PIECE_TIERS.get(p, 3)) for i, p in enumerate(white_pieces)]
                piece_tiers.sort(key=lambda x: x[1])
                idx, current_tier = piece_tiers[0]
                new_tier = min(current_tier + rng.randint(1, 2), 5)
                candidates = get_pieces_by_tier(new_tier)
                if candidates:
                    white_pieces[idx] = rng.choice(candidates)
            elif not needs_buff and white_pieces:
                # Downgrade a high-tier piece
                piece_tiers = [(i, PIECE_TIERS.get(p, 3)) for i, p in enumerate(white_pieces)]
                piece_tiers.sort(key=lambda x: -x[1])
                idx, current_tier = piece_tiers[0]
                new_tier = max(current_tier - rng.randint(1, 2), 0)
                candidates = get_pieces_by_tier(new_tier)
                if candidates:
                    white_pieces[idx] = rng.choice(candidates)

        else:
            # Severe imbalance
            if needs_buff:
                # Add high-tier piece or upgrade existing
                can_add = False
                if len(white_pieces) < 15:
                    if white_positions:
                        available = [p for p in WHITE_PIECE_ZONE if p not in white_positions]
                        if available:
                            high_tier = get_pieces_by_tier(5) + get_pieces_by_tier(4)
                            white_pieces.append(rng.choice(high_tier))
                            white_positions.append(rng.choice(available))
                            can_add = True
                if not can_add and white_pieces:
                    piece_tiers = [(i, PIECE_TIERS.get(p, 3)) for i, p in enumerate(white_pieces)]
                    piece_tiers.sort(key=lambda x: x[1])
                    idx, _ = piece_tiers[0]
                    high_tier = get_pieces_by_tier(5) + get_pieces_by_tier(4)
                    white_pieces[idx] = rng.choice(high_tier)
            else:
                # Remove high-tier piece or downgrade existing
                if len(white_pieces) > 8:
                    piece_tiers = [(i, PIECE_TIERS.get(p, 3)) for i, p in enumerate(white_pieces)]
                    piece_tiers.sort(key=lambda x: -x[1])
                    idx = piece_tiers[0][0]
                    white_pieces.pop(idx)
                    if white_positions and len(white_positions) > idx + 1:
                        white_positions.pop(idx + 1)
                elif white_pieces:
                    piece_tiers = [(i, PIECE_TIERS.get(p, 3)) for i, p in enumerate(white_pieces)]
                    piece_tiers.sort(key=lambda x: -x[1])
                    idx, current_tier = piece_tiers[0]
                    low_tier = get_pieces_by_tier(0) + get_pieces_by_tier(1)
                    white_pieces[idx] = rng.choice(low_tier)

        # Enforce W1/W2 constraint for white only
        if 'W1' in white_pieces and 'W2' in white_pieces:
            white_pieces.remove('W2')

        return RuleSet(
            white_pieces=white_pieces,
            black_pieces=black_pieces,
            white_template=white_template,
            black_template=black_template,
            white_king=white_king,
            black_king=black_king,
            white_positions=white_positions,
            black_positions=black_positions,
            white_facings=white_facings,
            black_facings=black_facings,
        )

    # =========================================================================
    # NORMAL MODE: Mutate both armies based on losing/winning
    # =========================================================================
    losing_pieces = white_pieces if white_losing else black_pieces
    winning_pieces = black_pieces if white_losing else white_pieces
    losing_positions = white_positions if white_losing else black_positions
    winning_positions = black_positions if white_losing else white_positions
    losing_piece_zone = WHITE_PIECE_ZONE if white_losing else BLACK_PIECE_ZONE
    winning_piece_zone = BLACK_PIECE_ZONE if white_losing else WHITE_PIECE_ZONE

    if imbalance < 0.05:
        # Very balanced (0.45-0.55): small random mutation to explore
        # Can't just shuffle positions - that doesn't change the signature!
        action = rng.choice(['swap_piece', 'add_piece', 'shuffle'])

        if action == 'swap_piece' and losing_pieces:
            # Swap one piece for a same-tier or adjacent-tier piece
            idx = rng.randrange(len(losing_pieces))
            current_tier = PIECE_TIERS.get(losing_pieces[idx], 3)
            # Pick from current tier or adjacent
            target_tier = current_tier + rng.choice([-1, 0, 0, 1])  # Bias toward same tier
            target_tier = max(0, min(5, target_tier))
            candidates = get_pieces_by_tier(target_tier)
            if candidates:
                losing_pieces[idx] = rng.choice(candidates)
        elif action == 'add_piece' and len(losing_pieces) < 15:
            # Add a low-tier piece (only if position available)
            if losing_positions:
                available = [p for p in losing_piece_zone if p not in losing_positions]
                if available:
                    pawn_tier = get_pieces_by_tier(0) + get_pieces_by_tier(1)
                    losing_pieces.append(rng.choice(pawn_tier))
                    losing_positions.append(rng.choice(available))
            else:
                pawn_tier = get_pieces_by_tier(0) + get_pieces_by_tier(1)
                losing_pieces.append(rng.choice(pawn_tier))
        else:  # shuffle - but also swap a piece to ensure signature changes
            if losing_positions and len(losing_positions) >= 3:
                # Shuffle piece positions only (index 1+), keep king at index 0
                i, j = rng.sample(range(1, len(losing_positions)), 2)
                losing_positions[i], losing_positions[j] = losing_positions[j], losing_positions[i]
            # Also swap one piece to change signature
            if losing_pieces:
                idx = rng.randrange(len(losing_pieces))
                current_tier = PIECE_TIERS.get(losing_pieces[idx], 3)
                candidates = get_pieces_by_tier(current_tier)
                if candidates:
                    losing_pieces[idx] = rng.choice(candidates)

    elif imbalance < 0.15:
        # Slightly imbalanced (0.35-0.45 or 0.55-0.65)
        # Try: add pawn to losing side, or remove pawn from winning side, or shuffle
        action = rng.choice(['add_pawn_losing', 'remove_pawn_winning', 'shuffle'])

        if action == 'add_pawn_losing' and len(losing_pieces) < 15:
            # Only add piece if position available
            if losing_positions:
                available = [p for p in losing_piece_zone if p not in losing_positions]
                if available:
                    pawn_tier = get_pieces_by_tier(0) + get_pieces_by_tier(1)
                    losing_pieces.append(rng.choice(pawn_tier))
                    losing_positions.append(rng.choice(available))
            else:
                pawn_tier = get_pieces_by_tier(0) + get_pieces_by_tier(1)
                losing_pieces.append(rng.choice(pawn_tier))

        elif action == 'remove_pawn_winning' and len(winning_pieces) > 8:
            # Find lowest tier piece to remove
            piece_tiers = [(i, PIECE_TIERS.get(p, 3)) for i, p in enumerate(winning_pieces)]
            piece_tiers.sort(key=lambda x: x[1])
            idx = piece_tiers[0][0]
            winning_pieces.pop(idx)
            if winning_positions and len(winning_positions) > idx + 1:
                winning_positions.pop(idx + 1)

        else:  # shuffle piece positions only (index 1+), keep king at index 0
            if losing_positions and len(losing_positions) >= 3:
                i, j = rng.sample(range(1, len(losing_positions)), 2)
                losing_positions[i], losing_positions[j] = losing_positions[j], losing_positions[i]

    elif imbalance < 0.25:
        # Moderately imbalanced (0.25-0.35 or 0.65-0.75)
        # Upgrade a piece on losing side, or downgrade on winning side
        action = rng.choice(['upgrade_losing', 'downgrade_winning'])

        if action == 'upgrade_losing' and losing_pieces:
            # Find a low-tier piece to upgrade
            piece_tiers = [(i, PIECE_TIERS.get(p, 3)) for i, p in enumerate(losing_pieces)]
            piece_tiers.sort(key=lambda x: x[1])
            idx, current_tier = piece_tiers[0]
            # Upgrade by 1-2 tiers
            new_tier = min(current_tier + rng.randint(1, 2), 5)
            candidates = get_pieces_by_tier(new_tier)
            if candidates:
                losing_pieces[idx] = rng.choice(candidates)

        elif action == 'downgrade_winning' and winning_pieces:
            # Find a high-tier piece to downgrade
            piece_tiers = [(i, PIECE_TIERS.get(p, 3)) for i, p in enumerate(winning_pieces)]
            piece_tiers.sort(key=lambda x: -x[1])  # Highest first
            idx, current_tier = piece_tiers[0]
            # Downgrade by 1-2 tiers
            new_tier = max(current_tier - rng.randint(1, 2), 0)
            candidates = get_pieces_by_tier(new_tier)
            if candidates:
                winning_pieces[idx] = rng.choice(candidates)

    else:
        # Severely imbalanced (>0.75 or <0.25)
        # Add a high-value piece to losing side (only if position available)
        can_add = False
        if len(losing_pieces) < 15:
            if losing_positions:
                available = [p for p in losing_piece_zone if p not in losing_positions]
                if available:
                    high_tier = get_pieces_by_tier(5) + get_pieces_by_tier(4)
                    losing_pieces.append(rng.choice(high_tier))
                    losing_positions.append(rng.choice(available))
                    can_add = True
            else:
                high_tier = get_pieces_by_tier(5) + get_pieces_by_tier(4)
                losing_pieces.append(rng.choice(high_tier))
                can_add = True
        if not can_add:
            # Can't add more, so upgrade existing
            piece_tiers = [(i, PIECE_TIERS.get(p, 3)) for i, p in enumerate(losing_pieces)]
            piece_tiers.sort(key=lambda x: x[1])
            idx, _ = piece_tiers[0]
            high_tier = get_pieces_by_tier(5) + get_pieces_by_tier(4)
            losing_pieces[idx] = rng.choice(high_tier)

    # Reassign back
    if white_losing:
        white_pieces = losing_pieces
        black_pieces = winning_pieces
        white_positions = losing_positions
        black_positions = winning_positions
    else:
        black_pieces = losing_pieces
        white_pieces = winning_pieces
        black_positions = losing_positions
        white_positions = winning_positions

    # Enforce W1/W2 constraint
    if 'W1' in white_pieces and 'W2' in white_pieces:
        white_pieces.remove('W2')
    if 'W1' in black_pieces and 'W2' in black_pieces:
        black_pieces.remove('W2')

    return RuleSet(
        white_pieces=white_pieces,
        black_pieces=black_pieces,
        white_template=white_template,
        black_template=black_template,
        white_king=white_king,
        black_king=black_king,
        white_positions=white_positions,
        black_positions=black_positions,
        white_facings=white_facings,
        black_facings=black_facings,
    )


def load_seed_rulesets(seed_dir: str) -> list[RuleSet]:
    """Load seed rulesets from a directory of JSON files.

    Supports multiple formats:
    1. Champion format: has 'ruleset' key with genome format
    2. Raw genome format: direct genome dict
    3. Board set format: has 'pieces' array with piece positions

    Args:
        seed_dir: Path to directory containing seed JSON files

    Returns:
        List of RuleSet objects
    """
    import json
    from pathlib import Path

    seed_path = Path(seed_dir)
    if not seed_path.exists():
        print(f"Warning: Seed directory {seed_dir} does not exist")
        return []

    rulesets = []
    for f in seed_path.glob('*.json'):
        try:
            with open(f) as fp:
                data = json.load(fp)

            # Handle board set format (has 'pieces' array)
            if 'pieces' in data and isinstance(data['pieces'], list):
                rs = board_set_to_ruleset(data)
                rulesets.append(rs)
            # Handle champion format (has 'ruleset' key) and raw genome format
            elif 'ruleset' in data:
                rs = genome_to_ruleset(data['ruleset'])
                rulesets.append(rs)
            else:
                rs = genome_to_ruleset(data)
                rulesets.append(rs)
        except Exception as e:
            print(f"Warning: Failed to load seed {f}: {e}")

    return rulesets


def board_set_to_ruleset(data: dict) -> RuleSet:
    """Convert a board set format to RuleSet.

    Board set format has:
    - pieces: list of {pieceId, color, pos, facing}
    - templates: {white, black} (optional)
    """
    white_pieces = []
    black_pieces = []
    white_positions = []
    black_positions = []
    white_facings = []
    black_facings = []
    white_king = 'K1'
    black_king = 'K1'
    white_template = data.get('templates', {}).get('white', 'E')
    black_template = data.get('templates', {}).get('black', 'E')

    for piece in data['pieces']:
        pid = piece['pieceId']
        color = piece['color']
        pos = tuple(piece['pos'])
        facing = piece.get('facing', 0 if color == 'white' else 3)

        if pid.startswith('K'):
            # King - position and facing go first in lists
            if color == 'white':
                white_king = pid
                white_positions.insert(0, pos)
                white_facings.insert(0, facing)
            else:
                black_king = pid
                black_positions.insert(0, pos)
                black_facings.insert(0, facing)
        else:
            # Regular piece
            if color == 'white':
                white_pieces.append(pid)
                white_positions.append(pos)
                white_facings.append(facing)
            else:
                black_pieces.append(pid)
                black_positions.append(pos)
                black_facings.append(facing)

    return RuleSet(
        white_pieces=white_pieces,
        black_pieces=black_pieces,
        white_template=white_template,
        black_template=black_template,
        white_king=white_king,
        black_king=black_king,
        white_positions=white_positions if white_positions else None,
        black_positions=black_positions if black_positions else None,
        white_facings=white_facings if white_facings else None,
        black_facings=black_facings if black_facings else None,
    )


def crossover_ruleset(rs1: RuleSet, rs2: RuleSet, rng: random.Random) -> RuleSet:
    """Crossover two rule sets by swapping factions (including positions and facings)."""
    # 50% chance to swap each faction
    if rng.random() < 0.5:
        white_pieces = list(rs1.white_pieces)
        white_template = rs1.white_template
        white_king = rs1.white_king
        white_positions = list(rs1.white_positions) if rs1.white_positions else None
        white_facings = list(rs1.white_facings) if rs1.white_facings else None
    else:
        white_pieces = list(rs2.white_pieces)
        white_template = rs2.white_template
        white_king = rs2.white_king
        white_positions = list(rs2.white_positions) if rs2.white_positions else None
        white_facings = list(rs2.white_facings) if rs2.white_facings else None

    if rng.random() < 0.5:
        black_pieces = list(rs1.black_pieces)
        black_template = rs1.black_template
        black_king = rs1.black_king
        black_positions = list(rs1.black_positions) if rs1.black_positions else None
        black_facings = list(rs1.black_facings) if rs1.black_facings else None
    else:
        black_pieces = list(rs2.black_pieces)
        black_template = rs2.black_template
        black_king = rs2.black_king
        black_positions = list(rs2.black_positions) if rs2.black_positions else None
        black_facings = list(rs2.black_facings) if rs2.black_facings else None

    return RuleSet(
        white_pieces=white_pieces,
        black_pieces=black_pieces,
        white_template=white_template,
        black_template=black_template,
        white_king=white_king,
        black_king=black_king,
        white_positions=white_positions,
        black_positions=black_positions,
        white_facings=white_facings,
        black_facings=black_facings,
    )


def create_game_from_ruleset(rs: RuleSet, seed: Optional[int] = None):
    """Create a GameState from a RuleSet.

    If the ruleset has fixed positions, uses those.
    Otherwise, places pieces randomly following placement rules (fixed king, no wings).
    Uses per-piece facings if available, otherwise defaults to owner's default facing.
    """
    from hexwar.game import GameState
    from hexwar.board import (
        WHITE_PIECE_ZONE, BLACK_PIECE_ZONE,
        WHITE_KING_POS, BLACK_KING_POS,
        default_facing
    )

    rng = random.Random(seed) if seed is not None else random.Random()

    def place_with_fixed_positions(piece_ids, king_id, positions, facings, default_face):
        """Place pieces at fixed positions with optional per-piece facings."""
        placed = []
        # First position is for king
        if positions:
            king_facing = facings[0] if facings and len(facings) > 0 else default_face
            placed.append((king_id, positions[0], king_facing))
        # Remaining positions for pieces
        for i, tid in enumerate(piece_ids):
            if i + 1 < len(positions):
                piece_facing = facings[i + 1] if facings and i + 1 < len(facings) else default_face
                placed.append((tid, positions[i + 1], piece_facing))
        return placed

    def place_pieces_random(piece_ids, king_id, piece_zone, king_pos, default_face):
        """Place pieces randomly in piece zone with fixed king position."""
        positions = list(piece_zone)
        rng.shuffle(positions)

        placed = []
        # King at fixed position with default facing
        placed.append((king_id, king_pos, default_face))

        # Place pieces in shuffled positions with default facing
        for i, tid in enumerate(piece_ids):
            if i < len(positions):
                placed.append((tid, positions[i], default_face))

        return placed

    # Use fixed positions if available, otherwise random with new placement rules
    if rs.white_positions:
        white_placed = place_with_fixed_positions(
            rs.white_pieces, rs.white_king, rs.white_positions,
            rs.white_facings, default_facing(0)
        )
    else:
        white_placed = place_pieces_random(
            rs.white_pieces, rs.white_king, WHITE_PIECE_ZONE, WHITE_KING_POS, default_facing(0)
        )

    if rs.black_positions:
        black_placed = place_with_fixed_positions(
            rs.black_pieces, rs.black_king, rs.black_positions,
            rs.black_facings, default_facing(1)
        )
    else:
        black_placed = place_pieces_random(
            rs.black_pieces, rs.black_king, BLACK_PIECE_ZONE, BLACK_KING_POS, default_facing(1)
        )

    return GameState.create_initial(
        white_placed, black_placed,
        white_template=rs.white_template,
        black_template=rs.black_template,
    )


def evaluate_ruleset_fitness(
    rs: RuleSet,
    heuristics: Heuristics,
    n_games: int = 20,
    depth: int = 2,
    max_moves_per_action: int = 15,
    seed: Optional[int] = None,
    verbose: bool = False,
    n_workers: int = 1,
    log_callback=None,
    ruleset_id: str = None,
    use_template_aware: bool = False,
) -> dict:
    """Evaluate a rule set's fitness via tournament with varied depths.

    Uses the tournament system to test skill gradient (deeper player should win)
    and color fairness (at equal depth, 50/50 balance).

    Args:
        use_template_aware: If True, ignore `heuristics` parameter and create
                           template-aware heuristics based on the ruleset's templates.
                           This makes the AI honestly value pieces based on how
                           useful they are with the given templates.

    Returns dict with:
        - fitness: combined score
        - white_wins, black_wins, draws
        - avg_rounds
        - skill_gradient
        - color_fairness
    """
    from hexwar.tournament import evaluate_ruleset_tournament

    # Use template-aware heuristics if requested
    if use_template_aware:
        heuristics = create_template_aware_heuristics(rs.white_template, rs.black_template)

    # Convert ruleset to genome dict for tournament
    rs_dict = ruleset_to_genome(rs)

    # Use reduced tournament during evolution (faster)
    # Full tournament can be used for final evaluation
    result = evaluate_ruleset_tournament(
        ruleset_dict=rs_dict,
        heuristics=heuristics,
        base_seed=seed if seed is not None else random.randint(0, 2**31),
        n_workers=n_workers,
        max_moves_per_action=max_moves_per_action,
        reduced=True,  # Use reduced matchup set for speed
        log_callback=log_callback,
        ruleset_id=ruleset_id,
        depth=depth,
        games_per_matchup=n_games,
    )

    # Compute derived metrics
    # Game length score: peaks around 20-40 rounds, lower for very short or very long games
    ideal_min, ideal_max = 15.0, 50.0
    if result.avg_rounds < ideal_min:
        game_length_score = result.avg_rounds / ideal_min
    elif result.avg_rounds > ideal_max:
        game_length_score = max(0.5, 1.0 - (result.avg_rounds - ideal_max) / 100.0)
    else:
        game_length_score = 1.0

    # Decisiveness: proportion of games with a clear winner (not draws)
    decisiveness = 1.0 - (result.draws / result.total_games) if result.total_games > 0 else 0.0

    return {
        'fitness': result.fitness,
        'white_wins': result.white_wins,
        'black_wins': result.black_wins,
        'draws': result.draws,
        'total_games': result.total_games,
        'avg_rounds': result.avg_rounds,
        'color_fairness': result.color_fairness,
        'skill_gradient': result.skill_gradient,
        'game_richness': result.game_richness,
        'game_length_score': game_length_score,
        'decisiveness': decisiveness,
        'matchups': result.matchups,
    }


def evolve_rulesets(
    heuristics: Heuristics,
    population_size: int = 10,
    generations: int = 10,
    games_per_eval: int = 10,
    depth: int = 2,
    max_moves_per_action: int = 15,
    seed: Optional[int] = None,
    n_workers: int = 1,
    verbose: bool = True,
    log_dir: Optional[str] = None,
    report_callback=None,
    use_template_aware: bool = False,
    forced_template: str = None,
    n_elites: int = 3,
    clones_per_elite: int = 2,
    mutants_per_elite: int = 1,
    ucb_c: float = 0.3,
    min_evals_for_winner: int = 8,
    seed_rulesets: Optional[list[RuleSet]] = None,
    smart_mutate: bool = False,
    fixed_white: Optional[RuleSet] = None,
    fixed_black: Optional[RuleSet] = None,
    no_cache: bool = False,
) -> tuple[RuleSet, dict]:
    """Evolve rule sets using a genetic algorithm with UCB selection.

    Uses UCB (Upper Confidence Bound) style selection to handle noisy fitness:
    - Tracks all historical fitness evaluations per ruleset config
    - Penalizes configs with few evaluations (uncertainty penalty)
    - Ensures final winner has enough evaluations to be trusted

    Args:
        heuristics: Heuristics to use for AI evaluation (ignored if use_template_aware=True)
        population_size: Number of individuals
        generations: Number of generations
        games_per_eval: Games per fitness evaluation
        depth: Search depth for AI
        max_moves_per_action: Move limit
        seed: Random seed
        n_workers: Parallel workers for fitness eval
        verbose: Print progress
        log_dir: Optional directory for game logs
        report_callback: Optional callback(gen, best_rs, best_result) for per-gen reports
        use_template_aware: If True, use template-aware heuristics per ruleset instead
                           of the provided heuristics. This creates "honest" AI play.
        n_elites: Number of top performers to preserve each generation
        clones_per_elite: Unchanged copies of each elite (for fitness verification)
        mutants_per_elite: Mutated children of each elite (for exploration)
        ucb_c: UCB uncertainty penalty constant (higher = more penalty for few evals)
        min_evals_for_winner: Minimum evaluations required before declaring winner
        seed_rulesets: Optional list of RuleSets to use as initial population
        smart_mutate: If True, use adaptive mutations based on win margins
        fixed_white: If provided, keep White's army fixed and only evolve Black.
                     Pass a RuleSet whose white_pieces, white_king, white_template,
                     and white_positions will be used for all individuals.
        fixed_black: If provided, keep Black's army fixed and only evolve White.
                     Pass a RuleSet whose black_pieces, black_king, black_template,
                     and black_positions will be used for all individuals.

    Returns:
        (best_ruleset, stats_dict)
    """
    from concurrent.futures import ProcessPoolExecutor, as_completed
    from pathlib import Path

    rng = random.Random(seed) if seed is not None else random.Random()

    # Initialize fitness tracker for UCB selection
    tracker = FitnessTracker(c=ucb_c, min_evals_for_confidence=min_evals_for_winner)

    # Set up game logging if log_dir provided
    game_log_file = None
    if log_dir:
        log_path = Path(log_dir)
        log_path.mkdir(parents=True, exist_ok=True)
        game_log_file = open(log_path / 'game_log.txt', 'w')

    def log_game(msg: str):
        if game_log_file:
            game_log_file.write(msg + '\n')
            game_log_file.flush()

    # Helper to apply fixed white army to a ruleset
    def apply_fixed_white(rs: RuleSet) -> RuleSet:
        if fixed_white is None:
            return rs
        return RuleSet(
            white_pieces=list(fixed_white.white_pieces),
            black_pieces=rs.black_pieces,
            white_template=fixed_white.white_template,
            black_template=rs.black_template,
            white_king=fixed_white.white_king,
            black_king=rs.black_king,
            white_positions=list(fixed_white.white_positions) if fixed_white.white_positions else None,
            black_positions=rs.black_positions,
            white_facings=list(fixed_white.white_facings) if fixed_white.white_facings else None,
            black_facings=rs.black_facings,
        )

    # Helper to apply fixed black army to a ruleset
    def apply_fixed_black(rs: RuleSet) -> RuleSet:
        if fixed_black is None:
            return rs
        return RuleSet(
            white_pieces=rs.white_pieces,
            black_pieces=list(fixed_black.black_pieces),
            white_template=rs.white_template,
            black_template=fixed_black.black_template,
            white_king=rs.white_king,
            black_king=fixed_black.black_king,
            white_positions=rs.white_positions,
            black_positions=list(fixed_black.black_positions) if fixed_black.black_positions else None,
            white_facings=rs.white_facings,
            black_facings=list(fixed_black.black_facings) if fixed_black.black_facings else None,
        )

    # Combined helper to apply any fixed armies
    def apply_fixed_armies(rs: RuleSet) -> RuleSet:
        rs = apply_fixed_white(rs)
        rs = apply_fixed_black(rs)
        return rs

    # Shorthand for mutation modes
    mutate_black_only = fixed_white is not None
    mutate_white_only = fixed_black is not None

    if verbose and fixed_white:
        print(f"  FIXED-WHITE MODE: Only evolving Black army", flush=True)
        print(f"    White: {fixed_white.white_king} + {len(fixed_white.white_pieces)} pieces", flush=True)

    if verbose and fixed_black:
        print(f"  FIXED-BLACK MODE: Only evolving White army", flush=True)
        print(f"    Black: {fixed_black.black_king} + {len(fixed_black.black_pieces)} pieces", flush=True)

    # Initialize population
    population = []

    if seed_rulesets:
        # Use provided seed rulesets as initial population
        for rs in seed_rulesets:
            if forced_template:
                rs = RuleSet(
                    white_pieces=rs.white_pieces,
                    black_pieces=rs.black_pieces,
                    white_template=forced_template,
                    black_template=forced_template,
                    white_king=rs.white_king,
                    black_king=rs.black_king,
                    white_positions=rs.white_positions,
                    black_positions=rs.black_positions,
                )
            rs = apply_fixed_armies(rs)  # Apply fixed white if set
            population.append(rs)
            if len(population) >= population_size:
                break

        if verbose and len(seed_rulesets) > 0:
            print(f"  Loaded {len(population)} seed rulesets", flush=True)

        # Fill remaining with mutations of seeds or random
        while len(population) < population_size:
            if seed_rulesets:
                parent = rng.choice(seed_rulesets)
                child = mutate_ruleset(parent, rng, forced_template, mutate_black_only, mutate_white_only)
                population.append(apply_fixed_armies(child))
            else:
                rs = create_random_ruleset(rng, forced_template)
                population.append(apply_fixed_armies(rs))
    else:
        # Default: bootstrap + random
        bootstrap_rs = create_bootstrap_ruleset(rng)
        if forced_template:
            bootstrap_rs = RuleSet(
                white_pieces=bootstrap_rs.white_pieces,
                black_pieces=bootstrap_rs.black_pieces,
                white_template=forced_template,
                black_template=forced_template,
                white_king=bootstrap_rs.white_king,
                black_king=bootstrap_rs.black_king,
                white_positions=bootstrap_rs.white_positions,
                black_positions=bootstrap_rs.black_positions,
            )
        bootstrap_rs = apply_fixed_armies(bootstrap_rs)
        population.append(bootstrap_rs)
        for _ in range(population_size - 1):
            rs = create_random_ruleset(rng, forced_template)
            population.append(apply_fixed_armies(rs))

    best_ever = None
    best_ever_fitness = float('-inf')
    generation_stats = []

    # Track which configs have been saved as champions (to avoid duplicates)
    saved_champions: set[str] = set()

    # Create champions directory if log_dir provided
    champions_dir = None
    if log_dir:
        champions_dir = Path(log_dir) / 'champions'
        champions_dir.mkdir(parents=True, exist_ok=True)

    for gen in range(generations):
        if verbose:
            print(f"\nGeneration {gen + 1}/{generations}", flush=True)

        # Evaluate fitness
        # Skip evaluation for configs that are already proven (n >= min_evals)
        # This saves compute by not re-evaluating converged configs
        eval_args = []
        eval_indices = []  # Track which population indices need evaluation
        cached_results = {}  # idx -> cached result for proven configs

        for i, rs in enumerate(population):
            stats = tracker.get_stats(rs)
            # When no_cache=True, always re-evaluate (skip caching)
            if not no_cache and stats['n_evals'] >= min_evals_for_winner:
                # Already proven - use cached stats, don't re-evaluate
                # Try to use the stored last result with full data
                last_result = tracker.get_last_result(rs)
                if last_result is not None:
                    cached_results[i] = {**last_result, 'cached': True}
                else:
                    # Fallback if no stored result (shouldn't happen normally)
                    cached_results[i] = {
                        'fitness': stats['mean'],
                        'white_wins': 0,
                        'black_wins': 0,
                        'draws': 0,
                        'total_games': 0,
                        'avg_rounds': 0,
                        'color_fairness': 0,
                        'skill_gradient': 0,
                        'game_richness': 0,
                        'cached': True,
                    }
                if verbose:
                    name = ruleset_name(rs)
                    ucb = tracker.get_ucb_score(rs)
                    print(f"    Ruleset {i+1} [{name}] CACHED UCB={ucb:.3f} (n={stats['n_evals']})", flush=True)
            else:
                eval_seed = rng.randint(0, 2**31)
                ruleset_id = f"G{gen+1}R{i+1}"
                eval_args.append((rs, heuristics, games_per_eval, depth, max_moves_per_action, eval_seed, n_workers, ruleset_id, use_template_aware))
                eval_indices.append(i)

        fitness_results = [None] * len(population)

        # Fill in cached results first
        for idx, result in cached_results.items():
            fitness_results[idx] = result

        # ======================================================================
        # WORKER UTILIZATION: Generate exploratory rulesets to fill idle workers
        # ======================================================================
        # When configs are cached, we have fewer evaluations than workers.
        # Generate novel exploratory mutants to keep all workers busy.
        exploratory_rulesets = []  # List of (rs, eval_args_tuple)
        exploratory_results = []   # Will be filled after evaluation

        n_evals_needed = len(eval_args)
        idle_workers = max(0, n_workers - n_evals_needed) if n_workers > 1 else 0

        if idle_workers > 0 and len(cached_results) > 0:
            # Get proven signatures to avoid generating duplicates
            proven_sigs = set()
            for rs in population:
                stats = tracker.get_stats(rs)
                if stats['n_evals'] >= min_evals_for_winner:
                    proven_sigs.add(ruleset_signature(rs))

            # Also avoid signatures already in eval queue
            pending_sigs = set(ruleset_signature(population[i]) for i in eval_indices)

            # Generate exploratory mutants from elites
            # Use current population's best (by cached UCB or prior knowledge)
            elite_candidates = []
            for i, rs in enumerate(population):
                ucb = tracker.get_ucb_score(rs)
                elite_candidates.append((rs, ucb))
            elite_candidates.sort(key=lambda x: x[1], reverse=True)

            exploratory_count = 0
            max_attempts = idle_workers * 5  # Safety limit
            attempts = 0

            while exploratory_count < idle_workers and attempts < max_attempts:
                attempts += 1
                # Pick a parent from top candidates
                parent = elite_candidates[exploratory_count % len(elite_candidates)][0]

                # Generate a novel mutant
                if smart_mutate:
                    # Use balanced 0.5 for exploratory since we don't have win rate info
                    child = smart_mutate_ruleset(parent, 0.5, rng, forced_template, mutate_black_only, mutate_white_only)
                else:
                    child = mutate_ruleset(parent, rng, forced_template, mutate_black_only, mutate_white_only)
                child = apply_fixed_armies(child)  # Ensure white stays fixed

                child_sig = ruleset_signature(child)

                # Only use if novel (not proven, not already pending)
                if child_sig not in proven_sigs and child_sig not in pending_sigs:
                    eval_seed = rng.randint(0, 2**31)
                    ruleset_id = f"G{gen+1}X{exploratory_count+1}"  # X = exploratory
                    args = (child, heuristics, games_per_eval, depth, max_moves_per_action,
                            eval_seed, n_workers, ruleset_id, use_template_aware)
                    exploratory_rulesets.append((child, args))
                    pending_sigs.add(child_sig)
                    exploratory_count += 1

            if verbose and exploratory_count > 0:
                print(f"  Generated {exploratory_count} exploratory rulesets to fill idle workers", flush=True)

        if eval_args or exploratory_rulesets:
            if n_workers > 1:
                # Parallel evaluation - include both regular and exploratory rulesets
                total_evals = len(eval_args) + len(exploratory_rulesets)
                if verbose:
                    print(f"  Evaluating {total_evals} rulesets in parallel ({len(cached_results)} cached, {len(exploratory_rulesets)} exploratory)...", flush=True)

                with ProcessPoolExecutor(max_workers=n_workers) as executor:
                    # Submit regular population evals
                    futures = {}
                    for j, args in enumerate(eval_args):
                        future = executor.submit(_eval_ruleset_worker, args)
                        futures[future] = ('pop', eval_indices[j])

                    # Submit exploratory evals
                    for j, (exp_rs, exp_args) in enumerate(exploratory_rulesets):
                        future = executor.submit(_eval_ruleset_worker, exp_args)
                        futures[future] = ('exp', j)

                    for future in as_completed(futures):
                        eval_type, idx = futures[future]
                        result = future.result()

                        if eval_type == 'pop':
                            # Regular population member
                            fitness_results[idx] = result
                            rs = population[idx]
                            tracker.record(rs, result['fitness'], result)
                            if verbose:
                                name = ruleset_name(rs)
                                stats = tracker.get_stats(rs)
                                ucb = tracker.get_ucb_score(rs)
                                print(f"    Ruleset {idx+1}/{len(population)} [{name}] "
                                      f"fitness={result['fitness']:.3f} UCB={ucb:.3f} (n={stats['n_evals']})", flush=True)
                        else:
                            # Exploratory ruleset
                            exp_rs = exploratory_rulesets[idx][0]
                            exploratory_results.append((exp_rs, result))
                            tracker.record(exp_rs, result['fitness'], result)
                            if verbose:
                                name = ruleset_name(exp_rs)
                                stats = tracker.get_stats(exp_rs)
                                ucb = tracker.get_ucb_score(exp_rs)
                                print(f"    Exploratory X{idx+1} [{name}] "
                                      f"fitness={result['fitness']:.3f} UCB={ucb:.3f} (n={stats['n_evals']})", flush=True)
            else:
                # Sequential - regular population
                for j, args in enumerate(eval_args):
                    idx = eval_indices[j]
                    rs = population[idx]
                    name = ruleset_name(rs)
                    if verbose:
                        print(f"  Evaluating ruleset {idx+1}/{len(population)} [{name}]...", flush=True)
                    result = _eval_ruleset_worker(args)
                    fitness_results[idx] = result
                    tracker.record(rs, result['fitness'], result)
                    if verbose:
                        stats = tracker.get_stats(rs)
                        ucb = tracker.get_ucb_score(rs)
                        print(f"    Done: fitness={result['fitness']:.3f} UCB={ucb:.3f} (n={stats['n_evals']})", flush=True)

                # Sequential - exploratory (shouldn't happen with n_workers=1, but be safe)
                for j, (exp_rs, exp_args) in enumerate(exploratory_rulesets):
                    if verbose:
                        name = ruleset_name(exp_rs)
                        print(f"  Evaluating exploratory X{j+1} [{name}]...", flush=True)
                    result = _eval_ruleset_worker(exp_args)
                    exploratory_results.append((exp_rs, result))
                    tracker.record(exp_rs, result['fitness'], result)
                    if verbose:
                        stats = tracker.get_stats(exp_rs)
                        ucb = tracker.get_ucb_score(exp_rs)
                        print(f"    Done: fitness={result['fitness']:.3f} UCB={ucb:.3f} (n={stats['n_evals']})", flush=True)
        elif verbose:
            print(f"  All {len(cached_results)} rulesets cached, no new evaluations needed", flush=True)

        # Sort by UCB score (not raw fitness) for selection
        scored_pop = []
        for rs, result in zip(population, fitness_results):
            ucb_score = tracker.get_ucb_score(rs)
            scored_pop.append((rs, result, ucb_score))

        # Include exploratory results in selection pool
        # This allows promising exploratory configs to become elites
        for exp_rs, exp_result in exploratory_results:
            ucb_score = tracker.get_ucb_score(exp_rs)
            scored_pop.append((exp_rs, exp_result, ucb_score))

        scored_pop.sort(key=lambda x: x[2], reverse=True)  # Sort by UCB score

        best_rs, best_result, best_ucb = scored_pop[0]
        if best_result['fitness'] > best_ever_fitness:
            best_ever = best_rs
            best_ever_fitness = best_result['fitness']

        gen_stat = {
            'generation': gen + 1,
            'best_fitness': best_result['fitness'],
            'best_ucb': best_ucb,
            'best_color_fairness': best_result['color_fairness'],
            'best_skill_gradient': best_result.get('skill_gradient', 0.0),
            'best_white_wins': best_result['white_wins'],
            'best_black_wins': best_result['black_wins'],
            'best_draws': best_result['draws'],
            'total_games': best_result['total_games'],
            'avg_rounds': best_result['avg_rounds'],
        }
        generation_stats.append(gen_stat)

        if verbose:
            # Show deduplicated elites (unique configs by signature)
            seen_sigs = set()
            unique_elites = []
            for rs, result, ucb in scored_pop:
                sig = ruleset_signature(rs)
                if sig not in seen_sigs:
                    seen_sigs.add(sig)
                    stats = tracker.get_stats(rs)
                    unique_elites.append((rs, ucb, stats['n_evals']))
                    if len(unique_elites) >= n_elites:
                        break

            # Format: [name] UCB=0.52 n=14 | [name2] UCB=0.35 n=3 | ...
            elite_strs = []
            for rs, ucb, n in unique_elites:
                name = ruleset_name(rs)
                elite_strs.append(f"[{name}] UCB={ucb:.2f} n={n}")

            print(f"  Elites: {' | '.join(elite_strs)}", flush=True)

        # Save new champions (configs that just reached min_evals)
        if champions_dir:
            for rs, result, ucb in scored_pop:
                sig = ruleset_signature(rs)
                stats = tracker.get_stats(rs)
                n_evals = stats['n_evals']

                # If this config just became a champion and hasn't been saved yet
                if n_evals >= min_evals_for_winner and sig not in saved_champions:
                    saved_champions.add(sig)
                    name = ruleset_name(rs)

                    # Save the ruleset
                    champion_data = {
                        'name': name,
                        'signature': sig,
                        'generation_reached': gen + 1,
                        'n_evals': n_evals,
                        'ucb_score': ucb,
                        'mean_fitness': stats['mean'],
                        'min_fitness': stats['min'],
                        'max_fitness': stats['max'],
                        'ruleset': ruleset_to_genome(rs),
                    }

                    champion_file = champions_dir / f'{name}.json'
                    with open(champion_file, 'w') as f:
                        json.dump(champion_data, f, indent=2)

                    if verbose:
                        print(f"    Saved champion: {name} (UCB={ucb:.2f}, n={n_evals})", flush=True)

        # Per-generation report callback
        if report_callback:
            report_callback(gen + 1, best_rs, best_result, heuristics)

        # Selection and reproduction
        # Adaptive allocation: uncertain elites get clones, proven elites get mutants

        # Deduplicate elites by signature - we want N different configs, not N copies of the best
        seen_sigs = set()
        unique_elites = []
        for rs, result, ucb in scored_pop:
            sig = ruleset_signature(rs)
            if sig not in seen_sigs:
                seen_sigs.add(sig)
                unique_elites.append((rs, result, ucb))
                if len(unique_elites) >= n_elites:
                    break

        # Track proven signatures to avoid filling population with them
        proven_sigs = set()
        for rs, _, _ in scored_pop:
            stats = tracker.get_stats(rs)
            if stats['n_evals'] >= min_evals_for_winner:
                proven_sigs.add(ruleset_signature(rs))

        # Helper to generate a novel mutant (not proven)
        def generate_novel_mutant(parent, white_win_rate, max_attempts=10):
            for _ in range(max_attempts):
                if smart_mutate:
                    child = smart_mutate_ruleset(parent, white_win_rate, rng, forced_template, mutate_black_only, mutate_white_only)
                else:
                    child = mutate_ruleset(parent, rng, forced_template, mutate_black_only, mutate_white_only)
                child = apply_fixed_armies(child)  # Ensure white stays fixed
                if ruleset_signature(child) not in proven_sigs:
                    return child
            # Fallback: use regular mutation which is more aggressive
            child = mutate_ruleset(parent, rng, forced_template, mutate_black_only, mutate_white_only)
            return apply_fixed_armies(child)

        # Ensure we don't exceed population
        effective_elites = min(len(unique_elites), population_size // 3)

        next_gen = []
        next_gen_sigs = set()  # Track what's already in next_gen

        # Each elite gets adaptive allocation based on how proven it is
        for i in range(effective_elites):
            elite, elite_result, _ = unique_elites[i]  # (rs, result, ucb_score)
            elite_stats = tracker.get_stats(elite)
            n_evals = elite_stats['n_evals']
            elite_sig = ruleset_signature(elite)

            # Compute white win rate for smart mutation
            elite_white_win_rate = 0.5  # Default to balanced
            if elite_result['total_games'] > 0:
                elite_white_win_rate = elite_result['white_wins'] / elite_result['total_games']

            # Add the elite itself (always 1) - but only if not already in next_gen
            if elite_sig not in next_gen_sigs:
                next_gen.append(elite)
                next_gen_sigs.add(elite_sig)

            # Adaptive allocation:
            # - Uncertain (n < min_evals): clones to verify, few mutants
            # - Proven (n >= min_evals): no clones (waste of compute), more mutants to explore
            if n_evals < min_evals_for_winner:
                # Uncertain elite: clone to verify
                actual_clones = clones_per_elite
                actual_mutants = mutants_per_elite
            else:
                # Proven elite: don't waste compute re-evaluating, explore instead
                actual_clones = 0
                actual_mutants = clones_per_elite + mutants_per_elite  # Redirect clone budget to mutants

            # Add clones (unchanged copies for fitness verification)
            # Only for unproven elites, and only if sig not already in next_gen
            for _ in range(actual_clones):
                if len(next_gen) >= population_size:
                    break
                if elite_sig not in next_gen_sigs:
                    next_gen.append(elite)
                    next_gen_sigs.add(elite_sig)

            # Add mutants (for exploration) - ensure they're novel
            for _ in range(actual_mutants):
                if len(next_gen) >= population_size:
                    break
                child = generate_novel_mutant(elite, elite_white_win_rate)
                child_sig = ruleset_signature(child)
                # Avoid duplicates in next_gen too
                if child_sig not in next_gen_sigs:
                    next_gen.append(child)
                    next_gen_sigs.add(child_sig)

        # Fill remaining slots with tournament selection + crossover
        # Ensure we generate novel configs (not proven, not duplicates)
        crossover_attempts = 0
        max_crossover_attempts = population_size * 10  # Safety limit

        while len(next_gen) < population_size and crossover_attempts < max_crossover_attempts:
            crossover_attempts += 1

            # Tournament selection - use UCB score (index 2) for selection
            tournament = rng.sample(scored_pop, min(3, len(scored_pop)))
            parent1_entry = max(tournament, key=lambda x: x[2])  # x[2] = ucb_score
            tournament = rng.sample(scored_pop, min(3, len(scored_pop)))
            parent2_entry = max(tournament, key=lambda x: x[2])

            parent1, parent1_result, _ = parent1_entry
            parent2, parent2_result, _ = parent2_entry

            # Crossover
            child = crossover_ruleset(parent1, parent2, rng)

            # Force template if required
            if forced_template and (child.white_template != forced_template or child.black_template != forced_template):
                child = RuleSet(
                    white_pieces=child.white_pieces,
                    black_pieces=child.black_pieces,
                    white_template=forced_template,
                    black_template=forced_template,
                    white_king=child.white_king,
                    black_king=child.black_king,
                    white_positions=child.white_positions,
                    black_positions=child.black_positions,
                )

            # Apply fixed white after crossover
            child = apply_fixed_armies(child)

            # Always mutate crossover children to ensure novelty
            p1_win_rate = parent1_result['white_wins'] / parent1_result['total_games'] if parent1_result['total_games'] > 0 else 0.5
            p2_win_rate = parent2_result['white_wins'] / parent2_result['total_games'] if parent2_result['total_games'] > 0 else 0.5
            avg_win_rate = (p1_win_rate + p2_win_rate) / 2
            child = generate_novel_mutant(child, avg_win_rate)

            child_sig = ruleset_signature(child)

            # Only add if novel (not proven and not already in next_gen)
            if child_sig not in proven_sigs and child_sig not in next_gen_sigs:
                next_gen.append(child)
                next_gen_sigs.add(child_sig)

        # If we still need more (rare), fill with random mutations
        while len(next_gen) < population_size:
            parent = rng.choice(list(unique_elites))[0]
            child = mutate_ruleset(parent, rng, forced_template, mutate_black_only, mutate_white_only)
            child = mutate_ruleset(child, rng, forced_template, mutate_black_only, mutate_white_only)  # Double mutate for more diversity
            child = apply_fixed_armies(child)
            child_sig = ruleset_signature(child)
            if child_sig not in next_gen_sigs:
                next_gen.append(child)
                next_gen_sigs.add(child_sig)

        population = next_gen

    # ==========================================================================
    # FINAL VERIFICATION PHASE
    # Ensure the winner has enough evaluations to be trusted
    # ==========================================================================

    if verbose:
        print(f"\n{'='*60}", flush=True)
        print("FINAL VERIFICATION PHASE", flush=True)
        print(f"{'='*60}", flush=True)

    # Find the best config with enough evaluations
    best_confident = tracker.get_best_confident()

    if best_confident is None:
        # No config has enough evaluations yet - need to run more
        if verbose:
            print(f"  No config has {min_evals_for_winner}+ evaluations yet.", flush=True)
            print(f"  Running additional evaluations on top candidates...", flush=True)

        # Get top candidates by UCB from last generation
        candidates = []
        for rs, result, ucb in scored_pop[:min(5, len(scored_pop))]:
            stats = tracker.get_stats(rs)
            evals_needed = min_evals_for_winner - stats['n_evals']
            if evals_needed > 0:
                candidates.append((rs, evals_needed))

        # Run additional evaluations in parallel
        # Collect all eval tasks
        verify_tasks = []  # List of (rs, name, eval_num, args_tuple)
        for rs, evals_needed in candidates:
            name = ruleset_name(rs)
            if verbose:
                print(f"  Evaluating [{name}] {evals_needed} more times...", flush=True)
            for eval_num in range(evals_needed):
                eval_seed = rng.randint(0, 2**31)
                args = (rs, heuristics, games_per_eval, depth, max_moves_per_action,
                        eval_seed, n_workers, f"VERIFY_{name}_{eval_num+1}", use_template_aware)
                verify_tasks.append((rs, name, eval_num, evals_needed, args))

        if verify_tasks:
            total_verify = len(verify_tasks)
            if verbose:
                print(f"  Running {total_verify} verification evaluations in parallel...", flush=True)

            if n_workers > 1:
                with ProcessPoolExecutor(max_workers=n_workers) as executor:
                    futures = {}
                    for task_idx, (rs, name, eval_num, evals_needed, args) in enumerate(verify_tasks):
                        future = executor.submit(_eval_ruleset_worker, args)
                        futures[future] = (rs, name, eval_num, evals_needed)

                    completed = 0
                    for future in as_completed(futures):
                        rs, name, eval_num, evals_needed = futures[future]
                        result = future.result()
                        tracker.record(rs, result['fitness'], result)
                        completed += 1
                        if verbose:
                            stats = tracker.get_stats(rs)
                            ucb = tracker.get_ucb_score(rs)
                            print(f"    [{name}] eval {eval_num+1}/{evals_needed}: fitness={result['fitness']:.3f} "
                                  f"UCB={ucb:.3f} (n={stats['n_evals']}) [{completed}/{total_verify}]", flush=True)
            else:
                # Sequential fallback
                for task_idx, (rs, name, eval_num, evals_needed, args) in enumerate(verify_tasks):
                    result = _eval_ruleset_worker(args)
                    tracker.record(rs, result['fitness'], result)
                    if verbose:
                        stats = tracker.get_stats(rs)
                        ucb = tracker.get_ucb_score(rs)
                        print(f"    [{name}] eval {eval_num+1}/{evals_needed}: fitness={result['fitness']:.3f} "
                              f"UCB={ucb:.3f} (n={stats['n_evals']})", flush=True)

        # Now find the best confident config
        best_confident = tracker.get_best_confident()

    if best_confident is not None:
        best_sig, best_ucb = best_confident
        # Find the ruleset object that matches this signature
        # First look in the last generation's population
        winner = None
        for rs in population:
            if ruleset_signature(rs) == best_sig:
                winner = rs
                break

        # If not found in population, recover from tracker's stored rulesets
        if winner is None:
            winner = tracker.rulesets.get(best_sig)
            if winner is not None and verbose:
                print(f"  Note: Recovered best config from tracker (was dropped from population)", flush=True)

        # Final fallback to best_ever (shouldn't happen now)
        if winner is None:
            winner = best_ever
            if verbose:
                print(f"  Warning: best confident config not found anywhere, using best_ever", flush=True)

        winner_stats = tracker.get_stats(winner)
        if verbose:
            winner_name = ruleset_name(winner)
            print(f"\n  VERIFIED WINNER: [{winner_name}]", flush=True)
            print(f"    UCB Score: {best_ucb:.3f}", flush=True)
            print(f"    Evaluations: {winner_stats['n_evals']}", flush=True)
            print(f"    Mean fitness: {winner_stats['mean']:.3f}", flush=True)
            print(f"    Min/Max: {winner_stats['min']:.3f} / {winner_stats['max']:.3f}", flush=True)

        best_ever = winner
        best_ever_fitness = winner_stats['mean']
    else:
        if verbose:
            print(f"  Warning: Could not verify winner with {min_evals_for_winner}+ evals", flush=True)

    # Close log file if open
    if game_log_file:
        game_log_file.close()

    # Include tracker stats in output
    return best_ever, {
        'generations': generation_stats,
        'best_fitness': best_ever_fitness,
        'fitness_history': dict(tracker.history),  # All recorded fitness values
        'ucb_c': ucb_c,
        'min_evals_for_winner': min_evals_for_winner,
    }


def _eval_ruleset_worker(args):
    """Worker function for parallel ruleset evaluation."""
    rs, heuristics, n_games, depth, max_moves, seed, n_workers, ruleset_id, use_template_aware = args
    # Note: n_workers here is for WITHIN the tournament evaluation
    # For now, run tournament games sequentially within each worker
    return evaluate_ruleset_fitness(
        rs, heuristics, n_games, depth, max_moves, seed,
        verbose=False, n_workers=1, log_callback=None, ruleset_id=ruleset_id,
        use_template_aware=use_template_aware
    )


# ============================================================================
# MAIN
# ============================================================================

if __name__ == '__main__':
    print("HEXWAR Evolution - Phase 4 Test")
    print("=" * 50)

    print("\nRunning heuristic evolution (small test)...")
    best = evolve_heuristics(
        population_size=5,
        generations=2,
        depth=2,
        games_per_eval=2,
        seed=42,
        max_moves_per_action=8,
        verbose=True,
    )

    print("\nBest evolved heuristics:")
    print(f"  White center weight: {best.white_center_weight:.3f}")
    print(f"  Black center weight: {best.black_center_weight:.3f}")
    print(f"  Sample piece values:")
    for pid in ['A1', 'D5', 'E1']:
        print(f"    {pid}: W={best.white_piece_values[pid]:.2f}, B={best.black_piece_values[pid]:.2f}")

    print("\nPhase 4: Heuristic Evolution complete!")
