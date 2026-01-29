"""
HEXWAR Tournament System

Runs depth matchups in parallel for fitness evaluation.
Implements the matchup schedule from the algorithmic spec.
"""

from __future__ import annotations
from dataclasses import dataclass
from typing import Optional, Callable
import multiprocessing as mp
from concurrent.futures import ProcessPoolExecutor, as_completed
import random
import time
import sys

from hexwar.ai import Heuristics
from hexwar.runner import create_bootstrap_game

# Import Rust module for accelerated games - REQUIRED for reasonable performance
try:
    from hexwar_core.hexwar_core import play_game as rust_play_game
    RUST_AVAILABLE = True
except ImportError:
    # Try alternate import path
    try:
        from hexwar_core import play_game as rust_play_game
        RUST_AVAILABLE = True
    except ImportError:
        RUST_AVAILABLE = False
        rust_play_game = None
        print("ERROR: Rust engine not available! Rebuild with: cd hexwar_core && maturin develop --release", file=sys.stderr)
        print("Python fallback is ~100x slower and NOT supported for evolution.", file=sys.stderr)


# Game log callback type
GameLogCallback = Callable[[str], None]


@dataclass
class MatchResult:
    """Result of a single game."""
    white_depth: int
    black_depth: int
    winner: int  # 0=White, 1=Black, -1=Draw
    rounds: int
    seed: int


@dataclass
class MatchupStats:
    """Statistics for a depth pairing."""
    deeper_depth: int
    shallower_depth: int
    deeper_wins: int
    shallower_wins: int
    draws: int
    games_played: int
    white_wins: int = 0  # Track actual color wins for fairness
    black_wins: int = 0
    total_rounds: int = 0  # Sum of rounds across all games

    @property
    def avg_rounds(self) -> float:
        if self.games_played == 0:
            return 0.0
        return self.total_rounds / self.games_played

    @property
    def deeper_win_rate(self) -> float:
        if self.games_played == 0:
            return 0.0
        return self.deeper_wins / self.games_played

    @property
    def white_win_rate(self) -> float:
        """Rate at which white wins (for color fairness)."""
        non_draws = self.white_wins + self.black_wins
        if non_draws == 0:
            return 0.5
        return self.white_wins / non_draws

    @property
    def upset_rate(self) -> float:
        """Rate at which shallower player wins."""
        if self.games_played == 0:
            return 0.0
        return self.shallower_wins / self.games_played


def _play_game_batch(args: tuple) -> list[MatchResult]:
    """Worker function to play MULTIPLE games (reduces dispatch overhead)."""
    game_specs, heuristics_dict, max_moves_per_action, ruleset_dict, use_rust = args

    results = []
    for white_depth, black_depth, seed in game_specs:
        # Create game state
        if ruleset_dict is not None:
            from hexwar.evolution import genome_to_ruleset, create_game_from_ruleset
            ruleset = genome_to_ruleset(ruleset_dict)
            state = create_game_from_ruleset(ruleset, seed=seed)
        else:
            state = create_bootstrap_game(seed=seed)

        # RUST IS REQUIRED - Python fallback is ~100x slower
        if not (use_rust and RUST_AVAILABLE and rust_play_game is not None):
            raise RuntimeError(
                "Rust engine required but not available! "
                "Rebuild with: cd hexwar_core && maturin develop --release"
            )

        white_pieces = []
        black_pieces = []
        for (q, r), piece in state.board.items():
            entry = (piece.type_id, (q, r), piece.facing)
            if piece.owner == 0:
                white_pieces.append(entry)
            else:
                black_pieces.append(entry)

        winner, rounds = rust_play_game(
            white_pieces, black_pieces,
            state.templates[0], state.templates[1],
            white_depth, black_depth,
            heuristics_dict,
            max_moves=500,
            max_moves_per_action=max_moves_per_action,
            seed=seed,
        )

        results.append(MatchResult(
            white_depth=white_depth,
            black_depth=black_depth,
            winner=winner,
            rounds=rounds,
            seed=seed,
        ))

    return results


def _play_single_game(args: tuple) -> MatchResult:
    """Worker function to play a single game (for multiprocessing)."""
    white_depth, black_depth, heuristics_dict, seed, max_moves_per_action, ruleset_dict, use_rust = args

    # Create game state from ruleset if provided, otherwise use bootstrap
    if ruleset_dict is not None:
        from hexwar.evolution import genome_to_ruleset, create_game_from_ruleset
        ruleset = genome_to_ruleset(ruleset_dict)
        state = create_game_from_ruleset(ruleset, seed=seed)
    else:
        state = create_bootstrap_game(seed=seed)

    # RUST IS REQUIRED - Python fallback is ~100x slower
    if not (use_rust and RUST_AVAILABLE and rust_play_game is not None):
        raise RuntimeError(
            "Rust engine required but not available! "
            "Rebuild with: cd hexwar_core && maturin develop --release"
        )

    # Extract pieces for Rust
    white_pieces = []
    black_pieces = []
    for (q, r), piece in state.board.items():
        entry = (piece.type_id, (q, r), piece.facing)
        if piece.owner == 0:
            white_pieces.append(entry)
        else:
            black_pieces.append(entry)

    white_template = state.templates[0]
    black_template = state.templates[1]

    winner, rounds = rust_play_game(
        white_pieces, black_pieces,
        white_template, black_template,
        white_depth, black_depth,
        heuristics_dict,
        max_moves=500,
        max_moves_per_action=max_moves_per_action,
        seed=seed,
    )

    return MatchResult(
        white_depth=white_depth,
        black_depth=black_depth,
        winner=winner,
        rounds=rounds,
        seed=seed,
    )


def run_matchup(
    depth1: int,
    depth2: int,
    n_games: int,
    heuristics: Heuristics,
    base_seed: int = 0,
    n_workers: int = 4,
    max_moves_per_action: int = 15,
    ruleset_dict: dict = None,
    log_callback: GameLogCallback = None,
    ruleset_id: str = None,
    use_rust: bool = True,
) -> MatchupStats:
    """Run a matchup between two depths.

    Plays n_games total, alternating colors.

    Args:
        depth1: First depth
        depth2: Second depth
        n_games: Total games to play
        heuristics: Evaluation heuristics
        base_seed: Base seed for reproducibility
        n_workers: Number of parallel workers
        max_moves_per_action: Move limit per search node
        ruleset_dict: Optional ruleset genome dict (if None, uses bootstrap)
        log_callback: Optional callback for per-game logging
        ruleset_id: Optional identifier for logging
        use_rust: Use Rust accelerated game engine if available (default: True)

    Returns:
        MatchupStats for the matchup
    """
    deeper = max(depth1, depth2)
    shallower = min(depth1, depth2)

    # Prepare game arguments
    # Serialize heuristics for multiprocessing
    h_dict = {
        'white_piece_values': heuristics.white_piece_values,
        'black_piece_values': heuristics.black_piece_values,
        'white_center_weight': heuristics.white_center_weight,
        'black_center_weight': heuristics.black_center_weight,
    }

    # Build game specs: (white_depth, black_depth, seed)
    game_specs = []
    for i in range(n_games):
        seed = base_seed + i
        if i % 2 == 0:
            game_specs.append((deeper, shallower, seed))
        else:
            game_specs.append((shallower, deeper, seed))

    # Run games in parallel with batching for efficiency
    results = []
    if n_workers > 1:
        # Distribute games into batches for each worker
        # More games per batch = less dispatch overhead but less load balancing
        games_per_batch = max(1, len(game_specs) // (n_workers * 2))  # ~2 batches per worker
        batches = []
        for i in range(0, len(game_specs), games_per_batch):
            batch_specs = game_specs[i:i + games_per_batch]
            batches.append((batch_specs, h_dict, max_moves_per_action, ruleset_dict, use_rust))

        with ProcessPoolExecutor(max_workers=n_workers) as executor:
            futures = [executor.submit(_play_game_batch, b) for b in batches]
            for future in as_completed(futures):
                results.extend(future.result())
    else:
        # Sequential - still use batch function for consistency
        batch_results = _play_game_batch((game_specs, h_dict, max_moves_per_action, ruleset_dict, use_rust))
        results.extend(batch_results)

    # Tally results and log each game
    deeper_wins = 0
    shallower_wins = 0
    white_wins = 0
    black_wins = 0
    draws = 0
    total_rounds = 0

    for r in results:
        total_rounds += r.rounds

        # Determine which depth won
        winner_str = "Draw"
        if r.winner == -1:
            draws += 1
        elif r.winner == 0:  # White won
            white_wins += 1
            winner_str = f"White(d{r.white_depth})"
            if r.white_depth == deeper:
                deeper_wins += 1
            else:
                shallower_wins += 1
        else:  # Black won
            black_wins += 1
            winner_str = f"Black(d{r.black_depth})"
            if r.black_depth == deeper:
                deeper_wins += 1
            else:
                shallower_wins += 1

        # Log the game if callback provided
        if log_callback:
            rs_label = f"RS#{ruleset_id}" if ruleset_id else "default"
            log_callback(
                f"[{rs_label}] d{r.white_depth}(W) vs d{r.black_depth}(B) -> "
                f"{winner_str} in {r.rounds} rounds (seed={r.seed})"
            )

    return MatchupStats(
        deeper_depth=deeper,
        shallower_depth=shallower,
        deeper_wins=deeper_wins,
        shallower_wins=shallower_wins,
        draws=draws,
        games_played=len(results),
        white_wins=white_wins,
        black_wins=black_wins,
        total_rounds=total_rounds,
    )


@dataclass
class TournamentResult:
    """Result of a full tournament evaluation."""
    matchups: dict[tuple[int, int], MatchupStats]
    skill_gradient: float
    color_fairness: float
    game_richness: float
    combined_fitness: float
    total_games: int
    elapsed_seconds: float


def run_fitness_tournament(
    heuristics: Heuristics,
    base_seed: int = 0,
    n_workers: int = 4,
    max_moves_per_action: int = 15,
    reduced: bool = False,
) -> TournamentResult:
    """Run full fitness evaluation tournament.

    Matchups from spec:
    - d2 vs d3: 20 games (10 per color)
    - d3 vs d4: 20 games
    - d4 vs d5: 20 games
    - d2 vs d4: 20 games (weight 1.5)
    - d3 vs d5: 20 games (weight 1.5)
    - d2 vs d5: 10 games (weight 2.0)

    Args:
        heuristics: Heuristics to evaluate
        base_seed: Base random seed
        n_workers: Parallel workers
        max_moves_per_action: Move limit per node
        reduced: If True, run reduced tournament (fewer games)

    Returns:
        TournamentResult with fitness components
    """
    start_time = time.time()

    # Define matchups: (depth1, depth2, n_games, weight)
    if reduced:
        # Reduced for faster testing
        matchup_spec = [
            (2, 3, 4, 1.0),
            (3, 4, 4, 1.0),
            (2, 4, 4, 1.5),
        ]
    else:
        matchup_spec = [
            (2, 3, 20, 1.0),
            (3, 4, 20, 1.0),
            (4, 5, 20, 1.0),
            (2, 4, 20, 1.5),
            (3, 5, 20, 1.5),
            (2, 5, 10, 2.0),
        ]

    matchups = {}
    total_games = 0
    seed_offset = 0

    for d1, d2, n_games, weight in matchup_spec:
        stats = run_matchup(
            d1, d2, n_games, heuristics,
            base_seed=base_seed + seed_offset,
            n_workers=n_workers,
            max_moves_per_action=max_moves_per_action,
            use_rust=use_rust,
        )
        matchups[(d1, d2)] = stats
        total_games += n_games
        seed_offset += n_games

    # Calculate fitness components

    # 1. Skill Gradient (weight 0.5)
    # Deeper player should win more often; larger gaps = more decisive
    weighted_sum = 0.0
    weight_total = 0.0
    for (d1, d2), stats in matchups.items():
        gap = abs(d1 - d2)
        weight = 1.0 + (gap - 1) * 0.5  # Higher weight for larger gaps
        weighted_sum += stats.deeper_win_rate * weight
        weight_total += weight

    skill_gradient = weighted_sum / weight_total if weight_total > 0 else 0.0

    # 2. Color Fairness (weight 0.3)
    # When upsets happen, they should be evenly distributed
    # Also at equal depth, wins should be 50/50
    white_upsets = 0
    black_upsets = 0
    total_upsets = 0

    for (d1, d2), stats in matchups.items():
        # This is simplified - would need per-game color tracking
        # For now, assume roughly even color distribution in upsets
        total_upsets += stats.shallower_wins

    # Fairness = 1.0 when perfect, decreases with imbalance
    # Without per-game color data, approximate as 1.0 - upset_rate
    overall_upset_rate = total_upsets / total_games if total_games > 0 else 0
    color_fairness = 1.0 - overall_upset_rate

    # 3. Game Richness (weight 0.2)
    # Average game length as fraction of max (50 rounds)
    # Would need to track this from actual games
    # For now, estimate based on typical random game length (~40 rounds)
    game_richness = 0.8  # Placeholder

    # Combined fitness
    combined_fitness = (
        0.5 * skill_gradient +
        0.3 * color_fairness +
        0.2 * game_richness
    )

    elapsed = time.time() - start_time

    return TournamentResult(
        matchups=matchups,
        skill_gradient=skill_gradient,
        color_fairness=color_fairness,
        game_richness=game_richness,
        combined_fitness=combined_fitness,
        total_games=total_games,
        elapsed_seconds=elapsed,
    )


@dataclass
class RulesetEvalResult:
    """Result of evaluating a ruleset via tournament."""
    fitness: float
    skill_gradient: float
    color_fairness: float
    game_richness: float
    white_wins: int
    black_wins: int
    draws: int
    total_games: int
    avg_rounds: float
    matchups: dict


def evaluate_ruleset_tournament(
    ruleset_dict: dict,
    heuristics: Heuristics,
    base_seed: int = 0,
    n_workers: int = 4,
    max_moves_per_action: int = 15,
    reduced: bool = True,
    log_callback: GameLogCallback = None,
    ruleset_id: str = None,
    use_rust: bool = True,
    depth: int = 2,
    games_per_matchup: int = None,
) -> RulesetEvalResult:
    """Evaluate a ruleset using the full tournament matchup spec.

    Runs games at varied depths to test skill gradient.
    Depths are relative to the base depth parameter.

    Args:
        ruleset_dict: Ruleset genome dict
        heuristics: Evaluation heuristics
        base_seed: Base random seed
        n_workers: Parallel workers
        max_moves_per_action: Move limit per node
        reduced: If True, run reduced matchups (faster for evolution)
        log_callback: Optional callback for per-game logging
        ruleset_id: Optional identifier for logging
        use_rust: Use Rust engine
        depth: Base search depth for AI (matchups use depth and depth-1)
        games_per_matchup: If provided, overrides the default games per matchup type.
                          Total games = games_per_matchup * number_of_matchup_types

    Returns:
        RulesetEvalResult with fitness components
    """
    # Define matchups across a WIDE depth range
    # Include games from d2 up to the target depth for comprehensive testing
    # This tests skill gradient at multiple depth levels
    d = max(depth, 2)

    # Default games per matchup if not specified
    if games_per_matchup is None:
        base_games = 2 if reduced else 4
    else:
        base_games = games_per_matchup

    # Build matchup spec including ALL depth tiers up to target
    # Lower tiers are cheap and provide good signal
    matchup_spec = []

    # Standard depth tiers: 2, 4, 6, 8, 10...
    tiers = [t for t in range(2, d + 1, 2)]
    if d not in tiers:
        tiers.append(d)
    tiers.sort()

    for tier in tiers:
        is_target = (tier == d)

        if reduced:
            # Reduced mode: fewer games, but still test all tiers
            if is_target:
                # Target depth gets 2x games and higher weight
                n_games = base_games * 2
                weight_equal = 1.5
                weight_skill_1ply = 1.5
                weight_skill_2ply = 2.5  # 2-ply gaps are more informative
            else:
                # Lower tiers get base games (they're cheap anyway)
                n_games = base_games
                # Weight increases with depth tier
                weight_equal = 0.6 + (tier / 10)
                weight_skill_1ply = 0.8 + (tier / 10)
                weight_skill_2ply = 1.2 + (tier / 10)  # Higher weight for 2-ply
        else:
            # Full mode: same games per tier, weights increase with depth
            n_games = base_games
            weight_equal = 0.6 + (tier / 10)
            weight_skill_1ply = 0.8 + (tier / 10)
            weight_skill_2ply = 1.2 + (tier / 10)
            if is_target:
                weight_equal += 0.3
                weight_skill_1ply += 0.3
                weight_skill_2ply += 0.5

        # Equal depth matchup (tests color fairness)
        matchup_spec.append((tier, tier, n_games, weight_equal))
        # 1-ply skill gradient: stronger (tier) vs weaker (tier-1) - handicap DOWN
        if tier >= 3:  # Need at least depth 2 for weaker player
            matchup_spec.append((tier, tier - 1, n_games, weight_skill_1ply))
        # 2-ply skill gradient: stronger (tier) vs weaker (tier-2) - handicap DOWN
        if tier >= 4:  # Need at least depth 2 for weaker player
            matchup_spec.append((tier, tier - 2, n_games, weight_skill_2ply))

    matchups = {}
    total_games = 0
    seed_offset = 0
    total_rounds = 0
    white_wins_total = 0
    black_wins_total = 0
    draws_total = 0

    for d1, d2, n_games, weight in matchup_spec:
        stats = run_matchup(
            d1, d2, n_games, heuristics,
            base_seed=base_seed + seed_offset,
            n_workers=n_workers,
            max_moves_per_action=max_moves_per_action,
            use_rust=use_rust,
            ruleset_dict=ruleset_dict,
            log_callback=log_callback,
            ruleset_id=ruleset_id,
        )
        matchups[(d1, d2)] = stats
        total_games += n_games
        seed_offset += n_games

        # Track overall stats using actual color wins
        white_wins_total += stats.white_wins
        black_wins_total += stats.black_wins
        draws_total += stats.draws
        total_rounds += stats.total_rounds

    # Calculate fitness components

    # 1. Skill Gradient (deeper player should win more often)
    weighted_sum = 0.0
    weight_total = 0.0
    for (d1, d2), stats in matchups.items():
        if d1 != d2:  # Only count asymmetric matchups
            gap = abs(d1 - d2)
            weight = 1.0 + (gap - 1) * 0.5
            weighted_sum += stats.deeper_win_rate * weight
            weight_total += weight

    skill_gradient = weighted_sum / weight_total if weight_total > 0 else 0.5

    # 2. Color Fairness (at equal depths, should be 50/50 white/black wins)
    equal_depth_games = 0
    equal_depth_balance = 0.0
    for (d1, d2), stats in matchups.items():
        if d1 == d2:
            equal_depth_games += stats.games_played
            # Use actual white win rate for color fairness
            win_rate = stats.white_win_rate
            # Penalize deviation from 50% - score of 1.0 means perfect 50/50
            equal_depth_balance += (1.0 - abs(win_rate - 0.5) * 2) * stats.games_played

    color_fairness = equal_depth_balance / equal_depth_games if equal_depth_games > 0 else 0.5

    # 3. Game Richness (average game length, normalized)
    # Good games last 15-50 rounds; too short or too long is less ideal
    avg_rounds = total_rounds / total_games if total_games > 0 else 0
    if avg_rounds < 10:
        game_richness = avg_rounds / 10.0  # Too short
    elif avg_rounds > 60:
        game_richness = max(0.5, 1.0 - (avg_rounds - 60) / 100.0)  # Too long
    else:
        game_richness = 1.0  # Ideal range

    # 4. Decisiveness (fewer draws is better)
    decisiveness = 1.0 - (draws_total / total_games) if total_games > 0 else 0.5

    # Combined fitness:
    # - Skill gradient: game rewards deeper thinking (MUST be near 100%)
    # - Color fairness: neither color has inherent advantage
    # - Decisiveness: games have clear outcomes
    #
    # IMPORTANT: Skill gradient should be near 100% like chess engines.
    # A d5 beating a d6 should be alarming. We use a non-linear penalty
    # that severely punishes skill gradients below 90%.
    if skill_gradient >= 0.95:
        skill_score = 1.0  # Perfect
    elif skill_gradient >= 0.90:
        skill_score = 0.9 + (skill_gradient - 0.90) * 2  # 0.9-1.0
    elif skill_gradient >= 0.80:
        skill_score = 0.6 + (skill_gradient - 0.80) * 3  # 0.6-0.9
    elif skill_gradient >= 0.65:
        skill_score = 0.3 + (skill_gradient - 0.65) * 2  # 0.3-0.6
    else:
        skill_score = skill_gradient * 0.5  # Harsh penalty below 65%

    fitness = (
        0.40 * skill_score +       # Skill is paramount (non-linear)
        0.35 * color_fairness +    # Balance is crucial
        0.15 * game_richness +     # Games have depth
        0.10 * decisiveness        # Clear outcomes
    )

    # CRITICAL: Penalize if one color never wins at equal depth
    if equal_depth_games >= 4:
        for (d1, d2), stats in matchups.items():
            if d1 == d2:
                # If one color wins 0%, massive penalty (indicates broken balance)
                if stats.white_wins == 0 or stats.black_wins == 0:
                    fitness *= 0.3  # Heavy penalty

    # CRITICAL: Penalize if deeper player loses too often
    if skill_gradient < 0.80:
        fitness *= 0.5  # Skill must be rewarded

    return RulesetEvalResult(
        fitness=fitness,
        skill_gradient=skill_gradient,
        color_fairness=color_fairness,
        game_richness=game_richness,
        white_wins=white_wins_total,
        black_wins=black_wins_total,
        draws=draws_total,
        total_games=total_games,
        avg_rounds=avg_rounds,
        matchups=matchups,
    )


def print_tournament_result(result: TournamentResult) -> None:
    """Print tournament results in human-readable format."""
    print("\n" + "=" * 60)
    print("TOURNAMENT RESULTS")
    print("=" * 60)

    print(f"\nGames played: {result.total_games}")
    print(f"Time elapsed: {result.elapsed_seconds:.1f}s")

    print("\nMatchup Results:")
    print("-" * 40)
    for (d1, d2), stats in sorted(result.matchups.items()):
        print(f"  d{stats.shallower_depth} vs d{stats.deeper_depth}: "
              f"deeper wins {stats.deeper_wins}/{stats.games_played} "
              f"({stats.deeper_win_rate*100:.0f}%)")

    print("\nFitness Components:")
    print(f"  Skill Gradient: {result.skill_gradient:.3f}")
    print(f"  Color Fairness: {result.color_fairness:.3f}")
    print(f"  Game Richness:  {result.game_richness:.3f}")
    print(f"  COMBINED:       {result.combined_fitness:.3f}")


if __name__ == '__main__':
    print("HEXWAR Tournament System - Phase 3 Test")
    print("=" * 50)

    heuristics = Heuristics.create_default()

    # Quick single matchup test
    print("\nRunning d2 vs d3 matchup (4 games)...")
    stats = run_matchup(2, 3, 4, heuristics, base_seed=42, n_workers=1, max_moves_per_action=10)
    print(f"  Deeper (d3) wins: {stats.deeper_wins}/{stats.games_played}")
    print(f"  Upsets (d2 wins): {stats.shallower_wins}")

    print("\nPhase 3: Tournament System complete!")
