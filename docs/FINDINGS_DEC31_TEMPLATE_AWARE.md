# Template-Aware Heuristics Evolution Findings

**Date:** December 31, 2024
**Run:** `balance_dec31_fixed`
**Evolution Time:** ~15 minutes (50 generations)

## Summary

This run tested the "template-aware" heuristics approach, which skips heuristic evolution and instead computes piece values dynamically based on each ruleset's action templates.

**Key Insight:** A piece's value depends heavily on its template. A Lancer (forward-only slider) with Template A (rotate-then-move) becomes nearly as powerful as a Queen, but with Template D (move-then-rotate-different) it's just a forward poker.

## Results

### Best Configuration Found

| Side | Template | King | Pieces |
|------|----------|------|--------|
| White | C (Move, Move, Rotate) | K3 (Ranger) | Pawn, Scout, Crab, Locust, Warper |
| Black | B (Move, Rotate, Rotate) | K1 (Guard) | Strider, Dancer, Hound, Pike, Chariot |

**Final Evaluation (16 games):**
- White wins: 8 (50.0%)
- Black wins: 8 (50.0%)
- Average game length: 13.3 rounds
- Fitness: 0.840

### Evolution Behavior

- Found fitness 1.0 configurations in **Generation 1** and multiple subsequent generations
- Consistently achieved skill gradient of 1.0 (deeper search always wins)
- Color fairness fluctuated between 0.6-1.0 across generations
- Best configs maintained 8-8 or 7-9 splits at equal depth

## Analysis

### Does This Look Like a Balanced Game?

**Positive signs:**
1. **Perfect 50/50 split** in final evaluation (8-8)
2. **Perfect skill gradient** (deeper search always wins)
3. **Small armies** (6 units each) - suggests tighter, tactical games
4. **Asymmetric design** - different templates, kings, and pieces
5. **No "fake balance"** - heuristics honestly value pieces

**Concerns:**
1. **Very short games** (13.3 rounds avg) - might be too quick/decisive
2. **Color fairness score of 0.6** - while 8-8 looks perfect, fitness formula penalizes inconsistency
3. **Small armies** - could mean less strategic depth
4. **Template C vs B** - both are "move first" templates; more diverse template matchups weren't selected

### Template Multiplier Effects

The template-aware heuristics apply these multipliers to directional pieces:

| Template | 1-dir piece | 3-dir piece | 6-dir piece |
|----------|-------------|-------------|-------------|
| A (Rotate-Move-Same) | 2.5x | 1.5x | 1.0x |
| B (Move-Rotate-Rotate) | 0.85x | 0.92x | 1.0x |
| C (Move-Move-Rotate) | 0.75x | 0.87x | 1.0x |
| D (Move-Rotate-Diff) | 0.6x | 0.8x | 1.0x |

This explains why:
- White (Template C) has pieces like Locust (3-dir jumper) valued low at 0.50
- Black (Template B) has better valuations for directional pieces
- Neither side has Template A, which would make directional pieces overpowered

### Army Composition Analysis

**White's approach:**
- Warper (teleport) for repositioning
- Locust (3-hex jump) for king threats
- Small mobile pieces (Scout, Crab) for control
- K3 Ranger king (2-hex omnidirectional) - good escape

**Black's approach:**
- Pike and Chariot (sliders) for reach
- Hound and Dancer for mid-range mobility
- K1 Guard king (1-hex omnidirectional) - defensive

Both armies avoid highly directional pieces (like Pawn-heavy armies) because their templates don't boost them.

## Comparison with Previous Runs

| Run | Heuristic Mode | Fitness | Color Fairness | Notes |
|-----|----------------|---------|----------------|-------|
| dec30_2043 | Evolved | 0.782 | 0.889 | "Fake balance" - AI handicapped itself |
| dec31_fixed | Template-aware | 0.840 | 0.600 | Honest valuation, small armies |

The template-aware approach produces:
- Higher fitness overall
- More consistent skill gradient
- Less "gaming" of the heuristics by evolution

## Recommendations

1. **Explore larger armies** - Current 5+1 might be too minimal
2. **Force Template A** - See if rotate-first template creates interesting asymmetry
3. **Longer evaluations** - 16 games may not be statistically robust
4. **Tune template multipliers** - Current values are theoretical; could be tuned empirically
5. **Add minimum piece count** - Prevent degenerate small-army solutions

## Conclusion

The template-aware heuristics approach successfully prevents the "fake balance" problem where evolution exploits the AI's piece valuations. The resulting configurations show genuine balance potential.

However, the evolution is converging on small armies with similar templates, suggesting the search space might need constraints or the fitness function might need adjustment to encourage more diverse, complex configurations.

**Verdict:** This is a real improvement over evolved heuristics. The balance looks legitimate, but the game might be too simplified. Next step: run with minimum army size constraints and explore Template A matchups.
