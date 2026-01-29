"""Tests for template enforcement in evolution.

These tests verify that templates are correctly restricted to E (single action)
to prevent exponential slowdown at D5+ search depths.
"""
import pytest
import json
import random

from hexwar.evolution import (
    create_random_ruleset,
    create_bootstrap_ruleset,
    mutate_ruleset,
    board_set_to_ruleset,
    RuleSet,
)


class TestTemplateEnforcement:
    """Test that templates are correctly restricted to E."""

    def test_create_random_ruleset_uses_template_e(self):
        """create_random_ruleset should always produce template E."""
        rng = random.Random(42)
        for _ in range(100):  # Test 100 random rulesets
            rs = create_random_ruleset(rng)
            assert rs.white_template == 'E', f"Expected white_template='E', got {rs.white_template}"
            assert rs.black_template == 'E', f"Expected black_template='E', got {rs.black_template}"

    def test_create_random_ruleset_with_forced_template(self):
        """create_random_ruleset should respect forced_template."""
        rng = random.Random(42)
        rs = create_random_ruleset(rng, forced_template='E')
        assert rs.white_template == 'E'
        assert rs.black_template == 'E'

    def test_create_bootstrap_ruleset_uses_template_e(self):
        """create_bootstrap_ruleset should use template E for fast performance."""
        rng = random.Random(42)
        rs = create_bootstrap_ruleset(rng)
        assert rs.white_template == 'E', f"Expected white_template='E', got {rs.white_template}"
        assert rs.black_template == 'E', f"Expected black_template='E', got {rs.black_template}"

    def test_mutate_ruleset_does_not_change_template(self):
        """mutate_ruleset should never change templates (they're disabled)."""
        rng = random.Random(42)

        # Create base ruleset with E template
        base = RuleSet(
            white_pieces=['A1', 'A1'],
            black_pieces=['A2', 'A2'],
            white_template='E',
            black_template='E',
            white_king='K1',
            black_king='K1',
            white_positions=[],
            black_positions=[],
        )

        # Mutate many times
        for _ in range(100):
            mutated = mutate_ruleset(base, rng)
            assert mutated.white_template == 'E', f"Template changed from E to {mutated.white_template}"
            assert mutated.black_template == 'E', f"Template changed from E to {mutated.black_template}"

    def test_board_set_to_ruleset_defaults_to_e(self):
        """board_set_to_ruleset should default to E if templates not specified."""
        data = {
            'pieces': [
                {'pieceId': 'K1', 'color': 'white', 'pos': [0, -3]},
                {'pieceId': 'K1', 'color': 'black', 'pos': [0, 3]},
            ]
        }
        rs = board_set_to_ruleset(data)
        assert rs.white_template == 'E'
        assert rs.black_template == 'E'

    def test_board_set_to_ruleset_with_explicit_templates(self):
        """board_set_to_ruleset should use specified templates."""
        data = {
            'pieces': [
                {'pieceId': 'K1', 'color': 'white', 'pos': [0, -3]},
                {'pieceId': 'K1', 'color': 'black', 'pos': [0, 3]},
            ],
            'templates': {
                'white': 'E',
                'black': 'E'
            }
        }
        rs = board_set_to_ruleset(data)
        assert rs.white_template == 'E'
        assert rs.black_template == 'E'

    def test_actual_board_sets_have_template_e(self):
        """All board sets in board_sets/ should have template E."""
        import glob

        board_set_files = glob.glob('board_sets/**/*.json', recursive=True)

        for filepath in board_set_files:
            with open(filepath) as f:
                data = json.load(f)

            # Check explicit templates if present
            if 'templates' in data:
                white_t = data['templates'].get('white', 'E')
                black_t = data['templates'].get('black', 'E')
                assert white_t == 'E', f"{filepath}: white_template={white_t}, expected E"
                assert black_t == 'E', f"{filepath}: black_template={black_t}, expected E"


class TestTemplatePerformance:
    """Test that template E games complete quickly."""

    @pytest.mark.slow
    def test_template_e_d5_completes_quickly(self):
        """A D5 game with template E should complete in under 10 seconds."""
        import time
        from hexwar.evolution import create_game_from_ruleset, create_template_aware_heuristics

        rng = random.Random(42)
        rs = create_random_ruleset(rng)

        # Verify templates are E
        assert rs.white_template == 'E'
        assert rs.black_template == 'E'

        state = create_game_from_ruleset(rs, seed=42)
        heuristics = create_template_aware_heuristics(rs, 1.0)
        h_dict = {
            'white_piece_values': heuristics.white_piece_values,
            'black_piece_values': heuristics.black_piece_values,
            'white_center_weight': heuristics.white_center_weight,
            'black_center_weight': heuristics.black_center_weight,
        }

        # Extract pieces for Rust
        white_pieces = []
        black_pieces = []
        for (q, r), piece in state.board.items():
            entry = (piece.type_id, (q, r), piece.facing)
            if piece.owner == 0:
                white_pieces.append(entry)
            else:
                black_pieces.append(entry)

        # Import Rust engine
        try:
            from hexwar_core.hexwar_core import play_game as rust_play_game
        except ImportError:
            pytest.skip("Rust engine not available")

        start = time.time()
        winner, rounds = rust_play_game(
            white_pieces, black_pieces,
            'E', 'E',  # templates
            5, 5,  # D5 depth
            h_dict,
            max_moves=500,
            max_moves_per_action=15,
            seed=42,
        )
        elapsed = time.time() - start

        # D5 games with template E should complete in under 10 seconds
        assert elapsed < 10.0, f"D5 game took {elapsed:.1f}s, expected < 10s"
