# HEXWAR Evolution System

A guide to how the genetic algorithm finds balanced army configurations.

## Overview

The system evolves army compositions (rulesets) to find configurations where:
1. **Skill Gradient**: Better players (deeper search) beat worse players
2. **Color Fairness**: At equal skill, white and black win ~50/50

## The Noise Problem

Individual game outcomes are noisy. The same ruleset can score 0.10 one evaluation
and 0.85 the next due to:
- Random seed differences (piece positions, game variation)
- Small sample sizes (16-64 games per evaluation)

This means: **A single high score means nothing. Only consistent performance matters.**

---

## System Components

### 1. Ruleset (Army Configuration)

```
RuleSet:
  white_pieces: ['A1', 'B2', 'D4', ...]  # Piece type IDs
  black_pieces: ['A3', 'E1', 'F1', ...]
  white_king: 'K4'                        # King type
  black_king: 'K3'
  white_template: 'E'                     # Turn structure
  black_template: 'E'
  positions: [...]                        # Fixed starting positions
```

Each ruleset gets a human-readable name based on its composition hash:
`iron-wolf`, `pale-wind`, `hard-fox`, etc.

### 2. Fitness Evaluation

For each ruleset, we play a tournament across multiple depth levels:

```
Matchup Specification (for depth=6, games_per_matchup=8):
  d2 vs d2:  8 games   (balance at low depth)
  d2 vs d3:  8 games   (skill gradient at low depth)
  d4 vs d4:  8 games   (balance at mid depth)
  d4 vs d5:  8 games   (skill gradient at mid depth)
  d6 vs d6: 16 games   (balance at target depth)
  d6 vs d7: 16 games   (skill gradient at target depth)
  -----------------------
  Total:    64 games per evaluation
```

### 3. Fitness Calculation

```python
skill_gradient = (games where deeper player won) / (total games)
color_fairness = 1.0 - abs(white_wins - black_wins) / total_games

fitness = skill_gradient * (color_fairness ^ 1.5)
```

**Ideal fitness = 1.0**: Deeper always wins, colors perfectly balanced.

---

## UCB Selection System

### The Problem with Raw Fitness

Lucky configs get high scores once, become elites, waste compute.

### Solution: Track Historical Performance

```python
FitnessTracker:
  history[ruleset_signature] = [0.34, 0.41, 0.38, 0.29, ...]  # All scores ever
```

### UCB Score Formula

```
UCB = mean - c * sqrt(1/n)

Where:
  mean = average of all historical scores for this config
  n    = number of evaluations
  c    = uncertainty penalty constant (default 0.3)
```

**Effect:**
- New config (n=1) scoring 0.80: UCB = 0.80 - 0.30 = 0.50
- Proven config (n=20) with mean 0.35: UCB = 0.35 - 0.07 = 0.28

The newcomer wins! But if it's just lucky, subsequent evals will drop its mean.

### Tuning c

- **c=0.3**: Newcomers can beat incumbents with ~0.3 higher score
- **c=0.5**: Newcomers need ~0.5 higher to beat incumbents (more conservative)
- **c=0.7**: Very conservative, hard for newcomers to displace proven configs

---

## Generation Lifecycle

### Each Generation:

```
1. EVALUATE
   For each ruleset in population:
     - Run tournament (64 games)
     - Record fitness in tracker
     - Compute UCB score from historical data

2. RANK
   Sort population by UCB score (not raw fitness!)

3. SELECT ELITES
   Top N configs by UCB become "elites"

4. REPRODUCE (Adaptive Allocation)
   For each elite:
     IF uncertain (n < min_evals):
       - Keep elite (1 copy)
       - Add clones (2 copies) → will be re-evaluated to verify
       - Add mutants (1 copy) → explore nearby
     ELSE proven (n >= min_evals):
       - Keep elite (1 copy)
       - NO clones (don't waste compute!)
       - Add MORE mutants (3 copies) → explore from known-good base

5. FILL REMAINING SLOTS
   Tournament selection + crossover + mutation
   (Explores new regions of search space)

6. REPEAT
```

### Population Composition Example (pop=18, elites=3)

**Early generations (all elites uncertain):**
```
Elite 1: 1 elite + 2 clones + 1 mutant = 4 slots
Elite 2: 1 elite + 2 clones + 1 mutant = 4 slots
Elite 3: 1 elite + 2 clones + 1 mutant = 4 slots
Exploration via crossover:            = 6 slots
                                       --------
                                        18 total
```

**Later generations (elite 1 proven, others uncertain):**
```
Elite 1 (proven): 1 elite + 0 clones + 3 mutants = 4 slots
Elite 2 (uncertain): 1 elite + 2 clones + 1 mutant = 4 slots
Elite 3 (uncertain): 1 elite + 2 clones + 1 mutant = 4 slots
Exploration via crossover:                         = 6 slots
                                                    --------
                                                     18 total
```

---

## Mutation Types

When creating a mutant from an elite:

```
Mutations (weighted random selection):
  add_white/black:      Add random piece to army
  add_copy_white/black: Add copy of existing piece (themed armies)
  remove_white/black:   Remove a piece
  swap_white/black:     Replace piece with random type
  swap_existing_*:      Replace piece with type already in army
  change_king:          Change king type
  shuffle_positions:    Randomize starting positions
```

Bias toward adding/copying existing pieces for "themed" armies.

---

## Final Verification Phase

After all generations complete:

```
1. Find best config with n >= min_evals_for_winner (default: 8)

2. If none exist:
   - Take top 5 candidates by UCB
   - Run additional evaluations until one reaches min_evals

3. Declare winner based on UCB score (not raw fitness!)

4. Report winner stats:
   - Mean fitness across all evaluations
   - Min/max range
   - Number of evaluations
```

---

## Key Parameters

| Parameter | Default | Effect |
|-----------|---------|--------|
| `--ruleset-pop` | 18 | Population size. Larger = more exploration |
| `--ruleset-gen` | 50 | Generations. More = more refinement |
| `--games` | 8 | Games per matchup type. More = lower variance |
| `--depth` | 6 | AI search depth. Higher = smarter play, slower |
| `--elites` | 3 | Elites preserved per generation |
| `--clones` | 2 | Clones per uncertain elite |
| `--mutants` | 1 | Mutants per uncertain elite (proven get clones+mutants) |
| `--ucb-c` | 0.3 | Uncertainty penalty. Higher = more conservative |
| `--min-evals` | 8 | Evals required before trusting a config |

---

## Reading the Output

### Per-Evaluation Output
```
Ruleset 5/18 [pale-wind] fitness=0.345 UCB=0.31 (n=12)
            ^            ^             ^         ^
            |            |             |         Number of historical evals
            |            |             UCB score (what matters for selection)
            |            This eval's raw score
            Config name (same name = same army composition)
```

### Per-Generation Summary
```
Fitness: [pale-wind](8) 0.76 [true-star] 0.68 [hard-fox](3) 0.38
         ^          ^   ^
         |          |   This generation's raw fitness
         |          Copies in population (8 = elite + 7 clones/mutants)
         Config name

UCB:     [pale-wind](8) UCB=0.32 n=24 [hard-fox](3) UCB=0.30 n=67
                        ^        ^
                        |        Historical eval count
                        UCB score (selection uses this)
```

### What to Watch For

**Good signs:**
- UCB scores creeping upward over generations
- One config accumulating many evaluations (being verified)
- UCB leaders and Fitness leaders converging

**Bad signs:**
- Same proven config stuck for 20+ generations (local optimum)
- Constant "breakthroughs" that crash next generation (too much noise)
- UCB scores staying flat or decreasing

---

## Typical Run Timeline

```
Gen 1-5:    Random exploration, lots of variance, no clear winner
Gen 5-15:   A few configs emerge as consistent performers
Gen 15-30:  Verification phase - clones test promising configs
Gen 30-50:  Refinement - proven winners spawn mutants, exploring nearby
Final:      Best verified config declared winner
```

---

## Example Command

```bash
python3 -m hexwar.balance \
    --template E \
    --fixed-heuristics \
    --depth 6 \
    --ruleset-gen 50 \
    --ruleset-pop 18 \
    --games 8 \
    --elites 3 \
    --clones 2 \
    --mutants 1 \
    --ucb-c 0.3 \
    --min-evals 8 \
    --output balance_jan04_test
```

**Expected:**
- ~64 games per evaluation
- ~18 * 50 = 900 evaluations total
- ~57,600 games total
- Runtime: varies by depth (d6 ≈ 10 min/generation with 10 workers)

---

## Troubleshooting

**"Lucky" configs keep becoming elites:**
- Increase `--ucb-c` to 0.4 or 0.5
- Increase `--games` to reduce per-eval variance
- Increase `--min-evals` to require more verification

**Progress too slow:**
- Decrease `--ucb-c` to let promising newcomers through faster
- Increase `--mutants` for more exploration
- Decrease `--depth` for faster games (but less realistic play)

**Getting stuck on one config:**
- Increase `--ruleset-pop` for more diversity
- Decrease `--elites` to give more slots to exploration
- Check if the config is actually good (high UCB with high n)
