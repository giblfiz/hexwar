//! Configuration types for tournament play
//!
//! Level 4 - Utilities and configuration

use hexwar_core::Heuristics;

/// Player type for games
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerType {
    /// Alpha-Beta pruning search
    AlphaBeta,
    /// Monte Carlo Tree Search (CPU-only mode)
    MCTS,
}

impl Default for PlayerType {
    fn default() -> Self {
        PlayerType::AlphaBeta
    }
}

/// AI configuration for game playing
#[derive(Clone, Debug)]
pub struct AiConfig {
    /// Player type (MCTS or AlphaBeta)
    pub player_type: PlayerType,
    /// Search depth for alpha-beta
    pub depth: u32,
    /// Number of simulations for MCTS
    pub simulations: u32,
    /// Optional time limit in milliseconds
    pub time_limit_ms: Option<u64>,
    /// Heuristics for evaluation
    pub heuristics: Heuristics,
    /// Maximum moves per action for move ordering
    pub max_moves_per_action: usize,
    /// Random seed for reproducibility (None = random)
    pub seed: Option<u64>,
}

impl Default for AiConfig {
    fn default() -> Self {
        Self {
            player_type: PlayerType::AlphaBeta,
            depth: 4,
            simulations: 1000,
            time_limit_ms: None,
            heuristics: Heuristics::default(),
            max_moves_per_action: 15,
            seed: None,
        }
    }
}

impl AiConfig {
    /// Create config for alpha-beta at given depth
    pub fn alpha_beta(depth: u32) -> Self {
        Self {
            player_type: PlayerType::AlphaBeta,
            depth,
            ..Default::default()
        }
    }

    /// Create config for MCTS with given simulations
    pub fn mcts(simulations: u32) -> Self {
        Self {
            player_type: PlayerType::MCTS,
            simulations,
            ..Default::default()
        }
    }

    /// Set custom heuristics
    pub fn with_heuristics(mut self, heuristics: Heuristics) -> Self {
        self.heuristics = heuristics;
        self
    }

    /// Set random seed
    pub fn with_seed(mut self, seed: u64) -> Self {
        self.seed = Some(seed);
        self
    }
}

/// Tournament format
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TournamentFormat {
    /// Everyone plays everyone
    RoundRobin,
    /// Pair by score, limited rounds
    Swiss { rounds: usize },
}

impl Default for TournamentFormat {
    fn default() -> Self {
        TournamentFormat::RoundRobin
    }
}

/// Tournament configuration
#[derive(Clone, Debug)]
pub struct TournamentConfig {
    /// Tournament format
    pub format: TournamentFormat,
    /// Number of games per match (should be even for color alternation)
    pub games_per_match: usize,
    /// AI configuration for all players
    pub ai_config: AiConfig,
    /// Whether to run games in parallel
    pub parallel: bool,
    /// Maximum rounds per game
    pub max_rounds: u32,
}

impl Default for TournamentConfig {
    fn default() -> Self {
        Self {
            format: TournamentFormat::RoundRobin,
            games_per_match: 10,
            ai_config: AiConfig::default(),
            parallel: true,
            max_rounds: 50,
        }
    }
}

impl TournamentConfig {
    /// Create round-robin tournament config
    pub fn round_robin(games_per_match: usize) -> Self {
        Self {
            format: TournamentFormat::RoundRobin,
            games_per_match,
            ..Default::default()
        }
    }

    /// Create Swiss tournament config
    pub fn swiss(rounds: usize, games_per_match: usize) -> Self {
        Self {
            format: TournamentFormat::Swiss { rounds },
            games_per_match,
            ..Default::default()
        }
    }
}

/// Configuration for fitness evaluation
#[derive(Clone, Debug)]
pub struct EvalConfig {
    /// AI configuration
    pub ai_config: AiConfig,
    /// Games per opponent
    pub games_per_opponent: usize,
    /// Whether to run games in parallel
    pub parallel: bool,
    /// Maximum rounds per game
    pub max_rounds: u32,
}

impl Default for EvalConfig {
    fn default() -> Self {
        Self {
            ai_config: AiConfig::default(),
            games_per_opponent: 10,
            parallel: true,
            max_rounds: 50,
        }
    }
}

impl EvalConfig {
    /// Create config with specified games per opponent
    pub fn new(games_per_opponent: usize) -> Self {
        Self {
            games_per_opponent,
            ..Default::default()
        }
    }

    /// Set AI configuration
    pub fn with_ai(mut self, ai_config: AiConfig) -> Self {
        self.ai_config = ai_config;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ai_config_defaults() {
        let config = AiConfig::default();
        assert_eq!(config.player_type, PlayerType::AlphaBeta);
        assert_eq!(config.depth, 4);
        assert_eq!(config.simulations, 1000);
    }

    #[test]
    fn test_ai_config_alpha_beta() {
        let config = AiConfig::alpha_beta(6);
        assert_eq!(config.player_type, PlayerType::AlphaBeta);
        assert_eq!(config.depth, 6);
    }

    #[test]
    fn test_ai_config_mcts() {
        let config = AiConfig::mcts(500);
        assert_eq!(config.player_type, PlayerType::MCTS);
        assert_eq!(config.simulations, 500);
    }

    #[test]
    fn test_tournament_config_defaults() {
        let config = TournamentConfig::default();
        assert_eq!(config.format, TournamentFormat::RoundRobin);
        assert_eq!(config.games_per_match, 10);
        assert!(config.parallel);
    }

    #[test]
    fn test_tournament_config_swiss() {
        let config = TournamentConfig::swiss(5, 4);
        assert_eq!(config.format, TournamentFormat::Swiss { rounds: 5 });
        assert_eq!(config.games_per_match, 4);
    }

    #[test]
    fn test_eval_config_defaults() {
        let config = EvalConfig::default();
        assert_eq!(config.games_per_opponent, 10);
        assert!(config.parallel);
    }
}
