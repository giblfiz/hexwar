# HEXWAR Evolutionary Game Balancer

An automated system for evolving balanced asymmetric armies for the HEXWAR hex-based strategy game. Uses genetic algorithms to co-evolve AI heuristics and army compositions.

## Quick Start

```bash
# Quick test run (~1 min)
python3 -m hexwar.balance --heuristic-gen 1 --heuristic-pop 4 --ruleset-gen 1 --ruleset-pop 3 --games 4 --depth 2 --workers 10 --output balance_test --seed 42

# Medium run (~10 min)
python3 -m hexwar.balance --heuristic-gen 8 --heuristic-pop 12 --ruleset-gen 8 --ruleset-pop 10 --games 16 --depth 2 --workers 40 --output balance_medium --seed 2024

# Long run (~4-6 hours, background)
python3 -m hexwar.balance --heuristic-gen 30 --heuristic-pop 20 --ruleset-gen 25 --ruleset-pop 15 --games 32 --depth 2 --workers 40 --output balance_dec29_1915 --seed 12345 > balance_dec29_1915.log 2>&1 &
```

**Output naming convention**: `balance_mon##_HHMM` (e.g., `balance_dec29_1915`)

## Project Structure

```
hexwar-balancer/
├── hexwar/
│   ├── __init__.py
│   ├── board.py          # Hex board geometry (axial coordinates)
│   ├── pieces.py         # 29 piece type definitions
│   ├── game.py           # Game state and rules
│   ├── ai.py             # Negamax AI with alpha-beta pruning
│   ├── tournament.py     # Tournament runner, fitness evaluation
│   ├── evolution.py      # Genetic algorithms for heuristics & rulesets
│   ├── balance.py        # Main pipeline orchestrator (CLI entry point)
│   ├── autoscale.py      # Dynamic worker scaling
│   └── runner.py         # Game runner utilities
├── hexwar_core/          # Rust acceleration module
│   ├── Cargo.toml
│   └── src/lib.rs        # Full game engine in Rust (~945 lines)
└── README.md
```

## Architecture Overview

### Three-Phase Pipeline

1. **Phase 1: Heuristic Evolution** (~80% of runtime)
   - Evolves per-color piece values (how much is each piece worth?)
   - Population plays round-robin against each other
   - Fitness = win rate against opponents
   - Seeded with: default heuristics, reachable-squares heuristic, max-distance heuristic

2. **Phase 2: Ruleset Evolution** (~15% of runtime)
   - Evolves army compositions (which pieces, how many, what positions)
   - Uses best heuristics from Phase 1
   - Fitness based on: skill gradient, color fairness, game richness, decisiveness

3. **Phase 3: Final Evaluation** (~5% of runtime)
   - Runs extended tournament on best ruleset
   - Generates human-readable game configuration

### Key Data Structures

**Heuristics** (`ai.py`):
```python
@dataclass
class Heuristics:
    white_piece_values: dict[str, float]  # piece_id -> value
    black_piece_values: dict[str, float]
    white_center_weight: float
    black_center_weight: float
```

**RuleSet** (`evolution.py`):
```python
@dataclass
class RuleSet:
    white_pieces: list[str]      # Piece IDs (not including king)
    black_pieces: list[str]
    white_template: str          # 'A', 'B', 'C', or 'D'
    black_template: str
    white_king: str              # King variant ID
    black_king: str
    white_positions: list[tuple[int, int]]  # (q, r) hex coordinates
    black_positions: list[tuple[int, int]]
```

**Action Templates**:
- Template A: Rotate, then Move (same piece)
- Template B: Move, Rotate, Rotate
- Template C: Move, Move, Rotate
- Template D: Move, then Rotate (different piece)

## Fitness Function (Ruleset Evaluation)

Located in `tournament.py:evaluate_ruleset_tournament()`:

```
fitness = 0.35 * skill_gradient +    # Deeper AI should win more often
          0.40 * color_fairness +    # 50/50 white/black at equal depth
          0.15 * game_richness +     # Games last 15-50 rounds
          0.10 * decisiveness        # Few draws
```

**Matchup spec (reduced mode for evolution)**:
- d1 vs d1: 6 games (baseline balance)
- d1 vs d2: 6 games (skill gradient)
- d2 vs d2: 4 games (higher quality)

## Seed Heuristics

Three starting points for heuristic evolution (`evolution.py`):

1. **Default**: Hand-tuned baseline values
2. **Reachable Squares**: `value = squares_reachable_in_one_move / 6`
   - Queen (D5) = 5.0, Pawn (A1) = 0.5
3. **Max Distance**: `value = max_move_distance / 2`
   - Sliding pieces valued highest

## Performance Optimizations

### Rust Acceleration (~100-500x speedup)
- Full game engine ported to Rust in `hexwar_core/src/lib.rs`
- PyO3 bindings for Python integration
- Build: `cd hexwar_core && maturin develop --release`
- Auto-detected at runtime (`RUST_AVAILABLE` flag in tournament.py)

### Autoscaling (`autoscale.py`)
- Probes throughput at different worker counts
- Finds optimal parallelism before diminishing returns
- Capped at `min(60, cpu_count * 4)` workers
- 5% improvement threshold to keep scaling

### Batched Game Execution
- Games grouped into batches per worker
- Reduces process dispatch overhead
- `games_per_batch = total_games // (n_workers * 2)`

## Output Files

Each run creates a directory with:

| File | Description |
|------|-------------|
| `GAME_CONFIG.txt` | Human-readable game rules, armies, positions |
| `report.json` | Full structured report with all stats |
| `heuristics.json` | Evolved piece values |
| `ruleset.json` | Army compositions and positions |
| `game_log.txt` | Per-game results during evolution |
| `gen_###_report.txt` | Per-generation reports |

## Key Parameters

| Parameter | Description | Typical Range |
|-----------|-------------|---------------|
| `--heuristic-gen` | Generations for heuristic evolution | 5-30 |
| `--heuristic-pop` | Population size for heuristics | 8-20 |
| `--ruleset-gen` | Generations for ruleset evolution | 5-25 |
| `--ruleset-pop` | Population size for rulesets | 6-15 |
| `--games` | Games per fitness evaluation | 8-32 |
| `--depth` | AI search depth (2=fast, 3=strong) | 2-3 |
| `--workers` | Parallel workers | 10-60 |

## Recent Bug Fixes (Dec 2024)

1. **Heuristic evaluation was broken**: Both players used same heuristics instead of candidate vs opponent. Fixed in `evolution.py:evaluate_heuristics_fitness()`.

2. **Stats showing 0.0**: `avg_rounds` was hardcoded to 0, `color_fairness` was using broken logic at equal depths. Fixed in `tournament.py` by tracking actual `white_wins`, `black_wins`, `total_rounds` per matchup.

3. **Bootstrap ruleset had no positions**: `create_bootstrap_ruleset()` now generates fixed positions like random rulesets do.

## Piece Types (29 total)

**Step Pieces** (move 1-3 hexes):
- A1-A5: Step-1 (Pawn, Guard, Scout, Crab, Flanker)
- B1-B4: Step-2 (Strider, Dancer, Ranger, Hound)
- C1-C3: Step-3 (Lancer, Dragoon, Courser)

**Slide Pieces** (move any distance):
- D1-D5: Pike, Rook, Bishop, Chariot, Queen

**Jump Pieces** (leap over others):
- E1-E2: Knight (2-hex), Frog (2-hex all dirs)
- F1: Locust (3-hex forward arc)

**Special Pieces**:
- W1 Warper: Swaps with ally instead of moving
- W2 Shifter: Can swap with adjacent ally after rotating
- P1 Phoenix: Can resurrect captured ally
- G1 Ghost: Phases through enemies

**Kings** (K1-K5): Guard, Scout, Ranger, Frog, Pike variants

## Board Geometry

- 61-hex board (radius 4 from center)
- Axial coordinates (q, r)
- White home zone: rows r=2,3,4 (south)
- Black home zone: rows r=-4,-3,-2 (north)
- 6 directions: Forward, Forward-Right, Back-Right, Backward, Back-Left, Forward-Left

## Monitoring Long Runs

```bash
# Check if running
ps aux | grep hexwar.balance

# Watch progress
tail -f balance_dec29_1915.log

# Check latest generation
grep "Best fitness" balance_dec29_1915.log | tail -5
```

## Development Notes

### Adding New Piece Types
1. Define in `pieces.py` with `PieceType` dataclass
2. Add to `PIECE_TYPES` dict and `REGULAR_PIECE_IDS`
3. Add movement logic in `game.py:get_legal_moves()`
4. Update Rust implementation in `hexwar_core/src/lib.rs`

### Tuning Evolution Parameters
- More generations = better convergence but diminishing returns after ~20
- Larger population = more diversity but slower (quadratic for heuristics)
- More games = more stable fitness but slower
- Higher depth = smarter AI but exponentially slower

### Common Issues
- If `RUST_AVAILABLE = False`, rebuild with `maturin develop --release`
- If stats look wrong, check `MatchupStats` has `white_wins`, `black_wins`, `total_rounds`
- If positions missing from output, check `RuleSet.white_positions` is not None
