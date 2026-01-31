//! Evolution command - run genetic algorithm to balance armies
//!
//! ## Architecture (4-layer granularity)
//!
//! - Level 1: run() - orchestration
//! - Level 2: setup_evolution(), run_evolution(), save_results()
//! - Level 3: create_fitness_fn(), load_population(), etc.
//! - Level 4: file I/O, formatting utilities

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use clap::Args;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use hexwar_core::RuleSet;
use hexwar_evolve::{evolve_with_callback, EvolutionConfig, EvolutionResult, MutateSide};

// ============================================================================
// COMMAND ARGUMENTS (Level 4 - Configuration)
// ============================================================================

#[derive(Args)]
pub struct EvolveArgs {
    /// Population size
    #[arg(long, default_value = "50")]
    pub population: usize,

    /// Number of generations to run
    #[arg(long, default_value = "100")]
    pub generations: usize,

    /// Games per fitness evaluation
    #[arg(long, default_value = "10")]
    pub games: usize,

    /// AI search depth for fitness evaluation
    #[arg(long, default_value = "4")]
    pub depth: u32,

    /// Lock white army (evolve black only) - path to ruleset JSON
    #[arg(long, value_name = "FILE")]
    pub fixed_white: Option<PathBuf>,

    /// Lock black army (evolve white only) - path to ruleset JSON
    #[arg(long, value_name = "FILE")]
    pub fixed_black: Option<PathBuf>,

    /// Seed population from directory of ruleset JSONs
    #[arg(long, value_name = "DIR")]
    pub seeds: Option<PathBuf>,

    /// Output directory for results
    #[arg(long, default_value = "evolution_output")]
    pub output: PathBuf,

    /// Mutation rate (0.0-1.0)
    #[arg(long, default_value = "0.1")]
    pub mutation_rate: f32,

    /// Crossover rate (0.0-1.0)
    #[arg(long, default_value = "0.7")]
    pub crossover_rate: f32,

    /// Number of elite individuals to preserve
    #[arg(long, default_value = "2")]
    pub elitism: usize,

    /// Output results as JSON
    #[arg(long)]
    pub json: bool,

    /// Maximum rounds per game
    #[arg(long, default_value = "50")]
    pub max_rounds: u32,

    /// Show detailed UCB stats for top individuals each generation
    #[arg(long)]
    pub show_stats: bool,

    /// Use multi-depth evaluation (skill gradient + color fairness)
    /// Plays games at multiple depth combinations to test balance
    #[arg(long)]
    pub multi_depth: bool,

    /// Use reduced matchup set for faster multi-depth evaluation
    #[arg(long)]
    pub reduced: bool,
}

// ============================================================================
// LEVEL 1 - ORCHESTRATION
// ============================================================================

/// Run evolution command
///
/// This function reads like a table of contents:
/// 1. Set up evolution configuration
/// 2. Load initial population
/// 3. Run evolution loop
/// 4. Save results
pub fn run(args: EvolveArgs, seed: Option<u64>) -> Result<()> {
    let config = build_evolution_config(&args);
    let mut rng = create_rng(seed);

    if args.multi_depth {
        tracing::info!("Starting evolution: pop={}, gen={}, depth={} (multi-depth mode{})",
            args.population, args.generations, args.depth,
            if args.reduced { ", reduced" } else { "" });
    } else {
        tracing::info!("Starting evolution: pop={}, gen={}, depth={}",
            args.population, args.generations, args.depth);
    }

    let population = load_initial_population(&args, &config, &mut rng)?;
    let result = run_evolution(population, &config, &args, &mut rng)?;

    save_results(&result, &args)?;

    print_summary(&result, &args);

    Ok(())
}

// ============================================================================
// LEVEL 2 - PHASES
// ============================================================================

/// Build evolution configuration from command arguments
fn build_evolution_config(args: &EvolveArgs) -> EvolutionConfig {
    let evolve_side = determine_evolve_side(args);

    EvolutionConfig {
        population_size: args.population,
        generations: args.generations,
        mutation_rate: args.mutation_rate,
        crossover_rate: args.crossover_rate,
        elitism: args.elitism,
        tournament_size: 3,
        evolve_side,
    }
}

/// Load initial population from seeds or generate random
fn load_initial_population(
    args: &EvolveArgs,
    config: &EvolutionConfig,
    rng: &mut ChaCha8Rng,
) -> Result<Vec<RuleSet>> {
    let mut population = load_seed_population(args)?;

    // Apply fixed side if specified
    apply_fixed_side(&mut population, args)?;

    // Fill to population size if needed
    fill_population(&mut population, config.population_size, rng);

    tracing::info!("Initial population: {} rulesets", population.len());

    Ok(population)
}

/// Run the evolution loop with progress callback
fn run_evolution(
    population: Vec<RuleSet>,
    config: &EvolutionConfig,
    args: &EvolveArgs,
    rng: &mut ChaCha8Rng,
) -> Result<EvolutionResult> {
    // Create fitness function that evaluates rulesets
    let fitness_fn = create_fitness_fn(args);

    // Progress callback
    let callback = |gen: usize, _pop: &[RuleSet], fitness: &[f32]| {
        let best = fitness.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
        let avg = fitness.iter().sum::<f32>() / fitness.len() as f32;
        tracing::info!("Generation {}: best={:.3}, avg={:.3}", gen + 1, best, avg);
    };

    let result = evolve_with_callback(population, config, fitness_fn, callback, rng);

    Ok(result)
}

/// Save evolution results to output directory
fn save_results(result: &EvolutionResult, args: &EvolveArgs) -> Result<()> {
    create_output_directory(&args.output)?;
    save_champions(result, &args.output)?;
    save_fitness_history(result, &args.output)?;

    if args.json {
        print_json_results(result)?;
    }

    // Show detailed stats if requested
    if args.show_stats {
        print_detailed_stats(result, args);
    }

    Ok(())
}

/// Print detailed UCB stats for top individuals
fn print_detailed_stats(result: &EvolutionResult, args: &EvolveArgs) {
    println!("\n=== Detailed Stats (Top 5) ===");

    if args.multi_depth {
        println!("Re-evaluating with multi-depth tournament...");
        for (i, rs) in result.population.iter().take(5).enumerate() {
            let md_result = evaluate_ruleset_multi_depth(
                rs, args.depth, args.games * 2, args.max_rounds, args.reduced
            );

            println!("\n#{} {} ({}W/{}B/{}D)",
                i + 1,
                rs.name,
                md_result.white_wins,
                md_result.black_wins,
                md_result.draws
            );
            println!("  Fitness:        {:.3}", md_result.fitness);
            println!("  Skill Gradient: {:.1}% (deeper player win rate)", md_result.skill_gradient * 100.0);
            println!("  Color Fairness: {:.3}", md_result.color_fairness);
            println!("  Game Richness:  {:.3}", md_result.game_richness);
            println!("  Avg length:     {:.1} rounds", md_result.avg_rounds);
            println!("  Total games:    {}", md_result.total_games);
        }
    } else {
        println!("Re-evaluating with {} games each...", args.games * 5);

        let total_evals = result.population.len() * args.games * 5;

        for (i, rs) in result.population.iter().take(5).enumerate() {
            // Run more games for better confidence
            let stats = evaluate_ruleset_detailed(rs, args.depth, args.games * 5, args.max_rounds);
            let (ci_low, ci_high) = stats.confidence_interval();
            let ucb = stats.ucb(total_evals, args.max_rounds);
            let fitness = stats.fitness(args.max_rounds);
            let avg_rounds = stats.total_rounds as f32 / stats.games as f32;

            println!("\n#{} {} ({}W/{}B/{}D)",
                i + 1,
                rs.name,
                stats.white_wins,
                stats.black_wins,
                stats.draws
            );
            println!("  Fitness: {:.3} (95% CI: {:.3}-{:.3})", fitness, ci_low, ci_high);
            println!("  UCB:     {:.3}", ucb);
            println!("  Avg len: {:.1} rounds", avg_rounds);
            println!("  Balance: {:.1}% white", stats.white_wins as f32 / stats.games as f32 * 100.0);
        }
    }
}

// ============================================================================
// LEVEL 3 - STEPS
// ============================================================================

/// Determine which side(s) to evolve based on fixed army flags
fn determine_evolve_side(args: &EvolveArgs) -> MutateSide {
    match (&args.fixed_white, &args.fixed_black) {
        (Some(_), Some(_)) => {
            tracing::warn!("Both sides fixed - nothing to evolve, using Both");
            MutateSide::Both
        }
        (Some(_), None) => MutateSide::Black,
        (None, Some(_)) => MutateSide::White,
        (None, None) => MutateSide::Both,
    }
}

/// Load seed population from directory if specified
fn load_seed_population(args: &EvolveArgs) -> Result<Vec<RuleSet>> {
    match &args.seeds {
        Some(seeds_dir) => load_rulesets_from_directory(seeds_dir),
        None => Ok(vec![RuleSet::default()]),
    }
}

/// Load all ruleset JSON files from a directory
fn load_rulesets_from_directory(dir: &Path) -> Result<Vec<RuleSet>> {
    let mut rulesets = Vec::new();

    if !dir.exists() {
        anyhow::bail!("Seeds directory does not exist: {}", dir.display());
    }

    for entry in std::fs::read_dir(dir).context("Failed to read seeds directory")? {
        let entry = entry?;
        let path = entry.path();

        if path.extension().map_or(false, |ext| ext == "json") {
            match RuleSet::load(&path) {
                Ok(rs) => {
                    tracing::debug!("Loaded seed: {}", path.display());
                    rulesets.push(rs);
                }
                Err(e) => {
                    tracing::warn!("Failed to load {}: {}", path.display(), e);
                }
            }
        }
    }

    if rulesets.is_empty() {
        tracing::warn!("No valid seeds found in {}, using default", dir.display());
        rulesets.push(RuleSet::default());
    }

    Ok(rulesets)
}

/// Apply fixed side ruleset to all population members
fn apply_fixed_side(population: &mut [RuleSet], args: &EvolveArgs) -> Result<()> {
    if let Some(ref fixed_white_path) = args.fixed_white {
        let fixed = RuleSet::load(fixed_white_path)
            .context("Failed to load fixed white ruleset")?;

        for rs in population.iter_mut() {
            rs.white_king = fixed.white_king;
            rs.white_pieces = fixed.white_pieces.clone();
            rs.white_positions = fixed.white_positions.clone();
            rs.white_facings = fixed.white_facings.clone();
            rs.white_template = fixed.white_template;
        }

        tracing::info!("Fixed white army from: {}", fixed_white_path.display());
    }

    if let Some(ref fixed_black_path) = args.fixed_black {
        let fixed = RuleSet::load(fixed_black_path)
            .context("Failed to load fixed black ruleset")?;

        for rs in population.iter_mut() {
            rs.black_king = fixed.black_king;
            rs.black_pieces = fixed.black_pieces.clone();
            rs.black_positions = fixed.black_positions.clone();
            rs.black_facings = fixed.black_facings.clone();
            rs.black_template = fixed.black_template;
        }

        tracing::info!("Fixed black army from: {}", fixed_black_path.display());
    }

    Ok(())
}

/// Fill population to target size by cloning random existing members
fn fill_population(population: &mut Vec<RuleSet>, target_size: usize, rng: &mut ChaCha8Rng) {
    use rand::Rng;

    while population.len() < target_size {
        if population.is_empty() {
            population.push(RuleSet::default());
        } else {
            let idx = rng.gen_range(0..population.len());
            let mut clone = population[idx].clone();
            clone.name = format!("{}-clone-{}", clone.name, population.len());
            population.push(clone);
        }
    }

    population.truncate(target_size);
}

/// Create fitness evaluation function
///
/// Plays actual games using alpha-beta AI to evaluate ruleset quality.
/// Returns win rate as fitness (0.0 to 1.0).
///
/// If multi_depth is enabled, uses the full tournament matchup spec
/// with skill gradient and color fairness testing.
fn create_fitness_fn(args: &EvolveArgs) -> impl Fn(&RuleSet) -> f32 {
    let depth = args.depth;
    let games = args.games;
    let max_rounds = args.max_rounds;
    let multi_depth = args.multi_depth;
    let reduced = args.reduced;

    move |rs: &RuleSet| {
        if multi_depth {
            evaluate_ruleset_multi_depth(rs, depth, games, max_rounds, reduced).fitness
        } else {
            evaluate_ruleset_fitness(rs, depth, games, max_rounds)
        }
    }
}

/// Stats from evaluating a ruleset
#[derive(Clone, Debug)]
struct EvalStats {
    white_wins: usize,
    black_wins: usize,
    draws: usize,
    total_rounds: u32,
    games: usize,
}

// ============================================================================
// MULTI-DEPTH EVALUATION (ported from Python tournament.py)
// ============================================================================

/// A single matchup specification: (depth1, depth2, num_games, weight)
#[derive(Clone, Debug)]
struct MatchupSpec {
    depth1: u32,
    depth2: u32,
    num_games: usize,
    weight: f32,
}

/// Stats for a single matchup
#[derive(Clone, Debug, Default)]
struct MatchupStats {
    deeper_depth: u32,
    shallower_depth: u32,
    deeper_wins: usize,
    shallower_wins: usize,
    draws: usize,
    games_played: usize,
    white_wins: usize,
    black_wins: usize,
    total_rounds: u32,
}

impl MatchupStats {
    fn deeper_win_rate(&self) -> f32 {
        if self.games_played == 0 {
            return 0.0;
        }
        self.deeper_wins as f32 / self.games_played as f32
    }

    fn white_win_rate(&self) -> f32 {
        let non_draws = self.white_wins + self.black_wins;
        if non_draws == 0 {
            return 0.5;
        }
        self.white_wins as f32 / non_draws as f32
    }
}

/// Result of multi-depth tournament evaluation
#[derive(Clone, Debug)]
struct MultiDepthResult {
    fitness: f32,
    skill_gradient: f32,
    color_fairness: f32,
    game_richness: f32,
    white_wins: usize,
    black_wins: usize,
    draws: usize,
    total_games: usize,
    avg_rounds: f32,
}

/// Build matchup specs based on depth (ported from Python evaluate_ruleset_tournament)
fn build_matchup_specs(depth: u32, games_per_matchup: usize, reduced: bool) -> Vec<MatchupSpec> {
    let d = depth.max(2);
    let base_games = games_per_matchup;

    let mut matchup_spec = Vec::new();

    // Build tiers: 2, 4, 6, 8, ... up to depth
    let mut tiers: Vec<u32> = (2..=d).step_by(2).collect();
    if !tiers.contains(&d) {
        tiers.push(d);
    }
    tiers.sort();

    for tier in tiers {
        let is_target = tier == d;

        let (n_games, weight_equal, weight_skill_1ply, weight_skill_2ply) = if reduced {
            if is_target {
                // Target depth gets 2x games and higher weight
                (
                    base_games * 2,
                    1.5f32,
                    1.5f32,
                    2.5f32,
                )
            } else {
                // Lower tiers get base games
                let tier_f = tier as f32;
                (
                    base_games,
                    0.6 + tier_f / 10.0,
                    0.8 + tier_f / 10.0,
                    1.2 + tier_f / 10.0,
                )
            }
        } else {
            let tier_f = tier as f32;
            let mut we = 0.6 + tier_f / 10.0;
            let mut ws1 = 0.8 + tier_f / 10.0;
            let mut ws2 = 1.2 + tier_f / 10.0;
            if is_target {
                we += 0.3;
                ws1 += 0.3;
                ws2 += 0.5;
            }
            (base_games, we, ws1, ws2)
        };

        // Equal depth matchup (tests color fairness)
        matchup_spec.push(MatchupSpec {
            depth1: tier,
            depth2: tier,
            num_games: n_games,
            weight: weight_equal,
        });

        // 1-ply skill gradient: stronger vs weaker (tier-1)
        if tier >= 3 {
            matchup_spec.push(MatchupSpec {
                depth1: tier,
                depth2: tier - 1,
                num_games: n_games,
                weight: weight_skill_1ply,
            });
        }

        // 2-ply skill gradient: stronger vs weaker (tier-2)
        if tier >= 4 {
            matchup_spec.push(MatchupSpec {
                depth1: tier,
                depth2: tier - 2,
                num_games: n_games,
                weight: weight_skill_2ply,
            });
        }
    }

    matchup_spec
}

/// Run a single matchup and return stats
fn run_matchup(
    rs: &RuleSet,
    depth1: u32,
    depth2: u32,
    n_games: usize,
    max_rounds: u32,
    base_seed: u64,
) -> MatchupStats {
    use hexwar_core::{AlphaBetaAI, Heuristics};

    let deeper = depth1.max(depth2);
    let shallower = depth1.min(depth2);
    let heuristics = Heuristics::default();

    let mut stats = MatchupStats {
        deeper_depth: deeper,
        shallower_depth: shallower,
        ..Default::default()
    };

    for game_idx in 0..n_games {
        // IMPORTANT: Use different seeds for each game
        let seed = base_seed.wrapping_add(game_idx as u64 * 12345);

        // Alternate colors: even games have deeper as white, odd games have shallower as white
        let (white_depth, black_depth) = if game_idx % 2 == 0 {
            (deeper, shallower)
        } else {
            (shallower, deeper)
        };

        let state = rs.to_game_state();

        // Use different seeds for white and black AIs
        let mut white_ai = AlphaBetaAI::with_seed(white_depth, heuristics.clone(), seed);
        let mut black_ai = AlphaBetaAI::with_seed(black_depth, heuristics.clone(), seed.wrapping_add(7777));

        let mut current = state;
        let mut rounds = 0u32;

        while current.result() == hexwar_core::GameResult::Ongoing && rounds < max_rounds {
            let ai = match current.current_player() {
                hexwar_core::Player::White => &mut white_ai,
                hexwar_core::Player::Black => &mut black_ai,
            };

            if let Some(mv) = ai.best_move(&current) {
                current = current.apply_move(mv);
                rounds += 1;
            } else {
                break;
            }
        }

        stats.total_rounds += rounds;
        stats.games_played += 1;

        match current.result() {
            hexwar_core::GameResult::WhiteWins => {
                stats.white_wins += 1;
                if white_depth == deeper {
                    stats.deeper_wins += 1;
                } else {
                    stats.shallower_wins += 1;
                }
            }
            hexwar_core::GameResult::BlackWins => {
                stats.black_wins += 1;
                if black_depth == deeper {
                    stats.deeper_wins += 1;
                } else {
                    stats.shallower_wins += 1;
                }
            }
            hexwar_core::GameResult::Ongoing => {
                stats.draws += 1;
            }
        }
    }

    stats
}

/// Evaluate ruleset using multi-depth tournament (ported from Python)
fn evaluate_ruleset_multi_depth(
    rs: &RuleSet,
    depth: u32,
    games_per_matchup: usize,
    max_rounds: u32,
    reduced: bool,
) -> MultiDepthResult {
    use rand::Rng;

    let matchup_specs = build_matchup_specs(depth, games_per_matchup, reduced);

    let mut all_matchups: Vec<(MatchupSpec, MatchupStats)> = Vec::new();
    let mut seed_offset = 0u64;
    let mut rng = rand::thread_rng();
    let base_seed: u64 = rng.gen();

    let mut total_games = 0usize;
    let mut total_rounds = 0u32;
    let mut white_wins_total = 0usize;
    let mut black_wins_total = 0usize;
    let mut draws_total = 0usize;

    for spec in &matchup_specs {
        let stats = run_matchup(
            rs,
            spec.depth1,
            spec.depth2,
            spec.num_games,
            max_rounds,
            base_seed.wrapping_add(seed_offset),
        );

        total_games += stats.games_played;
        total_rounds += stats.total_rounds;
        white_wins_total += stats.white_wins;
        black_wins_total += stats.black_wins;
        draws_total += stats.draws;

        seed_offset += spec.num_games as u64 * 1000;
        all_matchups.push((spec.clone(), stats));
    }

    // Calculate fitness components

    // 1. Skill Gradient (deeper player should win more often)
    let mut weighted_sum = 0.0f32;
    let mut weight_total = 0.0f32;
    for (spec, stats) in &all_matchups {
        if spec.depth1 != spec.depth2 {
            let gap = (spec.depth1 as i32 - spec.depth2 as i32).unsigned_abs();
            let weight = 1.0 + (gap as f32 - 1.0) * 0.5;
            weighted_sum += stats.deeper_win_rate() * weight;
            weight_total += weight;
        }
    }
    let skill_gradient = if weight_total > 0.0 {
        weighted_sum / weight_total
    } else {
        0.5
    };

    // 2. Color Fairness (at equal depths, should be 50/50)
    let mut equal_depth_games = 0usize;
    let mut equal_depth_balance = 0.0f32;
    for (spec, stats) in &all_matchups {
        if spec.depth1 == spec.depth2 {
            equal_depth_games += stats.games_played;
            let win_rate = stats.white_win_rate();
            // Score of 1.0 means perfect 50/50
            equal_depth_balance += (1.0 - (win_rate - 0.5).abs() * 2.0) * stats.games_played as f32;
        }
    }
    let color_fairness = if equal_depth_games > 0 {
        equal_depth_balance / equal_depth_games as f32
    } else {
        0.5
    };

    // 3. Game Richness (average game length, normalized)
    let avg_rounds = if total_games > 0 {
        total_rounds as f32 / total_games as f32
    } else {
        0.0
    };
    let game_richness = if avg_rounds < 10.0 {
        avg_rounds / 10.0
    } else if avg_rounds > 60.0 {
        (1.0 - (avg_rounds - 60.0) / 100.0).max(0.5)
    } else {
        1.0
    };

    // 4. Decisiveness (fewer draws is better)
    let decisiveness = if total_games > 0 {
        1.0 - draws_total as f32 / total_games as f32
    } else {
        0.5
    };

    // Combined fitness with non-linear skill gradient penalty
    let skill_score = if skill_gradient >= 0.95 {
        1.0
    } else if skill_gradient >= 0.90 {
        0.9 + (skill_gradient - 0.90) * 2.0
    } else if skill_gradient >= 0.80 {
        0.6 + (skill_gradient - 0.80) * 3.0
    } else if skill_gradient >= 0.65 {
        0.3 + (skill_gradient - 0.65) * 2.0
    } else {
        skill_gradient * 0.5
    };

    let mut fitness = 0.40 * skill_score + 0.35 * color_fairness + 0.15 * game_richness + 0.10 * decisiveness;

    // Penalty if one color never wins at equal depth
    if equal_depth_games >= 4 {
        for (spec, stats) in &all_matchups {
            if spec.depth1 == spec.depth2 && (stats.white_wins == 0 || stats.black_wins == 0) {
                fitness *= 0.3;
            }
        }
    }

    // Penalty if deeper player loses too often
    if skill_gradient < 0.80 {
        fitness *= 0.5;
    }

    MultiDepthResult {
        fitness,
        skill_gradient,
        color_fairness,
        game_richness,
        white_wins: white_wins_total,
        black_wins: black_wins_total,
        draws: draws_total,
        total_games,
        avg_rounds,
    }
}

impl EvalStats {
    fn fitness(&self, max_rounds: u32) -> f32 {
        let total = self.games as f32;
        let white_rate = self.white_wins as f32 / total;
        let draw_rate = self.draws as f32 / total;

        // Balance score: 1.0 when perfectly balanced (50/50), lower when skewed
        let balance = 1.0 - (white_rate - 0.5).abs() * 2.0;

        // Decisiveness: penalize draws
        let decisiveness = 1.0 - draw_rate;

        // Game length bonus: prefer games that don't drag on
        let avg_rounds = self.total_rounds as f32 / self.games as f32;
        let length_score = (1.0 - avg_rounds / max_rounds as f32).max(0.0);

        balance * 0.5 + decisiveness * 0.3 + length_score * 0.2
    }

    /// UCB1-style upper confidence bound
    /// fitness + C * sqrt(2 * ln(N) / n) where N = total evals, n = games
    fn ucb(&self, total_evals: usize, max_rounds: u32) -> f32 {
        let fitness = self.fitness(max_rounds);
        let c = 1.41; // sqrt(2)
        let n = self.games as f32;
        let big_n = total_evals.max(1) as f32;
        fitness + c * (2.0 * big_n.ln() / n).sqrt()
    }

    /// 95% confidence interval on fitness (Wilson score approximation)
    fn confidence_interval(&self) -> (f32, f32) {
        // Use wins/(wins+losses) as our estimate (ignoring draws for balance)
        let decisive = self.white_wins + self.black_wins;
        if decisive == 0 {
            return (0.0, 1.0); // No data
        }
        let p = self.white_wins as f32 / decisive as f32;
        let n = decisive as f32;
        let z = 1.96; // 95% confidence
        let z2 = z * z;

        // Wilson score interval
        let denom = 1.0 + z2 / n;
        let center = (p + z2 / (2.0 * n)) / denom;
        let margin = z * (p * (1.0 - p) / n + z2 / (4.0 * n * n)).sqrt() / denom;

        ((center - margin).max(0.0), (center + margin).min(1.0))
    }
}

/// Evaluate a ruleset by playing games
///
/// Plays the ruleset against itself (white vs black) and returns
/// a fitness score based on game balance and decisiveness.
fn evaluate_ruleset_fitness(rs: &RuleSet, depth: u32, games: usize, max_rounds: u32) -> f32 {
    evaluate_ruleset_detailed(rs, depth, games, max_rounds).fitness(max_rounds)
}

/// Evaluate with full stats returned
///
/// # Seed Handling
/// Each game gets a unique seed derived from the ruleset's name hash and game index:
/// - Base seed: hash of ruleset name (ensures different rulesets get different seeds)
/// - Per-game seed: base_seed + game_idx * 10000 (ensures each game is different)
/// - White AI: per-game seed
/// - Black AI: per-game seed + 1000 (ensures different play styles)
///
/// This ensures:
/// 1. Each game in an evaluation has different AI behavior
/// 2. Evaluations of the same ruleset are reproducible
/// 3. Different rulesets don't accidentally share game seeds
fn evaluate_ruleset_detailed(rs: &RuleSet, depth: u32, games: usize, max_rounds: u32) -> EvalStats {
    use hexwar_core::{AlphaBetaAI, Heuristics};
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let heuristics = Heuristics::default();

    // Generate a base seed from the ruleset name for reproducibility
    let mut hasher = DefaultHasher::new();
    rs.name.hash(&mut hasher);
    let base_seed = hasher.finish();

    let mut white_wins = 0;
    let mut black_wins = 0;
    let mut draws = 0;
    let mut total_rounds = 0u32;

    for game_idx in 0..games {
        // Each game gets a unique seed: base + game_idx * 10000
        // The 10000 multiplier ensures large gaps between game seeds
        let game_seed = base_seed.wrapping_add((game_idx as u64).wrapping_mul(10000));
        let white_seed = game_seed;
        let black_seed = game_seed.wrapping_add(1000);

        let state = rs.to_game_state();

        let mut white_ai = AlphaBetaAI::with_seed(depth, heuristics.clone(), white_seed);
        let mut black_ai = AlphaBetaAI::with_seed(depth, heuristics.clone(), black_seed);

        let mut current = state;
        let mut rounds = 0u32;

        while current.result() == hexwar_core::GameResult::Ongoing && rounds < max_rounds {
            let ai = match current.current_player() {
                hexwar_core::Player::White => &mut white_ai,
                hexwar_core::Player::Black => &mut black_ai,
            };

            if let Some(mv) = ai.best_move(&current) {
                current = current.apply_move(mv);
                rounds += 1;
            } else {
                break;
            }
        }

        total_rounds += rounds;

        match current.result() {
            hexwar_core::GameResult::WhiteWins => white_wins += 1,
            hexwar_core::GameResult::BlackWins => black_wins += 1,
            hexwar_core::GameResult::Ongoing => draws += 1,
        }
    }

    EvalStats {
        white_wins,
        black_wins,
        draws,
        total_rounds,
        games,
    }
}


/// Create output directory
fn create_output_directory(output: &Path) -> Result<()> {
    std::fs::create_dir_all(output).context("Failed to create output directory")?;

    let champions_dir = output.join("champions");
    std::fs::create_dir_all(&champions_dir).context("Failed to create champions directory")?;

    Ok(())
}

/// Save top champions to output directory
fn save_champions(result: &EvolutionResult, output: &Path) -> Result<()> {
    let champions_dir = output.join("champions");
    let num_champions = std::cmp::min(5, result.population.len());

    for (i, rs) in result.population.iter().take(num_champions).enumerate() {
        let path = champions_dir.join(format!("champion_{}.json", i + 1));
        rs.save(&path).context("Failed to save champion")?;
        tracing::info!("Saved champion {} to {}", i + 1, path.display());
    }

    Ok(())
}

/// Save fitness history to CSV
fn save_fitness_history(result: &EvolutionResult, output: &Path) -> Result<()> {
    let path = output.join("fitness_history.csv");
    let mut content = String::from("generation,best_fitness,avg_fitness\n");

    for (i, (best, avg)) in result
        .best_fitness_history
        .iter()
        .zip(&result.avg_fitness_history)
        .enumerate()
    {
        content.push_str(&format!("{},{:.4},{:.4}\n", i + 1, best, avg));
    }

    std::fs::write(&path, content).context("Failed to write fitness history")?;
    tracing::info!("Saved fitness history to {}", path.display());

    Ok(())
}

/// Print JSON results to stdout
fn print_json_results(result: &EvolutionResult) -> Result<()> {
    #[derive(serde::Serialize)]
    struct JsonOutput {
        best_fitness: f32,
        final_avg_fitness: f32,
        generations_run: usize,
        best_ruleset: RuleSet,
    }

    let output = JsonOutput {
        best_fitness: result.fitness.first().copied().unwrap_or(0.0),
        final_avg_fitness: result.fitness.iter().sum::<f32>() / result.fitness.len().max(1) as f32,
        generations_run: result.best_fitness_history.len(),
        best_ruleset: result.population.first().cloned().unwrap_or_default(),
    };

    let json = serde_json::to_string_pretty(&output)?;
    println!("{}", json);

    Ok(())
}

/// Print summary to console
fn print_summary(result: &EvolutionResult, args: &EvolveArgs) {
    println!("\n=== Evolution Complete ===");
    println!("Generations: {}", result.best_fitness_history.len());
    println!(
        "Best fitness: {:.4}",
        result.fitness.first().copied().unwrap_or(0.0)
    );
    println!(
        "Final avg fitness: {:.4}",
        result.fitness.iter().sum::<f32>() / result.fitness.len().max(1) as f32
    );
    println!("Output directory: {}", args.output.display());

    if let Some(best) = result.population.first() {
        println!("Best ruleset: {}", best.name);
    }
}

// ============================================================================
// LEVEL 4 - UTILITIES
// ============================================================================

/// Create RNG from seed or random
fn create_rng(seed: Option<u64>) -> ChaCha8Rng {
    match seed {
        Some(s) => ChaCha8Rng::seed_from_u64(s),
        None => ChaCha8Rng::from_entropy(),
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_evolve_side() {
        let args = EvolveArgs {
            population: 10,
            generations: 5,
            games: 2,
            depth: 2,
            fixed_white: None,
            fixed_black: None,
            seeds: None,
            output: PathBuf::from("test"),
            mutation_rate: 0.1,
            crossover_rate: 0.7,
            elitism: 2,
            json: false,
            max_rounds: 50,
            show_stats: false,
            multi_depth: false,
            reduced: false,
        };

        assert!(matches!(determine_evolve_side(&args), MutateSide::Both));
    }

    #[test]
    fn test_fill_population() {
        let mut pop = vec![RuleSet::default()];
        let mut rng = ChaCha8Rng::seed_from_u64(42);

        fill_population(&mut pop, 5, &mut rng);
        assert_eq!(pop.len(), 5);

        fill_population(&mut pop, 3, &mut rng);
        assert_eq!(pop.len(), 3);
    }

    #[test]
    fn test_create_rng_with_seed() {
        let rng1 = create_rng(Some(42));
        let rng2 = create_rng(Some(42));

        // Same seed should produce same first value
        use rand::Rng;
        let mut rng1 = rng1;
        let mut rng2 = rng2;
        assert_eq!(rng1.gen::<u64>(), rng2.gen::<u64>());
    }

    #[test]
    fn test_build_matchup_specs_depth_4() {
        // Depth 4 should produce matchups at tier 2 and tier 4
        let specs = build_matchup_specs(4, 2, true);

        // Expect:
        // - d2 vs d2 (equal depth)
        // - d4 vs d4 (equal depth)
        // - d4 vs d3 (1-ply gradient)
        // - d4 vs d2 (2-ply gradient)
        assert!(!specs.is_empty());

        // Check we have equal depth matchups
        let equal_depth: Vec<_> = specs.iter().filter(|s| s.depth1 == s.depth2).collect();
        assert!(equal_depth.len() >= 2, "Should have at least 2 equal-depth matchups");

        // Check we have asymmetric matchups for skill gradient
        let asymmetric: Vec<_> = specs.iter().filter(|s| s.depth1 != s.depth2).collect();
        assert!(asymmetric.len() >= 1, "Should have at least 1 asymmetric matchup for skill gradient");
    }

    #[test]
    fn test_build_matchup_specs_depth_2() {
        // Depth 2 is minimum, should only have d2 vs d2
        let specs = build_matchup_specs(2, 2, true);

        // At depth 2, we can only have equal-depth matchups
        // No tier-1 or tier-2 asymmetric matchups possible
        assert!(!specs.is_empty());
        for spec in &specs {
            assert!(spec.depth1 >= 2 && spec.depth2 >= 2);
        }
    }

    #[test]
    fn test_matchup_stats_win_rate() {
        let mut stats = MatchupStats::default();
        stats.deeper_depth = 4;
        stats.shallower_depth = 2;
        stats.games_played = 10;
        stats.deeper_wins = 8;
        stats.shallower_wins = 2;
        stats.white_wins = 5;
        stats.black_wins = 5;

        // 80% deeper win rate
        assert!((stats.deeper_win_rate() - 0.8).abs() < 0.01);
        // 50% white win rate
        assert!((stats.white_win_rate() - 0.5).abs() < 0.01);
    }
}
