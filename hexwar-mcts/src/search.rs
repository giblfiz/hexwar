//! MCTS Search Loop
//!
//! Implements the core MCTS algorithm:
//! 1. Selection - Use UCB1 to traverse tree
//! 2. Expansion - Add child node
//! 3. Simulation - Rollout to terminal state
//! 4. Backpropagation - Update statistics
//!
//! ## Architecture
//! - Level 2: Search loop coordination
//! - Level 3: Individual MCTS phases
//! - Level 4: Utilities

use crate::tree::{MctsTree, NodeId};
use crate::rollout::RolloutEngine;
use crate::MctsConfig;
use hexwar_core::{GameState, Move, GameResult};
use hexwar_gpu::GpuContext;

// ============================================================================
// SEARCH RESULT
// ============================================================================

/// Result of MCTS search
#[derive(Debug)]
pub struct SearchResult {
    /// The final tree after search
    pub tree: MctsTree,
    /// Total simulations performed
    pub total_simulations: u32,
    /// Statistics for each root move
    pub move_stats: Vec<MoveStatistics>,
}

/// Statistics for a single move at root
#[derive(Clone, Debug)]
pub struct MoveStatistics {
    pub mv: Move,
    pub visits: u32,
    pub win_rate: f32,
    pub ucb1: f32,
}

impl SearchResult {
    /// Get the best move (most visited)
    pub fn best_move(&self) -> Option<Move> {
        self.tree.best_move()
    }

    /// Get move with highest win rate
    pub fn highest_winrate_move(&self) -> Option<Move> {
        self.move_stats
            .iter()
            .max_by(|a, b| a.win_rate.partial_cmp(&b.win_rate).unwrap())
            .map(|s| s.mv)
    }

    /// Get all moves sorted by visits
    pub fn moves_by_visits(&self) -> Vec<(Move, u32)> {
        let mut moves: Vec<_> = self.move_stats
            .iter()
            .map(|s| (s.mv, s.visits))
            .collect();
        moves.sort_by(|a, b| b.1.cmp(&a.1));
        moves
    }
}

// ============================================================================
// SEARCH LOOP (Level 2 - Main Coordination)
// ============================================================================

/// Run MCTS search
///
/// Level 2 function - coordinates the search phases.
pub fn run_search(
    mut tree: MctsTree,
    config: &MctsConfig,
    gpu: Option<&GpuContext>,
) -> SearchResult {
    let rollout_engine = RolloutEngine::new(
        if config.use_gpu { gpu } else { None },
        config.max_rollout_depth,
        rand::random(), // Random seed for each search
    );

    // Run search iterations
    if config.batch_size > 1 && config.use_gpu && gpu.is_some() {
        run_batched_search(&mut tree, config, &rollout_engine);
    } else {
        run_sequential_search(&mut tree, config, &rollout_engine);
    }

    // Collect statistics
    let total_simulations = tree.total_simulations();
    let move_stats = collect_move_statistics(&tree, config.exploration);

    SearchResult {
        tree,
        total_simulations,
        move_stats,
    }
}

// ============================================================================
// SEQUENTIAL SEARCH (Level 3 - CPU Search)
// ============================================================================

/// Sequential MCTS (one simulation at a time)
fn run_sequential_search(
    tree: &mut MctsTree,
    config: &MctsConfig,
    rollout_engine: &RolloutEngine,
) {
    for _ in 0..config.simulations {
        run_single_iteration(tree, config.exploration, rollout_engine);
    }
}

/// Single MCTS iteration
///
/// Level 3 function - implements one complete MCTS cycle.
fn run_single_iteration(
    tree: &mut MctsTree,
    exploration: f32,
    rollout_engine: &RolloutEngine,
) {
    // Phase 1: Selection
    let path = tree.select_leaf(exploration);
    let leaf_id = *path.last().unwrap();

    // Phase 2: Expansion (if not terminal)
    let simulation_node = if !tree.get(leaf_id).is_terminal() && !tree.get(leaf_id).is_fully_expanded() {
        tree.expand(leaf_id).unwrap_or(leaf_id)
    } else {
        leaf_id
    };

    // Phase 3: Simulation (rollout)
    let result = simulate_node(tree, simulation_node, rollout_engine);

    // Phase 4: Backpropagation
    tree.backpropagate(simulation_node, result);
}

/// Simulate a node (rollout or use cached result)
fn simulate_node(
    tree: &MctsTree,
    node_id: NodeId,
    rollout_engine: &RolloutEngine,
) -> GameResult {
    let node = tree.get(node_id);

    // If terminal, use cached result
    if let Some(result) = node.cached_result {
        return result;
    }

    // Otherwise, run rollout
    let results = rollout_engine.rollout_batch(&[node.state.clone()]);
    results.first().map(|r| r.result).unwrap_or(GameResult::Ongoing)
}

// ============================================================================
// BATCHED SEARCH (Level 3 - GPU Search)
// ============================================================================

/// Batched MCTS (multiple rollouts in parallel on GPU)
fn run_batched_search(
    tree: &mut MctsTree,
    config: &MctsConfig,
    rollout_engine: &RolloutEngine,
) {
    let mut simulations_done = 0;

    while simulations_done < config.simulations {
        let batch_size = std::cmp::min(
            config.batch_size,
            config.simulations - simulations_done,
        );

        run_batch_iteration(tree, config.exploration, rollout_engine, batch_size);
        simulations_done += batch_size;
    }
}

/// Single batch iteration
///
/// Level 3 function - runs multiple simulations in parallel.
fn run_batch_iteration(
    tree: &mut MctsTree,
    exploration: f32,
    rollout_engine: &RolloutEngine,
    batch_size: usize,
) {
    // Phase 1: Selection (collect multiple leaves with virtual losses)
    let leaves = select_batch_leaves(tree, exploration, batch_size);

    // Phase 2: Expansion
    let states_for_rollout = expand_leaves(tree, &leaves);

    // If no states to simulate, we're done
    if states_for_rollout.is_empty() {
        // Remove virtual losses and backprop terminal results
        backprop_terminal_leaves(tree, &leaves);
        return;
    }

    // Phase 3: Batch simulation
    let states: Vec<GameState> = states_for_rollout.iter().map(|(_, s)| s.clone()).collect();
    let results = rollout_engine.rollout_batch(&states);

    // Phase 4: Backpropagation
    for ((node_id, _), rollout_result) in states_for_rollout.iter().zip(results.iter()) {
        tree.backpropagate(*node_id, rollout_result.result);
    }
}

/// Select multiple leaves for batch processing
fn select_batch_leaves(
    tree: &mut MctsTree,
    exploration: f32,
    batch_size: usize,
) -> Vec<NodeId> {
    let mut leaves = Vec::with_capacity(batch_size);

    for _ in 0..batch_size {
        let path = tree.select_leaf(exploration);
        let leaf = *path.last().unwrap();

        // Add virtual loss to prevent re-selection
        tree.add_virtual_loss(leaf);
        leaves.push(leaf);
    }

    leaves
}

/// Expand leaves and collect states for rollout
fn expand_leaves(tree: &mut MctsTree, leaves: &[NodeId]) -> Vec<(NodeId, GameState)> {
    let mut states = Vec::new();

    for &leaf_id in leaves {
        // Remove virtual loss (will be re-added during backprop)
        tree.remove_virtual_loss(leaf_id);

        let node = tree.get(leaf_id);

        if node.is_terminal() {
            // Terminal node - result known, backprop immediately
            continue;
        }

        if !node.is_fully_expanded() {
            // Expand and add new child for rollout
            if let Some(child_id) = tree.expand(leaf_id) {
                let state = tree.get(child_id).state.clone();
                states.push((child_id, state));
            }
        } else {
            // Fully expanded - use this node's state
            let state = tree.get(leaf_id).state.clone();
            states.push((leaf_id, state));
        }
    }

    states
}

/// Handle terminal leaves (backpropagate their known results)
fn backprop_terminal_leaves(tree: &mut MctsTree, leaves: &[NodeId]) {
    for &leaf_id in leaves {
        tree.remove_virtual_loss(leaf_id);

        if let Some(result) = tree.get(leaf_id).cached_result {
            tree.backpropagate(leaf_id, result);
        }
    }
}

// ============================================================================
// STATISTICS COLLECTION (Level 4 - Utilities)
// ============================================================================

/// Collect statistics for root moves
fn collect_move_statistics(tree: &MctsTree, exploration: f32) -> Vec<MoveStatistics> {
    let root = tree.get(tree.root());
    let parent_visits = root.stats.adjusted_visits();

    root.children
        .iter()
        .map(|(mv, child_id)| {
            let child = tree.get(*child_id);
            let visits = child.stats.visits;
            let win_rate = child.stats.win_rate();

            // Calculate UCB1 for information (not used for selection anymore)
            let ucb1 = if visits == 0 {
                f32::INFINITY
            } else {
                win_rate + exploration * ((parent_visits as f32).ln() / visits as f32).sqrt()
            };

            MoveStatistics {
                mv: *mv,
                visits,
                win_rate,
                ucb1,
            }
        })
        .collect()
}

// ============================================================================
// PROGRESSIVE WIDENING (Future Enhancement)
// ============================================================================

/// Progressive widening limits the number of children based on visits
///
/// This is useful for games with high branching factor.
/// n_children <= C * visits^alpha (typically alpha = 0.5)
#[allow(dead_code)]
fn should_expand_with_widening(visits: u32, children: usize, alpha: f32, c: f32) -> bool {
    let max_children = c * (visits as f32).powf(alpha);
    (children as f32) < max_children
}

// ============================================================================
// RAVE / AMAF (Future Enhancement)
// ============================================================================

/// RAVE (Rapid Action Value Estimation) uses all-moves-as-first
/// to estimate move values with fewer simulations.
///
/// This is a placeholder for future enhancement.
/// Note: To use Move as a HashMap key, it would need to implement Hash.
/// For now, we use a Vec-based approach which is simpler but slower.
#[derive(Clone, Debug, Default)]
pub struct RaveStats {
    /// (Move, visits, wins) tuples - using Vec since Move doesn't impl Hash
    pub entries: Vec<(Move, u32, f32)>,
}

impl RaveStats {
    /// Get RAVE value for a move
    #[allow(dead_code)]
    pub fn rave_value(&self, mv: &Move) -> Option<f32> {
        self.entries
            .iter()
            .find(|(m, _, _)| m == mv)
            .map(|(_, visits, wins)| {
                if *visits == 0 {
                    0.5
                } else {
                    wins / *visits as f32
                }
            })
    }

    /// Update RAVE stats for a move
    #[allow(dead_code)]
    pub fn update(&mut self, mv: Move, win: f32) {
        if let Some(entry) = self.entries.iter_mut().find(|(m, _, _)| *m == mv) {
            entry.1 += 1;
            entry.2 += win;
        } else {
            self.entries.push((mv, 1, win));
        }
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use hexwar_core::board::Hex;
    use hexwar_core::game::Template;
    use hexwar_core::pieces::piece_id_to_index;

    // Helper to create a minimal test state
    fn mock_state() -> GameState {
        let white = vec![
            (piece_id_to_index("K1").unwrap(), Hex::new(0, 3), 0),
        ];
        let black = vec![
            (piece_id_to_index("K1").unwrap(), Hex::new(0, -3), 3),
        ];
        GameState::new(&white, &black, Template::E, Template::E)
    }

    #[test]
    fn test_search_result_best_move() {
        let tree = MctsTree::new(mock_state());
        let result = SearchResult {
            tree,
            total_simulations: 0,
            move_stats: vec![
                MoveStatistics {
                    mv: Move::Pass,
                    visits: 100,
                    win_rate: 0.6,
                    ucb1: 1.0,
                },
                MoveStatistics {
                    mv: Move::Surrender,
                    visits: 50,
                    win_rate: 0.4,
                    ucb1: 0.8,
                },
            ],
        };

        // highest_winrate_move should return Pass (0.6 > 0.4)
        let best = result.highest_winrate_move();
        assert_eq!(best, Some(Move::Pass));
    }

    #[test]
    fn test_moves_by_visits() {
        let tree = MctsTree::new(mock_state());
        let result = SearchResult {
            tree,
            total_simulations: 150,
            move_stats: vec![
                MoveStatistics {
                    mv: Move::Pass,
                    visits: 100,
                    win_rate: 0.6,
                    ucb1: 1.0,
                },
                MoveStatistics {
                    mv: Move::Surrender,
                    visits: 50,
                    win_rate: 0.4,
                    ucb1: 0.8,
                },
            ],
        };

        let sorted = result.moves_by_visits();
        assert_eq!(sorted[0].1, 100); // Pass has 100 visits
        assert_eq!(sorted[1].1, 50);  // Surrender has 50 visits
    }

    #[test]
    fn test_should_expand_with_widening() {
        // With alpha=0.5, c=1.0, visits=100 -> max_children = 10
        assert!(should_expand_with_widening(100, 5, 0.5, 1.0));
        assert!(!should_expand_with_widening(100, 15, 0.5, 1.0));
    }

    #[test]
    fn test_collect_move_statistics_empty() {
        let tree = MctsTree::new(mock_state());
        let stats = collect_move_statistics(&tree, 1.41);
        assert!(stats.is_empty()); // No children expanded
    }

    #[test]
    fn test_run_search_basic() {
        let tree = MctsTree::new(mock_state());
        let config = crate::MctsConfig::cpu_only(10); // Just 10 simulations

        let result = run_search(tree, &config, None);

        assert!(result.total_simulations > 0);
        // Should have explored at least some moves
    }
}
