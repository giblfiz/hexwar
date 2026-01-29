# HEXWAR v2 Architecture - Rust + GPU Rewrite

## Overview

Complete rewrite from Python to Rust with GPU acceleration via CUDA.

**Goal:** Parallel evolution with GPU-accelerated MCTS game playing.

---

## Module Boundaries

```
┌─────────────────────────────────────────────────────────────────┐
│                         hexwar-cli                               │
│                    (Chunk 6: CLI Binary)                         │
└─────────────────────────────────────────────────────────────────┘
                               │
          ┌────────────────────┼────────────────────┐
          ▼                    ▼                    ▼
┌──────────────────┐  ┌──────────────────┐  ┌──────────────────┐
│  hexwar-evolve   │  │ hexwar-tournament│  │  hexwar-server   │
│  (Chunk 3: GA)   │  │  (Chunk 4)       │  │  (Chunk 7: HTTP) │
└──────────────────┘  └──────────────────┘  └──────────────────┘
          │                    │                    │
          └────────────────────┼────────────────────┘
                               ▼
┌─────────────────────────────────────────────────────────────────┐
│                       hexwar-mcts                                │
│                    (Chunk 5: MCTS Player)                        │
└─────────────────────────────────────────────────────────────────┘
                               │
          ┌────────────────────┴────────────────────┐
          ▼                                         ▼
┌──────────────────┐                      ┌──────────────────┐
│   hexwar-core    │                      │   hexwar-gpu     │
│ (Chunk 1: Engine)│                      │ (Chunk 2: CUDA)  │
└──────────────────┘                      └──────────────────┘
```

---

## Chunk 1: hexwar-core (Rust Game Engine)

**Owner:** Agent 1
**Status:** Existing code in `hexwar_core/src/lib.rs` - needs refactoring

### Public API

```rust
// ============================================================================
// TYPES
// ============================================================================

/// Axial hex coordinates
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Hex { pub q: i8, pub r: i8 }

/// Piece on the board
#[derive(Clone, Copy, Debug)]
pub struct Piece {
    pub piece_type: PieceTypeId,  // u8 index into PIECE_TYPES
    pub owner: Player,            // White or Black
    pub facing: u8,               // 0-5
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Player { White = 0, Black = 1 }

/// Game state (immutable, clone to mutate)
#[derive(Clone, Debug)]
pub struct GameState {
    // Internal fields...
}

/// A legal move
#[derive(Clone, Copy, Debug)]
pub enum Move {
    Pass,
    Surrender,
    Movement { from: Hex, to: Hex, new_facing: u8 },
    Rotate { pos: Hex, new_facing: u8 },
    Swap { from: Hex, target: Hex },
    Rebirth { dest: Hex, new_facing: u8 },
}

/// Game outcome
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameResult {
    Ongoing,
    WhiteWins,
    BlackWins,
}

/// Heuristic weights for evaluation
#[derive(Clone, Debug)]
pub struct Heuristics {
    pub piece_values: [f32; 30],      // Per piece type
    pub center_weight: f32,
    pub mobility_weight: f32,         // NEW: weight for attackable squares
}

// ============================================================================
// CORE FUNCTIONS
// ============================================================================

impl GameState {
    /// Create initial state from piece placements
    pub fn new(
        white_pieces: &[(PieceTypeId, Hex, u8)],  // (type, pos, facing)
        black_pieces: &[(PieceTypeId, Hex, u8)],
        white_template: Template,
        black_template: Template,
    ) -> Self;

    /// Get current player
    pub fn current_player(&self) -> Player;

    /// Get game result
    pub fn result(&self) -> GameResult;

    /// Generate all legal moves for current action
    pub fn legal_moves(&self) -> Vec<Move>;

    /// Apply move, return new state (immutable)
    pub fn apply_move(&self, mv: Move) -> Self;

    /// Count legal moves for a player (mobility heuristic)
    pub fn mobility(&self, player: Player) -> usize;

    /// Get pieces on board
    pub fn pieces(&self) -> impl Iterator<Item = (Hex, &Piece)>;

    /// Evaluate position from current player's perspective
    pub fn evaluate(&self, heuristics: &Heuristics) -> f32;
}

// ============================================================================
// AI (CPU Alpha-Beta)
// ============================================================================

pub struct AlphaBetaAI {
    pub depth: u32,
    pub max_moves_per_action: usize,
    pub heuristics: Heuristics,
}

impl AlphaBetaAI {
    pub fn best_move(&self, state: &GameState) -> Option<Move>;
    pub fn play_game(&self, initial: GameState, max_rounds: u32) -> (GameState, Vec<Move>);
}

// ============================================================================
// SERIALIZATION
// ============================================================================

/// RuleSet defines an army composition (for evolution)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RuleSet {
    pub name: String,
    pub white_king: PieceTypeId,
    pub white_pieces: Vec<PieceTypeId>,
    pub white_positions: Vec<Hex>,
    pub white_facings: Vec<u8>,
    pub white_template: Template,
    pub black_king: PieceTypeId,
    pub black_pieces: Vec<PieceTypeId>,
    pub black_positions: Vec<Hex>,
    pub black_facings: Vec<u8>,
    pub black_template: Template,
}

impl RuleSet {
    pub fn to_game_state(&self) -> GameState;
    pub fn load(path: &Path) -> Result<Self>;
    pub fn save(&self, path: &Path) -> Result<()>;
}
```

### Mobility Heuristic (NEW)

The evaluation function should include:
```rust
score = piece_value_sum
      + center_weight * king_centrality
      + mobility_weight * (my_legal_moves - opponent_legal_moves)
```

Where `mobility` = count of legal moves available.

---

## Chunk 2: hexwar-gpu (CUDA Game Simulation)

**Owner:** Agent 2
**Dependencies:** Chunk 1 types (can work from spec)

### Purpose

Simulate many games in parallel on GPU. Used by MCTS for rollouts.

### Public API

```rust
// ============================================================================
// GPU BATCH GAME SIMULATION
// ============================================================================

/// Handle to GPU resources
pub struct GpuContext {
    // CUDA context, streams, etc.
}

impl GpuContext {
    pub fn new() -> Result<Self>;

    /// Simulate N games from given states to completion (or max_moves)
    /// Returns results on GPU, call download() to get to CPU
    pub fn simulate_batch(
        &self,
        states: &[GameState],      // N initial states
        max_moves: u32,
        seed: u64,
    ) -> GpuGameResults;
}

/// Results of batch simulation (on GPU memory)
pub struct GpuGameResults {
    // Internal GPU buffers
}

impl GpuGameResults {
    pub fn len(&self) -> usize;

    /// Download results to CPU
    pub fn download(&self) -> Vec<GameOutcome>;
}

#[derive(Clone, Debug)]
pub struct GameOutcome {
    pub result: GameResult,
    pub rounds: u32,
    pub final_eval: f32,  // Heuristic score at end
}
```

### Implementation Notes

- Use random playout policy (pick random legal move)
- Game state must fit in GPU memory - use compact representation
- Target: 1000+ simultaneous games on RTX 3060

---

## Chunk 3: hexwar-evolve (Genetic Algorithm)

**Owner:** Agent 3
**Dependencies:** Chunk 1 RuleSet type

### Public API

```rust
/// Evolution configuration
pub struct EvolutionConfig {
    pub population_size: usize,
    pub generations: usize,
    pub mutation_rate: f32,
    pub crossover_rate: f32,
    pub elitism: usize,           // Top N preserved unchanged
    pub tournament_size: usize,   // For selection
}

/// Evolve a population of rulesets
pub fn evolve<F>(
    initial_population: Vec<RuleSet>,
    config: &EvolutionConfig,
    fitness_fn: F,
    rng: &mut impl Rng,
) -> Vec<RuleSet>
where
    F: Fn(&RuleSet) -> f32;  // Fitness function

/// Mutation operators
pub fn mutate_ruleset(rs: &RuleSet, rng: &mut impl Rng) -> RuleSet;

/// Crossover operators
pub fn crossover_rulesets(a: &RuleSet, b: &RuleSet, rng: &mut impl Rng) -> RuleSet;

/// Selection
pub fn tournament_select<'a>(
    population: &'a [RuleSet],
    fitness: &[f32],
    tournament_size: usize,
    rng: &mut impl Rng,
) -> &'a RuleSet;
```

---

## Chunk 4: hexwar-tournament (Fitness Evaluation)

**Owner:** Agent 4
**Dependencies:** Chunks 1, 3, 5

### Public API

```rust
/// Run games between rulesets to compute fitness
pub struct Tournament {
    pub games_per_matchup: usize,
    pub depth: u32,              // For CPU player
    pub use_gpu: bool,           // Use GPU MCTS or CPU alpha-beta
    pub workers: usize,          // CPU parallelism
}

impl Tournament {
    /// Evaluate fitness of a ruleset against a fixed opponent
    pub fn evaluate_vs_fixed(
        &self,
        candidate: &RuleSet,
        opponent: &RuleSet,
    ) -> FitnessResult;

    /// Round-robin tournament
    pub fn round_robin(&self, population: &[RuleSet]) -> Vec<FitnessResult>;
}

#[derive(Clone, Debug)]
pub struct FitnessResult {
    pub wins: u32,
    pub losses: u32,
    pub draws: u32,
    pub avg_rounds: f32,
    pub fitness_score: f32,  // Computed from above
}
```

---

## Chunk 5: hexwar-mcts (GPU-Accelerated MCTS)

**Owner:** Agent 5
**Dependencies:** Chunks 1, 2

### Public API

```rust
/// MCTS configuration
pub struct MctsConfig {
    pub simulations: usize,       // Total simulations per move
    pub batch_size: usize,        // GPU batch size
    pub exploration: f32,         // UCB exploration constant (C)
    pub max_rollout_depth: u32,
}

/// MCTS player using GPU for rollouts
pub struct MctsPlayer {
    config: MctsConfig,
    gpu: GpuContext,
}

impl MctsPlayer {
    pub fn new(config: MctsConfig, gpu: GpuContext) -> Self;

    /// Get best move using MCTS
    pub fn best_move(&self, state: &GameState) -> Option<Move>;

    /// Play a full game
    pub fn play_game(
        &self,
        initial: GameState,
        max_rounds: u32,
    ) -> (GameState, Vec<Move>);
}
```

### Implementation Notes

- Tree policy: UCB1 for selection
- Expansion: Add one child at a time
- Simulation: GPU batch random rollouts
- Backpropagation: Update win counts

---

## Chunk 6: hexwar-cli (Binary)

**Owner:** Agent 6
**Dependencies:** All above

### Commands

```bash
# Run evolution
hexwar evolve --population 50 --generations 100 --depth 4 --output results/

# Play a single game (for testing)
hexwar play --white board.json --black board.json --depth 6

# Start visualizer server
hexwar serve --port 8002

# Benchmark GPU vs CPU
hexwar benchmark --games 100 --depth 4
```

---

## Chunk 7: hexwar-server (HTTP API for Visualizer)

**Owner:** Agent 7
**Dependencies:** Chunk 1

### Endpoints

```
GET  /api/status              - Server health
POST /api/game/new            - Create game from ruleset
POST /api/game/move           - Apply move
GET  /api/game/{id}           - Get game state
POST /api/ai/move             - Get AI move suggestion
POST /api/designer/load       - Load ruleset for designer
POST /api/playback/load       - Load game record for playback
```

Keep existing HTML/JS files, just provide Rust backend.

---

## Directory Structure

```
hexwar/
├── Cargo.toml                 # Workspace
├── hexwar-core/               # Chunk 1
│   ├── Cargo.toml
│   └── src/lib.rs
├── hexwar-gpu/                # Chunk 2
│   ├── Cargo.toml
│   ├── src/lib.rs
│   └── src/kernels.cu         # CUDA kernels
├── hexwar-evolve/             # Chunk 3
│   ├── Cargo.toml
│   └── src/lib.rs
├── hexwar-tournament/         # Chunk 4
│   ├── Cargo.toml
│   └── src/lib.rs
├── hexwar-mcts/               # Chunk 5
│   ├── Cargo.toml
│   └── src/lib.rs
├── hexwar-server/             # Chunk 7
│   ├── Cargo.toml
│   └── src/lib.rs
├── hexwar-cli/                # Chunk 6
│   ├── Cargo.toml
│   └── src/main.rs
├── visualizer/                # Existing HTML/JS (moved from hexwar/visualizer/)
│   ├── index.html
│   ├── designer.html
│   └── player.html
└── tests/                     # Integration tests
    └── integration.rs
```

---

## Integration Points

### Core ↔ GPU
- `GameState` must have a compact serializable form for GPU transfer
- Define `GameStateCompact` struct that maps to GPU memory layout

### MCTS ↔ GPU
- MCTS calls `gpu.simulate_batch()` with leaf states
- Returns `Vec<GameOutcome>` for backpropagation

### Tournament ↔ MCTS
- Tournament creates `MctsPlayer` for GPU games
- Falls back to `AlphaBetaAI` for CPU-only mode

### CLI ↔ All
- CLI is just argument parsing + calling library functions
- No game logic in CLI itself

---

## Testing Strategy

Each chunk must have:
1. **Unit tests** in the crate
2. **Integration test** that exercises the public API

Integration tests run after merge:
- `test_core_game_rules` - Verify game logic
- `test_gpu_simulation` - GPU produces valid results
- `test_mcts_vs_alphabeta` - MCTS plays reasonably
- `test_evolution_improves` - Fitness increases over generations
- `test_server_api` - HTTP endpoints work

---

## Communication Protocol

Agents report to managing agent with:
1. **Progress updates** - What's done, what's blocking
2. **Interface changes** - If API needs to change, notify others
3. **Test results** - Unit tests passing?

Managing agent responsibilities:
1. **Propagate interface changes** to affected agents
2. **Unblock dependencies** - Help resolve issues
3. **Merge and test** - Integrate work, run integration tests
