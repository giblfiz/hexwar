"""Performance tests for HEXWAR game engine.

These tests establish baseline timings and detect performance regressions.
Run with: pytest tests/test_performance.py -v

Expected baselines (template E, single core):
- D2: < 0.1s per game
- D3: < 0.5s per game
- D4: < 3.0s per game
- D5: < 15.0s per game

If tests fail, there's a performance regression.
"""
import pytest
import time
import random
import json
from dataclasses import dataclass
from typing import Optional


@dataclass
class PerfResult:
    """Performance test result."""
    depth: int
    elapsed: float
    rounds: int
    winner: int
    pieces_white: int
    pieces_black: int
    template: str


# Expected maximum times per depth (seconds)
# These are conservative - actual should be much faster
DEPTH_LIMITS = {
    2: 0.1,   # D2 should be very fast
    3: 0.5,   # D3 still quick
    4: 3.0,   # D4 getting slower
    5: 15.0,  # D5 is the practical limit
}


def create_test_game(seed: int = 42, n_white: int = 10, n_black: int = 10):
    """Create a test game state with specified piece counts."""
    from hexwar.evolution import create_random_ruleset, create_game_from_ruleset, RuleSet

    rng = random.Random(seed)
    rs = create_random_ruleset(rng)

    # Trim to requested piece counts (RuleSet is a dataclass)
    rs = RuleSet(
        white_pieces=rs.white_pieces[:n_white],
        black_pieces=rs.black_pieces[:n_black],
        white_template=rs.white_template,
        black_template=rs.black_template,
        white_king=rs.white_king,
        black_king=rs.black_king,
        white_positions=rs.white_positions[:n_white + 1] if rs.white_positions else [],
        black_positions=rs.black_positions[:n_black + 1] if rs.black_positions else [],
        white_facings=rs.white_facings[:n_white + 1] if rs.white_facings else [],
        black_facings=rs.black_facings[:n_black + 1] if rs.black_facings else [],
    )

    return rs, create_game_from_ruleset(rs, seed=seed)


def run_rust_game(state, heuristics_dict: dict, depth: int, seed: int = 42) -> PerfResult:
    """Run a single game using Rust engine and return timing."""
    try:
        from hexwar_core.hexwar_core import play_game as rust_play_game
    except ImportError:
        pytest.skip("Rust engine not available")

    # Extract pieces
    white_pieces = []
    black_pieces = []
    for (q, r), piece in state.board.items():
        entry = (piece.type_id, (q, r), piece.facing)
        if piece.owner == 0:
            white_pieces.append(entry)
        else:
            black_pieces.append(entry)

    template = state.templates[0]  # Assume both same

    start = time.perf_counter()
    winner, rounds = rust_play_game(
        white_pieces, black_pieces,
        template, template,
        depth, depth,
        heuristics_dict,
        max_moves=500,
        max_moves_per_action=15,
        seed=seed,
    )
    elapsed = time.perf_counter() - start

    return PerfResult(
        depth=depth,
        elapsed=elapsed,
        rounds=rounds,
        winner=winner,
        pieces_white=len(white_pieces),
        pieces_black=len(black_pieces),
        template=template,
    )


def run_python_game(state, heuristics, depth: int, seed: int = 42) -> PerfResult:
    """Run a single game using Python engine and return timing."""
    from hexwar.ai import play_ai_game

    white_count = sum(1 for p in state.board.values() if p.owner == 0)
    black_count = sum(1 for p in state.board.values() if p.owner == 1)
    template = state.templates[0]

    start = time.perf_counter()
    final_state, winner = play_ai_game(
        state=state,
        white_depth=depth,
        black_depth=depth,
        heuristics=heuristics,
        seed=seed,
        max_moves=500,
        max_moves_per_action=15,
        noise_scale=0.01,
    )
    elapsed = time.perf_counter() - start

    return PerfResult(
        depth=depth,
        elapsed=elapsed,
        rounds=final_state.round_number,
        winner=winner,
        pieces_white=white_count,
        pieces_black=black_count,
        template=template,
    )


def get_heuristics(rs):
    """Get heuristics dict and object for a ruleset."""
    from hexwar.evolution import create_template_aware_heuristics, Heuristics

    h = create_template_aware_heuristics(rs, 1.0)
    h_dict = {
        'white_piece_values': h.white_piece_values,
        'black_piece_values': h.black_piece_values,
        'white_center_weight': h.white_center_weight,
        'black_center_weight': h.black_center_weight,
    }
    return h_dict, h


class TestRustEnginePerformance:
    """Test Rust engine performance at various depths."""

    def test_d2_performance(self):
        """D2 games should complete in < 0.1s."""
        rs, state = create_test_game(seed=42)
        h_dict, _ = get_heuristics(rs)

        result = run_rust_game(state, h_dict, depth=2)

        print(f"\nD2: {result.elapsed:.3f}s, {result.rounds} rounds, "
              f"{result.pieces_white}v{result.pieces_black} pieces")

        assert result.elapsed < DEPTH_LIMITS[2], \
            f"D2 took {result.elapsed:.3f}s, expected < {DEPTH_LIMITS[2]}s"

    def test_d3_performance(self):
        """D3 games should complete in < 0.5s."""
        rs, state = create_test_game(seed=42)
        h_dict, _ = get_heuristics(rs)

        result = run_rust_game(state, h_dict, depth=3)

        print(f"\nD3: {result.elapsed:.3f}s, {result.rounds} rounds, "
              f"{result.pieces_white}v{result.pieces_black} pieces")

        assert result.elapsed < DEPTH_LIMITS[3], \
            f"D3 took {result.elapsed:.3f}s, expected < {DEPTH_LIMITS[3]}s"

    def test_d4_performance(self):
        """D4 games should complete in < 3.0s."""
        rs, state = create_test_game(seed=42)
        h_dict, _ = get_heuristics(rs)

        result = run_rust_game(state, h_dict, depth=4)

        print(f"\nD4: {result.elapsed:.3f}s, {result.rounds} rounds, "
              f"{result.pieces_white}v{result.pieces_black} pieces")

        assert result.elapsed < DEPTH_LIMITS[4], \
            f"D4 took {result.elapsed:.3f}s, expected < {DEPTH_LIMITS[4]}s"

    def test_d5_performance(self):
        """D5 games should complete in < 15.0s."""
        rs, state = create_test_game(seed=42)
        h_dict, _ = get_heuristics(rs)

        result = run_rust_game(state, h_dict, depth=5)

        print(f"\nD5: {result.elapsed:.3f}s, {result.rounds} rounds, "
              f"{result.pieces_white}v{result.pieces_black} pieces")

        assert result.elapsed < DEPTH_LIMITS[5], \
            f"D5 took {result.elapsed:.3f}s, expected < {DEPTH_LIMITS[5]}s"

    def test_template_e_enforced(self):
        """Verify test games use template E."""
        rs, state = create_test_game(seed=42)

        assert state.templates[0] == 'E', f"White template is {state.templates[0]}, expected E"
        assert state.templates[1] == 'E', f"Black template is {state.templates[1]}, expected E"


class TestPythonEnginePerformance:
    """Test Python engine performance (slower but should still be reasonable)."""

    def test_d2_python_performance(self):
        """D2 Python games should complete in < 1.0s."""
        rs, state = create_test_game(seed=42)
        _, heuristics = get_heuristics(rs)

        result = run_python_game(state, heuristics, depth=2)

        print(f"\nD2 Python: {result.elapsed:.3f}s, {result.rounds} rounds")

        # Python is ~10x slower than Rust
        assert result.elapsed < 1.0, \
            f"D2 Python took {result.elapsed:.3f}s, expected < 1.0s"

    def test_d3_python_performance(self):
        """D3 Python games should complete in < 5.0s."""
        rs, state = create_test_game(seed=42)
        _, heuristics = get_heuristics(rs)

        result = run_python_game(state, heuristics, depth=3)

        print(f"\nD3 Python: {result.elapsed:.3f}s, {result.rounds} rounds")

        assert result.elapsed < 5.0, \
            f"D3 Python took {result.elapsed:.3f}s, expected < 5.0s"


class TestPieceCountScaling:
    """Test how performance scales with piece count."""

    def test_few_pieces_fast(self):
        """Games with few pieces should be very fast."""
        rs, state = create_test_game(seed=42, n_white=4, n_black=4)
        h_dict, _ = get_heuristics(rs)

        result = run_rust_game(state, h_dict, depth=4)

        print(f"\n4v4 D4: {result.elapsed:.3f}s, {result.rounds} rounds")

        # Few pieces = much faster
        assert result.elapsed < 0.5, \
            f"4v4 D4 took {result.elapsed:.3f}s, expected < 0.5s"

    def test_many_pieces_still_reasonable(self):
        """Games with many pieces should still complete."""
        rs, state = create_test_game(seed=42, n_white=12, n_black=12)
        h_dict, _ = get_heuristics(rs)

        result = run_rust_game(state, h_dict, depth=4)

        print(f"\n12v12 D4: {result.elapsed:.3f}s, {result.rounds} rounds")

        # More pieces = slower but still reasonable
        assert result.elapsed < 10.0, \
            f"12v12 D4 took {result.elapsed:.3f}s, expected < 10.0s"


class TestMultipleGames:
    """Test running multiple games (closer to real usage)."""

    def test_multiple_d5_games(self):
        """Multiple D5 games should maintain consistent performance."""
        rs, state = create_test_game(seed=42)
        h_dict, _ = get_heuristics(rs)

        times = []
        for i in range(3):
            # Recreate state with different seed for variety
            _, state = create_test_game(seed=42 + i)
            result = run_rust_game(state, h_dict, depth=5, seed=42 + i)
            times.append(result.elapsed)
            print(f"\n  Game {i+1}: {result.elapsed:.3f}s, {result.rounds} rounds")

        avg_time = sum(times) / len(times)
        max_time = max(times)

        print(f"\n  Average: {avg_time:.3f}s, Max: {max_time:.3f}s")

        assert avg_time < 10.0, f"Average D5 time {avg_time:.3f}s, expected < 10.0s"
        assert max_time < 20.0, f"Max D5 time {max_time:.3f}s, expected < 20.0s"


class TestSpecificBoardSets:
    """Test with actual board sets from the project."""

    def test_orcs_vs_necromancer_d5(self):
        """Test the actual orcs vs necromancer matchup."""
        from hexwar.evolution import board_set_to_ruleset, create_game_from_ruleset, RuleSet

        with open('board_sets/the_orcs.json') as f:
            orcs = json.load(f)
        with open('board_sets/the_necromancer.json') as f:
            necro = json.load(f)

        rs_orcs = board_set_to_ruleset(orcs)
        rs_necro = board_set_to_ruleset(necro)

        combined = RuleSet(
            white_pieces=rs_orcs.white_pieces,
            black_pieces=rs_necro.black_pieces,
            white_template='E',
            black_template='E',
            white_king=rs_orcs.white_king,
            black_king=rs_necro.black_king,
            white_positions=rs_orcs.white_positions,
            black_positions=rs_necro.black_positions,
            white_facings=rs_orcs.white_facings,
            black_facings=rs_necro.black_facings,
        )

        state = create_game_from_ruleset(combined, seed=42)
        h_dict, _ = get_heuristics(combined)

        result = run_rust_game(state, h_dict, depth=5)

        print(f"\nOrcs vs Necro D5: {result.elapsed:.3f}s, {result.rounds} rounds, "
              f"winner={'White' if result.winner == 0 else 'Black'}")

        assert result.elapsed < 15.0, \
            f"Orcs vs Necro D5 took {result.elapsed:.3f}s, expected < 15.0s"


def run_comprehensive_benchmark():
    """Run a comprehensive benchmark and print results table.

    Call this directly to get detailed timing info:
        python -c "from tests.test_performance import run_comprehensive_benchmark; run_comprehensive_benchmark()"
    """
    print("=" * 60)
    print("HEXWAR Performance Benchmark")
    print("=" * 60)

    rs, state = create_test_game(seed=42)
    h_dict, heuristics = get_heuristics(rs)

    print(f"\nTest config: {state.templates[0]} template, "
          f"{sum(1 for p in state.board.values() if p.owner == 0)} white, "
          f"{sum(1 for p in state.board.values() if p.owner == 1)} black pieces")

    print("\n--- Rust Engine ---")
    print(f"{'Depth':<8} {'Time':>10} {'Limit':>10} {'Status':>10} {'Rounds':>8}")
    print("-" * 50)

    for depth in [2, 3, 4, 5]:
        _, state = create_test_game(seed=42)
        result = run_rust_game(state, h_dict, depth=depth)
        limit = DEPTH_LIMITS[depth]
        status = "PASS" if result.elapsed < limit else "FAIL"
        print(f"D{depth:<7} {result.elapsed:>10.3f}s {limit:>10.1f}s {status:>10} {result.rounds:>8}")

    print("\n--- Python Engine ---")
    print(f"{'Depth':<8} {'Time':>10} {'Rounds':>8}")
    print("-" * 30)

    for depth in [2, 3]:
        _, state = create_test_game(seed=42)
        result = run_python_game(state, heuristics, depth=depth)
        print(f"D{depth:<7} {result.elapsed:>10.3f}s {result.rounds:>8}")

    print("\n" + "=" * 60)


if __name__ == "__main__":
    run_comprehensive_benchmark()
