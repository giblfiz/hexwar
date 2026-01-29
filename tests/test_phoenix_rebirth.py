"""Tests for Phoenix rebirth mechanics."""

import pytest
from hexwar.game import (
    GameState, Move,
    generate_legal_actions, generate_phoenix_rebirth,
    apply_move,
)
from hexwar.pieces import Piece


class TestPhoenixGraveyard:
    """Test that captured Phoenix goes to correct graveyard."""

    def test_captured_piece_goes_to_owner_graveyard(self):
        """When Black captures White's Phoenix, it should go to White's graveyard."""
        # Setup: White Phoenix that can be captured by Black
        white = [
            ('K1', (0, 3), 0),   # White king
            ('P1', (0, 0), 0),   # White Phoenix in center
        ]
        black = [
            ('K1', (0, -3), 3),  # Black king
            ('C1', (0, -1), 3),  # Black Lancer that can capture Phoenix
        ]
        state = GameState.create_initial(white, black, 'E', 'E')

        # Black's turn - move Lancer to capture Phoenix at (0, 0)
        state.current_player = 1  # Black to move

        # Find the capture move
        actions = generate_legal_actions(state)
        capture_move = None
        for action in actions:
            if action.to_pos == (0, 0) and action.action_type == 'MOVE':
                capture_move = action
                break

        assert capture_move is not None, "Black should be able to capture Phoenix"

        # Apply the capture
        new_state = apply_move(state, capture_move)

        # Phoenix should be in WHITE's graveyard (owner), not Black's (capturer)
        assert 'P1' in new_state.graveyards[0], \
            f"Phoenix should be in White's graveyard, got: White={new_state.graveyards[0]}, Black={new_state.graveyards[1]}"
        assert 'P1' not in new_state.graveyards[1], \
            f"Phoenix should NOT be in Black's graveyard, got: Black={new_state.graveyards[1]}"


class TestPhoenixRebirth:
    """Test Phoenix rebirth mechanics."""

    def test_rebirth_available_when_phoenix_in_own_graveyard(self):
        """Rebirth option should appear when Phoenix is in player's graveyard."""
        # Setup: White with Phoenix in graveyard
        white = [('K1', (0, 3), 0)]
        black = [('K1', (0, -3), 3)]
        state = GameState.create_initial(white, black, 'E', 'E')

        # Manually add Phoenix to White's graveyard
        state.graveyards[0].append('P1')
        state.current_player = 0  # White to move

        # Rebirth should be available
        rebirth_moves = list(generate_phoenix_rebirth(state))
        assert len(rebirth_moves) > 0, "Rebirth should be available when Phoenix in own graveyard"

        # Rebirth should place adjacent to king at (0, 3)
        rebirth_positions = {m.to_pos for m in rebirth_moves}
        # King at (0, 3), neighbors are (1, 2), (1, 3), (0, 2), (-1, 3), (-1, 4), (0, 4)
        # Some may be off-board
        assert len(rebirth_positions) > 0, "Should have at least one rebirth position"

    def test_rebirth_not_available_when_phoenix_in_enemy_graveyard(self):
        """Rebirth should NOT appear when Phoenix is in enemy's graveyard."""
        # Setup: White with Phoenix in BLACK's graveyard (wrong!)
        white = [('K1', (0, 3), 0)]
        black = [('K1', (0, -3), 3)]
        state = GameState.create_initial(white, black, 'E', 'E')

        # Put Phoenix in Black's graveyard (simulating bug)
        state.graveyards[1].append('P1')
        state.current_player = 0  # White to move

        # Rebirth should NOT be available
        rebirth_moves = list(generate_phoenix_rebirth(state))
        assert len(rebirth_moves) == 0, "Rebirth should NOT be available when Phoenix in enemy graveyard"

    def test_rebirth_places_phoenix_adjacent_to_king(self):
        """Rebirth should place Phoenix in hex adjacent to own king."""
        white = [('K1', (0, 2), 0)]  # King closer to center
        black = [('K1', (0, -3), 3)]
        state = GameState.create_initial(white, black, 'E', 'E')

        state.graveyards[0].append('P1')
        state.current_player = 0

        rebirth_moves = list(generate_phoenix_rebirth(state))

        # All rebirth positions should be neighbors of (0, 2)
        from hexwar.board import get_valid_neighbors
        king_neighbors = set(get_valid_neighbors(0, 2))

        for move in rebirth_moves:
            assert move.to_pos in king_neighbors, \
                f"Rebirth position {move.to_pos} should be adjacent to king at (0, 2)"

    def test_rebirth_removes_phoenix_from_graveyard(self):
        """After rebirth, Phoenix should be removed from graveyard."""
        white = [('K1', (0, 3), 0)]
        black = [('K1', (0, -3), 3)]
        state = GameState.create_initial(white, black, 'E', 'E')

        state.graveyards[0].append('P1')
        state.current_player = 0

        rebirth_moves = list(generate_phoenix_rebirth(state))
        assert len(rebirth_moves) > 0

        new_state = apply_move(state, rebirth_moves[0])

        assert 'P1' not in new_state.graveyards[0], "Phoenix should be removed from graveyard after rebirth"
        assert rebirth_moves[0].to_pos in new_state.board, "Phoenix should be on board after rebirth"
        assert new_state.board[rebirth_moves[0].to_pos].type_id == 'P1', "Piece on board should be Phoenix"


class TestUnlimitedRebirth:
    """Test that Phoenix can be reborn unlimited times."""

    def test_phoenix_can_rebirth_multiple_times(self):
        """Phoenix can be reborn unlimited times - simulated capture/rebirth cycle."""
        # Setup: White with Phoenix in graveyard (simulating first capture)
        white = [('K1', (0, 3), 0)]
        black = [('K1', (0, -3), 3)]
        state = GameState.create_initial(white, black, 'E', 'E')

        # Simulate 5 capture/rebirth cycles
        for cycle in range(5):
            # Simulate capture: Phoenix goes to owner's graveyard
            state.graveyards[0].append('P1')
            state.current_player = 0
            state.action_index = 0

            # Rebirth should be available
            rebirth_moves = list(generate_phoenix_rebirth(state))
            assert len(rebirth_moves) > 0, f"Rebirth {cycle + 1} should be available"

            # Apply rebirth
            state = apply_move(state, rebirth_moves[0])
            assert 'P1' not in state.graveyards[0], f"Phoenix should be out of graveyard after rebirth {cycle + 1}"
            assert rebirth_moves[0].to_pos in state.board, f"Phoenix should be on board after rebirth {cycle + 1}"

            # Remove Phoenix from board (simulating next capture)
            del state.board[rebirth_moves[0].to_pos]

    def test_full_capture_rebirth_cycle(self):
        """Test a full capture-rebirth-capture-rebirth cycle with actual game moves."""
        # Setup: Phoenix can be captured and reborn
        white = [
            ('K1', (0, 3), 0),   # White king
            ('P1', (0, 0), 0),   # White Phoenix in center
        ]
        black = [
            ('K1', (0, -3), 3),  # Black king
            ('C1', (0, -1), 3),  # Black Lancer facing south, can capture Phoenix
        ]
        state = GameState.create_initial(white, black, 'E', 'E')

        # Black captures Phoenix
        state.current_player = 1
        actions = generate_legal_actions(state)
        capture = next((a for a in actions if a.to_pos == (0, 0) and a.action_type == 'MOVE'), None)
        assert capture is not None, "Black should be able to capture Phoenix at (0,0)"

        state = apply_move(state, capture)

        # Phoenix should be in White's graveyard (not Black's!)
        assert 'P1' in state.graveyards[0], "Phoenix should be in White's graveyard"

        # White can rebirth
        state.current_player = 0
        state.action_index = 0
        rebirth_moves = list(generate_phoenix_rebirth(state))
        assert len(rebirth_moves) > 0, "Rebirth should be available"

        state = apply_move(state, rebirth_moves[0])
        assert 'P1' not in state.graveyards[0], "Phoenix should be removed from graveyard"

        # Phoenix is back on board - verify it can be captured AGAIN
        phoenix_pos = rebirth_moves[0].to_pos
        assert state.board[phoenix_pos].type_id == 'P1', "Phoenix should be on board"

        # Simulate second capture
        del state.board[phoenix_pos]
        state.graveyards[0].append('P1')

        # Second rebirth should work
        state.current_player = 0
        state.action_index = 0
        rebirth_moves_2 = list(generate_phoenix_rebirth(state))
        assert len(rebirth_moves_2) > 0, "Second rebirth should also work - unlimited!"
