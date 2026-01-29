"""Tests for special abilities (E4 milestone)."""

import pytest
from hexwar.game import (
    GameState, Move,
    generate_legal_actions, generate_special_moves,
    apply_move, is_game_over,
)
from hexwar.pieces import Piece
from hexwar.board import default_facing


class TestWarperSwap:
    """Test Warper's Swap (Move) ability."""

    @pytest.fixture
    def state_with_warper(self):
        """Create state with Warper and other pieces."""
        white = [
            ('K1', (0, 4), 0),   # King
            ('W1', (0, 0), 0),   # Warper at center
            ('A1', (1, 0), 0),   # Pawn nearby
        ]
        black = [('K1', (0, -4), 3)]
        return GameState.create_initial(white, black, 'D', 'A')

    def test_warper_can_swap_with_friendly(self, state_with_warper):
        """Warper should be able to swap with any friendly piece."""
        actions = generate_legal_actions(state_with_warper)
        specials = [a for a in actions if a.action_type == 'SPECIAL']

        # Should have swap options for King and Pawn
        assert len(specials) >= 2

        # Check that swap data is correct
        for s in specials:
            assert s.special_data['type'] == 'SWAP'
            assert s.from_pos == (0, 0)  # Warper position

    def test_warper_swap_exchanges_positions(self, state_with_warper):
        """Applying swap should exchange piece positions."""
        # Find swap with pawn
        actions = generate_legal_actions(state_with_warper)
        swap = [a for a in actions if a.action_type == 'SPECIAL'
                and a.special_data.get('target') == (1, 0)][0]

        new_state = apply_move(state_with_warper, swap)

        # Warper should now be at pawn's old position
        assert new_state.board[(1, 0)].type_id == 'W1'
        # Pawn should be at warper's old position
        assert new_state.board[(0, 0)].type_id == 'A1'

    def test_warper_has_no_normal_movement(self, state_with_warper):
        """Warper should not generate normal MOVE actions."""
        actions = generate_legal_actions(state_with_warper)
        warper_moves = [a for a in actions
                        if a.action_type == 'MOVE' and a.from_pos == (0, 0)]
        assert len(warper_moves) == 0

    def test_warper_cannot_swap_with_enemy(self, state_with_warper):
        """Warper should not be able to swap with enemy pieces."""
        actions = generate_legal_actions(state_with_warper)
        specials = [a for a in actions if a.action_type == 'SPECIAL']

        enemy_king_pos = (0, -4)
        swap_targets = [s.special_data['target'] for s in specials]
        assert enemy_king_pos not in swap_targets


class TestShifterSwap:
    """Test Shifter's Swap (Rotate) ability."""

    @pytest.fixture
    def state_with_shifter(self):
        """Create state with Shifter for template A (rotate first)."""
        white = [
            ('K1', (0, 4), 0),
            ('W2', (0, 0), 0),  # Shifter
            ('A1', (1, 0), 0),
        ]
        black = [('K1', (0, -4), 3)]
        # Template A: Rotate then Move Same
        return GameState.create_initial(white, black, 'A', 'A')

    def test_shifter_can_swap_on_rotate_action(self, state_with_shifter):
        """Shifter should be able to swap during rotate action."""
        # Template A starts with ROTATE
        actions = generate_legal_actions(state_with_shifter)
        specials = [a for a in actions if a.action_type == 'SPECIAL']

        # Should have swap options
        assert len(specials) >= 1

    def test_shifter_also_has_normal_movement(self, state_with_shifter):
        """Shifter should also have normal step-1 movement."""
        # After doing rotate, we get MOVE SAME action
        rotate = Move('ROTATE', (0, 0), None, 0, None)
        state2 = apply_move(state_with_shifter, rotate)

        actions = generate_legal_actions(state2)
        moves = [a for a in actions if a.action_type == 'MOVE' and a.from_pos == (0, 0)]

        # Shifter has step-1 in all directions
        assert len(moves) >= 4  # Depending on board edges


# NOTE: TestPhoenixResurrect was removed - Phoenix now has REBIRTH mechanic
# (Phoenix comes back from graveyard) instead of RESURRECT (Phoenix resurrects other pieces).
# See tests/test_phoenix_rebirth.py for current Phoenix tests.


class TestGhostPhased:
    """Test Ghost's Phased passive ability."""

    @pytest.fixture
    def state_with_ghost(self):
        """Create state with Ghost and enemies."""
        white = [
            ('K1', (0, 4), 0),
            ('G1', (0, 0), 0),  # Ghost at center
        ]
        black = [
            ('K1', (0, -4), 3),
            ('A1', (0, -1), 3),  # Enemy pawn in front of ghost
        ]
        return GameState.create_initial(white, black, 'D', 'A')

    def test_ghost_cannot_capture(self, state_with_ghost):
        """Ghost should not be able to capture enemy pieces."""
        actions = generate_legal_actions(state_with_ghost)
        ghost_moves = [a for a in actions
                       if a.action_type == 'MOVE' and a.from_pos == (0, 0)]

        # Ghost should not have a move to (0, -1) where enemy is
        destinations = [m.to_pos for m in ghost_moves]
        assert (0, -1) not in destinations

    def test_ghost_can_move_to_empty(self, state_with_ghost):
        """Ghost should be able to move to empty hexes."""
        actions = generate_legal_actions(state_with_ghost)
        ghost_moves = [a for a in actions
                       if a.action_type == 'MOVE' and a.from_pos == (0, 0)]

        # Should have moves to empty adjacent hexes
        assert len(ghost_moves) >= 3  # Some directions available

    def test_ghost_cannot_be_captured(self):
        """Ghost cannot be captured by enemy pieces."""
        # Set up Black's turn with piece that could capture ghost
        white = [
            ('K1', (0, 4), 0),
            ('G1', (0, 0), 0),  # Ghost
        ]
        black = [
            ('K1', (0, -4), 3),
            ('D5', (0, -1), 3),  # Queen that could capture ghost
        ]
        state = GameState.create_initial(white, black, 'D', 'A')

        # Pass White's turn to get to Black
        state = apply_move(state, Move('PASS', None, None, None, None))
        state = apply_move(state, Move('PASS', None, None, None, None))

        # Now Black's turn
        actions = generate_legal_actions(state)
        queen_moves = [a for a in actions
                       if a.action_type == 'MOVE' and a.from_pos == (0, -1)]

        # Queen should NOT have a move to (0, 0) where ghost is
        destinations = [m.to_pos for m in queen_moves]
        assert (0, 0) not in destinations

    def test_ghost_blocks_movement(self):
        """Ghost should block enemy movement through its hex."""
        white = [
            ('K1', (0, 4), 0),
            ('G1', (0, 0), 0),  # Ghost blocks center
        ]
        black = [
            ('K1', (0, -4), 3),
            ('D1', (0, -2), 3),  # Pike that wants to slide through (facing south)
        ]
        # Use template D for Black so first action is MOVE
        state = GameState.create_initial(white, black, 'D', 'D')

        # Pass White's turn (template D: 2 actions)
        state = apply_move(state, Move('PASS', None, None, None, None))
        state = apply_move(state, Move('PASS', None, None, None, None))

        # Black's turn - Pike slides south
        actions = generate_legal_actions(state)
        pike_moves = [a for a in actions
                      if a.action_type == 'MOVE' and a.from_pos == (0, -2)]

        destinations = [m.to_pos for m in pike_moves]

        # Pike can reach (0, -1) but NOT (0, 0) or beyond (blocked by ghost)
        assert (0, -1) in destinations
        assert (0, 0) not in destinations
        assert (0, 1) not in destinations
