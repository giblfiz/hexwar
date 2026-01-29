"""Tests for hexwar.game module."""

import pytest
from hexwar.game import (
    GameState, Move, TEMPLATES,
    generate_destinations, generate_moves_for_piece,
    generate_rotates_for_piece, generate_legal_actions,
    apply_move, is_game_over, get_winner, print_board,
)
from hexwar.pieces import Piece
from hexwar.board import default_facing


class TestActionTemplates:
    """Test action template definitions."""

    def test_template_a_rotate_move_same(self):
        """Template A: Rotate-Move Same."""
        assert TEMPLATES['A'] == (('ROTATE', 'ANY'), ('MOVE', 'SAME'))

    def test_template_b_move_rotate_rotate(self):
        """Template B: Move-Rotate-Rotate."""
        assert TEMPLATES['B'] == (('MOVE', 'ANY'), ('ROTATE', 'ANY'), ('ROTATE', 'ANY'))

    def test_template_c_move_move_rotate(self):
        """Template C: Move-Move-Rotate."""
        assert TEMPLATES['C'] == (('MOVE', 'ANY'), ('MOVE', 'DIFFERENT'), ('ROTATE', 'ANY'))

    def test_template_d_move_rotate_different(self):
        """Template D: Move-Rotate Different."""
        assert TEMPLATES['D'] == (('MOVE', 'ANY'), ('ROTATE', 'DIFFERENT'))


class TestGameStateCreation:
    """Test GameState creation and properties."""

    def test_create_empty_game(self):
        """Can create a game with no pieces (for testing)."""
        state = GameState.create_initial([], [], 'D', 'A')
        assert state.current_player == 0
        assert state.turn_number == 1
        assert state.round_number == 1

    def test_create_game_with_pieces(self):
        """Can create a game with pieces."""
        white = [('K1', (0, 3), 0)]  # White king
        black = [('K1', (0, -3), 3)]  # Black king
        state = GameState.create_initial(white, black, 'D', 'A')

        assert (0, 3) in state.board
        assert (0, -3) in state.board
        assert state.king_positions == ((0, 3), (0, -3))

    def test_initial_graveyards_empty(self):
        """Graveyards should start empty."""
        state = GameState.create_initial([], [], 'D', 'A')
        assert state.graveyards == ([], [])

    def test_current_template_white(self):
        """current_template returns White's template on White's turn."""
        state = GameState.create_initial([], [], 'D', 'A')
        assert state.current_template == TEMPLATES['D']

    def test_current_action_first(self):
        """current_action returns first action of template."""
        state = GameState.create_initial([], [], 'D', 'A')
        assert state.current_action == ('MOVE', 'ANY')


class TestMoveGeneration:
    """Test move generation for different piece types."""

    @pytest.fixture
    def empty_state(self):
        """Create a state with just kings."""
        white = [('K1', (0, 4), 0)]  # South edge
        black = [('K1', (0, -4), 3)]  # North edge
        return GameState.create_initial(white, black, 'D', 'A')

    def test_pawn_moves_forward(self, empty_state):
        """Pawn should generate forward moves."""
        # Add a white pawn at center facing north
        empty_state.board[(0, 0)] = Piece('A1', 0, 0)
        dests = list(generate_destinations(empty_state, (0, 0), empty_state.board[(0, 0)]))
        assert (0, -1) in dests  # Forward (north)
        assert len(dests) == 1

    def test_guard_moves_all_directions(self, empty_state):
        """Guard should move in all 6 directions."""
        empty_state.board[(0, 0)] = Piece('A2', 0, 0)
        dests = list(generate_destinations(empty_state, (0, 0), empty_state.board[(0, 0)]))
        assert len(dests) == 6

    def test_queen_slides_until_blocked(self, empty_state):
        """Queen should slide in all directions until edge."""
        empty_state.board[(0, 0)] = Piece('D5', 0, 0)
        dests = list(generate_destinations(empty_state, (0, 0), empty_state.board[(0, 0)]))
        # Each of 6 directions can slide up to 4 hexes (radius)
        # Total depends on board geometry
        assert len(dests) > 20

    def test_knight_forward_arc_jump(self, empty_state):
        """Knight should jump to forward arc at distance 2."""
        empty_state.board[(0, 0)] = Piece('E1', 0, 0)  # Knight facing north
        dests = list(generate_destinations(empty_state, (0, 0), empty_state.board[(0, 0)]))
        # Knight has FORWARD_ARC, distance 2
        # Forward arc is 150° (±75° from forward) = 5 positions at d2
        assert len(dests) == 5
        # All destinations should be exactly distance 2
        for q, r in dests:
            dist = (abs(q) + abs(r) + abs(q + r)) // 2
            assert dist == 2

    def test_frog_omni_jump(self, empty_state):
        """Frog should jump to all hexes at distance 2."""
        empty_state.board[(0, 0)] = Piece('E2', 0, 0)  # Frog facing north
        dests = list(generate_destinations(empty_state, (0, 0), empty_state.board[(0, 0)]))
        # Frog has ALL_DIRS (6 sectors), distance 2
        # Full ring at distance 2 = 12 positions
        assert len(dests) == 12
        # All destinations should be exactly distance 2
        for q, r in dests:
            dist = (abs(q) + abs(r) + abs(q + r)) // 2
            assert dist == 2

    def test_locust_forward_arc_jump(self, empty_state):
        """Locust should jump to forward arc at distance 3."""
        empty_state.board[(0, 0)] = Piece('F1', 0, 0)  # Locust facing north
        dests = list(generate_destinations(empty_state, (0, 0), empty_state.board[(0, 0)]))
        # Locust has FORWARD_ARC, distance 3
        # Forward arc is 150° (±75° from forward) = 7 positions at d3
        assert len(dests) == 7
        # All destinations should be exactly distance 3
        for q, r in dests:
            dist = (abs(q) + abs(r) + abs(q + r)) // 2
            assert dist == 3

    def test_cricket_omni_jump(self, empty_state):
        """Cricket should jump to all hexes at distance 3."""
        empty_state.board[(0, 0)] = Piece('F2', 0, 0)  # Cricket facing north
        dests = list(generate_destinations(empty_state, (0, 0), empty_state.board[(0, 0)]))
        # Cricket has ALL_DIRS (6 sectors), distance 3
        # Full ring at distance 3 = 18 positions
        assert len(dests) == 18
        # All destinations should be exactly distance 3
        for q, r in dests:
            dist = (abs(q) + abs(r) + abs(q + r)) // 2
            assert dist == 3

    def test_jump_ignores_pieces_in_path(self, empty_state):
        """Jumpers should leap over pieces (not blocked by them)."""
        empty_state.board[(0, 0)] = Piece('E2', 0, 0)  # Frog
        # Place blockers in the path (1 step away in each direction)
        for q, r in [(0, -1), (1, -1), (1, 0), (0, 1), (-1, 1), (-1, 0)]:
            empty_state.board[(q, r)] = Piece('A1', 0, 0)  # Friendly pawns
        dests = list(generate_destinations(empty_state, (0, 0), empty_state.board[(0, 0)]))
        # Jumper should still reach destinations (leaps over blockers)
        assert len(dests) > 0

    def test_blocked_by_friendly(self, empty_state):
        """Pieces cannot move through friendly pieces."""
        empty_state.board[(0, 0)] = Piece('D1', 0, 0)  # Pike facing north
        empty_state.board[(0, -2)] = Piece('A1', 0, 0)  # Friendly pawn blocking
        dests = list(generate_destinations(empty_state, (0, 0), empty_state.board[(0, 0)]))
        # Can only reach (0, -1), blocked at (0, -2)
        assert (0, -1) in dests
        assert (0, -2) not in dests
        assert len(dests) == 1

    def test_can_capture_enemy(self, empty_state):
        """Pieces can capture enemies."""
        empty_state.board[(0, 0)] = Piece('D1', 0, 0)  # White pike
        empty_state.board[(0, -2)] = Piece('A1', 1, 3)  # Black pawn
        dests = list(generate_destinations(empty_state, (0, 0), empty_state.board[(0, 0)]))
        # Can reach (0, -1) and capture on (0, -2)
        assert (0, -1) in dests
        assert (0, -2) in dests
        assert len(dests) == 2


class TestRotateGeneration:
    """Test rotate action generation."""

    def test_rotate_generates_6_facings(self):
        """Rotating should offer all 6 facings."""
        white = [('K1', (0, 4), 0), ('A1', (0, 0), 0)]
        black = [('K1', (0, -4), 3)]
        state = GameState.create_initial(white, black, 'A', 'A')

        # Template A starts with ROTATE
        rotates = list(generate_rotates_for_piece(state, (0, 0)))
        assert len(rotates) == 6


class TestLegalActions:
    """Test legal action generation with constraints."""

    def test_pass_always_available(self):
        """PASS should always be available."""
        state = GameState.create_initial([], [], 'D', 'A')
        actions = generate_legal_actions(state)
        assert any(a.action_type == 'PASS' for a in actions)

    def test_same_piece_constraint(self):
        """SAME constraint restricts to same piece."""
        white = [('K1', (0, 4), 0), ('A2', (0, 0), 0)]  # Guard
        black = [('K1', (0, -4), 3)]
        state = GameState.create_initial(white, black, 'A', 'A')

        # Template A: ROTATE then MOVE SAME
        # First action: rotate (no constraint yet)
        actions = generate_legal_actions(state)
        rotate_actions = [a for a in actions if a.action_type == 'ROTATE']
        assert len(rotate_actions) > 0

        # Do a rotate on the guard
        rotate = [a for a in rotate_actions if a.from_pos == (0, 0)][0]
        state2 = apply_move(state, rotate)

        # Now MOVE SAME - only guard can move
        actions2 = generate_legal_actions(state2)
        move_actions = [a for a in actions2 if a.action_type == 'MOVE']

        # All moves should be from the guard's new position
        for m in move_actions:
            assert m.from_pos == (0, 0)

    def test_different_piece_constraint(self):
        """DIFFERENT constraint excludes last piece."""
        white = [('K1', (0, 4), 0), ('A2', (0, 0), 0), ('A2', (1, 0), 0)]
        black = [('K1', (0, -4), 3)]
        state = GameState.create_initial(white, black, 'D', 'A')

        # Template D: MOVE then ROTATE DIFFERENT
        # First move with guard at (0,0)
        actions = generate_legal_actions(state)
        move = [a for a in actions if a.action_type == 'MOVE' and a.from_pos == (0, 0)][0]
        state2 = apply_move(state, move)

        # Now ROTATE DIFFERENT - cannot rotate piece that just moved
        actions2 = generate_legal_actions(state2)
        rotate_actions = [a for a in actions2 if a.action_type == 'ROTATE']

        # None should be from the guard's new position
        for r in rotate_actions:
            assert r.from_pos != move.to_pos


class TestApplyMove:
    """Test applying moves to game state."""

    def test_move_piece(self):
        """Moving a piece updates board."""
        white = [('A2', (0, 0), 0)]
        black = [('K1', (0, -4), 3)]
        state = GameState.create_initial(white, black, 'D', 'A')

        move = Move('MOVE', (0, 0), (0, -1), 0, None)
        state2 = apply_move(state, move)

        assert (0, 0) not in state2.board
        assert (0, -1) in state2.board
        assert state2.board[(0, -1)].type_id == 'A2'

    def test_capture_adds_to_graveyard(self):
        """Captured piece goes to owner's graveyard (for rebirth mechanics)."""
        white = [('A2', (0, 0), 0)]
        black = [('K1', (0, -4), 3), ('A1', (0, -1), 3)]
        state = GameState.create_initial(white, black, 'D', 'A')

        move = Move('MOVE', (0, 0), (0, -1), 0, None)
        state2 = apply_move(state, move)

        # White captured black pawn - goes to Black's graveyard (owner)
        assert 'A1' in state2.graveyards[1]

    def test_capture_king_wins(self):
        """Capturing enemy king wins the game."""
        white = [('A2', (0, 0), 0)]
        black = [('K1', (0, -1), 3)]
        state = GameState.create_initial(white, black, 'D', 'A')

        move = Move('MOVE', (0, 0), (0, -1), 0, None)
        state2 = apply_move(state, move)

        assert state2.winner == 0  # White wins

    def test_rotate_piece(self):
        """Rotating updates piece facing."""
        white = [('A1', (0, 0), 0)]
        black = [('K1', (0, -4), 3)]
        state = GameState.create_initial(white, black, 'A', 'A')

        rotate = Move('ROTATE', (0, 0), None, 3, None)
        state2 = apply_move(state, rotate)

        assert state2.board[(0, 0)].facing == 3

    def test_pass_advances_action(self):
        """PASS advances action index without board changes."""
        state = GameState.create_initial([], [], 'D', 'A')
        assert state.action_index == 0

        state2 = apply_move(state, Move('PASS', None, None, None, None))
        assert state2.action_index == 1


class TestTurnStructure:
    """Test turn progression."""

    def test_turn_ends_after_template(self):
        """Turn ends after all template actions."""
        state = GameState.create_initial([], [], 'D', 'A')
        # Template D has 2 actions

        state = apply_move(state, Move('PASS', None, None, None, None))
        assert state.current_player == 0  # Still White's turn

        state = apply_move(state, Move('PASS', None, None, None, None))
        assert state.current_player == 1  # Now Black's turn

    def test_round_increments_after_black(self):
        """Round number increments after Black's turn."""
        state = GameState.create_initial([], [], 'D', 'A')
        assert state.round_number == 1

        # White's turn (2 passes for template D)
        state = apply_move(state, Move('PASS', None, None, None, None))
        state = apply_move(state, Move('PASS', None, None, None, None))
        assert state.round_number == 1  # Still round 1

        # Black's turn (2 actions for template A)
        state = apply_move(state, Move('PASS', None, None, None, None))
        state = apply_move(state, Move('PASS', None, None, None, None))
        assert state.round_number == 2  # Now round 2


class TestVictoryConditions:
    """Test game victory conditions."""

    def test_king_capture_wins(self):
        """Capturing enemy king wins immediately."""
        white = [('D5', (0, 0), 0)]  # Queen
        black = [('K1', (0, -1), 3)]
        state = GameState.create_initial(white, black, 'D', 'A')

        move = Move('MOVE', (0, 0), (0, -1), 0, None)
        state = apply_move(state, move)

        assert is_game_over(state)
        assert get_winner(state) == 0

    def test_no_actions_when_game_over(self):
        """No legal actions after game is over."""
        white = [('D5', (0, 0), 0)]
        black = [('K1', (0, -1), 3)]
        state = GameState.create_initial(white, black, 'D', 'A')

        move = Move('MOVE', (0, 0), (0, -1), 0, None)
        state = apply_move(state, move)

        assert generate_legal_actions(state) == []


class TestStateCopy:
    """Test state copying."""

    def test_copy_is_independent(self):
        """Copied state is independent of original."""
        white = [('A1', (0, 0), 0)]
        black = [('K1', (0, -4), 3)]
        state1 = GameState.create_initial(white, black, 'D', 'A')

        state2 = state1.copy()

        # Modify copy
        state2.board[(0, 0)] = Piece('D5', 0, 0)

        # Original unchanged
        assert state1.board[(0, 0)].type_id == 'A1'

    def test_copy_graveyards_independent(self):
        """Graveyard lists are independent copies."""
        state1 = GameState.create_initial([], [], 'D', 'A')
        state2 = state1.copy()

        state2.graveyards[0].append('A1')

        assert len(state1.graveyards[0]) == 0
