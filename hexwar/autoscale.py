"""
Dynamic worker scaling for parallel game evaluation.

Automatically probes throughput and scales workers until diminishing returns.
"""

import os
import time
import multiprocessing as mp
from concurrent.futures import ProcessPoolExecutor, as_completed
from dataclasses import dataclass
from typing import Optional, Callable
import random


@dataclass
class ScaleResult:
    """Result of worker scaling probe."""
    optimal_workers: int
    throughput: float  # games per second
    probe_history: list[tuple[int, float]]  # (workers, throughput) pairs


# Global cache for optimal worker count
_cached_optimal_workers: Optional[int] = None
_cache_timestamp: float = 0
_CACHE_TTL = 300  # 5 minutes


def _simple_cpu_work(iterations: int = 100000) -> float:
    """Simple CPU-bound work to measure throughput."""
    total = 0.0
    for i in range(iterations):
        total += (i * 0.001) ** 0.5
    return total


def _probe_worker(args: tuple) -> float:
    """Worker function for throughput probing."""
    work_units, seed = args
    random.seed(seed)
    start = time.perf_counter()
    _simple_cpu_work(work_units)
    return time.perf_counter() - start


def probe_throughput(
    n_workers: int,
    work_units: int = 50000,
    n_tasks: int = None,
) -> float:
    """
    Probe throughput with a given number of workers.

    Returns: tasks per second
    """
    if n_tasks is None:
        n_tasks = max(n_workers * 4, 16)  # Enough tasks to saturate workers

    tasks = [(work_units, i) for i in range(n_tasks)]

    start = time.perf_counter()

    if n_workers == 1:
        for task in tasks:
            _probe_worker(task)
    else:
        with ProcessPoolExecutor(max_workers=n_workers) as executor:
            list(executor.map(_probe_worker, tasks))

    elapsed = time.perf_counter() - start
    return n_tasks / elapsed if elapsed > 0 else 0


def find_optimal_workers(
    max_workers: int = None,
    min_workers: int = 2,
    improvement_threshold: float = 0.05,  # 5% improvement required to keep scaling
    verbose: bool = False,
) -> ScaleResult:
    """
    Find optimal worker count by probing throughput.

    Algorithm:
    1. Start at min_workers
    2. Measure throughput
    3. Try more workers (scale by 1.5x or +2, whichever is larger)
    4. If improvement > threshold, continue
    5. Stop when improvement drops below threshold or hit ceiling

    Args:
        max_workers: Hard ceiling (default: min(60, CPU count * 4))
        min_workers: Starting point
        improvement_threshold: Min improvement to keep scaling (0.10 = 10%)
        verbose: Print progress

    Returns:
        ScaleResult with optimal worker count and history
    """
    if max_workers is None:
        # Allow oversubscription up to 4x CPU count, capped at 60
        max_workers = min(60, mp.cpu_count() * 4)

    # Clamp min_workers
    min_workers = max(1, min(min_workers, max_workers))

    history = []
    current_workers = min_workers
    best_workers = min_workers
    best_throughput = 0.0

    while current_workers <= max_workers:
        # Probe throughput at current worker count
        throughput = probe_throughput(current_workers)
        history.append((current_workers, throughput))

        if verbose:
            improvement = ((throughput / best_throughput) - 1) * 100 if best_throughput > 0 else 0
            print(f"  Workers={current_workers}: {throughput:.1f} tasks/sec "
                  f"({improvement:+.1f}% vs best)")

        # Check if this is better
        if throughput > best_throughput:
            improvement_ratio = (throughput / best_throughput) - 1 if best_throughput > 0 else 1.0

            if improvement_ratio >= improvement_threshold or best_throughput == 0:
                best_workers = current_workers
                best_throughput = throughput
            else:
                # Improvement too small, stop scaling
                if verbose:
                    print(f"  Improvement ({improvement_ratio*100:.1f}%) below threshold "
                          f"({improvement_threshold*100:.0f}%), stopping")
                break
        else:
            # Throughput decreased, stop scaling
            if verbose:
                print(f"  Throughput decreased, stopping")
            break

        # Scale up for next iteration
        next_workers = max(current_workers + 2, int(current_workers * 1.5))
        if next_workers == current_workers:
            next_workers = current_workers + 1

        if next_workers > max_workers:
            break

        current_workers = next_workers

    return ScaleResult(
        optimal_workers=best_workers,
        throughput=best_throughput,
        probe_history=history,
    )


def get_optimal_workers(
    max_workers: int = None,
    force_reprobe: bool = False,
    verbose: bool = False,
) -> int:
    """
    Get optimal worker count, using cached value if available.

    This is the main entry point - call this to get worker count.

    Args:
        max_workers: Hard ceiling
        force_reprobe: Ignore cache and re-probe
        verbose: Print probing progress

    Returns:
        Optimal number of workers
    """
    global _cached_optimal_workers, _cache_timestamp

    now = time.time()

    # Use cache if valid
    if not force_reprobe and _cached_optimal_workers is not None:
        if now - _cache_timestamp < _CACHE_TTL:
            return _cached_optimal_workers

    if verbose:
        print("Probing for optimal worker count...")

    result = find_optimal_workers(max_workers=max_workers, verbose=verbose)

    _cached_optimal_workers = result.optimal_workers
    _cache_timestamp = now

    if verbose:
        print(f"Optimal workers: {result.optimal_workers} "
              f"({result.throughput:.1f} tasks/sec)")

    return result.optimal_workers


def probe_game_throughput(
    game_runner: Callable,
    n_workers: int,
    n_games: int = 8,
) -> float:
    """
    Probe throughput using actual game execution.

    Args:
        game_runner: Function that takes (n_workers, n_games) and runs games
        n_workers: Number of workers to test
        n_games: Number of games to run

    Returns:
        Games per second
    """
    start = time.perf_counter()
    game_runner(n_workers, n_games)
    elapsed = time.perf_counter() - start
    return n_games / elapsed if elapsed > 0 else 0


def find_optimal_game_workers(
    game_runner: Callable,
    max_workers: int = None,
    min_workers: int = 2,
    games_per_probe: int = 4,
    improvement_threshold: float = 0.08,  # 8% for games (allow more scaling)
    verbose: bool = False,
) -> ScaleResult:
    """
    Find optimal workers using actual game execution for probing.

    More accurate than synthetic benchmark but slower.

    Args:
        game_runner: Function(n_workers, n_games) -> runs games
        max_workers: Hard ceiling (default: min(60, CPU count * 4))
        min_workers: Starting point
        games_per_probe: Games to run per probe
        improvement_threshold: Min improvement to keep scaling
        verbose: Print progress
    """
    if max_workers is None:
        max_workers = min(60, mp.cpu_count() * 4)

    min_workers = max(1, min(min_workers, max_workers))

    history = []
    current_workers = min_workers
    best_workers = min_workers
    best_throughput = 0.0

    while current_workers <= max_workers:
        throughput = probe_game_throughput(game_runner, current_workers, games_per_probe)
        history.append((current_workers, throughput))

        if verbose:
            print(f"  Workers={current_workers}: {throughput:.2f} games/sec")

        if throughput > best_throughput:
            improvement = (throughput / best_throughput) - 1 if best_throughput > 0 else 1.0

            if improvement >= improvement_threshold or best_throughput == 0:
                best_workers = current_workers
                best_throughput = throughput
            else:
                if verbose:
                    print(f"  Improvement too small, stopping")
                break
        else:
            if verbose:
                print(f"  Throughput decreased, stopping")
            break

        # Scale up
        next_workers = max(current_workers + 2, int(current_workers * 1.5))
        if next_workers > max_workers:
            break
        current_workers = next_workers

    return ScaleResult(
        optimal_workers=best_workers,
        throughput=best_throughput,
        probe_history=history,
    )


def find_optimal_rust_workers(
    depth: int = 2,
    games_per_probe: int = 20,
    max_workers: int = None,
    verbose: bool = False,
) -> ScaleResult:
    """
    Find optimal workers using actual Rust game execution.

    This probes with real games to find the best worker count for
    the actual workload. More accurate but slower than synthetic probing.

    Args:
        depth: Search depth for games (higher = more compute per game)
        games_per_probe: Games to run per probe point
        max_workers: Hard ceiling
        verbose: Print progress
    """
    from hexwar.tournament import run_matchup, RUST_AVAILABLE
    from hexwar.ai import Heuristics

    if not RUST_AVAILABLE:
        if verbose:
            print("Rust module not available, falling back to synthetic probe")
        return find_optimal_workers(max_workers=max_workers, verbose=verbose)

    h = Heuristics.create_default()

    def game_runner(n_workers: int, n_games: int):
        run_matchup(depth, depth, n_games, h, n_workers=n_workers, use_rust=True)

    if verbose:
        print(f"Probing with Rust games (d{depth} vs d{depth}, {games_per_probe} games/probe)...")

    return find_optimal_game_workers(
        game_runner,
        max_workers=max_workers,
        games_per_probe=games_per_probe,
        verbose=verbose,
    )


if __name__ == '__main__':
    print(f"CPU count: {mp.cpu_count()}")
    print()

    print("=== Synthetic Probe ===")
    result = find_optimal_workers(verbose=True)
    print(f"Result: {result.optimal_workers} workers optimal")
    print()

    print("=== Rust Game Probe (d2) ===")
    result = find_optimal_rust_workers(depth=2, games_per_probe=20, verbose=True)
    print(f"Result: {result.optimal_workers} workers optimal")
    print()
