"""Tests for hexwar.pieces module."""

import pytest
from hexwar.pieces import (
    PIECE_TYPES, REGULAR_PIECE_IDS, KING_IDS, SPECIAL_PIECE_IDS,
    get_piece_type, is_king, has_special, get_special,
    Piece, PieceType, INF,
    PAWN, GUARD, QUEEN, KNIGHT, WARPER, GHOST, KING_GUARD, KING_FROG,
    ALL_DIRS, FORWARD, FORWARD_LEFT, FORWARD_RIGHT,
)
from hexwar.board import FORWARD, FORWARD_RIGHT, BACK_RIGHT, BACKWARD, BACK_LEFT, FORWARD_LEFT


class TestPieceCatalog:
    """Test the piece catalog structure."""

    def test_total_piece_types(self):
        """Should have exactly 30 piece types (25 regular + 5 kings)."""
        assert len(PIECE_TYPES) == 30

    def test_regular_pieces_count(self):
        """Should have 25 regular (non-king) piece types."""
        assert len(REGULAR_PIECE_IDS) == 25

    def test_king_variants_count(self):
        """Should have 5 king variants."""
        assert len(KING_IDS) == 5

    def test_special_pieces_count(self):
        """Should have 4 pieces with special abilities."""
        assert len(SPECIAL_PIECE_IDS) == 4

    def test_all_ids_unique(self):
        """All piece IDs should be unique."""
        assert len(set(PIECE_TYPES.keys())) == 30

    def test_piece_id_format(self):
        """Piece IDs should follow the spec format."""
        for pid in PIECE_TYPES:
            # First char is category letter, rest is number
            assert pid[0].isalpha()
            assert pid[1:].isdigit()


class TestStepPieces:
    """Test step-movement piece definitions."""

    def test_pawn_moves_forward_only(self):
        """Pawn (A1) moves step-1, forward only."""
        assert PAWN.move_type == 'STEP'
        assert PAWN.move_range == 1
        assert PAWN.directions == (FORWARD,)

    def test_guard_moves_all_directions(self):
        """Guard (A2) moves step-1 in all 6 directions."""
        assert GUARD.move_type == 'STEP'
        assert GUARD.move_range == 1
        assert len(GUARD.directions) == 6
        assert set(GUARD.directions) == set(ALL_DIRS)

    def test_step2_pieces_have_range_2(self):
        """All B-series pieces should have range 2."""
        for pid in ['B1', 'B2', 'B3', 'B4']:
            piece = get_piece_type(pid)
            assert piece.move_type == 'STEP'
            assert piece.move_range == 2

    def test_step3_pieces_have_range_3(self):
        """All C-series pieces should have range 3."""
        for pid in ['C1', 'C2', 'C3']:
            piece = get_piece_type(pid)
            assert piece.move_type == 'STEP'
            assert piece.move_range == 3


class TestSlidePieces:
    """Test slide-movement piece definitions."""

    def test_queen_slides_all_directions(self):
        """Queen (D5) slides in all 6 directions."""
        assert QUEEN.move_type == 'SLIDE'
        assert QUEEN.move_range == INF
        assert len(QUEEN.directions) == 6

    def test_all_slide_pieces_have_inf_range(self):
        """All D-series pieces should have infinite range."""
        for pid in ['D1', 'D2', 'D3', 'D4', 'D5']:
            piece = get_piece_type(pid)
            assert piece.move_type == 'SLIDE'
            assert piece.move_range == INF


class TestJumpPieces:
    """Test jump-movement piece definitions."""

    def test_knight_jumps_2_forward_arc(self):
        """Knight (E1) jumps exactly 2 hexes, forward arc (forward, forward-left, forward-right)."""
        assert KNIGHT.move_type == 'JUMP'
        assert KNIGHT.move_range == 2
        assert KNIGHT.directions == (FORWARD, FORWARD_LEFT, FORWARD_RIGHT)

    def test_frog_jumps_2_all_directions(self):
        """Frog (E2) jumps exactly 2 hexes in all 6 directions."""
        piece = get_piece_type('E2')
        assert piece.move_type == 'JUMP'
        assert piece.move_range == 2
        assert len(piece.directions) == 6

    def test_locust_jumps_3(self):
        """Locust (F1) jumps exactly 3 hexes."""
        piece = get_piece_type('F1')
        assert piece.move_type == 'JUMP'
        assert piece.move_range == 3


class TestSpecialPieces:
    """Test pieces with special abilities."""

    def test_warper_has_swap_move(self):
        """Warper (W1) has SWAP_MOVE special and no normal movement."""
        assert WARPER.special == 'SWAP_MOVE'
        assert WARPER.move_type == 'NONE'
        assert WARPER.move_range == 0

    def test_shifter_has_swap_rotate(self):
        """Shifter (W2) has SWAP_ROTATE special and normal step-1 movement."""
        piece = get_piece_type('W2')
        assert piece.special == 'SWAP_ROTATE'
        assert piece.move_type == 'STEP'
        assert piece.move_range == 1

    def test_phoenix_has_rebirth(self):
        """Phoenix (P1) has REBIRTH special."""
        piece = get_piece_type('P1')
        assert piece.special == 'REBIRTH'

    def test_ghost_has_phased(self):
        """Ghost (G1) has PHASED special."""
        assert GHOST.special == 'PHASED'
        assert GHOST.move_type == 'STEP'
        assert len(GHOST.directions) == 6


class TestKingVariants:
    """Test king piece definitions."""

    def test_all_kings_marked_as_king(self):
        """All K-series pieces should have is_king=True."""
        for pid in KING_IDS:
            piece = get_piece_type(pid)
            assert piece.is_king, f"{pid} should be marked as king"

    def test_regular_pieces_not_kings(self):
        """Regular pieces should not be kings."""
        for pid in REGULAR_PIECE_IDS:
            assert not is_king(pid), f"{pid} should not be a king"

    def test_king_guard_moves_like_guard(self):
        """King (Guard) K1 moves step-1 in all directions."""
        assert KING_GUARD.move_type == 'STEP'
        assert KING_GUARD.move_range == 1
        assert len(KING_GUARD.directions) == 6

    def test_king_frog_jumps_like_frog(self):
        """King (Frog) K4 jumps-2 in all directions."""
        assert KING_FROG.move_type == 'JUMP'
        assert KING_FROG.move_range == 2
        assert len(KING_FROG.directions) == 6


class TestPieceInstance:
    """Test the Piece instance class."""

    def test_create_piece(self):
        """Can create a piece instance."""
        p = Piece('A1', owner=0, facing=0)
        assert p.type_id == 'A1'
        assert p.owner == 0
        assert p.facing == 0

    def test_piece_type_property(self):
        """piece_type property returns correct PieceType."""
        p = Piece('D5', owner=1, facing=3)
        assert p.piece_type == QUEEN

    def test_is_king_property(self):
        """is_king property works correctly."""
        regular = Piece('A1', 0, 0)
        king = Piece('K1', 0, 0)
        assert not regular.is_king
        assert king.is_king

    def test_piece_repr(self):
        """Piece repr shows owner and type."""
        white_pawn = Piece('A1', 0, 0)
        black_queen = Piece('D5', 1, 3)
        assert repr(white_pawn) == 'WA1'
        assert repr(black_queen) == 'BD5'


class TestHelperFunctions:
    """Test helper functions."""

    def test_get_piece_type(self):
        """get_piece_type returns correct PieceType."""
        assert get_piece_type('A1') == PAWN
        assert get_piece_type('D5') == QUEEN

    def test_get_piece_type_invalid(self):
        """get_piece_type raises KeyError for invalid ID."""
        with pytest.raises(KeyError):
            get_piece_type('X9')

    def test_is_king_function(self):
        """is_king function works correctly."""
        assert is_king('K1')
        assert is_king('K5')
        assert not is_king('A1')
        assert not is_king('D5')

    def test_has_special_function(self):
        """has_special function works correctly."""
        assert has_special('W1')
        assert has_special('G1')
        assert not has_special('A1')
        assert not has_special('D5')

    def test_get_special_function(self):
        """get_special returns correct special type."""
        assert get_special('W1') == 'SWAP_MOVE'
        assert get_special('W2') == 'SWAP_ROTATE'
        assert get_special('P1') == 'REBIRTH'
        assert get_special('G1') == 'PHASED'
        assert get_special('A1') is None
