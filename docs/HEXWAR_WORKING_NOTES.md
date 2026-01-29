# HEXWAR Evolutionary Balancer - Working Notes

## Session: 2025-12-28

### Environment Documentation

**Container:**
- Ubuntu 24.04.3 LTS (Noble Numbat)
- ARM64 (aarch64) - running on M4 Mac host
- Kernel: 6.12.54-linuxkit

**Hardware:**
- 14 CPU cores available (via nproc)
- 7.7 GiB RAM (6.8 GiB available)
- 1 GiB swap

**Available Languages/Tools:**
- Python 3.12.3 (primary - spec calls for 3.11+)
- Node.js v18.19.1
- GCC/G++ (available)
- Git (available)
- No Rust/Go/CMake

**Python Constraints:**
- System-managed Python (PEP 668)
- Must use venv for pip packages
- NumPy will need venv installation

**Disk:**
- / (overlay): 53GB free
- /workspace (host mount): 279GB free (70% used)

### Workflow Rules

1. **Work in ~/**: All development in ephemeral home directory
2. **Git repos in /workspace/git-repos/**: Persistent storage only
3. **Push to persist**: Container restarts lose ~/
4. **Copy, don't symlink**: Clone repos to ~/ for work

### Project Overview

HEXWAR is an asymmetric hex-based strategy game that needs algorithmic balancing via evolutionary optimization.

**Core Challenge:** Find piece valuations and game configurations where deeper minimax search reliably beats shallower search, regardless of which color plays which side.

**Key Numbers:**
- 61 hex board (8-edge hexagon)
- 29 piece types (24 regular + 5 king variants)
- 4 action templates
- 50 turn limit
- Target: d5 search in <2 min per game
- Full evolution: 2-4 days runtime

### Implementation Phases (from spec)

| Phase | Deliverable | Est. Hours |
|-------|-------------|------------|
| 1 | Game Engine - random games to completion | 8-12 |
| 2 | Minimax Engine - AI vs AI at d5 | 6-8 |
| 3 | Tournament System - parallel matchups | 4-6 |
| 4 | Heuristic Evolution - per-color values | 3-4 |
| 5 | Rule Set Evolution - balanced configs | 6-8 |
| 6 | Infrastructure - checkpoints, CLI | 3-5 |
| **Total** | | **30-43** |

### Performance Targets

| Metric | Target |
|--------|--------|
| d5 vs d5 game | < 2 min |
| d3 vs d4 game | < 15 sec |
| Rule set fitness (110 games, 8 cores) | < 15 min |
| One generation (30 rule sets) | < 1 hour |
| Memory | < 4 GB |

### Critical Design Decisions (To Be Made)

1. **Data representation**: NumPy arrays vs pure Python dicts for board state
2. **Move generation**: Lazy vs eager, caching strategy
3. **Parallelization**: multiprocessing Pool vs ProcessPoolExecutor
4. **Hot path optimization**: Numba JIT? Cython? Pure Python first?
5. **Testing strategy**: TDD? Property-based? Integration-heavy?

### Files Structure (from spec)

```
hexwar/
├── __init__.py
├── board.py          # Hex geometry, home zones
├── pieces.py         # Piece type definitions
├── game.py           # GameState, moves, victory
├── ai.py             # Minimax, per-color eval
├── evolution.py      # GA for heuristics & rules
├── tournament.py     # Matchups, fitness
├── checkpoint.py     # Save/restore
├── cli.py            # Entry point
└── tests/
```

---

## Council of Elders Analysis

### Consensus Decisions

1. **Pure Python First** - No Numba/Cython until profiling demands it
2. **Dict-Based Board Initially** - Convert to array if needed after Phase 2
3. **Game-Level Parallelism Only** - multiprocessing Pool for games, serial minimax
4. **Pieces as Pure Data** - Dict/dataclass structure, no OOP methods
5. **Per-Color Heuristics** - Separate piece value tables for White/Black
6. **Checkpoint Per Generation** - JSON save after each generation

### Key Architecture Choices

| Decision | Choice | Rationale |
|----------|--------|-----------|
| Move generation | Eager (list) not lazy | Need full list for move ordering/sorting |
| Search state | Make/unmake | 24M positions - copying too expensive |
| Test strategy | Hybrid | Unit tests for primitives, integration for milestones |
| Specials | 4 if-statements | Only 4 exist, no need for abstraction |

### Living Tensions

1. **Performance vs Simplicity** - Start simple, profile, optimize hot paths only
2. **Test Coverage vs Velocity** - Unit test primitives, integration test milestones
3. **Correctness vs Speed** - Validate in Phase 1, trust thereafter

### Implementation Notes

- Coordinates: `q, r` (axial), never `x, y`
- Owner: 0=White, 1=Black
- Facing: 0-5 for N, NE, SE, S, SW, NW
- Depth = turns (not rounds, not actions)
- Use pytest, parametrize over piece types
- One commit per spec milestone

---

## Implementation Log

(Session-by-session progress)

### Session 1 - 2025-12-28

- Read specs (Rules + Algorithmic v1.1)
- Documented environment
- Ran Council of Elders architecture analysis
- Created git repo in /workspace/git-repos/hexwar-balancer
- **COMPLETED Phase 1: Game Engine**
  - E1: Board geometry (61 hexes, axial coords, home zones)
  - E2: 29 piece types with movement patterns
  - E3: Game state, action templates, turn structure
  - E4: Special abilities (Swap, Resurrect, Ghost/Phased)
  - E5: Full game loop - random games complete in ~3ms

**121 tests passing**

**Key discoveries:**
- Spec has contradictory info (says 8 hexes per edge but 61 total - radius must be 4)
- Ghost implementation requires checking BOTH moving piece AND target for PHASED
- Random games average ~43 rounds, complete quickly

- **COMPLETED Phase 2: Minimax Engine**
  - M1: Negamax with alpha-beta pruning
  - M2: Per-color heuristics (separate piece values for White/Black)
  - M3: Move ordering (MVV-LVA, captures first)
  - M4: AI vs AI games working

- **COMPLETED Phase 3: Tournament System**
  - Depth matchups with multiprocessing
  - Fitness evaluation (skill gradient, color fairness, game richness)
  - Reduced tournament mode for faster testing

- **COMPLETED Phase 4: Heuristic Evolution**
  - Genetic algorithm for per-color piece values
  - Crossover, mutation operators
  - Tournament selection with elitism

- **COMPLETED Phase 5: Rule Set Evolution**
  - RuleSet dataclass for army compositions
  - Random ruleset generation (skeleton)
  - Integration with heuristic evolution

- **COMPLETED Phase 6: Infrastructure**
  - JSON checkpointing (save/restore evolution state)
  - CLI with subcommands: play, tournament, evolve, benchmark, info
  - Background task support

**138 tests passing** (14.76s)

### Performance Benchmarks (Session 1 Final)

| Metric | Result |
|--------|--------|
| Random game | 2.4ms |
| d1 move gen | 2.1ms |
| d2 move gen | 16.1ms |
| d3 move gen | 508.5ms |
| d2 vs d2 game | 3.8s |
| d3 vs d3 game | 20.8s |

**Notes:**
- d5 search target would require optimization (Numba, Cython, or C extension)
- Current depth ~d3 is practical for fast iteration
- max_moves_per_action parameter controls branching factor

---

## PROJECT COMPLETE

All 6 phases of the HEXWAR Evolutionary Balancer implemented:
1. Game Engine - hex board, pieces, actions, specials
2. Minimax Engine - alpha-beta, per-color heuristics
3. Tournament System - parallel depth matchups
4. Heuristic Evolution - genetic algorithm
5. Rule Set Evolution - army composition framework
6. Infrastructure - checkpointing, CLI

Git repo: `/workspace/git-repos/hexwar-balancer`
