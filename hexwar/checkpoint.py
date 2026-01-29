"""
HEXWAR Checkpoint System

Save and restore evolution state for long-running experiments.
"""

from __future__ import annotations
import json
from pathlib import Path
from datetime import datetime
from typing import Optional, Any
import random


def save_checkpoint(
    filepath: Path | str,
    generation: int,
    phase: str,
    population: list[dict],
    fitness_scores: list[float],
    best_heuristics: dict,
    best_ruleset: Optional[dict],
    rng_state: Any,
    elapsed_seconds: float,
    extra: Optional[dict] = None,
) -> None:
    """Save evolution checkpoint to JSON file.

    Args:
        filepath: Path to save checkpoint
        generation: Current generation number
        phase: Current phase ('heuristic' or 'ruleset')
        population: List of genome dicts
        fitness_scores: Fitness for each individual
        best_heuristics: Best heuristics found so far
        best_ruleset: Best rule set found (if applicable)
        rng_state: Random number generator state
        elapsed_seconds: Total elapsed time
        extra: Additional data to save
    """
    checkpoint = {
        'version': '1.0',
        'timestamp': datetime.now().isoformat(),
        'generation': generation,
        'phase': phase,
        'population': population,
        'fitness_scores': fitness_scores,
        'best_heuristics': best_heuristics,
        'best_ruleset': best_ruleset,
        'rng_state': rng_state,
        'elapsed_seconds': elapsed_seconds,
    }

    if extra:
        checkpoint['extra'] = extra

    filepath = Path(filepath)
    filepath.parent.mkdir(parents=True, exist_ok=True)

    with open(filepath, 'w') as f:
        json.dump(checkpoint, f, indent=2)


def load_checkpoint(filepath: Path | str) -> dict:
    """Load evolution checkpoint from JSON file.

    Args:
        filepath: Path to checkpoint file

    Returns:
        Checkpoint data dict
    """
    with open(filepath, 'r') as f:
        return json.load(f)


def get_latest_checkpoint(checkpoint_dir: Path | str) -> Optional[Path]:
    """Find the most recent checkpoint in a directory.

    Args:
        checkpoint_dir: Directory containing checkpoints

    Returns:
        Path to latest checkpoint, or None if no checkpoints exist
    """
    checkpoint_dir = Path(checkpoint_dir)
    if not checkpoint_dir.exists():
        return None

    checkpoints = list(checkpoint_dir.glob('checkpoint_*.json'))
    if not checkpoints:
        return None

    # Sort by modification time
    checkpoints.sort(key=lambda p: p.stat().st_mtime, reverse=True)
    return checkpoints[0]


def create_checkpoint_name(generation: int, phase: str) -> str:
    """Create a checkpoint filename.

    Args:
        generation: Current generation
        phase: Current phase

    Returns:
        Checkpoint filename
    """
    timestamp = datetime.now().strftime('%Y%m%d_%H%M%S')
    return f'checkpoint_{phase}_gen{generation:04d}_{timestamp}.json'


# Convenience functions for common checkpoint patterns

def checkpoint_heuristic_evolution(
    filepath: Path | str,
    generation: int,
    population: list[dict],
    fitness_scores: list[float],
    best_genome: dict,
    rng: random.Random,
    elapsed: float,
) -> None:
    """Save heuristic evolution checkpoint."""
    save_checkpoint(
        filepath=filepath,
        generation=generation,
        phase='heuristic',
        population=population,
        fitness_scores=fitness_scores,
        best_heuristics=best_genome,
        best_ruleset=None,
        rng_state=rng.getstate(),
        elapsed_seconds=elapsed,
    )


def checkpoint_ruleset_evolution(
    filepath: Path | str,
    generation: int,
    population: list[dict],
    fitness_scores: list[float],
    best_heuristics: dict,
    best_ruleset: dict,
    rng: random.Random,
    elapsed: float,
) -> None:
    """Save rule set evolution checkpoint."""
    save_checkpoint(
        filepath=filepath,
        generation=generation,
        phase='ruleset',
        population=population,
        fitness_scores=fitness_scores,
        best_heuristics=best_heuristics,
        best_ruleset=best_ruleset,
        rng_state=rng.getstate(),
        elapsed_seconds=elapsed,
    )


if __name__ == '__main__':
    import tempfile

    print("HEXWAR Checkpoint System - Test")
    print("=" * 50)

    # Test save/load
    with tempfile.TemporaryDirectory() as tmpdir:
        ckpt_path = Path(tmpdir) / 'test_checkpoint.json'

        rng = random.Random(42)
        save_checkpoint(
            filepath=ckpt_path,
            generation=5,
            phase='heuristic',
            population=[{'test': 1}, {'test': 2}],
            fitness_scores=[0.8, 0.6],
            best_heuristics={'white_center_weight': 0.5},
            best_ruleset=None,
            rng_state=rng.getstate(),
            elapsed_seconds=123.4,
        )

        print(f"Saved checkpoint to: {ckpt_path}")

        loaded = load_checkpoint(ckpt_path)
        print(f"Loaded generation: {loaded['generation']}")
        print(f"Phase: {loaded['phase']}")
        print(f"Population size: {len(loaded['population'])}")

        # Test latest checkpoint finder
        latest = get_latest_checkpoint(tmpdir)
        print(f"Latest checkpoint: {latest.name if latest else 'None'}")

    print("\nCheckpoint system working!")
