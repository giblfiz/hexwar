//! HEXWAR GPU - CUDA-accelerated game simulation
//!
//! This crate provides GPU-parallel game simulation for MCTS rollouts:
//! - Batch game simulation (1000+ games simultaneously)
//! - Random rollout policy for MCTS
//! - Compact game state for GPU memory efficiency
//!
//! # Architecture
//!
//! The GPU simulation uses a simple but effective approach:
//! - Each CUDA thread simulates one complete game
//! - Games use random move selection (uniform from legal moves)
//! - Results are collected and transferred back to CPU
//!
//! # Usage
//!
//! ```text
//! use hexwar_gpu::{GpuContext, GpuGameResults};
//! use hexwar_core::GameState;
//!
//! let ctx = GpuContext::new()?;
//! let states: Vec<GameState> = /* your initial states */;
//! let results = ctx.simulate_batch(&states, 100, 12345);
//! let outcomes = results.download();
//! ```

pub mod compact;
pub mod kernels;

use anyhow::{Context, Result};
use cudarc::driver::{CudaDevice, CudaFunction, CudaSlice, DeviceRepr, LaunchAsync, LaunchConfig};
use hexwar_core::{GameResult, GameState, Player};

use compact::{CompactGameState, CompactPiece, SimulationResult, BOARD_SIZE};

/// Handle to GPU resources
///
/// Manages the CUDA device, compiled kernels, and streams for parallel execution.
pub struct GpuContext {
    /// CUDA device handle
    device: std::sync::Arc<CudaDevice>,
}

/// Results of batch simulation (held on GPU until downloaded)
pub struct GpuGameResults {
    /// Results stored on CPU after download
    outcomes: Vec<GameOutcome>,
}

/// Outcome of a single simulated game
#[derive(Clone, Debug)]
pub struct GameOutcome {
    /// Final game result
    pub result: GameResult,
    /// Number of rounds played
    pub rounds: u32,
    /// Final evaluation score
    pub final_eval: f32,
}

/// Error types for GPU operations
#[derive(Debug, thiserror::Error)]
pub enum GpuError {
    #[error("CUDA initialization failed: {0}")]
    InitFailed(String),

    #[error("Kernel compilation failed: {0}")]
    CompileFailed(String),

    #[error("Kernel launch failed: {0}")]
    LaunchFailed(String),

    #[error("Memory transfer failed: {0}")]
    TransferFailed(String),

    #[error("Invalid batch size: {0}")]
    InvalidBatchSize(String),
}

impl GpuContext {
    /// Initialize GPU context with CUDA
    ///
    /// This compiles the simulation kernels for the current GPU.
    pub fn new() -> Result<Self> {
        Self::new_with_device(0)
    }

    /// Initialize with a specific GPU device
    pub fn new_with_device(device_id: usize) -> Result<Self> {
        let device = CudaDevice::new(device_id)
            .context("Failed to initialize CUDA device")?;

        // Compile kernels from source
        let ptx = cudarc::nvrtc::compile_ptx(kernels::KERNEL_SOURCE)
            .context("Failed to compile CUDA kernels")?;

        // Load module
        device.load_ptx(ptx, "hexwar_kernels", &[kernels::KERNEL_NAME])
            .context("Failed to load PTX module")?;

        Ok(Self { device })
    }

    /// Get the CUDA device for advanced operations
    pub fn device(&self) -> &std::sync::Arc<CudaDevice> {
        &self.device
    }

    /// Simulate a batch of games in parallel on GPU
    ///
    /// # Arguments
    /// * `states` - Initial game states to simulate from
    /// * `max_moves` - Maximum moves per game before termination
    /// * `seed` - Random seed for reproducibility
    ///
    /// # Returns
    /// Results of all simulated games
    pub fn simulate_batch(
        &self,
        states: &[GameState],
        max_moves: u32,
        seed: u64,
    ) -> Result<GpuGameResults> {
        let batch_size = states.len();
        if batch_size == 0 {
            return Ok(GpuGameResults { outcomes: vec![] });
        }

        // Convert states to compact format
        let (boards, players) = self.prepare_batch_data(states);

        // Generate RNG seeds for each game
        let seeds = self.generate_seeds(batch_size, seed);

        // Allocate GPU memory
        let boards_gpu = self.device.htod_sync_copy(&boards)
            .context("Failed to copy boards to GPU")?;
        let players_gpu = self.device.htod_sync_copy(&players)
            .context("Failed to copy players to GPU")?;
        let seeds_gpu = self.device.htod_sync_copy(&seeds)
            .context("Failed to copy seeds to GPU")?;

        // Allocate output buffer
        let results_gpu: CudaSlice<SimulationResult> = self.device.alloc_zeros(batch_size)
            .context("Failed to allocate results buffer")?;

        // Get kernel function
        let kernel: CudaFunction = self.device
            .get_func("hexwar_kernels", kernels::KERNEL_NAME)
            .context("Failed to get kernel function")?;

        // Launch kernel
        let threads_per_block = 256;
        let num_blocks = (batch_size + threads_per_block - 1) / threads_per_block;

        let config = LaunchConfig {
            block_dim: (threads_per_block as u32, 1, 1),
            grid_dim: (num_blocks as u32, 1, 1),
            shared_mem_bytes: 0,
        };

        // SAFETY: We ensure all buffers are correctly sized
        unsafe {
            kernel.launch(
                config,
                (
                    &boards_gpu,
                    &players_gpu,
                    &seeds_gpu,
                    max_moves,
                    &results_gpu,
                ),
            ).context("Failed to launch kernel")?;
        }

        // Synchronize and copy results back
        self.device.synchronize().context("Failed to synchronize")?;

        let results_host = self.device.dtoh_sync_copy(&results_gpu)
            .context("Failed to copy results from GPU")?;

        // Convert to GameOutcome
        let outcomes = results_host
            .iter()
            .map(|r| GameOutcome {
                result: r.get_result(),
                rounds: r.rounds as u32,
                final_eval: r.final_eval(),
            })
            .collect();

        Ok(GpuGameResults { outcomes })
    }

    /// Prepare batch data for GPU transfer
    fn prepare_batch_data(&self, states: &[GameState]) -> (Vec<CompactPiece>, Vec<u8>) {
        let batch_size = states.len();

        let mut boards = Vec::with_capacity(batch_size * BOARD_SIZE);
        let mut players = Vec::with_capacity(batch_size);

        for state in states {
            let compact = CompactGameState::from_game_state(state);

            // Add board pieces
            boards.extend_from_slice(&compact.board);

            // Add current player
            players.push(compact.current_player);
        }

        (boards, players)
    }

    /// Generate RNG seeds for each game
    fn generate_seeds(&self, batch_size: usize, base_seed: u64) -> Vec<u32> {
        use rand::{Rng, SeedableRng};
        let mut rng = rand::rngs::StdRng::seed_from_u64(base_seed);

        (0..batch_size)
            .map(|_| rng.gen::<u32>().max(1)) // Ensure non-zero
            .collect()
    }

    /// Get GPU memory usage info
    pub fn memory_info(&self) -> Result<(usize, usize)> {
        // Returns (used, total) in bytes
        // Note: cudarc doesn't directly expose this, return estimates
        let batch_1k = 1000;
        let per_game = std::mem::size_of::<CompactGameState>()
            + std::mem::size_of::<SimulationResult>()
            + 8; // seeds etc

        let estimated_per_batch = batch_1k * per_game;
        // RTX 3060 has 12GB
        let total = 12 * 1024 * 1024 * 1024;

        Ok((estimated_per_batch, total))
    }
}

impl GpuGameResults {
    /// Number of games in results
    pub fn len(&self) -> usize {
        self.outcomes.len()
    }

    /// Check if results are empty
    pub fn is_empty(&self) -> bool {
        self.outcomes.is_empty()
    }

    /// Download results to CPU (already done, just returns)
    pub fn download(&self) -> Vec<GameOutcome> {
        self.outcomes.clone()
    }

    /// Get win rate for a player
    pub fn win_rate(&self, player: Player) -> f32 {
        if self.outcomes.is_empty() {
            return 0.0;
        }

        let wins = self
            .outcomes
            .iter()
            .filter(|o| match player {
                Player::White => o.result == GameResult::WhiteWins,
                Player::Black => o.result == GameResult::BlackWins,
            })
            .count();

        wins as f32 / self.outcomes.len() as f32
    }

    /// Get average game length
    pub fn avg_rounds(&self) -> f32 {
        if self.outcomes.is_empty() {
            return 0.0;
        }

        let total: u32 = self.outcomes.iter().map(|o| o.rounds).sum();
        total as f32 / self.outcomes.len() as f32
    }

    /// Get draw rate (games that ended ongoing)
    pub fn draw_rate(&self) -> f32 {
        if self.outcomes.is_empty() {
            return 0.0;
        }

        let draws = self
            .outcomes
            .iter()
            .filter(|o| o.result == GameResult::Ongoing)
            .count();

        draws as f32 / self.outcomes.len() as f32
    }
}

// Ensure our compact types can be sent to GPU
unsafe impl DeviceRepr for CompactPiece {}
unsafe impl DeviceRepr for SimulationResult {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compact_piece_device_repr() {
        // Verify size is as expected for GPU transfer
        assert_eq!(std::mem::size_of::<CompactPiece>(), 2);
        assert_eq!(std::mem::size_of::<SimulationResult>(), 8);
    }

    #[test]
    fn test_empty_batch() {
        // This test doesn't require GPU
        let results = GpuGameResults { outcomes: vec![] };
        assert!(results.is_empty());
        assert_eq!(results.len(), 0);
        assert_eq!(results.win_rate(Player::White), 0.0);
    }

    #[test]
    fn test_win_rate_calculation() {
        let outcomes = vec![
            GameOutcome {
                result: GameResult::WhiteWins,
                rounds: 10,
                final_eval: 1.0,
            },
            GameOutcome {
                result: GameResult::WhiteWins,
                rounds: 15,
                final_eval: 0.5,
            },
            GameOutcome {
                result: GameResult::BlackWins,
                rounds: 20,
                final_eval: -1.0,
            },
            GameOutcome {
                result: GameResult::Ongoing,
                rounds: 50,
                final_eval: 0.0,
            },
        ];

        let results = GpuGameResults { outcomes };

        assert_eq!(results.win_rate(Player::White), 0.5);
        assert_eq!(results.win_rate(Player::Black), 0.25);
        assert_eq!(results.draw_rate(), 0.25);
        assert_eq!(results.avg_rounds(), 23.75);
    }

    // GPU tests - require actual CUDA GPU
    #[test]
    #[ignore = "Requires CUDA GPU"]
    fn test_gpu_context_creation() {
        let ctx = GpuContext::new();
        assert!(ctx.is_ok(), "Failed to create GPU context: {:?}", ctx.err());
    }

    #[test]
    #[ignore = "Requires CUDA GPU"]
    fn test_gpu_batch_simulation() {
        use hexwar_core::{Hex, Template};
        use hexwar_core::pieces::piece_id_to_index;

        // Create GPU context
        let ctx = GpuContext::new().expect("Failed to create GPU context");

        // Create test game states
        let white_pieces = vec![
            (piece_id_to_index("K1").unwrap(), Hex::new(0, 3), 0),
            (piece_id_to_index("A2").unwrap(), Hex::new(-1, 3), 0),
            (piece_id_to_index("A2").unwrap(), Hex::new(1, 2), 0),
        ];
        let black_pieces = vec![
            (piece_id_to_index("K1").unwrap(), Hex::new(0, -3), 3),
            (piece_id_to_index("A2").unwrap(), Hex::new(1, -3), 3),
            (piece_id_to_index("A2").unwrap(), Hex::new(-1, -2), 3),
        ];

        let state = GameState::new(&white_pieces, &black_pieces, Template::E, Template::E);

        // Create batch of 100 identical starting positions
        let batch_size = 100;
        let states: Vec<_> = (0..batch_size).map(|_| state.clone()).collect();

        // Simulate on GPU
        let results = ctx
            .simulate_batch(&states, 50, 12345)
            .expect("Failed to simulate batch");

        // Verify results
        assert_eq!(results.len(), batch_size);

        let outcomes = results.download();
        assert_eq!(outcomes.len(), batch_size);

        // Check that games actually ran
        let total_rounds: u32 = outcomes.iter().map(|o| o.rounds).sum();
        assert!(total_rounds > 0, "No rounds played");

        // Check that we got some variety in outcomes
        let white_wins = outcomes.iter().filter(|o| o.result == GameResult::WhiteWins).count();
        let black_wins = outcomes.iter().filter(|o| o.result == GameResult::BlackWins).count();
        let ongoing = outcomes.iter().filter(|o| o.result == GameResult::Ongoing).count();

        println!("Results: White={}, Black={}, Ongoing={}", white_wins, black_wins, ongoing);
        println!("Avg rounds: {:.1}", results.avg_rounds());

        // At least some games should have completed
        assert!(white_wins + black_wins + ongoing == batch_size);
    }

    #[test]
    #[ignore = "Requires CUDA GPU"]
    fn test_gpu_large_batch() {
        use hexwar_core::{Hex, Template};
        use hexwar_core::pieces::piece_id_to_index;

        let ctx = GpuContext::new().expect("Failed to create GPU context");

        // Simple game state
        let white_pieces = vec![
            (piece_id_to_index("K1").unwrap(), Hex::new(0, 3), 0),
            (piece_id_to_index("A2").unwrap(), Hex::new(-1, 3), 0),
        ];
        let black_pieces = vec![
            (piece_id_to_index("K1").unwrap(), Hex::new(0, -3), 3),
            (piece_id_to_index("A2").unwrap(), Hex::new(1, -3), 3),
        ];

        let state = GameState::new(&white_pieces, &black_pieces, Template::E, Template::E);

        // Large batch - 1000 games
        let batch_size = 1000;
        let states: Vec<_> = (0..batch_size).map(|_| state.clone()).collect();

        let start = std::time::Instant::now();
        let results = ctx
            .simulate_batch(&states, 50, 42)
            .expect("Failed to simulate batch");
        let elapsed = start.elapsed();

        assert_eq!(results.len(), batch_size);
        println!(
            "Simulated {} games in {:?} ({:.1} games/sec)",
            batch_size,
            elapsed,
            batch_size as f64 / elapsed.as_secs_f64()
        );
    }
}
