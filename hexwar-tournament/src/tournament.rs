//! Tournament execution - round-robin and Swiss formats
//!
//! Level 1 - Orchestration and Level 2 - Phases

use hexwar_core::RuleSet;
use rayon::prelude::*;

use crate::config::{TournamentConfig, TournamentFormat};
use crate::match_play::{play_match, play_match_parallel, MatchResult};

/// Standing of a participant in the tournament
#[derive(Clone, Debug)]
pub struct Standing {
    /// Index of the ruleset in the original array
    pub index: usize,
    /// Name of the ruleset
    pub name: String,
    /// Total score (wins + 0.5 * draws)
    pub score: f32,
    /// Total wins
    pub wins: u32,
    /// Total losses
    pub losses: u32,
    /// Total draws
    pub draws: u32,
    /// Games played
    pub games_played: u32,
    /// Average game length
    pub avg_rounds: f32,
    /// Buchholz score (sum of opponents' scores, for tiebreaking)
    pub buchholz: f32,
}

impl Standing {
    /// Win rate
    pub fn win_rate(&self) -> f32 {
        if self.games_played == 0 {
            0.0
        } else {
            self.wins as f32 / self.games_played as f32
        }
    }

    /// Normalized score (0.0 to 1.0)
    pub fn normalized_score(&self) -> f32 {
        if self.games_played == 0 {
            0.0
        } else {
            self.score / self.games_played as f32
        }
    }
}

/// Result of a tournament
#[derive(Clone, Debug)]
pub struct TournamentResult {
    /// Final standings sorted by score (descending)
    pub standings: Vec<Standing>,
    /// All match results (indexed by [white_idx][black_idx])
    pub match_results: Vec<Vec<Option<MatchResult>>>,
    /// Tournament format used
    pub format: TournamentFormat,
    /// Number of rounds played
    pub rounds_played: usize,
}

impl TournamentResult {
    /// Get winner (top standing)
    pub fn winner(&self) -> Option<&Standing> {
        self.standings.first()
    }

    /// Get top N performers
    pub fn top_n(&self, n: usize) -> &[Standing] {
        let n = n.min(self.standings.len());
        &self.standings[..n]
    }

    /// Get standing for a specific ruleset index
    pub fn standing_for(&self, index: usize) -> Option<&Standing> {
        self.standings.iter().find(|s| s.index == index)
    }
}

// ============================================================================
// Level 1 - Orchestration
// ============================================================================

/// Run a tournament (Level 1 orchestration)
///
/// # Arguments
/// * `rulesets` - Participants in the tournament
/// * `config` - Tournament configuration
///
/// # Returns
/// Tournament results with final standings
pub fn run_tournament(rulesets: &[RuleSet], config: &TournamentConfig) -> TournamentResult {
    match config.format {
        TournamentFormat::RoundRobin => run_round_robin(rulesets, config),
        TournamentFormat::Swiss { rounds } => run_swiss(rulesets, config, rounds),
    }
}

// ============================================================================
// Level 2 - Phases
// ============================================================================

/// Run a round-robin tournament (Level 2 phase)
fn run_round_robin(rulesets: &[RuleSet], config: &TournamentConfig) -> TournamentResult {
    let n = rulesets.len();
    let pairings = generate_round_robin_pairings(n);
    let match_results = execute_all_matches(rulesets, &pairings, config);
    let standings = compute_standings(rulesets, &match_results);

    TournamentResult {
        standings,
        match_results,
        format: TournamentFormat::RoundRobin,
        rounds_played: if n > 1 { n - 1 } else { 0 },
    }
}

/// Run a Swiss tournament (Level 2 phase)
fn run_swiss(rulesets: &[RuleSet], config: &TournamentConfig, rounds: usize) -> TournamentResult {
    let n = rulesets.len();
    let mut match_results: Vec<Vec<Option<MatchResult>>> = vec![vec![None; n]; n];
    let mut scores: Vec<f32> = vec![0.0; n];
    let mut played: Vec<Vec<bool>> = vec![vec![false; n]; n];

    for _round_num in 0..rounds {
        let pairings = generate_swiss_pairings(n, &scores, &played);
        let round_results = execute_round_matches(rulesets, &pairings, config);

        // Update results and scores
        for (i, j, result) in round_results {
            if i < n && j < n {
                match_results[i][j] = Some(result.clone());
                played[i][j] = true;
                played[j][i] = true;

                // Update scores
                scores[i] += result.score_for_white();
                scores[j] += result.score_for_black();
            }
        }
    }

    let standings = compute_standings(rulesets, &match_results);

    TournamentResult {
        standings,
        match_results,
        format: TournamentFormat::Swiss { rounds },
        rounds_played: rounds,
    }
}

// ============================================================================
// Level 3 - Steps
// ============================================================================

/// Generate all pairings for round-robin
fn generate_round_robin_pairings(n: usize) -> Vec<(usize, usize)> {
    let mut pairings = Vec::new();
    for i in 0..n {
        for j in (i + 1)..n {
            pairings.push((i, j));
        }
    }
    pairings
}

/// Generate pairings for a Swiss round
fn generate_swiss_pairings(
    n: usize,
    scores: &[f32],
    played: &[Vec<bool>],
) -> Vec<(usize, usize)> {
    // Sort indices by score (descending)
    let mut indices: Vec<usize> = (0..n).collect();
    indices.sort_by(|&a, &b| {
        scores[b]
            .partial_cmp(&scores[a])
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    let mut pairings = Vec::new();
    let mut paired = vec![false; n];

    // Pair adjacent players by score
    for &i in &indices {
        if paired[i] {
            continue;
        }

        // Find best unpaired opponent (highest score who hasn't played i)
        for &j in &indices {
            if i != j && !paired[j] && !played[i][j] {
                pairings.push((i.min(j), i.max(j)));
                paired[i] = true;
                paired[j] = true;
                break;
            }
        }
    }

    // If odd number of players, one gets a bye (handled elsewhere)
    pairings
}

/// Execute all matches for round-robin
fn execute_all_matches(
    rulesets: &[RuleSet],
    pairings: &[(usize, usize)],
    config: &TournamentConfig,
) -> Vec<Vec<Option<MatchResult>>> {
    let n = rulesets.len();
    let mut results: Vec<Vec<Option<MatchResult>>> = vec![vec![None; n]; n];

    let match_results: Vec<((usize, usize), MatchResult)> = if config.parallel {
        pairings
            .par_iter()
            .map(|&(i, j)| {
                let result = execute_match(&rulesets[i], &rulesets[j], config);
                ((i, j), result)
            })
            .collect()
    } else {
        pairings
            .iter()
            .map(|&(i, j)| {
                let result = execute_match(&rulesets[i], &rulesets[j], config);
                ((i, j), result)
            })
            .collect()
    };

    for ((i, j), result) in match_results {
        results[i][j] = Some(result);
    }

    results
}

/// Execute matches for a single Swiss round
fn execute_round_matches(
    rulesets: &[RuleSet],
    pairings: &[(usize, usize)],
    config: &TournamentConfig,
) -> Vec<(usize, usize, MatchResult)> {
    if config.parallel {
        pairings
            .par_iter()
            .map(|&(i, j)| {
                let result = execute_match(&rulesets[i], &rulesets[j], config);
                (i, j, result)
            })
            .collect()
    } else {
        pairings
            .iter()
            .map(|&(i, j)| {
                let result = execute_match(&rulesets[i], &rulesets[j], config);
                (i, j, result)
            })
            .collect()
    }
}

/// Execute a single match between two rulesets
fn execute_match(white: &RuleSet, black: &RuleSet, config: &TournamentConfig) -> MatchResult {
    if config.parallel {
        play_match_parallel(
            white,
            black,
            config.ai_config.clone(),
            config.games_per_match,
            config.max_rounds,
        )
    } else {
        play_match(
            white,
            black,
            config.ai_config.clone(),
            config.games_per_match,
            config.max_rounds,
        )
    }
}

/// Compute final standings from match results
fn compute_standings(rulesets: &[RuleSet], results: &[Vec<Option<MatchResult>>]) -> Vec<Standing> {
    let n = rulesets.len();
    let mut standings: Vec<Standing> = (0..n)
        .map(|i| {
            let (wins, losses, draws, games, total_rounds) = compute_record(i, results);
            let avg_rounds = if games > 0 {
                total_rounds / games as f32
            } else {
                0.0
            };

            Standing {
                index: i,
                name: rulesets[i].name.clone(),
                score: wins as f32 + 0.5 * draws as f32,
                wins,
                losses,
                draws,
                games_played: games,
                avg_rounds,
                buchholz: 0.0, // Computed later
            }
        })
        .collect();

    // Compute Buchholz scores
    let scores: Vec<f32> = standings.iter().map(|s| s.score).collect();
    for standing in &mut standings {
        standing.buchholz = compute_buchholz(standing.index, results, &scores);
    }

    // Sort by score (descending), then Buchholz (descending)
    standings.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| {
                b.buchholz
                    .partial_cmp(&a.buchholz)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
    });

    standings
}

/// Compute win/loss/draw record for a player
fn compute_record(player: usize, results: &[Vec<Option<MatchResult>>]) -> (u32, u32, u32, u32, f32) {
    let n = results.len();
    let mut wins = 0u32;
    let mut losses = 0u32;
    let mut draws = 0u32;
    let mut games = 0u32;
    let mut total_rounds = 0f32;

    for opponent in 0..n {
        if player == opponent {
            continue;
        }

        // Check if player played as white against opponent
        if let Some(ref result) = results[player][opponent] {
            wins += result.white_wins;
            losses += result.black_wins;
            draws += result.draws;
            games += result.games_played;
            total_rounds += result.avg_rounds * result.games_played as f32;
        }

        // Check if player played as black against opponent
        if let Some(ref result) = results[opponent][player] {
            wins += result.black_wins;
            losses += result.white_wins;
            draws += result.draws;
            games += result.games_played;
            total_rounds += result.avg_rounds * result.games_played as f32;
        }
    }

    (wins, losses, draws, games, total_rounds)
}

/// Compute Buchholz score (sum of opponents' scores)
fn compute_buchholz(player: usize, results: &[Vec<Option<MatchResult>>], scores: &[f32]) -> f32 {
    let n = results.len();
    let mut buchholz = 0.0f32;

    for opponent in 0..n {
        if player == opponent {
            continue;
        }

        // Check if they played
        let played = results[player][opponent].is_some() || results[opponent][player].is_some();
        if played {
            buchholz += scores[opponent];
        }
    }

    buchholz
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::AiConfig;
    use hexwar_core::board::Hex;
    use hexwar_core::game::Template;

    fn make_test_ruleset(name: &str) -> RuleSet {
        RuleSet {
            name: name.to_string(),
            white_king: 25,
            white_pieces: vec![1, 1, 1, 1],
            white_positions: vec![
                Hex::new(0, 3),
                Hex::new(-1, 3),
                Hex::new(1, 2),
                Hex::new(-2, 3),
                Hex::new(2, 1),
            ],
            white_facings: vec![0; 5],
            white_template: Template::E,
            black_king: 25,
            black_pieces: vec![1, 1, 1, 1],
            black_positions: vec![
                Hex::new(0, -3),
                Hex::new(1, -3),
                Hex::new(-1, -2),
                Hex::new(2, -3),
                Hex::new(-2, -1),
            ],
            black_facings: vec![3; 5],
            black_template: Template::E,
        }
    }

    #[test]
    fn test_generate_round_robin_pairings() {
        let pairings = generate_round_robin_pairings(4);
        assert_eq!(pairings.len(), 6); // C(4,2) = 6
        assert!(pairings.contains(&(0, 1)));
        assert!(pairings.contains(&(0, 2)));
        assert!(pairings.contains(&(0, 3)));
        assert!(pairings.contains(&(1, 2)));
        assert!(pairings.contains(&(1, 3)));
        assert!(pairings.contains(&(2, 3)));
    }

    #[test]
    fn test_generate_round_robin_pairings_empty() {
        let pairings = generate_round_robin_pairings(0);
        assert!(pairings.is_empty());

        let pairings = generate_round_robin_pairings(1);
        assert!(pairings.is_empty());
    }

    #[test]
    fn test_generate_swiss_pairings() {
        let scores = vec![3.0, 1.0, 2.0, 0.0];
        let played = vec![
            vec![false, false, false, false],
            vec![false, false, false, false],
            vec![false, false, false, false],
            vec![false, false, false, false],
        ];

        let pairings = generate_swiss_pairings(4, &scores, &played);
        assert_eq!(pairings.len(), 2);
        // Should pair 0 (3.0) with 2 (2.0), and 1 (1.0) with 3 (0.0)
    }

    #[test]
    fn test_standing_calculations() {
        let standing = Standing {
            index: 0,
            name: "test".to_string(),
            score: 7.0,
            wins: 6,
            losses: 2,
            draws: 2,
            games_played: 10,
            avg_rounds: 25.0,
            buchholz: 10.0,
        };

        assert_eq!(standing.win_rate(), 0.6);
        assert_eq!(standing.normalized_score(), 0.7);
    }

    #[test]
    fn test_run_tournament_round_robin() {
        let rulesets = vec![
            make_test_ruleset("A"),
            make_test_ruleset("B"),
            make_test_ruleset("C"),
        ];

        let config = TournamentConfig {
            format: TournamentFormat::RoundRobin,
            games_per_match: 2,
            ai_config: AiConfig::alpha_beta(1).with_seed(42),
            parallel: false,
            max_rounds: 20,
        };

        let result = run_tournament(&rulesets, &config);

        assert_eq!(result.standings.len(), 3);
        assert_eq!(result.format, TournamentFormat::RoundRobin);

        // Each player should have played against 2 opponents
        for standing in &result.standings {
            assert!(standing.games_played > 0);
        }

        // Winner should be first
        assert!(result.winner().is_some());
    }

    #[test]
    fn test_run_tournament_swiss() {
        let rulesets = vec![
            make_test_ruleset("A"),
            make_test_ruleset("B"),
            make_test_ruleset("C"),
            make_test_ruleset("D"),
        ];

        let config = TournamentConfig {
            format: TournamentFormat::Swiss { rounds: 2 },
            games_per_match: 2,
            ai_config: AiConfig::alpha_beta(1).with_seed(42),
            parallel: false,
            max_rounds: 20,
        };

        let result = run_tournament(&rulesets, &config);

        assert_eq!(result.standings.len(), 4);
        assert_eq!(result.format, TournamentFormat::Swiss { rounds: 2 });
        assert_eq!(result.rounds_played, 2);
    }

    #[test]
    fn test_tournament_result_accessors() {
        let rulesets = vec![make_test_ruleset("A"), make_test_ruleset("B")];

        let config = TournamentConfig {
            format: TournamentFormat::RoundRobin,
            games_per_match: 2,
            ai_config: AiConfig::alpha_beta(1).with_seed(42),
            parallel: false,
            max_rounds: 20,
        };

        let result = run_tournament(&rulesets, &config);

        // Test accessors
        assert!(result.winner().is_some());
        assert_eq!(result.top_n(1).len(), 1);
        assert!(result.standing_for(0).is_some());
        assert!(result.standing_for(1).is_some());
        assert!(result.standing_for(99).is_none());
    }

    #[test]
    fn test_compute_standings() {
        let rulesets = vec![make_test_ruleset("A"), make_test_ruleset("B")];

        // Create mock results
        let mut results: Vec<Vec<Option<MatchResult>>> = vec![vec![None; 2]; 2];
        results[0][1] = Some(MatchResult {
            white_wins: 3,
            black_wins: 2,
            draws: 1,
            avg_rounds: 20.0,
            games_played: 6,
            game_outcomes: vec![],
        });

        let standings = compute_standings(&rulesets, &results);

        assert_eq!(standings.len(), 2);
        // A played white and got 3 wins, B played black and got 2 wins
        // A's score: 3 + 0.5 = 3.5
        // B's score: 2 + 0.5 = 2.5
        // A should be ranked higher
        assert_eq!(standings[0].name, "A");
        assert_eq!(standings[1].name, "B");
    }
}
