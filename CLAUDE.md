# HEXWAR Balancer - Claude Context

## What This Is
Evolutionary game balancer for HEXWAR, an asymmetric hex-based strategy game. Uses genetic algorithms to evolve army compositions with template-aware piece valuation.

---

## Definition of Done (ENFORCED)

**A task is NOT considered complete unless ALL of the following are true:**

1. **Unit tests pass** - All relevant unit tests run and pass
2. **Integration tests pass** - Manual or automated integration testing confirms the feature works end-to-end
3. **Code follows CODESTYLE.md** - Check `/home/giblfiz/CODESTYLE.md` for granularity rules (4-layer structure)
4. **Changes committed to git** - Every meaningful change gets its own commit with a descriptive message
5. **Documentation updated** - NOTES.md and README.md reflect the current state

### Testing Requirements

- **Unit tests**: Run with `pytest` or `python -m pytest`. Write tests for all new logic.
- **Integration tests**: Can be done by hand. Document what you tested and the results.
- **If tests don't exist**: Write them before claiming the task is done.

### Git Discipline

- Commit early, commit often
- Each commit should be atomic (one logical change)
- Commit message explains WHY, not just WHAT
- Don't let changes pile up uncommitted

### Documentation Discipline

- **NOTES.md**: Keep session notes, what was tried, what worked/failed
- **README.md**: Keep usage instructions and project overview current
- Update docs AS YOU GO, not at the end

---

## User Access Pattern

**IMPORTANT:** The user accesses this box remotely and views web pages from their remote machine. They cannot directly access local files.

- To show game records: Send to designer via `curl -X POST -H "Content-Type: application/json" -d '{"path": "path/to/game.json"}' http://localhost:8002/api/playback/load`
- To show champions: Use the designer's load API
- The designer runs on port 8002 and is the primary way to visualize games/boards

---

## Refactoring Goals (Active)

This codebase is undergoing macro-scale refactoring with these objectives:

1. **Move toward functional programming** - Pure functions, immutable data, explicit data flow
2. **Write unit tests** - Cover all critical logic with focused, isolated tests
3. **Clean up directory structure** - Logical organization, clear module boundaries
4. **Follow the 4-layer granularity approach** - See below

---

## Code Granularity (ENFORCED)

**Rule: One function, one zoom level.** Never mix orchestration and implementation detail in the same function.

### Required Structure

Organize code into **3–4 abstraction layers**:

| Layer | Purpose | Allowed Content |
|-------|---------|-----------------|
| **Level 1 (Orchestration)** | High-level flow only | No loops. No complex branching. No algorithmic work. Only calls to Level 2 with clear names. |
| **Level 2 (Phases)** | Major subsystems or phases | Minimal control flow, delegating all detailed work to Level 3. |
| **Level 3 (Steps)** | Concrete, small, testable steps | Detail allowed here (loops, branching), but keep each function focused on one step. |
| **Level 4 (Utilities)** | Helpers and direct library calls | Optional layer for reusable primitives. |

### Hard Constraints

- If a function is intended to be higher-level, it **must not "drop down"** into low-level detail
- If you see loops, parsing, complex conditionals, or data-structure manipulation inside Level 1 or Level 2, **extract** that logic into a lower-level function
- Each function body should be readable at its level as a short narrative of what happens next
- Names must match granularity: higher-level names describe phases, lower-level names describe actions

### Output Requirements

- The **top-level function reads like a table of contents**
- Prefer many small, well-named functions over fewer large mixed-granularity functions
- Refactor aggressively to restore consistent granularity before adding features or optimizing
- A reader should understand the system by reading only Level 1, then zoom in layer by layer as needed

### Example Structure
```python
# Level 1 - Orchestration (reads like a table of contents)
def run_evolution():
    config = load_configuration()
    population = initialize_population(config)
    champion = evolve_population(population, config)
    save_results(champion)

# Level 2 - Phases (delegates to steps)
def evolve_population(population, config):
    for generation in range(config.generations):
        fitness_scores = evaluate_population(population)
        survivors = select_survivors(population, fitness_scores)
        population = breed_next_generation(survivors)
    return get_champion(population)

# Level 3 - Steps (concrete work happens here)
def evaluate_population(population):
    return [compute_fitness(individual) for individual in population]

# Level 4 - Utilities
def compute_fitness(individual):
    ...
```

---

## Quick Orientation

**To run a test:**
```bash
python3 -m hexwar.balance --ruleset-gen 1 --ruleset-pop 3 --games 4 --depth 2 --workers 10 --output balance_test --seed 42
```

**Entry point:** `hexwar/balance.py` (CLI orchestrator)

## Key Files

| File | Purpose |
|------|---------|
| `hexwar/balance.py` | Main CLI - runs the 3-phase pipeline |
| `hexwar/evolution.py` | Genetic algorithms, RuleSet/Heuristics evolution |
| `hexwar/tournament.py` | Game runner, fitness evaluation |
| `hexwar/ai.py` | Negamax AI with alpha-beta pruning |
| `hexwar/game.py` | Game state, rules, move generation |
| `hexwar/pieces.py` | 29 piece type definitions |
| `hexwar/board.py` | Hex geometry (axial coordinates) |
| `hexwar/autoscale.py` | Dynamic worker scaling |
| `hexwar_core/src/lib.rs` | Rust game engine (~500x speedup) |

## Documentation

| Doc | Contents |
|-----|----------|
| `README.md` | Full project documentation, parameters, usage |
| `docs/SESSION_NOTES.md` | Dec 29 session - bug fixes, optimizations |
| `docs/HEXWAR_WORKING_NOTES.md` | Dec 28 session - initial implementation |
| `docs/HEXWAR_Rules_Specification.docx` | Original game rules |
| `docs/HEXWAR_Algorithmic_Specification_v1.1.docx` | Evolution/balancing spec |

## Recent Bug Fixes (Dec 2024)
1. **Heuristic eval broken** - was playing candidate vs itself, not vs opponent
2. **Stats showing 0.0** - avg_rounds hardcoded, color_fairness logic wrong at equal depths
3. **Missing positions** - bootstrap ruleset didn't generate piece positions

## Output Convention
**IMPORTANT:** Name output directories as: `balance_monDD_HHMM` (e.g., `balance_jan06_0930`)

DO NOT use descriptive names like `balance_fixed`, `balance_test2`, `balance_final`.
Date-time names are bulletproof and avoid the `final.final.really_final` antipattern.

## Rust Module
If `RUST_AVAILABLE = False`, rebuild with:
```bash
cd hexwar_core && maturin develop --release
```

## Git Workflow

**IMPORTANT:** Commit every code change to git with a descriptive message explaining WHY the change was made.

- After completing any feature, bug fix, or meaningful change, run `git add -A && git commit`
- Commit messages should explain the purpose/reasoning, not just list files changed
- Include the conversation log (`docs/CONVERSATION_LOG.md`) in every commit

## Conversation Log

**IMPORTANT:** Maintain `docs/CONVERSATION_LOG.md` as a running log of all conversations.

- Log everything: user messages and assistant responses
- Update the log as the conversation progresses
- Include the log in every git commit
- This provides project history and context for future sessions

## Housekeeping (Automatic)

**IMPORTANT:** As part of general housekeeping, automatically clean up old files without being asked.

- Balance runs (`balance_*/`) and log files (`*.log`) more than a day old should be moved to `archive/`
- Old experiment outputs and intermediate files should be archived, not left in the project root
- The `archive/` directory is gitignored and serves as a holding area for old data
- Do this proactively during sessions when appropriate, don't wait for explicit requests

---

## Birds-Eye View

### Project Goal
Find balanced army compositions for HEXWAR where neither side has an unfair advantage. Uses genetic algorithms to evolve army rulesets with template-aware piece valuation.

### Pipeline
1. **Template-aware Heuristics** - Pieces valued based on how useful they are with the ruleset's action template (computed dynamically)
2. **Ruleset Evolution** - Evolve army compositions (which pieces, positions, facings)
3. **Final Validation** - Test best rulesets at higher depth

### Key Design Patterns
- **Fixed-army evolution**: Lock one side (white or black), evolve only the other. See `--fixed-white` and `--fixed-black` flags.
- **Board sets**: Human-designed armies stored in `board_sets/` (e.g., "the_necromancer", "the_orcs")
- **Seeds**: Pre-designed starting populations in `board_sets/*/` subdirs
- **Templates**: Piece placement templates (A-E) controlling starting zones

### Current State (Jan 2026)
- Evolution system works at D2-D4
- Board designer UI at `hexwar/visualizer/` (port 8002)
- Best champions in `balance_jan10_d5_orcs_1952/champions/`
- Heuristic evolution needs optimization before D6+ is practical

---

## Observed Run Times (Jan 2026)

### Single Game Duration (max_moves=50)
| Depth | Time | Notes |
|-------|------|-------|
| D2 | 1.7s | Reasonable for testing |
| D4 | 736s (12 min) | ~430x slower than D2 |
| D6 | ~20+ hours | Extrapolated, impractical |
| D8 | Days | One game ran 4.7 hours before killed |

### Historical: Heuristic Evolution (removed Jan 2026)
NOTE: Heuristic evolution was removed. We now use template-aware heuristics computed dynamically.
These timings are kept for reference:

#### Old Heuristic Evolution Timings (20 gens, 8 pop, 4 games/eval)
| Depth | Expected Total | Actual |
|-------|----------------|--------|
| D2 | ~10 min | ~9s for 2 gens (test) |
| D4 | ~16 hours | Gen 1 took 16 min (100-475s/game variance) |
| D8 | Weeks | Ran 47 hours, never completed |

### Ruleset Evolution (50 gens, D5)
- `balance_jan10_d5_orcs_1952`: Completed in ~1 hour
- D5 ruleset games are faster than D5 heuristic games (games end on captures)

---

## Expensive Gotchas (Hard-Won Lessons)

### 1. AI Depth Scaling is BRUTAL
```
D2: ~2 seconds/game
D4: ~12 minutes/game (430x slower!)
D6: ~20+ hours/game (impractical)
D8: days per game
```
**Historical note:** Heuristic evolution was removed. If reimplemented, never run above D4 without serious optimization. Ruleset evolution is faster because games end on captures.

### 2. max_moves_per_action vs max_moves
- `max_moves_per_action`: Limits branching factor in AI search (default 10-15)
- `max_moves`: Total game length limit (default 150)
Don't confuse these. Increasing max_moves_per_action explodes search time.

### 3. Game End Conditions
Games end by: king capture, OR 50-round proximity rule (king closer to center wins). The AI heuristics need `king_center_weight` to align with the proximity rule, otherwise AI plays aimlessly in late game.

### 4. RuleSet Facings
Added Jan 2026. `white_facings` and `black_facings` track piece rotations. Important for directional pieces (Pike, Ranger). Facings are preserved through mutation/crossover.

### 5. Worker Processes Don't Die Easily
`pkill -f pattern` often fails silently. Use explicit `kill PID1 PID2...` or `kill -9` for stubborn processes.

### 6. Stdout Buffering
Python subprocess output is buffered. Always use `flush=True` on prints for real-time logging, or output goes to /dev/null until process ends.

---

## Trailheads

### Design Specs (Start Here for Understanding)
- `docs/HEXWAR_Rules_Specification.docx` - Game rules, piece types, win conditions
- `docs/HEXWAR_Algorithmic_Specification_v1.1.docx` - Evolution algorithm design, fitness functions
- `docs/CONVERSATION_LOG.md` - Full conversation history with context

### If Reimplementing Heuristic Evolution
The heuristic evolution code was removed (Jan 2026) because D6+ was impractically slow.
If reimplementing, consider:
1. Keep it D4 max (12 min/game is tolerable)
2. Optimize `hexwar/ai.py` negamax search first
3. Add Rust acceleration for game simulation
4. Follow the 4-layer granularity approach in CLAUDE.md

### To Resume Ruleset Evolution
```bash
python -m hexwar.balance --fixed-black board_sets/the_necromancer.json \
  --seeds board_sets/lower_ring_seeds --depth 5 --output balance_jan13_xxxx
```

### To Use the Board Designer
```bash
python3 -m hexwar.visualizer.server  # port 8002
```

**To push a champion to display:**
```bash
curl -X POST -H "Content-Type: application/json" \
  -d '{"name": "champion-name"}' \
  http://localhost:8002/api/designer/load
```
The server searches `balance_*/champions/` for the named champion. Response is slow (~30s) but works.

### To View Champions
Best results in `balance_*/champions/*.json`. Load to visualizer to see them.

### Key Files for AI Performance
- `hexwar/ai.py`: `negamax()` function is the hot path
- `hexwar_core/src/lib.rs`: Rust game engine (not currently used for AI search)
