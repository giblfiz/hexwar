//! MCTS Tree structure and node management
//!
//! Uses arena allocation for efficient tree operations.
//!
//! ## Architecture
//! - Level 2: Tree operations (expand, select_child)
//! - Level 3: UCB1 calculation, node accessors
//! - Level 4: Statistics, utilities

use hexwar_core::{GameState, Move, GameResult, Player};

// ============================================================================
// TYPES
// ============================================================================

/// Node identifier (index into arena)
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);

impl NodeId {
    pub const ROOT: NodeId = NodeId(0);
}

/// Statistics for a tree node
#[derive(Clone, Debug, Default)]
pub struct NodeStats {
    /// Number of times this node was visited
    pub visits: u32,
    /// Total wins (from perspective of player who moved TO this node)
    pub wins: f32,
    /// Virtual loss counter (for parallel MCTS)
    pub virtual_losses: u32,
}

impl NodeStats {
    /// Win rate from this node's perspective
    pub fn win_rate(&self) -> f32 {
        if self.visits == 0 {
            0.5 // Prior for unexplored nodes
        } else {
            self.wins / self.visits as f32
        }
    }

    /// Adjusted visits including virtual losses
    pub fn adjusted_visits(&self) -> u32 {
        self.visits + self.virtual_losses
    }
}

/// A node in the MCTS tree
#[derive(Clone, Debug)]
pub struct MctsNode {
    /// Game state at this node
    pub state: GameState,
    /// Parent node (None for root)
    pub parent: Option<NodeId>,
    /// Move that led to this node (None for root)
    pub incoming_move: Option<Move>,
    /// Children: (move, node_id) pairs
    pub children: Vec<(Move, NodeId)>,
    /// Moves not yet expanded
    pub untried_moves: Vec<Move>,
    /// Visit/win statistics
    pub stats: NodeStats,
    /// Cached game result (if terminal)
    pub cached_result: Option<GameResult>,
}

impl MctsNode {
    /// Create a new node
    pub fn new(state: GameState, parent: Option<NodeId>, incoming_move: Option<Move>) -> Self {
        let result = state.result();
        let cached_result = if result != GameResult::Ongoing {
            Some(result)
        } else {
            None
        };

        // Generate untried moves only for non-terminal nodes
        let untried_moves = if cached_result.is_none() {
            state.legal_moves()
        } else {
            Vec::new()
        };

        Self {
            state,
            parent,
            incoming_move,
            children: Vec::new(),
            untried_moves,
            stats: NodeStats::default(),
            cached_result,
        }
    }

    /// Is this a terminal node?
    pub fn is_terminal(&self) -> bool {
        self.cached_result.is_some()
    }

    /// Is this node fully expanded?
    pub fn is_fully_expanded(&self) -> bool {
        self.untried_moves.is_empty()
    }

    /// Has this node been visited?
    pub fn is_visited(&self) -> bool {
        self.stats.visits > 0
    }
}

// ============================================================================
// MCTS TREE (Level 2 - Tree Operations)
// ============================================================================

/// MCTS search tree with arena allocation
#[derive(Debug)]
pub struct MctsTree {
    /// Arena storage for nodes
    nodes: Vec<MctsNode>,
}

impl MctsTree {
    /// Create a new tree with the given root state
    pub fn new(root_state: GameState) -> Self {
        let root = MctsNode::new(root_state, None, None);
        Self {
            nodes: vec![root],
        }
    }

    /// Get the root node id
    pub fn root(&self) -> NodeId {
        NodeId::ROOT
    }

    /// Get a reference to a node
    pub fn get(&self, id: NodeId) -> &MctsNode {
        &self.nodes[id.0]
    }

    /// Get a mutable reference to a node
    pub fn get_mut(&mut self, id: NodeId) -> &mut MctsNode {
        &mut self.nodes[id.0]
    }

    /// Get the number of nodes in the tree
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Is the tree empty?
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    // ========================================================================
    // Level 2: Tree Operations
    // ========================================================================

    /// Select a leaf node using tree policy (UCB1)
    ///
    /// Returns the path from root to the selected leaf.
    pub fn select_leaf(&self, exploration: f32) -> Vec<NodeId> {
        let mut path = vec![self.root()];
        let mut current = self.root();

        while self.get(current).is_fully_expanded() && !self.get(current).is_terminal() {
            if let Some(best_child) = self.select_best_child(current, exploration) {
                path.push(best_child);
                current = best_child;
            } else {
                break;
            }
        }

        path
    }

    /// Expand a node by adding one child
    ///
    /// Returns the new child's NodeId, or None if node is fully expanded.
    pub fn expand(&mut self, node_id: NodeId) -> Option<NodeId> {
        let mv = self.get_mut(node_id).untried_moves.pop()?;
        let parent_state = &self.get(node_id).state;
        let child_state = parent_state.apply_move(mv);

        let child_id = NodeId(self.nodes.len());
        let child = MctsNode::new(child_state, Some(node_id), Some(mv));
        self.nodes.push(child);

        self.get_mut(node_id).children.push((mv, child_id));

        Some(child_id)
    }

    /// Expand multiple nodes (for batch GPU rollouts)
    ///
    /// Returns vector of (node_id, state) pairs for rollout.
    pub fn expand_batch(&mut self, node_ids: &[NodeId]) -> Vec<(NodeId, GameState)> {
        let mut leaves = Vec::new();

        for &node_id in node_ids {
            if let Some(child_id) = self.expand(node_id) {
                let state = self.get(child_id).state.clone();
                leaves.push((child_id, state));
            } else if self.get(node_id).is_terminal() {
                // Terminal node - use its state directly
                let state = self.get(node_id).state.clone();
                leaves.push((node_id, state));
            }
        }

        leaves
    }

    // ========================================================================
    // Level 3: Selection Helpers
    // ========================================================================

    /// Select best child using UCB1
    fn select_best_child(&self, node_id: NodeId, exploration: f32) -> Option<NodeId> {
        let node = self.get(node_id);
        if node.children.is_empty() {
            return None;
        }

        let parent_visits = node.stats.adjusted_visits();

        node.children
            .iter()
            .max_by(|(_, a), (_, b)| {
                let ucb_a = self.ucb1(*a, parent_visits, exploration);
                let ucb_b = self.ucb1(*b, parent_visits, exploration);
                ucb_a.partial_cmp(&ucb_b).unwrap_or(std::cmp::Ordering::Equal)
            })
            .map(|(_, id)| *id)
    }

    /// Calculate UCB1 value for a node
    ///
    /// UCB1 = wins/visits + C * sqrt(ln(parent_visits) / visits)
    fn ucb1(&self, node_id: NodeId, parent_visits: u32, exploration: f32) -> f32 {
        let node = self.get(node_id);
        let visits = node.stats.adjusted_visits();

        if visits == 0 {
            return f32::INFINITY; // Prioritize unexplored nodes
        }

        let exploitation = node.stats.win_rate();
        let exploration_term = exploration * ((parent_visits as f32).ln() / visits as f32).sqrt();

        exploitation + exploration_term
    }

    // ========================================================================
    // Level 2: Backpropagation
    // ========================================================================

    /// Backpropagate a result from leaf to root
    ///
    /// `result` should be the game result.
    /// We propagate wins from the perspective of who won.
    pub fn backpropagate(&mut self, leaf_id: NodeId, result: GameResult) {
        let mut current = Some(leaf_id);

        while let Some(node_id) = current {
            let node = self.get_mut(node_id);
            node.stats.visits += 1;

            // Clear any virtual losses
            if node.stats.virtual_losses > 0 {
                node.stats.virtual_losses -= 1;
            }

            // Add win value based on result
            // The node stores the state AFTER a move, so we check whose turn it is
            // and award the point if the PREVIOUS player (who made the move) won.
            let reward = match result {
                GameResult::Ongoing => 0.5, // Draw/incomplete - half point
                GameResult::WhiteWins => {
                    // If it's black's turn now, white just moved and won
                    if node.state.current_player() == Player::Black {
                        1.0
                    } else {
                        0.0
                    }
                }
                GameResult::BlackWins => {
                    if node.state.current_player() == Player::White {
                        1.0
                    } else {
                        0.0
                    }
                }
            };

            node.stats.wins += reward;
            current = node.parent;
        }
    }

    /// Add virtual loss for parallel MCTS (prevents selecting same node)
    pub fn add_virtual_loss(&mut self, node_id: NodeId) {
        let mut current = Some(node_id);
        while let Some(id) = current {
            self.get_mut(id).stats.virtual_losses += 1;
            current = self.get(id).parent;
        }
    }

    /// Remove virtual loss (used if we need to undo selection)
    pub fn remove_virtual_loss(&mut self, node_id: NodeId) {
        let mut current = Some(node_id);
        while let Some(id) = current {
            let node = self.get_mut(id);
            if node.stats.virtual_losses > 0 {
                node.stats.virtual_losses -= 1;
            }
            current = node.parent;
        }
    }

    // ========================================================================
    // Level 3: Best Move Selection
    // ========================================================================

    /// Get the best move from root (most visits)
    pub fn best_move(&self) -> Option<Move> {
        let root = self.get(self.root());

        root.children
            .iter()
            .max_by_key(|(_, id)| self.get(*id).stats.visits)
            .map(|(mv, _)| *mv)
    }

    /// Get all moves with their visit counts (for analysis)
    pub fn move_statistics(&self) -> Vec<(Move, u32, f32)> {
        let root = self.get(self.root());

        root.children
            .iter()
            .map(|(mv, id)| {
                let node = self.get(*id);
                (*mv, node.stats.visits, node.stats.win_rate())
            })
            .collect()
    }

    /// Get total simulations run (root visits)
    pub fn total_simulations(&self) -> u32 {
        self.get(self.root()).stats.visits
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
        // Create a simple game with just two kings
        let white = vec![
            (piece_id_to_index("K1").unwrap(), Hex::new(0, 3), 0),
        ];
        let black = vec![
            (piece_id_to_index("K1").unwrap(), Hex::new(0, -3), 3),
        ];
        GameState::new(&white, &black, Template::E, Template::E)
    }

    #[test]
    fn test_node_creation() {
        let state = mock_state();
        let node = MctsNode::new(state, None, None);

        assert!(node.parent.is_none());
        assert!(node.incoming_move.is_none());
        assert!(node.children.is_empty());
        assert_eq!(node.stats.visits, 0);
        assert_eq!(node.stats.wins, 0.0);
    }

    #[test]
    fn test_tree_creation() {
        let state = mock_state();
        let tree = MctsTree::new(state);

        assert_eq!(tree.len(), 1);
        assert_eq!(tree.root(), NodeId::ROOT);
    }

    #[test]
    fn test_node_stats_win_rate() {
        let mut stats = NodeStats::default();
        assert_eq!(stats.win_rate(), 0.5); // Prior for unvisited

        stats.visits = 10;
        stats.wins = 7.0;
        assert!((stats.win_rate() - 0.7).abs() < 0.001);
    }

    #[test]
    fn test_ucb1_unexplored() {
        let state = mock_state();
        let tree = MctsTree::new(state);

        // UCB1 for unexplored node should be infinity
        let ucb = tree.ucb1(NodeId::ROOT, 100, 1.41);
        assert!(ucb.is_infinite());
    }

    #[test]
    fn test_virtual_loss() {
        let state = mock_state();
        let mut tree = MctsTree::new(state);

        assert_eq!(tree.get(NodeId::ROOT).stats.virtual_losses, 0);

        tree.add_virtual_loss(NodeId::ROOT);
        assert_eq!(tree.get(NodeId::ROOT).stats.virtual_losses, 1);

        tree.remove_virtual_loss(NodeId::ROOT);
        assert_eq!(tree.get(NodeId::ROOT).stats.virtual_losses, 0);
    }

    #[test]
    fn test_tree_expansion() {
        let state = mock_state();
        let mut tree = MctsTree::new(state);

        // Root should have untried moves
        assert!(!tree.get(NodeId::ROOT).untried_moves.is_empty());

        // Expand root
        let child_id = tree.expand(NodeId::ROOT);
        assert!(child_id.is_some());

        let child_id = child_id.unwrap();
        assert_eq!(tree.len(), 2);
        assert_eq!(tree.get(child_id).parent, Some(NodeId::ROOT));
    }

    #[test]
    fn test_backpropagation() {
        let state = mock_state();
        let mut tree = MctsTree::new(state);

        // Backpropagate a white win
        tree.backpropagate(NodeId::ROOT, GameResult::WhiteWins);

        let root = tree.get(NodeId::ROOT);
        assert_eq!(root.stats.visits, 1);
        // Root state has white to play, so after white wins,
        // the reward depends on perspective
    }
}
