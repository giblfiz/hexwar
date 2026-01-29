"""
HEXWAR AI - Heuristics Configuration

The actual AI search is implemented in Rust (hexwar_core).
This module only provides the Heuristics configuration class.
"""

from __future__ import annotations
from dataclasses import dataclass, field


@dataclass
class Heuristics:
    """Heuristic parameters for evaluation.

    Per-color piece values as specified in the algorithmic spec.
    These are passed to the Rust engine for game evaluation.
    """
    white_piece_values: dict[str, float] = field(default_factory=dict)
    black_piece_values: dict[str, float] = field(default_factory=dict)
    white_center_weight: float = 0.5
    black_center_weight: float = 0.5
    # King-specific center bonus (for proximity win condition)
    white_king_center_weight: float = 1.0
    black_king_center_weight: float = 1.0

    @classmethod
    def create_default(cls) -> Heuristics:
        """Create default heuristics with mobility-based piece values.

        Value = average legal moves from center (0,0) and corner (-2,-2 bad facing).
        Simple and direct: more mobility = higher value.
        """
        base_values = {
            # Step-1
            'A1': 1.0,   # Pawn: avg 1
            'A2': 5.0,   # Guard: avg 5
            'A3': 3.0,   # Scout: avg 3
            'A4': 2.5,   # Crab: avg 2.5
            'A5': 2.0,   # Flanker: avg 2
            # Step-2
            'B1': 2.0,   # Strider: avg 2
            'B2': 4.0,   # Dancer: avg 4
            'B3': 10.0,  # Ranger: avg 10
            'B4': 6.0,   # Hound: avg 6
            # Step-3
            'C1': 3.0,   # Lancer: avg 3
            'C2': 8.5,   # Dragoon: avg 8.5
            'C3': 14.0,  # Courser: avg 14
            # Slide
            'D1': 5.0,   # Pike: avg 5
            'D2': 7.0,   # Rook: avg 7
            'D3': 13.0,  # Bishop: avg 13
            'D4': 13.0,  # Chariot: avg 13
            'D5': 20.0,  # Queen: avg 20
            # Jump
            'E1': 5.0,   # Knight: avg 5
            'E2': 9.5,   # Frog: avg 9.5
            'F1': 6.5,   # Locust: avg 6.5
            'F2': 13.0,  # Cricket: avg 13
            # Special (adjusted for abilities)
            'W1': 8.0,   # Warper: can swap with any friendly (~11 start, ~4 end, avg ~8)
            'W2': 5.0,   # Shifter: avg 5 moves
            'P1': 1.5,   # Phoenix: low value - capturing it just repositions it
            'G1': 2.5,   # Ghost: avg 5 but can't capture (value doesn't matter much - can't be taken)
        }

        return cls(
            white_piece_values=dict(base_values),
            black_piece_values=dict(base_values),
            white_center_weight=0.5,
            black_center_weight=0.5,
            white_king_center_weight=1.0,
            black_king_center_weight=1.0,
        )

    def get_piece_value(self, type_id: str, owner: int) -> float:
        """Get the value of a piece type for a given owner."""
        if type_id.startswith('K'):
            return 100000.0  # Kings are invaluable
        if owner == 0:
            return self.white_piece_values.get(type_id, 1.0)
        else:
            return self.black_piece_values.get(type_id, 1.0)

    def get_center_weight(self, owner: int) -> float:
        """Get the center proximity weight for a player."""
        return self.white_center_weight if owner == 0 else self.black_center_weight

    def get_king_center_weight(self, owner: int) -> float:
        """Get king-specific center weight for proximity win condition."""
        return self.white_king_center_weight if owner == 0 else self.black_king_center_weight
