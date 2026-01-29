# HEXWAR Piece Values

Heuristic values used by the AI for piece evaluation. Higher = more valuable.

## Direction Key
- **F** = Forward, **B** = Backward
- **FL/FR** = Forward-Left/Right, **BL/BR** = Back-Left/Right

## Movement Types
- **Step N** = Move 1 to N hexes in direction
- **Slide** = Move any distance in direction (until blocked)
- **Jump N** = Leap exactly N hexes (ignores pieces in between)

## Piece Table

| Name | ID | Pts | Movement |
|------|-----|-----|----------|
| Pawn | A1 | 0.50 | Step 1 F |
| Guard | A2 | 1.00 | Step 1 all dirs |
| Scout | A3 | 0.50 | Step 1 F,FL,FR |
| Crab | A4 | 0.50 | Step 1 FL,FR,B |
| Flanker | A5 | 0.50 | Step 1 FL,FR |
| Strider | B1 | 0.50 | Step 1-2 F |
| Dancer | B2 | 0.67 | Step 1-2 FL,FR |
| Ranger | B3 | 2.00 | Step 1-2 all dirs |
| Hound | B4 | 1.00 | Step 1-2 F,FL,FR |
| Lancer | C1 | 0.50 | Step 1-3 F |
| Dragoon | C2 | 1.50 | Step 1-3 F,FL,FR |
| Courser | C3 | 3.00 | Step 1-3 all dirs |
| Pike | D1 | 0.83 | Slide F |
| Rook | D2 | 1.67 | Slide F,B |
| Bishop | D3 | 3.33 | Slide FL,FR,BL,BR |
| Chariot | D4 | 2.50 | Slide F,FL,FR |
| Queen | D5 | 5.00 | Slide all dirs |
| Knight | E1 | 0.50 | Jump 2 F,FL,FR |
| Frog | E2 | 1.00 | Jump 2 all dirs |
| Locust | F1 | 0.50 | Jump 3 F,FL,FR |
| Cricket | F2 | 1.00 | Jump 3 all dirs |
| Ghost | G1 | 1.50 | Step 1 all dirs, *cannot be captured* |
| Phoenix | P1 | 0.50 | Step 1 F,FL,FR, *respawns once* |
| Warper | W1 | 0.83 | Swap places with adjacent friendly |
| Shifter | W2 | 1.50 | Step 1 all dirs, *can swap on rotate* |

## Kings (not counted in army value)

| Name | ID | Movement |
|------|-----|----------|
| King (Guard) | K1 | Step 1 all dirs |
| King (Scout) | K2 | Step 1 F,FL,FR |
| King (Ranger) | K3 | Step 1-2 all dirs |
| King (Frog) | K4 | Jump 2 all dirs |

## Notes

- Values derived from mobility analysis under Template E (single action per turn)
- "all dirs" = F,FR,BR,B,BL,FL (all 6 hex directions)
- Ghost's PHASED ability (uncapturable) is likely undervalued at 1.50
- Directional pieces (Pike, Lancer, etc.) need good facing to be effective
