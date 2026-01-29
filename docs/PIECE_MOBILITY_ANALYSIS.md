# HEXWAR Piece Mobility Analysis

## Movement Types

| Type | Description |
|------|-------------|
| **STEP** | Move 1 to N hexes in a straight line. Blocked by any piece. |
| **SLIDE** | Move any distance in a straight line until blocked. |
| **JUMP** | Land on any hex at exact distance N. Not blocked by pieces in path. |
| **NONE** | Cannot move normally (Warper uses swap ability). |

## Direction Abbreviations

- **F** = Forward (relative to facing)
- **FR** = Forward-Right
- **BR** = Back-Right
- **B** = Backward
- **BL** = Back-Left
- **FL** = Forward-Left

---

## Complete Piece Catalog

### Step-1 Pieces (A-tier)

| ID | Name | Directions | @0,0 | @-2,-2 (bad facing) | Current Heur | Notes |
|----|------|------------|------|---------------------|--------------|-------|
| A1 | Pawn | F | 1 | 1 | 1.0 | Weakest - single direction |
| A2 | Guard | All 6 | 6 | 4 | 1.5 | Omni but short range |
| A3 | Scout | F,FL,FR | 3 | 3 | 1.2 | Forward arc |
| A4 | Crab | FL,FR,B | 3 | 2 | 1.1 | Sideways + retreat |
| A5 | Flanker | FL,FR | 2 | 2 | 1.0 | Diagonal only |

### Step-2 Pieces (B-tier)

| ID | Name | Directions | @0,0 | @-2,-2 (bad facing) | Current Heur | Notes |
|----|------|------------|------|---------------------|--------------|-------|
| B1 | Strider | F | 2 | 2 | 1.5 | Fast pawn |
| B2 | Dancer | FL,FR | 4 | 4 | 1.5 | Fast flanker |
| B3 | Ranger | All 6 | 12 | 8 | 2.0 | Fast guard |
| B4 | Hound | F,FL,FR | 6 | 6 | 1.8 | Fast scout |

### Step-3 Pieces (C-tier)

| ID | Name | Directions | @0,0 | @-2,-2 (bad facing) | Current Heur | Notes |
|----|------|------------|------|---------------------|--------------|-------|
| C1 | Lancer | F | 3 | 3 | 2.0 | Long reach, narrow |
| C2 | Dragoon | F,FL,FR | 9 | 8 | 2.5 | Long reach, arc |
| C3 | Courser | All 6 | 18 | 10 | 3.0 | Long reach, omni |

### Slide Pieces (D-tier)

| ID | Name | Directions | @0,0 | @-2,-2 (bad facing) | Current Heur | Notes |
|----|------|------------|------|---------------------|--------------|-------|
| D1 | Pike | F | 4 | 6 | 2.5 | Rook-like, forward only |
| D2 | Rook | F,B | 8 | 6 | 4.0 | Chess rook (2 dirs) |
| D3 | Bishop | FL,FR,BL,BR | 16 | 10 | 3.5 | Chess bishop (4 dirs) |
| D4 | Chariot | F,FL,FR | 12 | 14 | 4.5 | Forward arc slider |
| D5 | Queen | All 6 | 24 | 16 | 6.0 | Most mobile piece |

### Jump Pieces (E/F-tier)

| ID | Name | Distance | Directions | @0,0 | @-2,-2 (bad facing) | Current Heur | Notes |
|----|------|----------|------------|------|---------------------|--------------|-------|
| E1 | Knight | 2 | F,FL,FR (arc) | 5 | 5 | 2.5 | Like chess knight, forward arc |
| E2 | Frog | 2 | All 6 | 12 | 7 | 3.5 | Omni knight |
| F1 | Locust | 3 | F,FL,FR (arc) | 7 | 6 | 3.0 | Long jump, forward arc |
| F2 | Cricket | 3 | All 6 | 18 | 8 | 3.5 | Long jump, omni |

### Special Pieces

| ID | Name | Movement | @0,0 | @-2,-2 | Current Heur | Special Ability |
|----|------|----------|------|--------|--------------|-----------------|
| W1 | Warper | None | 0 | 0 | 2.0 | SWAP_MOVE: swap with any friendly instead of moving |
| W2 | Shifter | Step-1 omni | 6 | 4 | 2.5 | SWAP_ROTATE: swap with any friendly instead of rotating |
| P1 | Phoenix | Step-1 arc | 3 | 3 | 3.0 | REBIRTH: can return from graveyard |
| G1 | Ghost | Step-1 omni | 6 | 4 | 1.5 | PHASED: cannot capture or be captured |

---

## Mobility vs Heuristic Analysis

### Observations

1. **Pawn/Flanker mismatch**: Both valued at 1.0, but Flanker has 2x mobility
2. **Guard undervalued**: 6x Pawn mobility but only 1.5x value
3. **Ranger undervalued**: 12 moves but only 2.0 value (same as Lancer with 3 moves)
4. **Jump pieces**: Mobility doesn't drop much at corners (can jump over blockers)

### Suggested Formula

A simple mobility-based heuristic could be:
```
value = base + (mobility_factor * avg_moves)
```

Where `avg_moves = (moves_at_center + moves_at_corner) / 2`

### Mobility Ranking (by average moves)

| Rank | Piece | Avg Moves | Current Heur | Suggested |
|------|-------|-----------|--------------|-----------|
| 1 | D5 Queen | 20.0 | 6.0 | 6.0 |
| 2 | C3 Courser | 14.0 | 3.0 | 4.5 |
| 3 | D4 Chariot | 13.0 | 4.5 | 4.0 |
| 4 | D3 Bishop | 13.0 | 3.5 | 4.0 |
| 5 | F2 Cricket | 13.0 | 3.5 | 4.0 |
| 6 | B3 Ranger | 10.0 | 2.0 | 3.0 |
| 7 | E2 Frog | 9.5 | 3.5 | 3.0 |
| 8 | C2 Dragoon | 8.5 | 2.5 | 2.5 |
| 9 | D2 Rook | 7.0 | 4.0 | 2.5 |
| 10 | F1 Locust | 6.5 | 3.0 | 2.0 |
| 11 | B4 Hound | 6.0 | 1.8 | 2.0 |
| 12 | D1 Pike | 5.0 | 2.5 | 1.5 |
| 13 | E1 Knight | 5.0 | 2.5 | 1.5 |
| 14 | A2 Guard | 5.0 | 1.5 | 1.5 |
| 15 | W2 Shifter | 5.0 | 2.5 | 1.5 |
| 16 | G1 Ghost | 5.0 | 1.5 | 1.5 |
| 17 | B2 Dancer | 4.0 | 1.5 | 1.2 |
| 18 | C1 Lancer | 3.0 | 2.0 | 1.0 |
| 19 | P1 Phoenix | 3.0 | 3.0 | 1.0* |
| 20 | A3 Scout | 3.0 | 1.2 | 1.0 |
| 21 | A4 Crab | 2.5 | 1.1 | 0.8 |
| 22 | A5 Flanker | 2.0 | 1.0 | 0.7 |
| 23 | B1 Strider | 2.0 | 1.5 | 0.7 |
| 24 | A1 Pawn | 1.0 | 1.0 | 0.5 |
| 25 | W1 Warper | 0.0 | 2.0 | ?** |

*Phoenix has rebirth ability - value should be higher than mobility suggests
**Warper has no moves but can swap - needs special valuation

---

## Notes on Position Testing

- **@0,0**: Center of board, optimal position, pieces have maximum freedom
- **@-2,-2**: Corner-ish position with sub-optimal facing (south), simulates worst-case mobility
- Sliders actually gain moves at edges (longer lines to slide)
- Jumpers maintain mobility well (not blocked by pieces)
