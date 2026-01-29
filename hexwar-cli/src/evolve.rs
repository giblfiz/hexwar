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

    tracing::info!("Starting evolution: pop={}, gen={}, depth={}",
        args.population, args.generations, args.depth);

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

    Ok(())
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
/// NOTE: This is a placeholder that uses a simple heuristic-based fitness.
/// For production use, this should integrate with hexwar-tournament for
/// actual game-based fitness evaluation.
fn create_fitness_fn(args: &EvolveArgs) -> impl Fn(&RuleSet) -> f32 {
    let _depth = args.depth;
    let _games = args.games;
    let _max_rounds = args.max_rounds;

    // Placeholder fitness function based on piece diversity and count
    // TODO: Replace with actual tournament-based evaluation when hexwar-tournament
    // has the match_play module implemented
    move |rs: &RuleSet| {
        // Simple heuristic: more diverse and numerous pieces = higher fitness
        let white_diversity = count_unique_pieces(&rs.white_pieces) as f32;
        let black_diversity = count_unique_pieces(&rs.black_pieces) as f32;
        let piece_count = (rs.white_pieces.len() + rs.black_pieces.len()) as f32;

        // Fitness: diversity + piece count (normalized)
        (white_diversity + black_diversity) * 0.1 + piece_count * 0.05
    }
}

/// Count unique piece types in a list
fn count_unique_pieces(pieces: &[u8]) -> usize {
    let mut seen = std::collections::HashSet::new();
    for &p in pieces {
        seen.insert(p);
    }
    seen.len()
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
        };

        assert!(matches!(determine_evolve_side(&args), MutateSide::Both));
    }

    #[test]
    fn test_count_unique_pieces() {
        assert_eq!(count_unique_pieces(&[1, 1, 2, 2, 3]), 3);
        assert_eq!(count_unique_pieces(&[1, 1, 1, 1]), 1);
        assert_eq!(count_unique_pieces(&[]), 0);
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
}
