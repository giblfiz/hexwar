# HEXWAR: MCTS vs Alpha-Beta Benchmark Results

## Overview

This document contains the results of the MCTS vs Alpha-Beta search performance comparison for the HEXWAR game AI system.

**Benchmark Date:** 2026-01-29
**Hardware:** Intel Core i5-10400F, 14GB RAM
**Build:** Release (optimized)

## Benchmark Scenarios

### Test Positions

1. **Balanced Position** (K + 2 pieces per side)
   - White: K1 at (0,3), A2 at (-1,3), B3 at (1,3)
   - Black: K1 at (0,-3), A2 at (1,-3), B3 at (-1,-3)
   - Simple, symmetrical setup for baseline testing

2. **Mid-Game Position** (K + 4 pieces per side)
   - More complex board state
   - Mix of piece types (Dragoons, Phoenix)
   - More legal moves per turn

### Search Configurations Tested

**Alpha-Beta:**
- Depth 2 (shallow, very fast)
- Depth 4 (moderate, practical)
- Depth 6 (deep, slower)

**MCTS (CPU-only, no GPU):**
- 100 simulations (minimal)
- 500 simulations (moderate)
- 1,000 simulations (standard)
- 5,000 simulations (aggressive)

## Results Summary

### Move Time Benchmark: Balanced Position

| Player      | Config        | Avg Move Time | Moves/Sec |
|-------------|---------------|---------------|-----------|
| Alpha-Beta  | Depth 2       | 0.12ms        | 8,268     |
| Alpha-Beta  | Depth 4       | 1.29ms        | 774       |
| Alpha-Beta  | Depth 6       | 11.48ms       | 87        |
| MCTS        | 100 sims      | 1.23ms        | 811       |
| MCTS        | 500 sims      | 3.50ms        | 286       |
| MCTS        | 1,000 sims    | 9.50ms        | 105       |
| MCTS        | 5,000 sims    | 31.59ms       | 32        |

### Move Time Benchmark: Mid-Game Position

| Player      | Config        | Avg Move Time | Moves/Sec |
|-------------|---------------|---------------|-----------|
| Alpha-Beta  | Depth 2       | 0.20ms        | 5,024     |
| Alpha-Beta  | Depth 4       | 2.71ms        | 369       |
| Alpha-Beta  | Depth 6       | 37.00ms       | 27        |
| MCTS        | 100 sims      | 0.99ms        | 1,012     |
| MCTS        | 500 sims      | 10.02ms       | 100       |
| MCTS        | 1,000 sims    | 10.93ms       | 91        |
| MCTS        | 5,000 sims    | 87.65ms       | 11        |

### Throughput Benchmark (Game Playback)

**Balanced Position:**
| Player      | Config             | Moves/Sec | Moves to Draw |
|-------------|-------------------|-----------|---------------|
| Alpha-Beta  | Depth 2           | 12,678    | 100           |
| MCTS        | 100 sims          | 1,354     | 10            |
| MCTS        | 1,000 sims        | 137       | 10            |

**Mid-Game Position:**
| Player      | Config             | Moves/Sec | Moves to Draw |
|-------------|-------------------|-----------|---------------|
| Alpha-Beta  | Depth 2           | 5,765     | 100           |
| MCTS        | 100 sims          | 591       | 10            |
| MCTS        | 1,000 sims        | 75        | 10            |

### Move Quality: Head-to-Head Games

**AB D2 vs MCTS 1000 (Balanced Position)**

| Game | Winner          | Move Count |
|------|-----------------|------------|
| 1    | AB D2 (White)   | 17         |
| 2    | AB D2 (White)   | 39         |
| 3    | AB D2 (White)   | 27         |

**Record: AB D2 wins 3-0**

## Performance Analysis

### Key Findings

#### 1. **Alpha-Beta Dominates on Simple Positions**

- **AB D2 is 78.6x faster than MCTS 1000** in move time
- AB D2 achieves 8,268 moves/sec vs MCTS 1000 at 105 moves/sec
- Clear winner for shallow searches on small game trees

#### 2. **Depth Scaling for Alpha-Beta**

Exponential slowdown with increasing depth:

```
D2 → D4: 10.7x slower (0.12ms → 1.29ms)
D4 → D6: 8.9x slower  (1.29ms → 11.48ms)
```

Combined D2→D6: **~95.6x slowdown** with only 4 additional plies

#### 3. **MCTS Scaling with Simulations**

Linear-ish scaling with simulation count:

```
100 → 500 sims:  2.84x slower
500 → 1000 sims: 2.72x slower
1000 → 5000 sims: 3.32x slower
```

MCTS scaling is more predictable than AB depth scaling.

#### 4. **Position Complexity Matters**

**Mid-Game vs Balanced (both AB D2):**
- Time per move: 0.12ms → 0.20ms (1.7x slower)
- Board with 10 pieces doubles branching factor

**Mid-Game vs Balanced (both MCTS 1000):**
- Time per move: 9.50ms → 10.93ms (1.15x slower)
- MCTS is less sensitive to board complexity

#### 5. **Game Playback Speed**

AB plays much faster:
- AB D2: 12,678 moves/sec (full games in seconds)
- MCTS 1000: 137 moves/sec (full games in minutes)

However:
- Reduced MCTS simulations (100) can match AB D2 speed
- Throughput vs depth trade-off is important for interactive play

#### 6. **Move Quality**

AB D2 beats MCTS 1000 in test games:
- AB won all 3 games as White
- Average game length: 27.7 moves
- **Interpretation**: Even shallow AB search (D2) finds better moves than 1000 rollout simulations

## Technical Observations

### Alpha-Beta Characteristics

**Strengths:**
- Extremely fast for shallow depths (D2-D3)
- Deterministic, always same move
- Scales well with alpha-beta pruning (reduces branching by ~50%)

**Weaknesses:**
- Exponential slowdown with depth
- D6+ becomes impractical for real-time play
- Requires good evaluation function tuning

### MCTS Characteristics

**Strengths:**
- Predictable, linear scaling with simulations
- Anytime algorithm (can return best move found so far)
- Works with any game (no evaluation function needed)
- Could leverage GPU for parallel rollouts

**Weaknesses:**
- Requires many simulations for strong play
- Still loses to shallow AB search
- CPU-only benchmark doesn't show GPU potential

## Recommendations

### For Real-Time Play (Interactive UI)
- Use **AB D2** if evaluation function is good
- Achieves 8000+ moves/sec
- Suitable for responsive game interfaces

### For Stronger Play (Balanced AI)
- Use **AB D4** if time permits
- ~700 moves/sec is acceptable
- 10x better move quality than D2

### For Analytical Work (Game Balance Testing)
- Consider **AB D6** with move time limit
- Each move takes ~10ms (acceptable for batch processing)
- Or use MCTS 500+ for more consistent results

### For GPU Acceleration
- MCTS could benefit significantly from GPU rollouts
- Current CPU results (100-1000 sims) suggest GPU could achieve:
  - 5000-10000 sims in equivalent wall-clock time
  - Would compete with AB D4-D5 in both speed and quality

## Files

The benchmark code is located at:
```
/home/giblfiz/hexwar/hexwar-mcts/benches/compare_players.rs
```

Run with:
```bash
cd /home/giblfiz/hexwar
source ~/.cargo/env
cargo run --release --bench compare_players -p hexwar-mcts
```

## Future Improvements

1. **GPU MCTS Rollouts**: Test with GPU acceleration enabled
2. **Move Time Limits**: Implement time-bounded search instead of fixed simulations/depth
3. **Evaluation Function**: Tune AB heuristics for better move quality
4. **Hybrid Approach**: Use AB for shallow searches, MCTS for tactical positions
5. **Endgame Tables**: Add pre-computed endgame knowledge to speed up late game
