# HEXWAR Balancer - Session Notes

## Session: Dec 29, 2024

### What Was Accomplished

#### 1. Performance Optimization
- **Rust game engine**: Ported entire game engine to Rust (`hexwar_core/src/lib.rs`)
  - ~481x speedup for single games
  - ~121x speedup for tournament runs
  - Uses PyO3 for Python bindings
  - Build command: `cd hexwar_core && maturin develop --release`

- **Dynamic worker autoscaling** (`hexwar/autoscale.py`):
  - Probes throughput at increasing worker counts
  - Stops when improvement < 5% threshold
  - Max workers capped at `min(60, cpu_count * 4)`
  - Has separate probing for synthetic work vs actual Rust games

- **Batched game execution** (`tournament.py:run_matchup`):
  - Groups multiple games per worker dispatch
  - Reduces overhead from process spawning
  - `games_per_batch = total_games // (n_workers * 2)`

#### 2. Bug Fixes

**Heuristic Evolution Bug** (CRITICAL):
- Location: `evolution.py:evaluate_heuristics_fitness()`
- Problem: Both white and black AI used the SAME heuristics during evaluation
- Effect: Piece values stayed random because there was no selection pressure
- Fix: Added `white_heuristics` and `black_heuristics` params to `play_ai_game()`
  - Now candidate plays against actual opponent heuristics

**Stats Bug** (avg_rounds=0.0, color_fairness=0.0):
- Location: `tournament.py:evaluate_ruleset_tournament()`
- Problems:
  1. `avg_rounds` was hardcoded to 0.0 (line 606: `avg_rounds=0.0, # TODO`)
  2. At equal depths, `deeper_wins` == `shallower_wins` because both depths are same
     - ALL wins went to `deeper_wins`, none to `shallower_wins`
     - Color fairness calc: `win_rate = deeper_wins/games = 1.0` → fairness = 0
- Fix: Added `white_wins`, `black_wins`, `total_rounds` to `MatchupStats`
  - Track actual color wins separately from depth-based wins
  - Calculate `avg_rounds` from accumulated `total_rounds`

**Bootstrap Ruleset Missing Positions**:
- Location: `evolution.py:create_bootstrap_ruleset()`
- Problem: Returned `white_positions=None, black_positions=None`
- Fix: Now generates positions using `_generate_positions()` like random rulesets

#### 3. Seed Heuristics
Added two computed seed heuristics to bootstrap evolution (in addition to default):

1. **Reachable Squares** (`create_reachable_squares_heuristics()`):
   - Value = number of squares piece can reach in one move
   - STEP: `range * num_directions`
   - SLIDE: `5 * num_directions` (avg slide ~5 hexes)
   - JUMP: `num_directions`
   - Plus bonuses for special abilities
   - Normalized to 0.5-5.0 range

2. **Max Distance** (`create_max_distance_heuristics()`):
   - Value = maximum move distance
   - SLIDE pieces capped at 8 (board diameter)
   - Plus small bonus for more directions
   - Plus bonuses for special abilities

#### 4. Reporting Improvements
- Full heuristic value dump (all 24 pieces, sorted by value)
- Starting positions now included in `GAME_CONFIG.txt`
- Output directory naming convention: `balance_mon##_HHMM`

### Key Files Modified
- `hexwar/ai.py` - Added per-player heuristics support
- `hexwar/tournament.py` - Fixed stats tracking, added white/black wins
- `hexwar/evolution.py` - Fixed evaluation, added seed heuristics, fixed bootstrap positions
- `hexwar/balance.py` - Updated reporting format
- `hexwar/autoscale.py` - NEW: Dynamic worker scaling
- `hexwar_core/` - NEW: Rust game engine

### Test Results
After fixes, a typical run shows:
```
Avg Rounds:      17.9  (was 0.0)
Color Fairness:  0.800 (was 0.0)
Overall Fitness: 0.862
```

Heuristics now evolve asymmetrically (white vs black get different values).

### Remaining Considerations
- Depth 3 is significantly slower than depth 2 (exponential search tree)
- Heuristic evolution is ~80% of runtime due to round-robin evaluation
- Consider caching game results for identical matchups
- Phoenix resurrect and Ghost phase abilities may need balance tuning

### Running Long Jobs
```bash
# 4-6 hour run
python3 -m hexwar.balance \
  --heuristic-gen 30 --heuristic-pop 20 \
  --ruleset-gen 25 --ruleset-pop 15 \
  --games 32 --depth 2 --workers 40 \
  --output balance_dec29_1915 --seed 12345 \
  > balance_dec29_1915.log 2>&1 &

# Monitor
tail -f balance_dec29_1915.log
grep "Best fitness" balance_dec29_1915.log | tail -5
```
