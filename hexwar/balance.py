#!/usr/bin/env python3
"""
HEXWAR Balancing Pipeline

Runs evolutionary balancing to find balanced army compositions.
Uses template-aware heuristics that value pieces based on how useful
they are with each ruleset's action templates.

Usage:
    python -m hexwar.balance [options]
"""

import argparse
import json
import multiprocessing
import os
import time
from datetime import datetime
from pathlib import Path

from hexwar.ai import Heuristics
from hexwar.evolution import (
    evolve_rulesets,
    heuristics_to_genome,
    ruleset_to_genome,
    RuleSet,
    create_bootstrap_ruleset,
    evaluate_ruleset_fitness,
    create_template_aware_heuristics,
    load_seed_rulesets,
)
from hexwar.pieces import PIECE_TYPES


# Direction names for human-readable output
DIRECTION_NAMES = {
    0: 'Forward',
    1: 'Forward-Right',
    2: 'Back-Right',
    3: 'Backward',
    4: 'Back-Left',
    5: 'Forward-Left',
}

# Template descriptions
TEMPLATE_DESCRIPTIONS = {
    'A': 'Rotate, then Move (same piece)',
    'B': 'Move, Rotate, Rotate',
    'C': 'Move, Move, Rotate',
    'D': 'Move, then Rotate (different piece)',
    'E': 'Move OR Rotate (chess-like)',
    'F': 'Move, then Rotate (same piece)',
}


def get_piece_description(piece_id: str) -> str:
    """Get human-readable description of a piece's movement."""
    pt = PIECE_TYPES.get(piece_id)
    if pt is None:
        return f"Unknown piece: {piece_id}"

    # Build direction list
    dir_names = [DIRECTION_NAMES[d] for d in pt.directions]
    dir_str = ', '.join(dir_names) if dir_names else 'None'

    # Movement description
    if pt.move_type == 'STEP':
        if pt.move_range == 1:
            move_desc = f"Steps 1 hex"
        else:
            move_desc = f"Steps up to {pt.move_range} hexes"
    elif pt.move_type == 'SLIDE':
        move_desc = "Slides any distance"
    elif pt.move_type == 'JUMP':
        move_desc = f"Jumps exactly {pt.move_range} hexes (over pieces)"
    else:
        move_desc = "Cannot move normally"

    # Special ability
    special_desc = ""
    if pt.special == 'SWAP_MOVE':
        special_desc = " | SPECIAL: Swaps with ally instead of moving"
    elif pt.special == 'SWAP_ROTATE':
        special_desc = " | SPECIAL: Can swap with adjacent ally after rotating"
    elif pt.special == 'RESURRECT':
        special_desc = " | SPECIAL: Can resurrect a captured ally in graveyard"
    elif pt.special == 'PHASED':
        special_desc = " | SPECIAL: Cannot be captured (but can capture)"

    king_note = " [KING - must protect!]" if pt.is_king else ""

    return f"{pt.name} ({piece_id}){king_note}: {move_desc} in directions: {dir_str}{special_desc}"


def generate_human_readable_report(
    ruleset: RuleSet,
    heuristics: Heuristics,
    eval_result: dict,
    config: dict,
    timings: dict,
) -> str:
    """Generate a human-readable report for the balanced game."""
    lines = []
    lines.append("=" * 70)
    lines.append("HEXWAR BALANCED GAME CONFIGURATION")
    lines.append("=" * 70)
    lines.append("")
    lines.append(f"Generated: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    lines.append("")

    # Configuration used
    lines.append("-" * 70)
    lines.append("EVOLUTION PARAMETERS")
    lines.append("-" * 70)
    lines.append(f"Search Depth: {config.get('depth', 2)} (AI looks ahead this many turns)")
    lines.append("Heuristics: Template-aware (computed per-ruleset)")
    lines.append(f"Ruleset Generations: {config.get('ruleset_generations', '?')}")
    lines.append(f"Ruleset Population: {config.get('ruleset_population', '?')}")
    lines.append(f"Games per Evaluation: {config.get('games_per_eval', '?')}")
    lines.append(f"Total Evolution Time: {format_duration(timings.get('total', 0))}")
    lines.append("")

    # Balance stats
    lines.append("-" * 70)
    lines.append("BALANCE STATISTICS")
    lines.append("-" * 70)
    lines.append(f"Evaluation Games: {eval_result['total_games']}")
    lines.append(f"White Wins: {eval_result['white_wins']} ({eval_result['white_wins']/eval_result['total_games']*100:.1f}%)")
    lines.append(f"Black Wins: {eval_result['black_wins']} ({eval_result['black_wins']/eval_result['total_games']*100:.1f}%)")
    lines.append(f"Draws: {eval_result['draws']} ({eval_result['draws']/eval_result['total_games']*100:.1f}%)")
    lines.append(f"Average Game Length: {eval_result['avg_rounds']:.1f} rounds")
    lines.append(f"Color Fairness Score: {eval_result['color_fairness']:.3f} (1.0 = perfect 50/50)")
    lines.append(f"Overall Fitness: {eval_result['fitness']:.3f}")
    lines.append("")

    # Templates explanation
    lines.append("-" * 70)
    lines.append("ACTION TEMPLATES")
    lines.append("-" * 70)
    lines.append("Each turn, a player performs actions according to their template:")
    lines.append("")
    for t, desc in TEMPLATE_DESCRIPTIONS.items():
        marker = ""
        if t == ruleset.white_template:
            marker += " <- WHITE"
        if t == ruleset.black_template:
            marker += " <- BLACK"
        lines.append(f"  Template {t}: {desc}{marker}")
    lines.append("")

    # White Army
    lines.append("-" * 70)
    lines.append(f"WHITE ARMY (Template {ruleset.white_template}: {TEMPLATE_DESCRIPTIONS[ruleset.white_template]})")
    lines.append("-" * 70)
    lines.append("")
    lines.append("KING:")
    lines.append(f"  {get_piece_description(ruleset.white_king)}")
    lines.append("")
    lines.append("PIECES:")

    # Count and describe white pieces
    white_counts = {}
    for p in ruleset.white_pieces:
        white_counts[p] = white_counts.get(p, 0) + 1

    for piece_id in sorted(white_counts.keys()):
        count = white_counts[piece_id]
        desc = get_piece_description(piece_id)
        lines.append(f"  {count}x {desc}")
    lines.append(f"\n  TOTAL: 1 King + {len(ruleset.white_pieces)} pieces = {len(ruleset.white_pieces) + 1} units")

    # Show fixed positions if available
    if ruleset.white_positions:
        lines.append("")
        lines.append("STARTING POSITIONS (q, r hex coordinates):")
        lines.append(f"  King: {ruleset.white_positions[0]}")
        for i, pos in enumerate(ruleset.white_positions[1:]):
            if i < len(ruleset.white_pieces):
                piece_name = PIECE_TYPES[ruleset.white_pieces[i]].name
                lines.append(f"  {piece_name}: {pos}")
    lines.append("")

    # Black Army
    lines.append("-" * 70)
    lines.append(f"BLACK ARMY (Template {ruleset.black_template}: {TEMPLATE_DESCRIPTIONS[ruleset.black_template]})")
    lines.append("-" * 70)
    lines.append("")
    lines.append("KING:")
    lines.append(f"  {get_piece_description(ruleset.black_king)}")
    lines.append("")
    lines.append("PIECES:")

    # Count and describe black pieces
    black_counts = {}
    for p in ruleset.black_pieces:
        black_counts[p] = black_counts.get(p, 0) + 1

    for piece_id in sorted(black_counts.keys()):
        count = black_counts[piece_id]
        desc = get_piece_description(piece_id)
        lines.append(f"  {count}x {desc}")
    lines.append(f"\n  TOTAL: 1 King + {len(ruleset.black_pieces)} pieces = {len(ruleset.black_pieces) + 1} units")

    # Show fixed positions if available
    if ruleset.black_positions:
        lines.append("")
        lines.append("STARTING POSITIONS (q, r hex coordinates):")
        lines.append(f"  King: {ruleset.black_positions[0]}")
        for i, pos in enumerate(ruleset.black_positions[1:]):
            if i < len(ruleset.black_pieces):
                piece_name = PIECE_TYPES[ruleset.black_pieces[i]].name
                lines.append(f"  {piece_name}: {pos}")
    lines.append("")

    # Heuristics summary
    lines.append("-" * 70)
    lines.append("AI HEURISTICS (Piece Values)")
    lines.append("-" * 70)
    lines.append("These values represent how the AI evaluates each piece type.")
    lines.append("Higher = more valuable. Used for balancing, not gameplay rules.")
    lines.append("")
    lines.append(f"White center weight: {heuristics.white_center_weight:.3f}")
    lines.append(f"Black center weight: {heuristics.black_center_weight:.3f}")
    lines.append("")
    lines.append("Piece values (sorted by value):")
    lines.append("")
    white_sorted = sorted(heuristics.white_piece_values.items(), key=lambda x: -x[1])
    black_sorted = sorted(heuristics.black_piece_values.items(), key=lambda x: -x[1])

    lines.append("  WHITE:")
    for pid, val in white_sorted:
        lines.append(f"    {pid} ({PIECE_TYPES[pid].name}): {val:.2f}")
    lines.append("")
    lines.append("  BLACK:")
    for pid, val in black_sorted:
        lines.append(f"    {pid} ({PIECE_TYPES[pid].name}): {val:.2f}")
    lines.append("")

    # Setup instructions
    lines.append("-" * 70)
    lines.append("GAME SETUP")
    lines.append("-" * 70)
    lines.append("1. Use a hexagonal board with 61 hexes (radius 4 from center)")
    lines.append("2. White sets up in the 3 rows closest to their edge (south)")
    lines.append("3. Black sets up in the 3 rows closest to their edge (north)")
    if ruleset.white_positions or ruleset.black_positions:
        lines.append("4. Place pieces at the fixed (q, r) coordinates listed above")
        lines.append("5. All pieces start facing toward the center")
    else:
        lines.append("4. Kings go in the back row, center position")
        lines.append("5. Other pieces fill remaining home zone hexes")
        lines.append("6. All pieces start facing toward the center")
    lines.append("")
    lines.append("VICTORY CONDITIONS:")
    lines.append("- Capture the enemy King to win immediately")
    lines.append("- After 50 rounds: King closest to center wins")
    lines.append("- Tiebreaker: Player with more pieces wins")
    lines.append("- Final tiebreaker: White wins")
    lines.append("")

    lines.append("=" * 70)
    lines.append("END OF CONFIGURATION")
    lines.append("=" * 70)

    return '\n'.join(lines)


def format_duration(seconds: float) -> str:
    """Format seconds as human-readable duration."""
    if seconds < 60:
        return f"{seconds:.1f}s"
    elif seconds < 3600:
        return f"{seconds/60:.1f}m"
    else:
        return f"{seconds/3600:.1f}h"


def write_generation_report(
    output_dir: Path,
    gen: int,
    ruleset: 'RuleSet',
    result: dict,
    heuristics: 'Heuristics',
):
    """Write a partial report for a single generation."""
    from hexwar.evolution import RuleSet

    report_path = output_dir / f'gen_{gen:03d}_report.txt'

    lines = []
    lines.append(f"HEXWAR Generation {gen} Report")
    lines.append("=" * 50)
    lines.append(f"Generated: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}")
    lines.append("")

    lines.append(f"Fitness: {result['fitness']:.3f}")
    lines.append(f"Skill Gradient: {result.get('skill_gradient', 0.0):.3f}")
    lines.append(f"Color Fairness: {result['color_fairness']:.3f}")
    lines.append(f"Games: W:{result['white_wins']} B:{result['black_wins']} D:{result['draws']}")
    lines.append("")

    lines.append("WHITE ARMY:")
    lines.append(f"  Template: {ruleset.white_template}")
    lines.append(f"  King: {ruleset.white_king}")
    piece_counts = {}
    for p in ruleset.white_pieces:
        piece_counts[p] = piece_counts.get(p, 0) + 1
    lines.append(f"  Pieces: {dict(sorted(piece_counts.items()))}")
    lines.append("")

    lines.append("BLACK ARMY:")
    lines.append(f"  Template: {ruleset.black_template}")
    lines.append(f"  King: {ruleset.black_king}")
    piece_counts = {}
    for p in ruleset.black_pieces:
        piece_counts[p] = piece_counts.get(p, 0) + 1
    lines.append(f"  Pieces: {dict(sorted(piece_counts.items()))}")
    lines.append("")

    # Matchup breakdown if available
    if 'matchups' in result:
        lines.append("MATCHUP BREAKDOWN:")
        for (d1, d2), stats in sorted(result['matchups'].items()):
            lines.append(f"  d{d1} vs d{d2}: deeper wins {stats.deeper_wins}/{stats.games_played}")
        lines.append("")

    with open(report_path, 'w') as f:
        f.write('\n'.join(lines))


def run_balance_pipeline(
    ruleset_generations: int = 10,
    ruleset_population: int = 8,
    games_per_eval: int = 10,
    depth: int = 2,
    max_moves_per_action: int = 15,
    n_workers: int = None,
    seed: int = None,
    output_dir: str = "balance_output",
    verbose: bool = True,
    forced_template: str = None,
    n_elites: int = 3,
    clones_per_elite: int = 2,
    mutants_per_elite: int = 1,
    ucb_c: float = 0.3,
    min_evals_for_winner: int = 8,
    seed_dir: str = None,
    smart_mutate: bool = False,
    fixed_white_file: str = None,
    fixed_black_file: str = None,
    no_cache: bool = False,
) -> dict:
    """Run the balancing pipeline (ruleset evolution only).

    Heuristic evolution has been removed. The pipeline now always uses
    template-aware heuristics that honestly value pieces based on how
    useful they are with each ruleset's action templates.

    Args:
        ruleset_generations: Generations for ruleset evolution
        ruleset_population: Population size for rulesets
        games_per_eval: Games per fitness evaluation
        depth: AI search depth
        max_moves_per_action: Move limit per search node
        n_workers: Parallel workers (default: CPU count)
        seed: Random seed
        output_dir: Directory for output files
        verbose: Print progress
        ucb_c: UCB uncertainty penalty constant (higher = more penalty for few evals)
        min_evals_for_winner: Minimum evaluations required before declaring winner
        seed_dir: Directory containing seed JSON files to use as initial population
        smart_mutate: If True, use adaptive mutations based on win margins
        fixed_white_file: Path to a JSON file containing the fixed White army.
                         When provided, only Black's army will be evolved.
        fixed_black_file: Path to a JSON file containing the fixed Black army.
                         When provided, only White's army will be evolved.

    Returns:
        Report dictionary with all results
    """
    if n_workers is None:
        # Auto-scale workers based on throughput probing
        from hexwar.autoscale import get_optimal_workers
        if verbose:
            print("Auto-detecting optimal worker count...")
        n_workers = get_optimal_workers(verbose=verbose)
        if verbose:
            print(f"Using {n_workers} workers (auto-detected)")
            print()

    # Create output directory
    output_path = Path(output_dir)
    output_path.mkdir(parents=True, exist_ok=True)

    report = {
        'timestamp': datetime.now().isoformat(),
        'config': {
            'ruleset_generations': ruleset_generations,
            'ruleset_population': ruleset_population,
            'games_per_eval': games_per_eval,
            'depth': depth,
            'max_moves_per_action': max_moves_per_action,
            'n_workers': n_workers,
            'seed': seed,
            'n_elites': n_elites,
            'clones_per_elite': clones_per_elite,
            'mutants_per_elite': mutants_per_elite,
            'ucb_c': ucb_c,
            'min_evals_for_winner': min_evals_for_winner,
            'seed_dir': seed_dir,
            'smart_mutate': smart_mutate,
            'fixed_white_file': fixed_white_file,
            'fixed_black_file': fixed_black_file,
        },
        'timings': {},
        'results': {},
    }

    # Load seed rulesets if directory provided
    seed_rulesets = None
    if seed_dir:
        seed_rulesets = load_seed_rulesets(seed_dir)
        if verbose:
            print(f"Loaded {len(seed_rulesets)} seed rulesets from {seed_dir}")

    # Load fixed white army if provided
    fixed_white = None
    if fixed_white_file:
        from hexwar.evolution import genome_to_ruleset
        with open(fixed_white_file, 'r') as f:
            fixed_white_data = json.load(f)
        # Handle both champion format (with 'ruleset' key) and direct format
        if 'ruleset' in fixed_white_data:
            fixed_white = genome_to_ruleset(fixed_white_data['ruleset'])
        else:
            fixed_white = genome_to_ruleset(fixed_white_data)
        if verbose:
            print(f"FIXED-WHITE MODE: Only evolving Black army")
            print(f"  White: {fixed_white.white_king} + {len(fixed_white.white_pieces)} pieces from {fixed_white_file}")

    # Load fixed black army if provided
    fixed_black = None
    if fixed_black_file:
        from hexwar.evolution import genome_to_ruleset, board_set_to_ruleset
        with open(fixed_black_file, 'r') as f:
            fixed_black_data = json.load(f)
        # Handle board set format (has 'pieces' array)
        if 'pieces' in fixed_black_data and isinstance(fixed_black_data['pieces'], list):
            fixed_black = board_set_to_ruleset(fixed_black_data)
        # Handle champion format (with 'ruleset' key) and direct format
        elif 'ruleset' in fixed_black_data:
            fixed_black = genome_to_ruleset(fixed_black_data['ruleset'])
        else:
            fixed_black = genome_to_ruleset(fixed_black_data)
        if verbose:
            print(f"FIXED-BLACK MODE: Only evolving White army")
            print(f"  Black: {fixed_black.black_king} + {len(fixed_black.black_pieces)} pieces from {fixed_black_file}")

    total_start = time.time()

    # Heuristics are computed dynamically per-ruleset based on templates.
    # This ensures the AI honestly values pieces based on how useful they
    # are with the given action template.
    if verbose:
        print("=" * 60)
        print("HEURISTICS: Using template-aware computation")
        print("=" * 60)
        print("Heuristics computed per-ruleset based on action templates.")
        print()

    report['results']['heuristics'] = {
        'mode': 'template_aware',
        'description': 'Heuristics computed dynamically based on ruleset templates',
    }
    # Use a placeholder - actual heuristics computed dynamically for each ruleset
    best_heuristics = Heuristics.create_default()

    # =========================================================================
    # RULESET EVOLUTION
    # =========================================================================
    if verbose:
        print("=" * 60)
        print("RULESET EVOLUTION")
        print("=" * 60)
        print(f"Population: {ruleset_population}, Generations: {ruleset_generations}")

    phase2_start = time.time()

    # Create per-generation report callback
    def on_generation(gen, best_rs, best_result, heuristics):
        write_generation_report(output_path, gen, best_rs, best_result, heuristics)
        if verbose:
            print(f"    Wrote gen_{gen:03d}_report.txt")

    best_ruleset, ruleset_stats = evolve_rulesets(
        heuristics=best_heuristics,
        population_size=ruleset_population,
        generations=ruleset_generations,
        games_per_eval=games_per_eval,
        depth=depth,
        max_moves_per_action=max_moves_per_action,
        seed=seed,
        n_workers=n_workers,
        verbose=verbose,
        log_dir=str(output_path),
        report_callback=on_generation,
        use_template_aware=True,  # Always use template-aware heuristics
        forced_template=forced_template,
        n_elites=n_elites,
        clones_per_elite=clones_per_elite,
        mutants_per_elite=mutants_per_elite,
        ucb_c=ucb_c,
        min_evals_for_winner=min_evals_for_winner,
        seed_rulesets=seed_rulesets,
        smart_mutate=smart_mutate,
        fixed_white=fixed_white,
        fixed_black=fixed_black,
        no_cache=no_cache,
    )

    phase2_time = time.time() - phase2_start
    report['timings']['ruleset_evolution'] = phase2_time

    # Save ruleset
    ruleset_genome = ruleset_to_genome(best_ruleset)
    with open(output_path / 'ruleset.json', 'w') as f:
        json.dump(ruleset_genome, f, indent=2)

    report['results']['ruleset'] = ruleset_genome
    report['results']['ruleset_evolution_stats'] = ruleset_stats

    if verbose:
        print(f"\nPhase 2 complete in {format_duration(phase2_time)}")
        print(f"Saved ruleset to {output_path / 'ruleset.json'}")

    # =========================================================================
    # PHASE 3: Final Evaluation
    # =========================================================================
    if verbose:
        print("\n" + "=" * 60)
        print("PHASE 3: FINAL EVALUATION")
        print("=" * 60)

    phase3_start = time.time()

    # Run more games for final stats
    final_eval = evaluate_ruleset_fitness(
        best_ruleset,
        best_heuristics,
        n_games=games_per_eval * 3,  # More games for better stats
        depth=depth,
        max_moves_per_action=max_moves_per_action,
        seed=seed,
        use_template_aware=True,  # Always use template-aware heuristics
    )

    # Get the actual template-aware heuristics for reporting
    best_heuristics = create_template_aware_heuristics(
        best_ruleset.white_template, best_ruleset.black_template
    )
    # Save these for the report
    heuristics_genome = heuristics_to_genome(best_heuristics)
    with open(output_path / 'heuristics.json', 'w') as f:
        json.dump({
            'mode': 'template_aware',
            'white_template': best_ruleset.white_template,
            'black_template': best_ruleset.black_template,
            **heuristics_genome
        }, f, indent=2)
    report['results']['heuristics'] = {
        'mode': 'template_aware',
        'white_template': best_ruleset.white_template,
        'black_template': best_ruleset.black_template,
        'white_center_weight': best_heuristics.white_center_weight,
        'black_center_weight': best_heuristics.black_center_weight,
        'white_piece_values': dict(best_heuristics.white_piece_values),
        'black_piece_values': dict(best_heuristics.black_piece_values),
    }

    phase3_time = time.time() - phase3_start
    report['timings']['final_evaluation'] = phase3_time
    # Convert matchup tuple keys to strings and MatchupStats to dicts for JSON serialization
    final_eval_json = dict(final_eval)
    if 'matchups' in final_eval_json:
        from dataclasses import asdict
        final_eval_json['matchups'] = {
            f"{k[0]}v{k[1]}": asdict(v) if hasattr(v, '__dataclass_fields__') else v
            for k, v in final_eval_json['matchups'].items()
        }
    report['results']['final_evaluation'] = final_eval_json

    total_time = time.time() - total_start
    report['timings']['total'] = total_time

    # =========================================================================
    # Generate Report
    # =========================================================================
    if verbose:
        print("\n" + "=" * 60)
        print("BALANCE REPORT")
        print("=" * 60)

        print(f"\n--- TIMINGS ---")
        print(f"Ruleset Evolution:   {format_duration(phase2_time)}")
        print(f"Final Evaluation:    {format_duration(phase3_time)}")
        print(f"TOTAL:               {format_duration(total_time)}")

        print(f"\n--- BEST RULESET ---")
        print(f"White Army ({best_ruleset.white_template}):")
        print(f"  King: {best_ruleset.white_king}")
        piece_counts = {}
        for p in best_ruleset.white_pieces:
            piece_counts[p] = piece_counts.get(p, 0) + 1
        print(f"  Pieces: {dict(sorted(piece_counts.items()))}")
        print(f"  Total: {len(best_ruleset.white_pieces) + 1} pieces")

        print(f"\nBlack Army ({best_ruleset.black_template}):")
        print(f"  King: {best_ruleset.black_king}")
        piece_counts = {}
        for p in best_ruleset.black_pieces:
            piece_counts[p] = piece_counts.get(p, 0) + 1
        print(f"  Pieces: {dict(sorted(piece_counts.items()))}")
        print(f"  Total: {len(best_ruleset.black_pieces) + 1} pieces")

        print(f"\n--- BALANCE STATS ---")
        print(f"Games Played: {final_eval['total_games']}")
        print(f"White Wins:   {final_eval['white_wins']} ({final_eval['white_wins']/final_eval['total_games']*100:.1f}%)")
        print(f"Black Wins:   {final_eval['black_wins']} ({final_eval['black_wins']/final_eval['total_games']*100:.1f}%)")
        print(f"Draws:        {final_eval['draws']} ({final_eval['draws']/final_eval['total_games']*100:.1f}%)")
        print(f"Avg Rounds:   {final_eval['avg_rounds']:.1f}")
        print(f"\nColor Fairness:  {final_eval['color_fairness']:.3f} (1.0 = perfect)")
        print(f"Game Length:     {final_eval['game_length_score']:.3f}")
        print(f"Decisiveness:    {final_eval['decisiveness']:.3f}")
        print(f"Overall Fitness: {final_eval['fitness']:.3f}")

        print(f"\n--- HEURISTIC SUMMARY ---")
        print(f"White center weight: {best_heuristics.white_center_weight:.3f}")
        print(f"Black center weight: {best_heuristics.black_center_weight:.3f}")

        # Full piece values
        white_sorted = sorted(best_heuristics.white_piece_values.items(), key=lambda x: -x[1])
        black_sorted = sorted(best_heuristics.black_piece_values.items(), key=lambda x: -x[1])
        print(f"\nWhite piece values:")
        for pid, val in white_sorted:
            print(f"  {pid}: {val:.2f}")
        print(f"\nBlack piece values:")
        for pid, val in black_sorted:
            print(f"  {pid}: {val:.2f}")

    # Save full report (JSON)
    with open(output_path / 'report.json', 'w') as f:
        json.dump(report, f, indent=2)

    # Generate and save human-readable report
    human_report = generate_human_readable_report(
        ruleset=best_ruleset,
        heuristics=best_heuristics,
        eval_result=final_eval,
        config=report['config'],
        timings=report['timings'],
    )
    with open(output_path / 'GAME_CONFIG.txt', 'w') as f:
        f.write(human_report)

    if verbose:
        print(f"\n--- OUTPUT FILES ---")
        print(f"  {output_path / 'heuristics.json'}")
        print(f"  {output_path / 'ruleset.json'}")
        print(f"  {output_path / 'report.json'}")
        print(f"  {output_path / 'GAME_CONFIG.txt'}  <- Human-readable game setup!")

    return report


def main():
    parser = argparse.ArgumentParser(
        description='HEXWAR Full Balancing Pipeline',
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )

    parser.add_argument('--ruleset-gen', type=int, default=10,
                        help='Generations for ruleset evolution (default: 10)')
    parser.add_argument('--ruleset-pop', type=int, default=8,
                        help='Population size for rulesets (default: 8)')
    parser.add_argument('--games', type=int, default=10,
                        help='Games per fitness evaluation (default: 10)')
    parser.add_argument('--depth', type=int, default=2,
                        help='AI search depth (default: 2)')
    parser.add_argument('--max-moves', type=int, default=15,
                        help='Max moves per search node (default: 15)')
    parser.add_argument('--workers', type=int, default=None,
                        help='Parallel workers (default: CPU count - 1)')
    parser.add_argument('--seed', type=int, default=None,
                        help='Random seed')
    parser.add_argument('--output', type=str, default='balance_output',
                        help='Output directory (default: balance_output)')
    parser.add_argument('--quick', action='store_true',
                        help='Quick test run (small parameters)')
    parser.add_argument('--template', type=str, default=None,
                        help='Force both sides to use a specific template (A-F). E=move-or-rotate for fast deep search')
    parser.add_argument('--elites', type=int, default=3,
                        help='Number of top performers to preserve each generation (default: 3)')
    parser.add_argument('--clones', type=int, default=2,
                        help='Unchanged copies of each elite for fitness verification (default: 2)')
    parser.add_argument('--mutants', type=int, default=1,
                        help='Mutated children of each elite for exploration (default: 1)')
    parser.add_argument('--ucb-c', type=float, default=0.3,
                        help='UCB uncertainty penalty constant (default: 0.3). Higher = more penalty for few evals')
    parser.add_argument('--min-evals', type=int, default=8,
                        help='Minimum evaluations required before declaring winner (default: 8)')
    parser.add_argument('--seed-dir', type=str, default=None,
                        help='Directory containing seed JSON files to use as initial population')
    parser.add_argument('--smart-mutate', action='store_true',
                        help='Use adaptive mutations based on win margins')
    parser.add_argument('--fixed-white', type=str, default=None,
                        help='Path to JSON file with fixed White army. Only Black will evolve.')
    parser.add_argument('--fixed-black', type=str, default=None,
                        help='Path to JSON file with fixed Black army. Only White will evolve.')
    parser.add_argument('--no-cache', action='store_true',
                        help='Disable caching - force re-evaluation of all rulesets every generation')
    parser.add_argument('--evaluate-only', type=str, default=None,
                        help='Just evaluate a ruleset file N times (no evolution). Use with --evals')
    parser.add_argument('--evals', type=int, default=14,
                        help='Number of evaluations for --evaluate-only mode (default: 14)')

    args = parser.parse_args()

    # Quick mode for testing
    if args.quick:
        args.ruleset_gen = 2
        args.ruleset_pop = 3
        args.games = 2
        args.max_moves = 10  # Very limited branching for speed

    run_balance_pipeline(
        ruleset_generations=args.ruleset_gen,
        ruleset_population=args.ruleset_pop,
        games_per_eval=args.games,
        depth=args.depth,
        max_moves_per_action=args.max_moves,
        n_workers=args.workers,
        seed=args.seed,
        output_dir=args.output,
        verbose=True,
        forced_template=args.template,
        n_elites=args.elites,
        clones_per_elite=args.clones,
        mutants_per_elite=args.mutants,
        ucb_c=args.ucb_c,
        min_evals_for_winner=args.min_evals,
        seed_dir=args.seed_dir,
        smart_mutate=args.smart_mutate,
        fixed_white_file=args.fixed_white,
        fixed_black_file=args.fixed_black,
        no_cache=args.no_cache,
    )


if __name__ == '__main__':
    main()
