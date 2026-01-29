"""Tests for hexwar.game_record module."""

import pytest
import json
import tempfile
from pathlib import Path

from hexwar.game_record import (
    MoveRecord, GameRecord, GamePlayer,
    record_game, record_ai_game,
)
from hexwar.game import Move, GameState
from hexwar.evolution import create_random_ruleset, ruleset_to_genome
import random


class TestMoveRecord:
    """Test MoveRecord class."""

    def test_from_move(self):
        """Can create MoveRecord from Move."""
        move = Move('MOVE', (0, 0), (1, 0), 2, None)
        record = MoveRecord.from_move(move)

        assert record.action_type == 'MOVE'
        assert record.from_pos == (0, 0)
        assert record.to_pos == (1, 0)
        assert record.new_facing == 2

    def test_to_move(self):
        """Can convert MoveRecord back to Move."""
        record = MoveRecord('ROTATE', (0, 0), None, 3, None)
        move = record.to_move()

        assert move.action_type == 'ROTATE'
        assert move.from_pos == (0, 0)
        assert move.to_pos is None
        assert move.new_facing == 3

    def test_round_trip(self):
        """Move -> MoveRecord -> Move preserves data."""
        original = Move('SPECIAL', (1, 2), (3, 4), 5, {'type': 'SWAP', 'target': (0, 0)})
        record = MoveRecord.from_move(original)
        restored = record.to_move()

        assert original == restored

    def test_to_dict(self):
        """Can serialize to dict."""
        record = MoveRecord('MOVE', (0, 0), (1, 0), 2, None)
        d = record.to_dict()

        assert d['action_type'] == 'MOVE'
        assert d['from_pos'] == [0, 0]
        assert d['to_pos'] == [1, 0]
        assert d['new_facing'] == 2

    def test_from_dict(self):
        """Can deserialize from dict."""
        d = {
            'action_type': 'PASS',
        }
        record = MoveRecord.from_dict(d)

        assert record.action_type == 'PASS'
        assert record.from_pos is None
        assert record.to_pos is None

    def test_dict_round_trip(self):
        """to_dict -> from_dict preserves data."""
        original = MoveRecord('MOVE', (0, 1), (2, 3), 4, {'key': 'value'})
        d = original.to_dict()
        restored = MoveRecord.from_dict(d)

        assert original.action_type == restored.action_type
        assert original.from_pos == restored.from_pos
        assert original.to_pos == restored.to_pos
        assert original.new_facing == restored.new_facing
        assert original.special_data == restored.special_data


class TestGameRecord:
    """Test GameRecord class."""

    def test_create_empty(self):
        """Can create empty GameRecord."""
        record = GameRecord()
        assert record.num_moves() == 0
        assert record.winner is None

    def test_add_move(self):
        """Can add moves to record."""
        record = GameRecord()
        record.add_move(Move('MOVE', (0, 0), (1, 0), 0, None))
        record.add_move(Move('PASS', None, None, None, None))

        assert record.num_moves() == 2
        assert record.get_move(0).action_type == 'MOVE'
        assert record.get_move(1).action_type == 'PASS'

    def test_to_json(self):
        """Can serialize to JSON."""
        rng = random.Random(42)
        rs = create_random_ruleset(rng)
        ruleset_dict = ruleset_to_genome(rs)

        record = GameRecord(
            white_ai_depth=2,
            black_ai_depth=3,
            seed=42,
            ruleset=ruleset_dict,
            winner=0,
            final_round=25,
            end_reason='king_capture',
        )
        record.add_move(Move('MOVE', (0, 3), (0, 2), 0, None))

        json_str = record.to_json()
        data = json.loads(json_str)

        assert data['version'] == 1
        assert data['white_ai_depth'] == 2
        assert data['black_ai_depth'] == 3
        assert data['winner'] == 0
        assert len(data['moves']) == 1

    def test_from_json(self):
        """Can deserialize from JSON."""
        json_str = '''
        {
            "version": 1,
            "recorded_at": "2026-01-22T12:00:00",
            "white_ai_depth": 4,
            "black_ai_depth": 4,
            "seed": 123,
            "ruleset": {},
            "moves": [
                {"action_type": "MOVE", "from_pos": [0, 3], "to_pos": [0, 2], "new_facing": 0}
            ],
            "winner": 1,
            "final_round": 30,
            "end_reason": "king_capture"
        }
        '''
        record = GameRecord.from_json(json_str)

        assert record.white_ai_depth == 4
        assert record.winner == 1
        assert record.num_moves() == 1
        assert record.get_move(0).from_pos == (0, 3)

    def test_json_round_trip(self):
        """to_json -> from_json preserves data."""
        rng = random.Random(42)
        rs = create_random_ruleset(rng)
        ruleset_dict = ruleset_to_genome(rs)

        original = GameRecord(
            white_ai_depth=5,
            black_ai_depth=5,
            seed=999,
            ruleset=ruleset_dict,
            winner=0,
            final_round=45,
            end_reason='timeout',
        )
        original.add_move(Move('MOVE', (0, 3), (0, 2), 0, None))
        original.add_move(Move('ROTATE', (0, -3), None, 4, None))

        json_str = original.to_json()
        restored = GameRecord.from_json(json_str)

        assert restored.white_ai_depth == original.white_ai_depth
        assert restored.black_ai_depth == original.black_ai_depth
        assert restored.seed == original.seed
        assert restored.winner == original.winner
        assert restored.final_round == original.final_round
        assert restored.end_reason == original.end_reason
        assert restored.num_moves() == original.num_moves()

    def test_save_and_load(self):
        """Can save to file and load back."""
        rng = random.Random(42)
        rs = create_random_ruleset(rng)
        ruleset_dict = ruleset_to_genome(rs)

        original = GameRecord(
            white_ai_depth=2,
            black_ai_depth=2,
            ruleset=ruleset_dict,
        )
        original.add_move(Move('MOVE', (0, 3), (0, 2), 0, None))

        with tempfile.NamedTemporaryFile(mode='w', suffix='.json', delete=False) as f:
            path = f.name

        try:
            original.save(path)
            loaded = GameRecord.from_file(path)

            assert loaded.white_ai_depth == original.white_ai_depth
            assert loaded.num_moves() == original.num_moves()
        finally:
            Path(path).unlink()


class TestGamePlayer:
    """Test GamePlayer class for replay."""

    @pytest.fixture
    def sample_record(self):
        """Create a sample game record for testing."""
        from hexwar.evolution import create_bootstrap_ruleset, ruleset_to_genome, create_game_from_ruleset

        rs = create_bootstrap_ruleset()
        ruleset_dict = ruleset_to_genome(rs)
        state = create_game_from_ruleset(rs)

        record = GameRecord(
            white_ai_depth=2,
            black_ai_depth=2,
            ruleset=ruleset_dict,
        )

        # Add some dummy moves (just PASses to test playback)
        for _ in range(5):
            record.add_move(Move('PASS', None, None, None, None))

        return record

    def test_initial_state(self, sample_record):
        """Player starts at initial position."""
        player = GamePlayer(sample_record)

        assert player.move_index == 0
        assert player.at_start
        assert not player.at_end
        assert player.total_moves == 5

    def test_forward(self, sample_record):
        """Can step forward through game."""
        player = GamePlayer(sample_record)

        player.forward()
        assert player.move_index == 1
        assert not player.at_start

        player.forward()
        assert player.move_index == 2

    def test_forward_at_end(self, sample_record):
        """Forward at end returns None."""
        player = GamePlayer(sample_record)

        for _ in range(5):
            player.forward()

        assert player.at_end
        assert player.forward() is None

    def test_backward(self, sample_record):
        """Can step backward through game."""
        player = GamePlayer(sample_record)

        player.forward()
        player.forward()
        player.backward()

        assert player.move_index == 1

    def test_backward_at_start(self, sample_record):
        """Backward at start returns None."""
        player = GamePlayer(sample_record)

        assert player.at_start
        assert player.backward() is None

    def test_reset(self, sample_record):
        """Can reset to initial position."""
        player = GamePlayer(sample_record)

        player.forward()
        player.forward()
        player.reset()

        assert player.move_index == 0
        assert player.at_start

    def test_goto(self, sample_record):
        """Can jump to specific move index."""
        player = GamePlayer(sample_record)

        player.goto(3)
        assert player.move_index == 3

        player.goto(1)
        assert player.move_index == 1

        player.goto(0)
        assert player.move_index == 0

    def test_goto_clamps(self, sample_record):
        """goto clamps to valid range."""
        player = GamePlayer(sample_record)

        player.goto(-5)
        assert player.move_index == 0

        player.goto(100)
        assert player.move_index == 5

    def test_get_last_move(self, sample_record):
        """Can get last move played."""
        player = GamePlayer(sample_record)

        assert player.get_last_move() is None  # At start

        player.forward()
        last = player.get_last_move()
        assert last is not None
        assert last.action_type == 'PASS'

    def test_get_next_move(self, sample_record):
        """Can get next move to play."""
        player = GamePlayer(sample_record)

        next_move = player.get_next_move()
        assert next_move is not None
        assert next_move.action_type == 'PASS'

        # Go to end
        for _ in range(5):
            player.forward()

        assert player.get_next_move() is None


class TestRecordAIGame:
    """Test AI game recording."""

    def test_records_game(self):
        """record_ai_game produces complete record."""
        rng = random.Random(42)
        rs = create_random_ruleset(rng)
        ruleset_dict = ruleset_to_genome(rs)

        record = record_ai_game(
            ruleset_dict=ruleset_dict,
            white_depth=1,  # Very shallow for speed
            black_depth=1,
            seed=42,
            max_rounds=10,  # Short game
        )

        assert record.num_moves() > 0
        assert record.final_round > 0
        assert record.end_reason in ('king_capture', 'timeout')

    def test_playback_matches_recording(self):
        """Replaying recorded game reaches same end state."""
        rng = random.Random(42)
        rs = create_random_ruleset(rng)
        ruleset_dict = ruleset_to_genome(rs)

        record = record_ai_game(
            ruleset_dict=ruleset_dict,
            white_depth=1,
            black_depth=1,
            seed=42,
            max_rounds=10,
        )

        # Replay the game
        player = GamePlayer(record)
        while not player.at_end:
            player.forward()

        # Should match recorded outcome
        assert player.state.winner == record.winner
