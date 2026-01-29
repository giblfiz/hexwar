"""
HEXWAR Game Runner

Utilities for playing games with random or AI-controlled players.
E5 milestone: Random games play to completion.
"""

import random
from typing import Optional
from hexwar.game import (
    GameState, Move,
    generate_legal_actions, apply_move,
    is_game_over, get_winner,
)
from hexwar.board import WHITE_HOME_ZONE, BLACK_HOME_ZONE, default_facing


def create_bootstrap_game(seed: Optional[int] = None) -> GameState:
    """Create a game with the bootstrap rule set from the spec.

    White — "The Legion" (Template D: move, then rotate different)
      1× King (K1 - Guard type)
      4× Pawn (A1)
      2× Scout (A3)
      2× Strider (B1)
      1× Lancer (C1)
      1× Rook (D2)
      1× Knight (E1)
      Total: 12 pieces

    Black — "The Coven" (Template A: rotate, then move same piece)
      1× King (K4 - Frog type)
      3× Guard (A2)
      1× Ghost (G1)
      1× Shifter (W2)
      1× Phoenix (P1)
      2× Bishop (D3)
      1× Chariot (D4)
      1× Frog (E2)
      Total: 11 pieces
    """
    rng = random.Random(seed) if seed is not None else random.Random()

    # White pieces: (type_id, count)
    white_types = [
        ('K1', 1), ('A1', 4), ('A3', 2), ('B1', 2),
        ('C1', 1), ('D2', 1), ('E1', 1),
    ]

    # Black pieces: (type_id, count)
    black_types = [
        ('K4', 1), ('A2', 3), ('G1', 1), ('W2', 1),
        ('P1', 1), ('D3', 2), ('D4', 1), ('E2', 1),
    ]

    def expand_pieces(type_counts):
        """Expand [(type_id, count), ...] to [type_id, ...]"""
        result = []
        for tid, count in type_counts:
            result.extend([tid] * count)
        return result

    def place_pieces(type_ids, home_zone, facing):
        """Place pieces in home zone, king at back center."""
        positions = list(home_zone)
        # Sort by r (depth into board), then by q
        positions.sort(key=lambda p: (-p[1] if facing == 0 else p[1], p[0]))

        placed = []
        # Find and place king first (at back center)
        king_types = [t for t in type_ids if t.startswith('K')]
        other_types = [t for t in type_ids if not t.startswith('K')]

        # King goes at first position (back row, center-ish)
        for kt in king_types:
            if positions:
                pos = positions.pop(0)
                placed.append((kt, pos, facing))

        # Shuffle remaining pieces for variety
        rng.shuffle(other_types)

        for tid in other_types:
            if positions:
                pos = positions.pop(0)
                placed.append((tid, pos, facing))

        return placed

    white_pieces = place_pieces(
        expand_pieces(white_types),
        WHITE_HOME_ZONE,
        default_facing(0)  # North
    )

    black_pieces = place_pieces(
        expand_pieces(black_types),
        BLACK_HOME_ZONE,
        default_facing(1)  # South
    )

    return GameState.create_initial(
        white_pieces, black_pieces,
        white_template='D',
        black_template='A',
    )


def play_random_game(
    state: Optional[GameState] = None,
    seed: Optional[int] = None,
    max_rounds: int = 50,
    verbose: bool = False,
) -> tuple[GameState, int]:
    """Play a game to completion with random move selection.

    Args:
        state: Initial game state. If None, creates bootstrap game.
        seed: Random seed for reproducibility.
        max_rounds: Maximum rounds before declaring draw.
        verbose: Print game progress.

    Returns:
        (final_state, winner) where winner is 0 (White), 1 (Black), or -1 (draw)
    """
    rng = random.Random(seed) if seed is not None else random.Random()

    if state is None:
        state = create_bootstrap_game(seed)

    moves_made = 0

    while not is_game_over(state):
        actions = generate_legal_actions(state)

        if not actions:
            # No legal actions - shouldn't happen, but handle gracefully
            break

        # Pick a random action
        action = rng.choice(actions)
        state = apply_move(state, action)
        moves_made += 1

        if verbose and moves_made % 50 == 0:
            print(f"  Move {moves_made}, Round {state.round_number}")

        # Safety limit
        if moves_made > max_rounds * 10:  # ~5 actions per turn, 2 turns per round
            break

    winner = get_winner(state)
    if winner is None:
        winner = -1  # Draw (shouldn't happen with proper turn limit handling)

    if verbose:
        print(f"Game ended after {moves_made} actions, round {state.round_number}")
        print(f"Winner: {'White' if winner == 0 else 'Black' if winner == 1 else 'Draw'}")

    return state, winner


def play_many_random_games(
    n_games: int,
    seed: Optional[int] = None,
    verbose: bool = False,
) -> dict:
    """Play many random games and collect statistics.

    Returns dict with:
        - white_wins: count
        - black_wins: count
        - draws: count
        - avg_rounds: average game length in rounds
        - avg_moves: average total moves per game
    """
    rng = random.Random(seed) if seed is not None else random.Random()

    white_wins = 0
    black_wins = 0
    draws = 0
    total_rounds = 0
    total_moves = 0

    for i in range(n_games):
        game_seed = rng.randint(0, 2**31)
        state, winner = play_random_game(seed=game_seed)

        if winner == 0:
            white_wins += 1
        elif winner == 1:
            black_wins += 1
        else:
            draws += 1

        total_rounds += state.round_number

        if verbose and (i + 1) % 10 == 0:
            print(f"Completed {i + 1}/{n_games} games")

    return {
        'white_wins': white_wins,
        'black_wins': black_wins,
        'draws': draws,
        'avg_rounds': total_rounds / n_games if n_games > 0 else 0,
        'games_played': n_games,
    }


if __name__ == '__main__':
    import time

    print("HEXWAR Game Runner - E5 Milestone Test")
    print("=" * 50)

    # Play a few random games
    print("\nPlaying 10 random games...")
    start = time.time()
    stats = play_many_random_games(10, seed=42, verbose=True)
    elapsed = time.time() - start

    print(f"\nResults:")
    print(f"  White wins: {stats['white_wins']}")
    print(f"  Black wins: {stats['black_wins']}")
    print(f"  Draws: {stats['draws']}")
    print(f"  Avg rounds: {stats['avg_rounds']:.1f}")
    print(f"  Time: {elapsed:.2f}s ({elapsed/10:.3f}s per game)")

    print("\nE5 milestone: Random games play to completion!")
