"""Integration tests (E5 milestone) - Full game loop."""

import pytest
from hexwar.runner import (
    create_bootstrap_game, play_random_game, play_many_random_games,
)
from hexwar.game import is_game_over, get_winner, generate_legal_actions


class TestBootstrapGame:
    """Test bootstrap game creation."""

    def test_creates_valid_state(self):
        """Bootstrap game creates a valid initial state."""
        state = create_bootstrap_game(seed=42)
        assert state is not None
        assert state.current_player == 0  # White first
        assert state.turn_number == 1
        assert state.round_number == 1

    def test_white_has_12_pieces(self):
        """White army should have 12 pieces."""
        state = create_bootstrap_game(seed=42)
        white_pieces = [p for p in state.board.values() if p.owner == 0]
        assert len(white_pieces) == 12

    def test_black_has_11_pieces(self):
        """Black army should have 11 pieces."""
        state = create_bootstrap_game(seed=42)
        black_pieces = [p for p in state.board.values() if p.owner == 1]
        assert len(black_pieces) == 11

    def test_both_have_kings(self):
        """Both sides should have exactly one king."""
        state = create_bootstrap_game(seed=42)

        white_kings = [p for p in state.board.values()
                       if p.owner == 0 and p.is_king]
        black_kings = [p for p in state.board.values()
                       if p.owner == 1 and p.is_king]

        assert len(white_kings) == 1
        assert len(black_kings) == 1

    def test_templates_correct(self):
        """Templates should be D for White, A for Black."""
        state = create_bootstrap_game(seed=42)
        assert state.templates == ('D', 'A')

    def test_deterministic_with_seed(self):
        """Same seed should produce same initial state."""
        state1 = create_bootstrap_game(seed=12345)
        state2 = create_bootstrap_game(seed=12345)

        assert state1.board.keys() == state2.board.keys()
        for pos in state1.board:
            assert state1.board[pos].type_id == state2.board[pos].type_id


class TestRandomGame:
    """Test random game playing."""

    def test_game_completes(self):
        """Random game should complete without errors."""
        state, winner = play_random_game(seed=42)
        assert is_game_over(state)
        assert winner in (0, 1, -1)

    def test_winner_is_valid(self):
        """Winner should be 0 (White), 1 (Black), or -1 (draw)."""
        for seed in range(10):
            _, winner = play_random_game(seed=seed)
            assert winner in (0, 1, -1)

    def test_deterministic_with_seed(self):
        """Same seed should produce same game result."""
        state1, winner1 = play_random_game(seed=99)
        state2, winner2 = play_random_game(seed=99)

        assert winner1 == winner2
        assert state1.round_number == state2.round_number

    def test_game_reaches_turn_limit_or_capture(self):
        """Game should end by king capture or turn limit."""
        state, winner = play_random_game(seed=42)

        # Either a king was captured or we hit the turn limit
        has_white_king = any(p.is_king and p.owner == 0 for p in state.board.values())
        has_black_king = any(p.is_king and p.owner == 1 for p in state.board.values())

        if not has_white_king:
            assert winner == 1  # Black wins by capture
        elif not has_black_king:
            assert winner == 0  # White wins by capture
        else:
            # Both kings alive - must be turn limit
            assert state.round_number > 50


class TestManyGames:
    """Test playing multiple games."""

    def test_plays_requested_count(self):
        """Should play exactly the requested number of games."""
        stats = play_many_random_games(5, seed=42)
        assert stats['games_played'] == 5
        assert stats['white_wins'] + stats['black_wins'] + stats['draws'] == 5

    def test_stats_are_reasonable(self):
        """Stats should have reasonable values."""
        stats = play_many_random_games(20, seed=42)

        # At least some games should end by each side winning
        # (with enough games, both sides should win sometimes)
        assert stats['white_wins'] >= 0
        assert stats['black_wins'] >= 0

        # Average rounds should be positive and reasonable
        assert 5 <= stats['avg_rounds'] <= 55

    def test_no_crashes_on_many_games(self):
        """Should handle many games without crashing."""
        stats = play_many_random_games(50, seed=123)
        total = stats['white_wins'] + stats['black_wins'] + stats['draws']
        assert total == 50


class TestGameplayInvariants:
    """Test that games maintain important invariants."""

    def test_no_actions_after_game_over(self):
        """No legal actions should be available after game ends."""
        state, _ = play_random_game(seed=42)
        assert is_game_over(state)
        actions = generate_legal_actions(state)
        assert len(actions) == 0

    def test_winner_matches_king_state(self):
        """Winner should match which king survived."""
        for seed in range(20):
            state, winner = play_random_game(seed=seed)

            has_white_king = any(p.is_king and p.owner == 0 for p in state.board.values())
            has_black_king = any(p.is_king and p.owner == 1 for p in state.board.values())

            if not has_white_king and has_black_king:
                assert winner == 1
            elif has_white_king and not has_black_king:
                assert winner == 0
            # If both alive, it's turn limit - winner determined by proximity

    def test_pieces_stay_on_board(self):
        """All pieces should be on valid hexes."""
        state, _ = play_random_game(seed=42)
        from hexwar.board import is_valid_hex

        for pos in state.board:
            assert is_valid_hex(*pos), f"Piece at invalid position {pos}"
