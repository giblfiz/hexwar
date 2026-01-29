"""
Game Record Format for HEXWAR

Provides a structured format for recording and replaying games.
Records include initial setup, all moves, and game outcome.

Format supports:
- JSON serialization for storage/transmission
- Step-by-step replay with forward/backward navigation
- Full game state reconstruction at any point
"""

from __future__ import annotations
from dataclasses import dataclass, field, asdict
from typing import Literal
import json
from datetime import datetime

from hexwar.game import GameState, Move, apply_move
from hexwar.evolution import RuleSet, ruleset_to_genome, genome_to_ruleset


@dataclass
class MoveRecord:
    """A single move in the game record."""
    action_type: str  # 'MOVE', 'ROTATE', 'SPECIAL', 'PASS'
    from_pos: tuple[int, int] | None
    to_pos: tuple[int, int] | None
    new_facing: int | None
    special_data: dict | None = None

    @classmethod
    def from_move(cls, move: Move) -> 'MoveRecord':
        """Create a MoveRecord from a Move."""
        return cls(
            action_type=move.action_type,
            from_pos=move.from_pos,
            to_pos=move.to_pos,
            new_facing=move.new_facing,
            special_data=move.special_data,
        )

    def to_move(self) -> Move:
        """Convert back to a Move object."""
        return Move(
            action_type=self.action_type,
            from_pos=self.from_pos,
            to_pos=self.to_pos,
            new_facing=self.new_facing,
            special_data=self.special_data,
        )

    def to_dict(self) -> dict:
        """Convert to dictionary for JSON serialization."""
        d = {
            'action_type': self.action_type,
        }
        if self.from_pos:
            d['from_pos'] = list(self.from_pos)
        if self.to_pos:
            d['to_pos'] = list(self.to_pos)
        if self.new_facing is not None:
            d['new_facing'] = self.new_facing
        if self.special_data:
            d['special_data'] = self.special_data
        return d

    @classmethod
    def from_dict(cls, d: dict) -> 'MoveRecord':
        """Create from dictionary."""
        from_pos = tuple(d['from_pos']) if 'from_pos' in d else None
        to_pos = tuple(d['to_pos']) if 'to_pos' in d else None
        special_data = d.get('special_data')

        # Infer special_data for Rust-recorded games that don't include it
        if d['action_type'] == 'SPECIAL' and special_data is None:
            if from_pos is None and to_pos is not None:
                # Rebirth: Phoenix returns from graveyard to destination
                special_data = {'type': 'REBIRTH', 'dest': to_pos}
            elif from_pos is not None and to_pos is not None:
                # Swap: exchange positions
                special_data = {'type': 'SWAP', 'target': to_pos}

        return cls(
            action_type=d['action_type'],
            from_pos=from_pos,
            to_pos=to_pos,
            new_facing=d.get('new_facing'),
            special_data=special_data,
        )


@dataclass
class GameRecord:
    """Complete record of a HEXWAR game."""

    # Metadata
    recorded_at: str = field(default_factory=lambda: datetime.now().isoformat())
    white_ai_depth: int = 0
    black_ai_depth: int = 0
    seed: int = 0

    # Initial setup (ruleset format)
    ruleset: dict = field(default_factory=dict)

    # Move sequence
    moves: list[MoveRecord] = field(default_factory=list)

    # Outcome
    winner: int | None = None  # 0=White, 1=Black, None=draw/ongoing
    final_round: int = 0
    end_reason: str = ''  # 'king_capture', 'timeout', 'proximity', etc.

    def add_move(self, move: Move) -> None:
        """Record a move."""
        self.moves.append(MoveRecord.from_move(move))

    def get_move(self, index: int) -> MoveRecord:
        """Get move at given index."""
        return self.moves[index]

    def num_moves(self) -> int:
        """Return total number of moves."""
        return len(self.moves)

    def to_dict(self) -> dict:
        """Convert to dictionary for JSON serialization."""
        return {
            'version': 1,  # Format version for future compatibility
            'recorded_at': self.recorded_at,
            'white_ai_depth': self.white_ai_depth,
            'black_ai_depth': self.black_ai_depth,
            'seed': self.seed,
            'ruleset': self.ruleset,
            'moves': [m.to_dict() for m in self.moves],
            'winner': self.winner,
            'final_round': self.final_round,
            'end_reason': self.end_reason,
        }

    def to_json(self, indent: int = 2) -> str:
        """Serialize to JSON string."""
        return json.dumps(self.to_dict(), indent=indent)

    @classmethod
    def from_dict(cls, d: dict) -> 'GameRecord':
        """Create from dictionary."""
        record = cls(
            recorded_at=d.get('recorded_at', ''),
            white_ai_depth=d.get('white_ai_depth', 0),
            black_ai_depth=d.get('black_ai_depth', 0),
            seed=d.get('seed', 0),
            ruleset=d.get('ruleset', {}),
            winner=d.get('winner'),
            final_round=d.get('final_round', 0),
            end_reason=d.get('end_reason', ''),
        )
        record.moves = [MoveRecord.from_dict(m) for m in d.get('moves', [])]
        return record

    @classmethod
    def from_json(cls, json_str: str) -> 'GameRecord':
        """Deserialize from JSON string."""
        return cls.from_dict(json.loads(json_str))

    @classmethod
    def from_file(cls, path: str) -> 'GameRecord':
        """Load from JSON file."""
        with open(path) as f:
            return cls.from_dict(json.load(f))

    def save(self, path: str) -> None:
        """Save to JSON file."""
        with open(path, 'w') as f:
            json.dump(self.to_dict(), f, indent=2)


class GamePlayer:
    """Replays a recorded game step by step."""

    def __init__(self, record: GameRecord):
        """Initialize player with a game record."""
        self.record = record
        self.current_move_index = 0

        # Create initial state from ruleset
        self._initial_state = self._create_initial_state()
        self._current_state = self._initial_state.copy()
        self._state_cache: dict[int, GameState] = {0: self._initial_state.copy()}

    def _create_initial_state(self) -> GameState:
        """Create initial game state from ruleset."""
        from hexwar.evolution import genome_to_ruleset, create_game_from_ruleset

        rs = genome_to_ruleset(self.record.ruleset)
        return create_game_from_ruleset(rs)

    @property
    def state(self) -> GameState:
        """Get current game state."""
        return self._current_state

    @property
    def move_index(self) -> int:
        """Get current move index (0 = initial position)."""
        return self.current_move_index

    @property
    def total_moves(self) -> int:
        """Get total number of moves."""
        return len(self.record.moves)

    @property
    def at_start(self) -> bool:
        """Check if at the start of the game."""
        return self.current_move_index == 0

    @property
    def at_end(self) -> bool:
        """Check if at the end of the game."""
        return self.current_move_index >= len(self.record.moves)

    def reset(self) -> GameState:
        """Reset to initial position."""
        self.current_move_index = 0
        self._current_state = self._initial_state.copy()
        return self._current_state

    def forward(self) -> GameState | None:
        """Move forward one step. Returns new state or None if at end."""
        if self.current_move_index >= len(self.record.moves):
            return None

        move_record = self.record.moves[self.current_move_index]
        move = move_record.to_move()
        self._current_state = apply_move(self._current_state, move)
        self.current_move_index += 1

        # Cache for efficient backward navigation
        if self.current_move_index not in self._state_cache:
            self._state_cache[self.current_move_index] = self._current_state.copy()

        return self._current_state

    def backward(self) -> GameState | None:
        """Move backward one step. Returns new state or None if at start."""
        if self.current_move_index <= 0:
            return None

        self.current_move_index -= 1

        # Use cache if available
        if self.current_move_index in self._state_cache:
            self._current_state = self._state_cache[self.current_move_index].copy()
        else:
            # Rebuild from start (shouldn't happen often due to caching)
            self._current_state = self._initial_state.copy()
            for i in range(self.current_move_index):
                move = self.record.moves[i].to_move()
                self._current_state = apply_move(self._current_state, move)
            self._state_cache[self.current_move_index] = self._current_state.copy()

        return self._current_state

    def goto(self, move_index: int) -> GameState:
        """Jump to a specific move index."""
        if move_index < 0:
            move_index = 0
        elif move_index > len(self.record.moves):
            move_index = len(self.record.moves)

        # Check cache first
        if move_index in self._state_cache:
            self.current_move_index = move_index
            self._current_state = self._state_cache[move_index].copy()
            return self._current_state

        # Find nearest cached state
        cached_indices = sorted(self._state_cache.keys())
        nearest = max([i for i in cached_indices if i <= move_index], default=0)

        self.current_move_index = nearest
        self._current_state = self._state_cache[nearest].copy()

        # Replay from there
        while self.current_move_index < move_index:
            self.forward()

        return self._current_state

    def get_last_move(self) -> MoveRecord | None:
        """Get the move that led to current state."""
        if self.current_move_index <= 0:
            return None
        return self.record.moves[self.current_move_index - 1]

    def get_next_move(self) -> MoveRecord | None:
        """Get the next move to be played."""
        if self.current_move_index >= len(self.record.moves):
            return None
        return self.record.moves[self.current_move_index]


def record_game(
    state: GameState,
    white_depth: int,
    black_depth: int,
    ruleset_dict: dict,
    seed: int = 0,
) -> GameRecord:
    """Create a GameRecord ready for recording moves.

    Call record.add_move(move) after each move during gameplay.
    """
    return GameRecord(
        white_ai_depth=white_depth,
        black_ai_depth=black_depth,
        seed=seed,
        ruleset=ruleset_dict,
    )


def record_ai_game(
    ruleset_dict: dict,
    white_depth: int,
    black_depth: int,
    seed: int = 0,
    max_rounds: int = 50,
    max_moves_per_action: int = 15,
) -> GameRecord:
    """Play an AI game and return a complete GameRecord.

    Uses Rust engine for fast AI vs AI games.
    """
    from hexwar.ai import Heuristics
    from hexwar.evolution import genome_to_ruleset
    from hexwar.visualizer.server import rust_play_game_with_record

    rs = genome_to_ruleset(ruleset_dict)
    heuristics = Heuristics.create_default()

    # Prepare pieces for Rust
    # Positions list: index 0 is king, indices 1+ are pieces
    # Facings list: parallel to positions
    white_facings_list = rs.white_facings or []
    black_facings_list = rs.black_facings or []

    # Build white pieces: king + pieces
    white_pieces = []
    if rs.white_positions:
        # King at position 0
        king_facing = white_facings_list[0] if white_facings_list else 0
        white_pieces.append((rs.white_king, tuple(rs.white_positions[0]), king_facing))
        # Pieces at positions 1+
        for i, piece_id in enumerate(rs.white_pieces):
            pos_idx = i + 1
            if pos_idx < len(rs.white_positions):
                facing = white_facings_list[pos_idx] if pos_idx < len(white_facings_list) else 0
                white_pieces.append((piece_id, tuple(rs.white_positions[pos_idx]), facing))

    # Build black pieces: king + pieces
    black_pieces = []
    if rs.black_positions:
        # King at position 0
        king_facing = black_facings_list[0] if black_facings_list else 3
        black_pieces.append((rs.black_king, tuple(rs.black_positions[0]), king_facing))
        # Pieces at positions 1+
        for i, piece_id in enumerate(rs.black_pieces):
            pos_idx = i + 1
            if pos_idx < len(rs.black_positions):
                facing = black_facings_list[pos_idx] if pos_idx < len(black_facings_list) else 3
                black_pieces.append((piece_id, tuple(rs.black_positions[pos_idx]), facing))

    # Convert heuristics to dict for Rust
    heuristics_dict = {
        'white_piece_values': heuristics.white_piece_values,
        'black_piece_values': heuristics.black_piece_values,
        'white_center_weight': heuristics.white_center_weight,
        'black_center_weight': heuristics.black_center_weight,
    }

    # Play game with Rust
    winner_int, rounds, move_tuples = rust_play_game_with_record(
        white_pieces=white_pieces,
        black_pieces=black_pieces,
        white_template=rs.white_template,
        black_template=rs.black_template,
        white_depth=white_depth,
        black_depth=black_depth,
        heuristics_dict=heuristics_dict,
        max_moves=max_rounds * 6,  # 6 actions per round max
        max_moves_per_action=max_moves_per_action,
        seed=seed,
    )

    # Create record
    record = GameRecord(
        white_ai_depth=white_depth,
        black_ai_depth=black_depth,
        seed=seed,
        ruleset=ruleset_dict,
    )

    # Convert move tuples to MoveRecords
    for action_type, from_pos, to_pos, new_facing in move_tuples:
        record.moves.append(MoveRecord(
            action_type=action_type,
            from_pos=tuple(from_pos) if from_pos else None,
            to_pos=tuple(to_pos) if to_pos else None,
            new_facing=new_facing,
        ))

    record.winner = winner_int if winner_int >= 0 else None
    record.final_round = rounds
    record.end_reason = 'king_capture' if winner_int >= 0 else 'timeout'

    return record
