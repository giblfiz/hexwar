# Three-Team Balance Architecture for HEXWAR

**Version 3.0** - With design constraints, timing corrections, and autonomous operation
**Date**: 2026-01-31

---

## Executive Summary

This document describes an architecture for evolving three optimally balanced HEXWAR teams (Orc, Necro, Fey) such that ANY pairwise matchup produces excellent fitness scores.

**Critical Constraints:**
- 72-hour compute budget
- Hardware: RTX 3060 (12GB), 6-core i5, 14GB RAM
- **D6 evolution games: ~2.4 seconds** (measured from overnight run of 17,920 games)
- Armies must feel "themed" for human players (similar pieces, visual symmetry)

**Known Starting State:**
- Existing Fey seeds in `board_sets/fey_seeds/` and `board_sets/fey_warper_seeds/` (21 variants)
- Recent Fey-vs-Necro evolution achieved fitness ~0.85 over 80 generations
- Orc-Necro balance is **problematic** (~31-35% white win rate at D8)
- We cannot simply "fix two and evolve one" - all three may need adjustment

---

## Design Constraints (HARD REQUIREMENTS)

### Signature Pieces

Each army has **signature pieces** that define its identity. These are non-negotiable:

| Army | MUST Have | MUST NOT Have | Rationale |
|------|-----------|---------------|-----------|
| **Necro** | G1 (Ghost) ×1+, P1 (Phoenix) ×1+ | W1 (Warper) | Resurrection/undead theme |
| **Fey** | W1 (Warper) ×1+ | G1, P1 | Teleportation/trickster theme |
| **Orc** | (TBD - aggressive pieces) | G1, P1, W1 | Swarm/aggression theme |

**Implementation**: The mutation operator MUST preserve signature pieces. When mutating:
- Never remove the last signature piece
- Never add opponent's signature pieces
- Can adjust quantities (1→2 Ghosts is fine, 1→0 is not)

### Army Theming (Human Aesthetics)

Armies should feel cohesive for human players:

1. **Piece Repetition**: Use 2-4 copies of key pieces (not 11 unique pieces)
2. **Visual Symmetry**: Mirror positioning where possible (e.g., rangers on both flanks)
3. **Thematic Consistency**: Pieces should "make sense" together:
   - Necro: Slow, durable, resurrection-focused
   - Fey: Mobile, tricky, repositioning-focused
   - Orc: Fast, aggressive, swarm-focused

**Implementation**: Add a "theme coherence" term to fitness:
```
theme_score = piece_repetition_bonus + symmetry_bonus + signature_piece_bonus
fitness = balance * skill_gradient * theme_score
```

### Mutation Constraints

The evolution system must respect these constraints:

```rust
fn is_valid_mutation(army: &Army, mutation: &Mutation) -> bool {
    let after = apply_mutation(army, mutation);

    // Check signature piece constraints
    match army.faction {
        Necro => after.has_piece("G1") && after.has_piece("P1") && !after.has_piece("W1"),
        Fey => after.has_piece("W1") && !after.has_piece("G1") && !after.has_piece("P1"),
        Orc => !after.has_piece("G1") && !after.has_piece("P1") && !after.has_piece("W1"),
    }
}
```

---

## Problem Statement

We seek three army configurations (O, N, F) such that for all pairs:
- `Fitness(O vs N) >= 0.70`
- `Fitness(N vs F) >= 0.70`
- `Fitness(F vs O) >= 0.70`

Where `Fitness` measures (from existing codebase):
- **Skill Gradient** (40%): Deeper-searching AI beats shallower AI
- **Color Fairness** (35%): At equal skill, white/black win ~50/50
- **Game Richness** (15%): Games last 20-50 rounds (not instant kills or endless stalemates)
- **Decisiveness** (10%): Clear outcomes (few draws)

### Formal Objective

Minimize the **pairwise imbalance vector**:

```
I = [|w(O,N) - 0.5|, |w(N,F) - 0.5|, |w(F,O) - 0.5|]
```

Where `w(A,B)` = win rate of A vs B at equal depth.

**Ideal**: I = [0, 0, 0] (all matchups perfectly balanced)

### Known Starting Imbalances

| Matchup | Known Data | Balance Score |
|---------|------------|---------------|
| Orc vs Necro | ~31-35% white win rate (D8 data) | **0.38** (problematic) |
| Fey vs Necro | ~48.6% white after 80 gens | ~0.97 (good) |
| Fey vs Orc | Unknown | Unknown |

**Critical insight**: The "fix two, evolve one" approach won't work because Orc-Necro is already imbalanced. We need a strategy that can adjust all three teams.

---

## Council of ML Architects

### Phase 1: The Discussion

**[Room: A virtual conference room. Six architects face a hex grid diagram and timing benchmarks on the screen showing "D4: 736s/game (general), D6: 2-5s/game (evolution)". Coffee cups everywhere.]**

**Dean**: *pointing at the screen* Let me start with the brutal math. D4 games take 12 minutes each - 736 seconds measured. D6 is impractical - over 20 hours per game in some cases. We have 72 hours total. That's roughly 360 D4 games if we do nothing else. For context, a single evolution run with pop=16, 50 generations, evaluating against TWO opponents needs... *calculates* ...at minimum 1,600 evaluations. At 40 games per eval against 2 opponents, that's 128,000 games. At D4, that's 25,600 hours serial. Even with 12 threads, that's 2,100 hours. We're off by a factor of 30.

**Hotz**: *leaning forward* So the entire premise is broken. You can't do D4 evolution in 72 hours. Period. What's D2 timing?

**Dean**: About 1.7 seconds per game. That's 430x faster than D4.

**Karpathy**: But D2 is tactically shallow. Armies that look balanced at D2 might be completely broken at deeper search. The AI doesn't see threats two moves ahead.

**Olah**: *drawing on whiteboard* This is actually a question about the fitness landscape. Is the D2 fitness landscape a good approximation of the D4/D5 landscape? If they're correlated, D2 filtering works. If they're orthogonal, we're wasting compute. But here's a deeper question: is the three-way balance landscape even **convex**? If there are multiple local optima - which seems likely given the rock-paper-scissors dynamics - gradient descent via evolution might never find the global optimum.

**Chollet**: That's a fundamental concern. Can you elaborate on what non-convexity looks like here?

**Olah**: *draws triangle* Imagine each army configuration as a point in high-dimensional space. The "balanced region" is where all three pairwise fitness scores exceed threshold. This region might be:
1. A single connected blob (good - any path works)
2. Multiple disconnected islands (bad - you can get trapped)
3. A thin manifold that's easy to fall off (very bad - unstable equilibrium)

Given that rock-paper-scissors is a stable equilibrium in game theory, I suspect option 3. Small perturbations to any army could tip the balance and cascade through all three matchups.

**Hotz**: *frustrated* Great, so we might be solving an impossible problem. Let me push back harder. Why are we doing full three-way co-evolution at all? That's the most complex approach. The user has existing Orc and Necro armies. They have 21 Fey seeds already created. Why not just pick the best Fey manually? Evolution is overkill if we already have decent starting points.

**Howard**: *pulls up data* Actually, I checked the existing data. Fey-vs-Necro evolution just completed - 80 generations, achieved fitness ~0.80 with balance ~48.6% white. That's already good! But we don't have Fey-vs-Orc data.

**Karpathy**: And critically, we have Orc-vs-Necro data. *pulls up numbers* It shows ~31-35% white win rate at D8. That's a balance score of about 0.38. The foundation is already broken.

**Hotz**: Wait, so the "fix two, evolve one" strategy fails immediately because Orc-Necro isn't balanced?

**Karpathy**: Exactly. If we fix Orc and Necro and only evolve Fey, we're building on a cracked foundation. Fey might achieve balance against both, but Orc-Necro remains imbalanced.

**Chollet**: But here's the problem - they were designed in isolation or against a single opponent. The Orc army might be optimized to beat Necro specifically. Add Fey, and suddenly Orc is terrible against Fey but still beats Necro.

**Chollet**: But here's the problem - they were designed in isolation or against a single opponent. The Orc army might be optimized to beat Necro specifically. Add Fey, and suddenly Orc is terrible against Fey but still beats Necro.

**Dean**: Right, we need to think about this as a **Pareto frontier** problem. Each team has three objectives:
1. Beat opponent A at rate ~0.5
2. Beat opponent B at rate ~0.5
3. Maintain skill gradient

A team is "Pareto optimal" if you can't improve one objective without hurting another.

**Hotz**: I hate multi-objective optimization. It's always wishy-washy. Can we collapse this into a single number?

**Olah**: What if we define fitness as the **minimum** across pairwise balances? A team is only as good as its worst matchup.

**Karpathy**: *nods* That's actually elegant. If Orc beats Necro 50/50 but loses to Fey 80/20, the Orc fitness is 0.3 (from the Fey matchup). You can't cheese the system by being great at one matchup.

**Howard**: But that means we need to evaluate every team against BOTH opponents every generation. Double the games.

**Dean**: *worried* And here's what's really concerning about the validation tournament cost. If we want statistical confidence in our balance measurements, we need enough games. At 100 games per matchup, three matchups, that's 300 D4 games just for ONE validation checkpoint. That's 60 hours serial, or 5 hours with 12 threads. If we validate every 10 generations over a 50-gen run, that's 5 validations = 25 hours just for validation. That's a third of our budget!

**Hotz**: Then don't validate that often. Validate at the end.

**Dean**: But if we only validate at the end and discover we went down a bad path, we've wasted the entire run. Validation is a feedback signal - we need it during evolution, not just after.

**Dean**: Maybe not. We could do **staged evaluation**:
- Stage 1: Quick eval against ONE random opponent (cheap filter)
- Stage 2: Full eval against BOTH opponents (only for promising candidates)

Like coarse-to-fine search in vision.

**Chollet**: I like that pattern. The expensive full-evaluation becomes a "verification" step, not the main selection pressure.

**Hotz**: But how do we handle the co-evolution? If I'm evolving Orcs and Necros simultaneously, the fitness landscape is shifting under me every generation.

**Karpathy**: That's the core question. Three approaches:
1. **Sequential**: Evolve Orc vs fixed Necro. Then evolve Fey vs fixed (Orc, Necro). Repeat.
2. **Parallel**: Evolve all three simultaneously, evaluate against current-gen opponents.
3. **Round-robin generations**: Each gen, one team evolves, others stay fixed.

**Olah**: Sequential is simplest to understand but might not converge. You could get oscillating - Orc beats Necro, then Necro evolves to beat Orc, then Orc evolves back...

**Dean**: Classic Red Queen dynamics. In evolutionary game theory, this is called the "arms race" problem.

**Howard**: Has anyone tried just... not co-evolving? Pick one strong Necro, one strong Orc, both fixed, and only evolve Fey to beat both?

**Chollet**: That could work as a bootstrap! Start with known-good Orc and Necro, evolve Fey against both. Then freeze Fey, evolve Orc against (Necro, Fey). Then freeze Orc, evolve Necro against (Orc, Fey). One round of this might be enough.

**Karpathy**: I like that. It's sequential but with multiple passes. Let's call it **Cyclic Refinement**.

**Hotz**: How many cycles though? Each cycle is three evolution runs. That's a lot of compute.

**Dean**: Let me re-estimate. If we can do D4 games in ~12 minutes, and we run 20-game evaluations... *calculates* With pop=30, 50 generations, that's 30K games per team. Three teams, two cycles = 180K games. At 12 min/game, that's 36,000 hours. Still way over budget.

**Olah**: We need cheaper evaluation. What about D2 for most games, D4 only for verification?

**Karpathy**: The danger is D2 doesn't capture real strategic depth. Armies that look balanced at D2 might be completely broken at D6.

**Howard**: But D2 is correlated with D6, right? If something is totally unbalanced at D2, it won't magically become balanced at D6.

**Dean**: True. We could use **depth-stratified sampling**:
- 80% of evals at D2 (fast, noisy but directional)
- 15% at D4 (validation)
- 5% at D6 (final verification for elite candidates only)

**Chollet**: That's a good abstraction. The fitness function becomes a **multi-fidelity estimate**. Cheap approximation for filtering, expensive truth for selection.

**Hotz**: I want to push back on the three-team structure. Do we actually need THREE armies? What if we only balance two, then check if a third can be designed manually?

**Karpathy**: The user specifically asked for three. But your point stands - maybe we shouldn't evolve all three from scratch. Evolve ONE team to be balanced against TWO fixed opponents.

**Olah**: That's actually much simpler! Fix Orc and Necro (already designed), evolve Fey to be balanced against both. Then we have a balanced triple.

**Howard**: And if Orc vs Necro isn't balanced? We tweak one of them manually or run a quick evolution pass.

**Chollet**: I think we're converging on a hybrid approach:
1. Validate Orc vs Necro is already decent
2. Evolve Fey against both (one evolution run)
3. If Orc-Necro drifted, do refinement pass

**Dean**: Let me budget this. One evolution run with 30 pop, 50 generations:
- Each candidate plays 20 games vs Orc + 20 games vs Necro at D2 = 40 games
- 30 * 50 = 1,500 evaluations = 60,000 games
- At D2 (~2s/game) = 33 hours

That fits in 72 hours!

**Hotz**: But you're not testing Orc vs Necro at all in this model.

**Karpathy**: Good catch. We need a **validation tournament** every N generations that tests all three pairings.

**Olah**: What metrics do we track in the validation tournament?

**Dean**: For each pairing:
- Win rate at equal depth (D4)
- Skill gradient (D4 vs D5)
- Game length distribution
- Draw rate

**Chollet**: The fitness function for Fey evolution should be:

```
fitness(Fey) = min(balance(Fey,Orc), balance(Fey,Necro)) * skill_gradient
```

Where `balance(A,B) = 1 - 2*|win_rate - 0.5|` (1.0 = perfect, 0.0 = completely one-sided)

**Karpathy**: That's clean. And the `min` ensures Fey can't specialize against one opponent.

**Howard**: What about the Fey design itself? Are we evolving from scratch or using seeds?

**Hotz**: Seeds are critical. Random initialization wastes compute. Start with something reasonable.

**Olah**: Could we design Fey seeds heuristically? Look at what Orc and Necro are weak against?

**Karpathy**: That's interesting. Orc is aggro with scouts and warper. Necro has ghosts and phoenix for resurrection. Fey could be... ranged and defensive? Counter the Orc swarm, outvalue the Necro attrition?

**Howard**: Let's not over-engineer the seeds. Just make 5-10 variants with different piece mixes and let evolution figure it out.

**Chollet**: Agreed. The GA should do the heavy lifting, not our intuition.

**Dean**: One more optimization - **incremental evaluation**. If a candidate is a mutation of an elite, we might be able to predict its fitness without full evaluation.

**Hotz**: That sounds like premature optimization. Let's get the basic system working first.

**Karpathy**: Agreed. Okay, let me summarize what I'm hearing:

**Architecture Summary:**

1. **Fixed opponents**: Use existing Orc and Necro as fixed targets
2. **Single evolution**: Evolve Fey population against both opponents
3. **Min-fitness**: Fey fitness = min(balance_vs_orc, balance_vs_necro) * skill_gradient
4. **Depth stratification**: Mostly D2, validate at D4, elite verification at D6
5. **Cyclic refinement** (optional): After Fey stabilizes, refine Orc/Necro if needed
6. **Validation tournaments**: Every 10 generations, test all three pairings

**Chollet**: I think we're missing one thing - what if the initial Orc vs Necro matchup is unbalanced?

**Dean**: Good point. First pass should be a diagnostic:
- Run 100 games Orc vs Necro at D4
- If balance > 0.7, proceed
- If balance < 0.7, evolve Orc vs fixed Necro first (or vice versa)

**Howard**: That's a reasonable pre-check. Total time budget:

```
Diagnostic: 100 games @ D4 = ~20 hours
Fey evolution: 60K games @ D2 = ~33 hours
Validation: 3 pairings * 100 games @ D4 = ~60 hours (can run overnight)
Buffer: ~5 hours

Total: ~72 hours (fits!)
```

**Olah**: Wait, that validation math is wrong. 300 D4 games at 12 min each is 60 hours, not including Fey evolution.

**Dean**: *recalculates* You're right. Let me redo:
- D4 game: ~10 minutes (recent benchmarks show it's faster with Rust)
- Diagnostic: 100 games = 16 hours
- Fey evolution at D2: 60K games @ 2s = 33 hours
- Final validation: 300 games @ D4 = 50 hours

That's 99 hours. Over budget.

**Hotz**: Parallelize more aggressively. We have 12 threads and a GPU.

**Karpathy**: The Rust engine is single-threaded per game, but we can run multiple games in parallel. With 12 threads, validation drops to 50/12 = 4 hours.

**Dean**: And diagnostic to 1.5 hours. That's:
- Diagnostic: 1.5 hours
- Fey evolution: 33 hours (can parallelize eval too)
- Validation: 4 hours

Total: ~40 hours with parallelism. We have margin!

**Chollet**: The Fey evolution can also be parallelized. Population of 30, evaluate in parallel = 1/12 the serial time.

**Howard**: Final sanity check - are we confident D2 evolution will produce D4/D6-viable armies?

**Karpathy**: No, that's a real risk. Proposal: **depth annealing**. Start at D2, switch to D3 after gen 25, switch to D4 after gen 40.

**Olah**: I like that. The fitness landscape at D4 is similar enough to D2 that early gains transfer, but we refine in the later generations.

**Dean**: Budget impact: Last 10 generations at D4 adds... 30 * 10 * 40 = 12,000 games at D4 = 2,000 hours serial. Parallelized = 170 hours. That blows the budget.

**Hotz**: Cut population in later generations. At D4, use pop=10.

**Chollet**: Or cut games per eval. At D4, we don't need 40 games for signal. 10 might be enough.

**Karpathy**: Let's compromise:
- Gen 1-30: D2, pop=30, 40 games/eval
- Gen 31-40: D3, pop=20, 20 games/eval
- Gen 41-50: D4, pop=10, 10 games/eval

**Dean**: That's... *calculating*
- Phase 1: 30 * 30 * 40 * 2s = 72,000s = 20 hours
- Phase 2: 20 * 10 * 20 * 60s = 240,000s = 67 hours (serial)
- Phase 3: 10 * 10 * 10 * 600s = 600,000s = 167 hours (serial)

Even parallelized, phase 3 is 14 hours. Total = 20 + 6 + 14 = 40 hours. Plus validation. We're okay.

**Howard**: Ship it.

**Chollet**: Wait - we haven't discussed data structures. How does the evolution system know it's balancing against TWO opponents?

**Karpathy**: Good point. The current `EvolveArgs` has `fixed_white` and `fixed_black` for a single opponent. We need to extend this.

**Olah**: New field: `opponents: Vec<PathBuf>` - list of fixed opponent rulesets.

**Hotz**: And the fitness function iterates over opponents, takes min.

**Dean**: The `evaluate_fitness` function becomes:

```rust
fn evaluate_multi_opponent(candidate: &RuleSet, opponents: &[RuleSet], config: &EvalConfig) -> f32 {
    opponents.iter()
        .map(|opp| evaluate_vs_single(candidate, opp, config))
        .map(|result| result.balance_score())
        .fold(f32::INFINITY, f32::min)  // Min over all opponents
}
```

**Chollet**: Clean. And we can add skill_gradient as a multiplier:

```rust
fn fitness(candidate: &RuleSet, opponents: &[RuleSet], config: &EvalConfig) -> f32 {
    let min_balance = /* as above */;
    let skill_gradient = evaluate_skill_gradient(candidate, config);
    min_balance * skill_gradient
}
```

**Karpathy**: The skill gradient only needs to be computed once, not per-opponent. That saves compute.

**Howard**: Are we all aligned? I think we have a plan.

**Olah**: One more thing - how do we visualize progress? The user should be able to see if the three-team balance is improving.

**Chollet**: Emit a CSV every generation:
```
generation, best_fitness, balance_vs_orc, balance_vs_necro, skill_gradient, avg_length
```

**Hotz**: And a final report comparing all three pairings.

**Karpathy**: Alright, I think we're done with the design discussion. Let me write up the consensus.

---

### Phase 2: Structured Synthesis

#### CONSENSUS DECISIONS

| Decision | Rationale | Confidence |
|----------|-----------|------------|
| **Fix two opponents, evolve one** | Reduces problem from 3-way co-evolution to single-target evolution. Much simpler and faster. | High (95%) |
| **Use min-fitness over opponents** | Prevents specialization against one opponent. Team must be balanced against ALL opponents. | High (90%) |
| **Depth stratification (D2/D3/D4)** | D2 is 100x faster than D6. Early generations can use cheap evaluation; refine later. | Medium (80%) |
| **Start with existing Orc/Necro** | Human-designed armies are known-reasonable. Don't waste compute evolving from random. | High (95%) |
| **Run diagnostic first** | Validate Orc-Necro balance before evolving Fey. Don't build on broken foundation. | High (90%) |
| **Parallelize aggressively** | 12 threads available. Game evaluation is embarrassingly parallel. | High (95%) |

#### FORK-IN-ROAD CHOICES

| Choice | Option A | Option B | Our Pick | Rationale |
|--------|----------|----------|----------|-----------|
| **Co-evolution strategy** | Full 3-way simultaneous co-evolution | Cyclic: fix 2, evolve 1 | **Option B** | 3-way is unstable (Red Queen), harder to debug, no clear advantage |
| **Depth schedule** | Fixed D4 throughout | Anneal D2->D3->D4 | **Option B** | Annealing is 10x faster early, still converges to D4-viable armies |
| **Fitness aggregation** | Average over opponents | Min over opponents | **Min** | Average allows gaming (great vs one, terrible vs other) |
| **Population strategy** | Fixed pop throughout | Shrink pop at higher depths | **Shrink** | Compute-limited at D4; smaller pop with better eval is more efficient |
| **Seed design** | Random initialization | Human-designed Fey seeds | **Seeds** | Random wastes first 10-20 generations discovering basic viability |

#### LIVING TENSIONS

| Tension | Heuristic to Navigate |
|---------|----------------------|
| **Exploration vs exploitation** | Higher mutation rate early (0.2), lower late (0.05). Elitism preserves best but doesn't dominate. |
| **Depth fidelity vs speed** | Trust D2 for filtering, D4 for selection, D6 only for final validation. |
| **Specialization vs generality** | Min-fitness forces generality, but check for rock-paper-scissors drift every 10 gens. |
| **Fixed vs co-evolving opponents** | Start fixed. If Fey evolution hits plateau AND Orc-Necro drifted, do one refinement cycle. |

#### IMPLEMENTATION NOTES

**Data Structures:**

```rust
// Extended evolution config
pub struct MultiOpponentEvolveArgs {
    pub population: usize,
    pub generations: usize,
    pub opponents: Vec<PathBuf>,  // Multiple fixed opponents
    pub seeds: Option<PathBuf>,
    pub output: PathBuf,
    // Depth annealing
    pub depth_schedule: Vec<(usize, u32)>,  // (gen_threshold, depth)
    pub pop_schedule: Vec<(usize, usize)>,  // (gen_threshold, pop_size)
}

// Fitness result for multi-opponent
pub struct MultiOpponentFitness {
    pub overall: f32,           // min(balances) * skill_gradient
    pub per_opponent: Vec<(String, f32)>,  // (opponent_name, balance)
    pub skill_gradient: f32,
    pub worst_matchup: String,  // Name of worst-balanced opponent
}
```

**Algorithm:**

```python
# Pseudocode for main evolution loop
def evolve_balanced_team(seeds, opponents, config):
    population = load_seeds(seeds)

    for gen in range(config.generations):
        depth = get_depth_for_gen(gen, config.depth_schedule)
        pop_size = get_pop_for_gen(gen, config.pop_schedule)

        # Trim population if shrinking
        population = population[:pop_size]

        # Evaluate each candidate against ALL opponents
        fitness = []
        for candidate in population:
            balances = [evaluate_vs(candidate, opp, depth) for opp in opponents]
            min_balance = min(balances)
            skill = evaluate_skill_gradient(candidate, depth)
            fitness.append(min_balance * skill)

        # Selection, crossover, mutation
        population = evolve_generation(population, fitness, config)

        # Periodic validation
        if gen % 10 == 0:
            run_validation_tournament(population[0], opponents)

    return population[0]  # Best candidate
```

**Evaluation Function:**

```rust
fn evaluate_vs_single(candidate: &RuleSet, opponent: &RuleSet, depth: u32, games: usize) -> BalanceResult {
    let mut white_wins = 0;
    let mut black_wins = 0;
    let mut total_rounds = 0;

    // Half games with candidate as white, half as black
    for i in 0..games {
        let (white, black) = if i % 2 == 0 {
            (candidate, opponent)
        } else {
            (opponent, candidate)
        };

        let result = play_game(white, black, depth);
        match result.winner {
            Some(Player::White) => if i % 2 == 0 { white_wins += 1 } else { black_wins += 1 },
            Some(Player::Black) => if i % 2 == 0 { black_wins += 1 } else { white_wins += 1 },
            None => {},
        }
        total_rounds += result.rounds;
    }

    // Balance = 1.0 when 50/50, 0.0 when 100/0
    let win_rate = (white_wins + black_wins) > 0
        ? white_wins as f32 / (white_wins + black_wins) as f32
        : 0.5;
    let balance = 1.0 - 2.0 * (win_rate - 0.5).abs();

    BalanceResult { balance, avg_rounds: total_rounds as f32 / games as f32 }
}
```

---

## Proposed Architecture

### System Overview

```
┌─────────────────────────────────────────────────────────────────────┐
│                      Three-Team Balancer                             │
├─────────────────────────────────────────────────────────────────────┤
│  Phase 0: DIAGNOSTIC                                                 │
│  ├── Load Orc and Necro rulesets                                    │
│  ├── Run 100 games Orc vs Necro @ D4                                │
│  └── Check balance > 0.7 (proceed) or < 0.7 (fix first)            │
├─────────────────────────────────────────────────────────────────────┤
│  Phase 1: FEY EVOLUTION                                              │
│  ├── Load Fey seeds (5-10 variants)                                 │
│  ├── Opponents: [Orc, Necro] (fixed)                                │
│  ├── Fitness: min(balance_vs_orc, balance_vs_necro) * skill_grad    │
│  ├── Gen 1-30: D2, pop=30, 40 games/eval                           │
│  ├── Gen 31-40: D3, pop=20, 20 games/eval                          │
│  └── Gen 41-50: D4, pop=10, 10 games/eval                          │
├─────────────────────────────────────────────────────────────────────┤
│  Phase 2: VALIDATION                                                 │
│  ├── All pairings: Orc-Necro, Necro-Fey, Fey-Orc                   │
│  ├── 100 games each @ D4                                            │
│  └── Report balance scores and skill gradients                      │
├─────────────────────────────────────────────────────────────────────┤
│  Phase 3: REFINEMENT (Optional)                                      │
│  ├── If any pairing < 0.6 balance, evolve that team                │
│  └── Single pass: fix 2, evolve 1                                   │
└─────────────────────────────────────────────────────────────────────┘
```

### Module Changes

**New file: `hexwar-cli/src/balance_three.rs`**

Orchestrates the three-team balancing process. Implements:
- Multi-opponent evaluation
- Depth annealing schedule
- Validation tournaments
- Cyclic refinement logic

**Modified: `hexwar-evolve/src/lib.rs`**

Add support for multi-opponent fitness:
- New trait `MultiOpponentFitness`
- Callback for per-generation validation

**Modified: `hexwar-tournament/src/fitness.rs`**

Add `evaluate_multi_opponent()` function that computes min-balance.

### CLI Interface

```bash
# Basic three-team balance
hexwar balance-three \
    --opponent board_sets/the_orcs.json \
    --opponent board_sets/the_necromancer.json \
    --seeds board_sets/fey_seeds/ \
    --output balance_three_jan31

# With custom depth schedule
hexwar balance-three \
    --opponent board_sets/the_orcs.json \
    --opponent board_sets/the_necromancer.json \
    --seeds board_sets/fey_seeds/ \
    --depth-schedule "0:2,30:3,40:4" \
    --pop-schedule "0:30,30:20,40:10" \
    --generations 50 \
    --output balance_three_jan31

# Skip diagnostic (trust Orc-Necro is balanced)
hexwar balance-three \
    --opponent board_sets/the_orcs.json \
    --opponent board_sets/the_necromancer.json \
    --seeds board_sets/fey_seeds/ \
    --skip-diagnostic \
    --output balance_three_jan31
```

---

## Implementation Plan

### Step 1: Create Fey Seeds (Day 1, 2 hours)

Design 5-10 initial Fey army configurations. Themes to try:
- **Ranged Fey**: Heavy on B3/B4 rangers, defensive positioning
- **Mobile Fey**: C1/C3 lancers, king near center
- **Attrition Fey**: Phoenix + Ghosts, slow grind
- **Swarm Fey**: Many cheap pieces, Template A for tempo
- **Control Fey**: Warper + sliders, board control

### Step 2: Implement Multi-Opponent Fitness (Day 1, 4 hours)

New function in `hexwar-tournament/src/fitness.rs`:

```rust
pub fn evaluate_multi_opponent(
    candidate: &RuleSet,
    opponents: &[RuleSet],
    config: &EvalConfig,
) -> MultiOpponentFitness {
    // ...implementation
}
```

### Step 3: Implement Depth Annealing (Day 1, 2 hours)

Modify evolution loop to respect depth/pop schedules:

```rust
pub struct AnnealingSchedule {
    depth_at_gen: Vec<(usize, u32)>,
    pop_at_gen: Vec<(usize, usize)>,
}

impl AnnealingSchedule {
    pub fn depth_for(&self, gen: usize) -> u32;
    pub fn pop_for(&self, gen: usize) -> usize;
}
```

### Step 4: Implement `balance-three` Command (Day 2, 4 hours)

New CLI command that orchestrates:
1. Diagnostic check
2. Fey evolution with multi-opponent fitness
3. Final validation tournament
4. Optional refinement pass

### Step 5: Run Diagnostic (Day 2, 2 hours)

```bash
hexwar balance-three \
    --opponent board_sets/the_orcs.json \
    --opponent board_sets/the_necromancer.json \
    --diagnostic-only \
    --output diagnostic_jan31
```

Expected output:
```
Orc vs Necro: 100 games @ D4
  White wins: 48, Black wins: 47, Draws: 5
  Balance: 0.98
  Skill gradient: 0.87
  Avg rounds: 34.2

PASS: Orc-Necro balance is good (>0.7)
Proceeding to Fey evolution...
```

### Step 6: Run Full Evolution (Days 2-3, ~40 hours compute)

```bash
hexwar balance-three \
    --opponent board_sets/the_orcs.json \
    --opponent board_sets/the_necromancer.json \
    --seeds board_sets/fey_seeds/ \
    --generations 50 \
    --output balance_three_jan31 \
    2>&1 | tee balance_three.log &
```

### Step 7: Analyze Results (Day 3, 2 hours)

- Review fitness history
- Run extended validation tournament
- Check for rock-paper-scissors patterns
- Manual inspection of best Fey army

---

## Evaluation Strategy

### Success Criteria

| Metric | Target | Acceptable |
|--------|--------|------------|
| Balance (all pairings) | > 0.85 | > 0.70 |
| Skill gradient (all pairings) | > 0.80 | > 0.65 |
| No rock-paper-scissors | RPS < 0.2 | RPS < 0.3 |

**RPS metric**: Maximum cycle strength in win graph.
- RPS = max(|w(A,B) - w(B,C) - w(C,A)|) for all rotations
- Low RPS = balanced triangle, no dominant cycle

### Validation Protocol

After evolution completes:

1. **D4 Tournament** (300 games total):
   - Orc vs Necro: 100 games
   - Necro vs Fey: 100 games
   - Fey vs Orc: 100 games

2. **D6 Spot Check** (30 games total):
   - 10 games per pairing at D6
   - Verify D4 balance holds at higher depth

3. **Game Quality Review**:
   - Sample 5 games from each pairing
   - Manual review: are games interesting? Decisive? Strategic?

---

## Risk Mitigation

### Risk 1: Orc-Necro is Unbalanced

**Detection**: Diagnostic phase shows balance < 0.7

**Mitigation**: Run single-opponent evolution:
```bash
hexwar evolve --fixed-black board_sets/the_necromancer.json \
    --seeds board_sets/orc_seeds/ \
    --depth 4 --generations 30 \
    --output fix_orcs_jan31
```

### Risk 2: Fey Evolution Doesn't Converge

**Detection**: Fitness plateau before gen 40, best fitness < 0.6

**Mitigation**:
1. Increase population size
2. Add more diverse seeds
3. Lower mutation rate (exploration -> exploitation)
4. Check if problem is structural (Fey can't beat one opponent by design)

### Risk 3: Rock-Paper-Scissors Emerges

**Detection**: RPS metric > 0.3 in final validation

**Mitigation**:
1. Run refinement pass: fix Fey, evolve worst team
2. If persists, may need to redesign base armies
3. Consider structural constraints (all armies use same piece budget)

### Risk 4: D2 Evolution Doesn't Transfer to D4

**Detection**: Great D2 fitness, terrible D4 validation

**Mitigation**:
1. Start depth annealing earlier (gen 20 instead of 30)
2. Use longer annealing (D2->D4 directly since D3 is not commonly used)
3. Increase games at D4 phase

### Risk 5: Orc-Necro CANNOT Be Balanced

**Detection**: After 50+ generations of Orc evolution, balance remains below 0.5

**Root Cause Analysis**: The armies may be **structurally incompatible**:
- Orcs are designed as aggressive swarm (A3 Scouts, W1 Warper)
- Necros have resurrection mechanics (G1 Ghost, P1 Phoenix)
- In attrition warfare, resurrection wins - this is a DESIGN flaw, not solvable by evolution

**Mitigation Options**:

1. **Asymmetric Mutation Rates**: Instead of evolving just Orc, evolve BOTH Orc and Necro simultaneously with asymmetric rates:
   - Apply 3x mutation rate to the army that's currently winning (Necro)
   - This "nerfs" the dominant army while "buffing" the weak one
   - Converges toward balance from both sides

2. **Piece Budget Constraints**: Enforce that both armies have similar "piece value budgets". If Necro has 2 Phoenixes (high value), Orc should have equivalent value.

3. **Manual Redesign**: Accept that the original designs are flawed. Manually redesign one or both armies with balance in mind, then use evolution for fine-tuning.

4. **Redefine Success**: If 50/50 is impossible, target 45/55 as "acceptable". Some asymmetry may be inherent to the game design.

**Implementation of Asymmetric Mutation**:
```rust
fn get_mutation_rate(army: ArmyType, balance_scores: &BalanceScores) -> f32 {
    let base_rate = 0.10;

    // Find the matchup where this army is dominant
    let dominance = match army {
        Orc => balance_scores.orc_vs_necro - 0.5,  // Positive if Orc is winning
        Necro => 0.5 - balance_scores.orc_vs_necro, // Positive if Necro is winning
        Fey => // ... similar logic
    };

    // Higher mutation rate for dominant armies (to nerf them)
    // Lower mutation rate for weak armies (to preserve good genes)
    if dominance > 0.1 {
        base_rate * 2.0  // Winning too much - mutate more to find weaker variants
    } else if dominance < -0.1 {
        base_rate * 0.5  // Losing too much - preserve what works, mutate less
    } else {
        base_rate  // Balanced - normal mutation
    }
}
```

This approach is controversial (intentionally making armies worse) but may be necessary when structural imbalance exists.

---

## Resource Budget

### Compute Breakdown (Based on Actual D6 Measurements)

**Measured**: D6 evolution games take ~2.4 seconds each (from `fey_vs_necro_overnight`)

| Phase | Games | Time @ 2.4s/game | Time (12 threads) | Notes |
|-------|-------|------------------|-------------------|-------|
| Phase A: Fix Orc-Necro | 30 gen × 16 pop × 14 games = 6,720 | 4.5 hrs | **23 min** | D6, multi-depth |
| Phase B: Fey Evolution | 40 gen × 16 pop × 28 games = 17,920 | 12 hrs | **1 hr** | D6, 2 opponents |
| Validation Checkpoints | 3 × 100 games × 3 matchups = 900 | 36 min | **3 min** | Every 10 gens |
| Final Validation | 16 games × 7 matchup types × 3 pairs = 336 | 13 min | **1 min** | Full multi-depth |
| **Total** | ~26,000 | ~17 hrs | **~1.5 hrs** | Massive buffer! |

**Reality check**: The overnight evolution completed 17,920 D6 games in 12 hours. With 12-thread parallelism, we have enormous headroom.

**Revised Recommendation**: Run everything at D6!
- No need for depth annealing (D2→D4→D6)
- D6 is fast enough for full evolution
- Better strategic depth = better results

| Phase | Duration (parallel) | Notes |
|-------|---------------------|-------|
| Phase A: Fix Orc-Necro | ~30 min | D6, 30 generations |
| Phase B: Fey Evolution | ~1.5 hrs | D6, 40 generations, 2 opponents |
| Phase C: Validation | ~10 min | All pairings |
| **Total compute** | **~2.5 hours** | |
| **Buffer** | **~70 hours** | For refinement, re-runs, D8 experiments |

This is a dramatic improvement over the original estimates!

### Memory Requirements

- Peak: ~4GB (population in memory + game states)
- GPU: Not used for core evolution (alpha-beta is CPU)
- Disk: ~100MB for output (champions, logs, game records)

### Parallelism Strategy

The existing Rust evolution uses **Rayon** for parallelism. Here's how we parallelize:

| Level | What's Parallelized | Threads Used | Notes |
|-------|---------------------|--------------|-------|
| **Games within matchup** | Multiple games in one matchup | Up to 12 | Each game is independent |
| **Candidates within generation** | Evaluate multiple candidates | Up to 12 | Main source of speedup |
| **Matchups within evaluation** | Eval vs Orc and vs Necro | 2 | If evaluating against 2 opponents, can run in parallel |

**Current implementation** (in `hexwar-tournament/src/match_play.rs`):
```rust
if config.parallel {
    games.par_iter().map(|seed| play_single_game(...)).collect()
} else {
    games.iter().map(|seed| play_single_game(...)).collect()
}
```

**For multi-opponent evaluation**, we can parallelize at the opponent level:
```rust
opponents.par_iter().map(|opp| evaluate_vs_single(candidate, opp, config)).collect()
```

This gives us 2x speedup for two-opponent evaluation (Orc + Necro in parallel).

### GPU Utilization Analysis

**Current state**: GPU is NOT used. The game engine uses CPU alpha-beta search.

**Potential GPU uses** (not implemented, listed for future consideration):

1. **Batch position evaluation**: If we had a neural network evaluator, GPU could evaluate many positions in parallel. The `hexwar-nn` module exists but is not integrated into evolution.

2. **MCTS rollouts**: The `hexwar-mcts` module supports GPU-accelerated rollouts via `hexwar-gpu`. However, MCTS quality at equivalent compute is lower than alpha-beta.

3. **Parallel game simulation**: Could theoretically run many games on GPU if game logic were ported to CUDA. Significant engineering effort for marginal benefit.

**Recommendation**: For this 72-hour project, ignore GPU. The engineering cost of GPU integration exceeds the time budget. Revisit for future projects if D6+ becomes necessary.

---

## Appendix A: Existing Fey Seeds (Correct JSON Format)

**Note**: We already have 21 Fey seed variants in:
- `board_sets/fey_seeds/` (11 variants including still-hammer, fey-dancers, fey-knights, etc.)
- `board_sets/fey_warper_seeds/` (10 warper-focused variants)

The correct JSON format (matching `board_sets/fey_seeds/still-hammer.json`):

```json
{
  "name": "still-hammer",
  "ruleset": {
    "name": "still-hammer",
    "white_king": "K4",
    "white_pieces": ["B3", "E2", "E2", "D6", "D6", "A4", "A4", "A2", "B3", "C3", "C3"],
    "white_positions": [[-2, 4], [-4, 4], [-2, 2], [-1, 2], [-1, 4], [-2, 3], [0, 2], [0, 4], [-3, 4], [-1, 3], [-3, 3], [0, 3]],
    "white_facings": [0, 1, 1, 0, 0, 0, 5, 0, 0, 0, 0, 0],
    "white_template": "E",
    "black_king": "K1",
    "black_pieces": ["G1", "P1", "P1", "D1", "D1", "B3", "B3", "B3", "B3"],
    "black_positions": [[2, -4], [1, -2], [2, -2], [0, -2], [3, -3], [0, -3], [2, -3], [1, -3], [3, -4], [1, -4]],
    "black_facings": [3, 4, 3, 4, 2, 4, 3, 4, 2, 4],
    "black_template": "E"
  }
}
```

### Available Fey Seed Variants

| Seed Name | Theme | Key Pieces |
|-----------|-------|------------|
| `still-hammer` | Balanced, jumpers + sliders | E2, D6, C3 |
| `fey-dancers` | Mobile flankers | E1, E2, F1 |
| `fey-knights` | Heavy jumpers | E1 x4, D2, C2 |
| `fey-rangers` | Ranged steppers | B3, B4, D1 |
| `fey-bishops` | Diagonal sliders | D2, D3 |
| `fey-warper-for-*` | Warper + themed piece | W1 + various |
| `fey-warper-x2-*` | Double warper | W1 x2 + various |

**Recommendation**: Start evolution with all 21 seeds to maximize initial diversity.

---

## Revised Strategy: Accounting for Known Orc-Necro Imbalance

### Critical Update

The diagnostic phase revealed a significant problem: **Orc-Necro is already imbalanced** (~31-35% white win rate at higher depths). This means we cannot simply "fix two and evolve one."

### Revised Multi-Phase Approach (Using Full 72 Hours)

Given D6 is fast (~2.4s/game), we can run MANY evolution cycles. Use the time wisely:

**Phase 0: Heuristic Re-tuning (30 min)**
- The Zenith heuristic was built before some pieces existed (B5 Triton, D6 Triskelion, etc.)
- Quick re-tune of piece values for new pieces
- This is fast and ensures the AI properly values all pieces

**Phase 1: Diagnostic & Baseline (1 hour)**
- Evaluate current Orc-Necro, Fey-Necro, Fey-Orc at D6/D8
- Establish baseline fitness scores for all matchups
- Identify which matchups need the most work

**Phase 2: Fix Orc-Necro Foundation (4 hours)**
```bash
# Multiple evolution runs with different strategies
for strategy in aggressive defensive balanced; do
  hexwar evolve \
    --fixed-black board_sets/the_necromancer.json \
    --seeds board_sets/orc_seeds/ \
    --depth 6 --generations 50 --population 20 \
    --multi-depth --games 16 \
    --output orc_fix_${strategy}_$(date +%H%M)
done
```
- Run 3-4 parallel evolution attempts with different seeds
- Keep best champion from each
- Validate at D8 before proceeding

**Phase 3: Evolve Fey Against Both (8 hours)**
```bash
hexwar evolve \
    --opponent orc_fix_best/champion_1.json \
    --opponent board_sets/the_necromancer.json \
    --seeds board_sets/fey_warper_seeds/ \
    --depth 6 --generations 80 --population 20 \
    --multi-depth --games 16 \
    --constraint "must_have:W1" \
    --output fey_balanced_$(date +%H%M)
```
- Use warper seeds (Fey MUST have W1)
- Longer evolution (80 gens) for better convergence
- Evaluate against BOTH opponents, take min fitness

**Phase 4: Cyclic Refinement (12 hours)**
- If any matchup is < 0.70 fitness, evolve that army
- Run 2-3 refinement cycles:
  - Cycle 1: Adjust worst matchup
  - Cycle 2: Re-validate, adjust if needed
  - Cycle 3: Final polish

**Phase 5: Deep Validation at D8 (4 hours)**
- 100 games per matchup at D8
- Full multi-depth evaluation
- Human review of champion armies

**Phase 6: Theming & Polish (8 hours)**
- Adjust piece positioning for visual symmetry
- Ensure signature pieces are prominent
- Generate final army files with clean formatting

**Reserve Buffer (35 hours)**
- For unexpected issues, restarts, additional experiments
- Can run D10 validation if time permits
- Room for manual iteration

### Time Budget (72 Hours Total)

| Phase | Duration | Purpose |
|-------|----------|---------|
| Phase 0: Heuristic Re-tune | 0.5 hr | Update Zenith for new pieces |
| Phase 1: Diagnostic | 1 hr | Baseline measurements |
| Phase 2: Fix Orc-Necro | 4 hrs | Multiple parallel attempts |
| Phase 3: Fey Evolution | 8 hrs | Main evolution with constraints |
| Phase 4: Cyclic Refinement | 12 hrs | Iterative balance adjustment |
| Phase 5: D8 Validation | 4 hrs | Deep verification |
| Phase 6: Theming & Polish | 8 hrs | Human aesthetics |
| **Active Work** | **37.5 hrs** | |
| **Reserve Buffer** | **34.5 hrs** | For issues, experiments |

### Key Code Changes Required

1. **Multi-opponent fitness in `hexwar-cli/src/evolve.rs`**:
   - Add `--opponent` flag that can be specified multiple times
   - Modify `create_fitness_fn()` to evaluate against all opponents
   - Take minimum fitness across opponents

2. **New `balance-three` command**:
   - Orchestrate the three-phase pipeline
   - Track progress across phases
   - Support checkpoint/resume

3. **Validation tournament reporting**:
   - Generate human-readable reports for all three pairings
   - Include RPS (rock-paper-scissors) metric

---

## Autonomous Operation

This task runs autonomously with periodic heartbeat reports. The system self-manages CPU resources and spawns sub-agents as needed.

### Heartbeat Schedule (Every 10 Minutes)

Claude checks in every 10 minutes with the prompt:

> "How's it going? Is anything stalled out? Does anything need to be kicked off?
> Any results in that need analysis? Sanity check run times? What needs to be done,
> or is everything as it should be and I can just snooze?"

The heartbeat should:
1. Check running processes (`ps aux | grep evolve`)
2. Check if any evolution completed (new files in output dirs)
3. Analyze results if available
4. Kick off next phase if current one finished
5. Report issues or confirm all is well
6. Push status to display at `http://192.168.86.36:8888`

### CPU Queue Management

Only ONE evolution process should run at a time (CPU-bound, uses all 12 threads).

**Before starting any evolution:**
```bash
# Check for running evolutions
ps aux | grep -E "hexwar.*evolve" | grep -v grep

# If found, either:
# 1. Wait for completion
# 2. Kill if stale: kill <PID>
# 3. Queue the new task for later
```

**Process priority:**
1. Currently running evolution (let it finish)
2. Validation/diagnostic tasks (quick, can interrupt)
3. New evolution (start when CPU is free)

### Sub-Agent Spawning

The orchestrator agent spawns sub-agents for:

| Task | Agent Type | When to Spawn |
|------|------------|---------------|
| Run evolution | Bash background | When CPU is free |
| Analyze results | general-purpose | After evolution completes |
| Debug issues | general-purpose | When errors occur |
| Generate reports | general-purpose | Every 4 hours |
| Code changes | general-purpose | When constraints need implementation |

**Agent coordination:**
- Only ONE agent modifies files at a time
- Use file locks or sequential execution
- Log all agent actions to `/home/giblfiz/hexwar/autonomous_log.md`

### State Tracking

Maintain state in `/home/giblfiz/hexwar/balance_state.json`:
```json
{
  "phase": 2,
  "phase_name": "Fix Orc-Necro",
  "started_at": "2026-02-01T00:00:00Z",
  "last_heartbeat": "2026-02-01T04:00:00Z",
  "current_evolution": {
    "pid": 12345,
    "output_dir": "orc_fix_aggressive_0100",
    "generation": 35,
    "best_fitness": 0.72
  },
  "completed_phases": [1],
  "best_champions": {
    "orc": "orc_fix_aggressive_0100/champion_1.json",
    "fey": null,
    "necro": "board_sets/the_necromancer.json"
  },
  "issues": []
}
```

### Heartbeat Cron Setup

To set up the heartbeat (run once at start):
```bash
# Create heartbeat script
cat > /home/giblfiz/hexwar/heartbeat.sh << 'EOF'
#!/bin/bash
cd /home/giblfiz/hexwar
claude --print "How's it going? Is anything stalled out? Does anything need to be kicked off? Any results in that need analysis? Sanity check run times? What needs to be done, or is everything as it should be and I can just snooze?" 2>&1 | tee -a heartbeat.log
EOF
chmod +x /home/giblfiz/hexwar/heartbeat.sh

# Add to crontab (every 10 minutes)
(crontab -l 2>/dev/null; echo "*/10 * * * * /home/giblfiz/hexwar/heartbeat.sh") | crontab -
```

### Error Recovery

If an evolution fails or hangs:
1. Check the log file in the output directory
2. Kill the stuck process
3. Analyze what went wrong
4. Restart from last checkpoint or re-run phase
5. Log the issue to `autonomous_log.md`

### Manual Override

The user can intervene at any time by:
1. Sending a message to Claude
2. Modifying `balance_state.json` directly
3. Killing processes manually
4. Leaving a note in `/home/giblfiz/hexwar/USER_OVERRIDE.txt`

---

## Conclusion

This architecture provides a pragmatic path to three-team balance:

1. **Fix the known problem first**: Orc-Necro is imbalanced; address this before adding Fey.
2. **Run at D6**: Evolution games are fast (~2.4s/game). Use the depth for quality.
3. **Respect signature pieces**: Necro keeps Ghost/Phoenix, Fey keeps Warper.
4. **Human aesthetics matter**: Armies should feel themed and visually balanced.
5. **Cyclic refinement**: Multiple passes to converge on three-way balance.

The 72-hour budget is split: ~37 hours active work, ~35 hours buffer. This allows for multiple evolution attempts, deep validation at D8, and theming polish.

**Autonomous execution will:**
1. Report progress every 4 hours
2. Manage CPU queue (one evolution at a time)
3. Spawn sub-agents for analysis and code changes
4. Self-recover from errors when possible
5. Push final results to the display for human review

---

## Appendix: Updated Game Time Estimates

**ACTUAL MEASUREMENTS** from `fey_vs_necro_overnight` evolution (D6, 80 generations):
- 17,920 games completed in ~12 hours
- **D6 evolution: ~2.4 seconds per game**

| Depth | Evolution Games | Notes |
|-------|-----------------|-------|
| D2 | ~0.5 seconds | Very fast, good for filtering |
| D4 | ~1-2 seconds | Solid middle ground |
| D5 | ~1.5-3 seconds | Recommended for production |
| D6 | ~2-5 seconds | **Measured: 2.4s average** |
| D8 | ~10-30 seconds | Slower but still practical |

**Key insight**: Evolution games end on king captures, typically in 10-25 moves. "General" games running to 50-round proximity rule would be much slower, but evolution doesn't encounter those.

**Recommendation**: Use D6 for evolution. It's only ~2.4 seconds per game and provides much better strategic depth than D4.

---

## Appendix B: Alternative Architectures Considered

### Option 1: Full Three-Way Simultaneous Co-Evolution

**Description**: All three armies evolve simultaneously, evaluating against the current generation of the other two.

**Why Rejected**:
- Red Queen dynamics: fitness landscape shifts every generation
- 3x compute cost per generation
- Debugging/analysis is very difficult
- No clear stopping criterion

### Option 2: Neural Network-Based Evaluation

**Description**: Train a neural network to predict game outcomes, use it for fast fitness estimation.

**Why Rejected**:
- Requires significant training data (millions of games)
- NN quality may not match alpha-beta
- Engineering cost exceeds 72-hour budget
- Would need validation against alpha-beta anyway

### Option 3: Human-in-the-Loop Design

**Description**: Use evolution to generate candidates, have human expert select promising ones.

**Why Rejected**:
- Doesn't scale to 72-hour continuous runs
- Human availability is the bottleneck
- Introduces subjective bias

### Option 4: Pareto-Front Multi-Objective Evolution (NSGA-II)

**Description**: Use proper multi-objective optimization with Pareto dominance.

**Why Rejected**:
- More complex implementation
- Min-fitness achieves similar goals more simply
- Pareto fronts are hard to visualize/interpret for three objectives

### Option 5: Start Fresh - Evolve All Three From Random

**Description**: Ignore existing armies, evolve all three from random initialization.

**Why Rejected**:
- Wastes first 20+ generations finding basic viability
- Existing seeds are valuable starting points
- Human design provides thematic coherence

---

## Appendix C: Glossary

| Term | Definition |
|------|------------|
| **Balance Score** | 1.0 - 2 * abs(win_rate - 0.5). Ranges from 0 (complete domination) to 1 (perfect 50/50) |
| **Skill Gradient** | Rate at which deeper-searching AI beats shallower AI. Should be high (>80%) |
| **Color Fairness** | Win rate balance between white and black at equal depth |
| **RPS** | Rock-Paper-Scissors. Cyclic dominance pattern: A beats B, B beats C, C beats A |
| **Min-Fitness** | Fitness = min(balance vs all opponents). Forces generalist strategies |
| **Depth Annealing** | Starting evolution at low depth (fast) and increasing over time |
| **Multi-Depth Evaluation** | Testing at multiple depths (D2, D4, D5, D6) to verify balance holds |
| **Champion** | Best-performing army configuration in a population |
| **Elitism** | Preserving top N configurations unchanged between generations |

---

*Document version: 2.0*
*Last updated: 2026-01-31*
*Author: Claude Opus 4.5, synthesizing Council of ML Architects discussion*
