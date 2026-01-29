"""Tests for hexwar.board module."""

import pytest
from hexwar.board import (
    BOARD_RADIUS, DIRECTIONS, ALL_HEXES, NUM_HEXES,
    WHITE_HOME_ZONE, BLACK_HOME_ZONE,
    is_valid_hex, hex_distance, distance_to_center,
    get_direction_vector, get_neighbor, get_neighbors, get_valid_neighbors,
    opposite_direction, default_facing, get_home_zone,
    FORWARD, FORWARD_RIGHT, BACK_RIGHT, BACKWARD, BACK_LEFT, FORWARD_LEFT,
)


class TestBoardGeometry:
    """Test basic board geometry."""

    def test_board_has_61_hexes(self):
        """Spec: 8-edge hexagon has 61 total hexes."""
        assert NUM_HEXES == 61
        assert len(ALL_HEXES) == 61

    def test_center_is_valid(self):
        """Center hex (0, 0) should be valid."""
        assert is_valid_hex(0, 0)

    def test_corners_are_valid(self):
        """All corners of the hex should be valid."""
        # For radius 4, corners are at distance 4 from center
        corners = [
            (0, -4), (4, -4), (4, 0),
            (0, 4), (-4, 4), (-4, 0),
        ]
        for q, r in corners:
            assert is_valid_hex(q, r), f"Corner ({q}, {r}) should be valid"

    def test_just_outside_corners_invalid(self):
        """Points just outside corners should be invalid."""
        invalid_points = [
            (0, -5), (5, -5), (5, 0),
            (0, 5), (-5, 5), (-5, 0),
            (3, 3), (-3, -3),  # Violates |q + r| <= 4
        ]
        for q, r in invalid_points:
            assert not is_valid_hex(q, r), f"({q}, {r}) should be invalid"

    def test_all_hexes_are_unique(self):
        """All hexes in ALL_HEXES should be unique."""
        assert len(set(ALL_HEXES)) == NUM_HEXES


class TestHexDistance:
    """Test hex distance calculations."""

    def test_distance_to_self_is_zero(self):
        """Distance from a hex to itself should be 0."""
        for q, r in ALL_HEXES:
            assert hex_distance(q, r, q, r) == 0

    def test_distance_to_center(self):
        """Distance to center should match distance_to_center function."""
        for q, r in ALL_HEXES:
            assert distance_to_center(q, r) == hex_distance(q, r, 0, 0)

    def test_adjacent_hexes_have_distance_1(self):
        """Adjacent hexes should have distance 1."""
        for dq, dr in DIRECTIONS:
            assert hex_distance(0, 0, dq, dr) == 1

    def test_distance_is_symmetric(self):
        """Distance should be symmetric."""
        test_pairs = [
            ((0, 0), (3, -2)),
            ((2, 3), (-1, 4)),
            ((-5, 2), (3, -3)),
        ]
        for (q1, r1), (q2, r2) in test_pairs:
            assert hex_distance(q1, r1, q2, r2) == hex_distance(q2, r2, q1, r1)

    def test_corner_to_corner_distance(self):
        """Corners should be 8 apart (diameter for radius 4)."""
        # North corner to South corner
        assert hex_distance(0, -4, 0, 4) == 8

    def test_center_distance_values(self):
        """Verify specific distance-to-center values."""
        assert distance_to_center(0, 0) == 0
        assert distance_to_center(1, 0) == 1
        assert distance_to_center(0, -4) == 4
        assert distance_to_center(3, -2) == 3


class TestDirections:
    """Test direction vectors and operations."""

    def test_six_directions(self):
        """There should be exactly 6 directions."""
        assert len(DIRECTIONS) == 6

    def test_directions_are_unit_length(self):
        """Each direction should move to an adjacent hex (distance 1)."""
        for dq, dr in DIRECTIONS:
            assert hex_distance(0, 0, dq, dr) == 1

    def test_opposite_directions(self):
        """Opposite directions should sum to zero."""
        for i in range(3):
            dq1, dr1 = DIRECTIONS[i]
            dq2, dr2 = DIRECTIONS[i + 3]
            assert dq1 + dq2 == 0
            assert dr1 + dr2 == 0

    def test_opposite_direction_function(self):
        """opposite_direction should return the direction 180 degrees away."""
        for i in range(6):
            assert opposite_direction(i) == (i + 3) % 6

    def test_get_direction_vector_forward(self):
        """Forward direction should match facing."""
        for facing in range(6):
            assert get_direction_vector(facing, FORWARD) == DIRECTIONS[facing]

    def test_get_direction_vector_backward(self):
        """Backward direction should be opposite of facing."""
        for facing in range(6):
            expected = DIRECTIONS[(facing + 3) % 6]
            assert get_direction_vector(facing, BACKWARD) == expected

    def test_relative_directions_wrap(self):
        """Relative directions should wrap around correctly."""
        # Facing North (0), Forward-Left should be NW (5)
        assert get_direction_vector(0, FORWARD_LEFT) == DIRECTIONS[5]
        # Facing South (3), Forward-Right should be SW (4)
        assert get_direction_vector(3, FORWARD_RIGHT) == DIRECTIONS[4]


class TestNeighbors:
    """Test neighbor finding functions."""

    def test_center_has_six_neighbors(self):
        """Center hex should have 6 valid neighbors."""
        neighbors = get_valid_neighbors(0, 0)
        assert len(neighbors) == 6

    def test_corner_has_three_neighbors(self):
        """Corner hexes should have only 3 valid neighbors."""
        # North corner (for radius 4)
        neighbors = get_valid_neighbors(0, -4)
        assert len(neighbors) == 3

    def test_get_neighbor_matches_direction(self):
        """get_neighbor should return hex in the given direction."""
        q, r = 0, 0
        for d in range(6):
            nq, nr = get_neighbor(q, r, d)
            dq, dr = DIRECTIONS[d]
            assert nq == q + dq
            assert nr == r + dr

    def test_neighbors_are_adjacent(self):
        """All neighbors should have distance 1."""
        for q, r in ALL_HEXES:
            for neighbor in get_valid_neighbors(q, r):
                assert hex_distance(q, r, neighbor[0], neighbor[1]) == 1


class TestHomeZones:
    """Test home zone definitions."""

    def test_white_home_zone_size(self):
        """White home zone should have correct number of hexes."""
        # Rows r=5,6,7 in valid range
        # r=7: 1 hex, r=6: 3 hexes, r=5: 5 hexes... let's count
        assert len(WHITE_HOME_ZONE) == 18

    def test_black_home_zone_size(self):
        """Black home zone should be same size as white."""
        assert len(BLACK_HOME_ZONE) == 18

    def test_home_zones_dont_overlap(self):
        """Home zones should not overlap."""
        assert WHITE_HOME_ZONE.isdisjoint(BLACK_HOME_ZONE)

    def test_home_zones_at_edges(self):
        """White at south (high r), black at north (low r)."""
        for q, r in WHITE_HOME_ZONE:
            assert r >= 2
        for q, r in BLACK_HOME_ZONE:
            assert r <= -2

    def test_get_home_zone(self):
        """get_home_zone should return correct zone."""
        assert get_home_zone(0) == WHITE_HOME_ZONE
        assert get_home_zone(1) == BLACK_HOME_ZONE


class TestFacing:
    """Test facing-related functions."""

    def test_default_facing_white(self):
        """White pieces should face north (0)."""
        assert default_facing(0) == 0

    def test_default_facing_black(self):
        """Black pieces should face south (3)."""
        assert default_facing(1) == 3
