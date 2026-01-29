//! Selection operators for genetic algorithms
//!
//! Implements tournament selection where individuals compete
//! in small tournaments, with the winner being selected for breeding.

use hexwar_core::RuleSet;
use rand::Rng;

/// Tournament selection: select an individual by running a tournament.
///
/// Randomly picks `tournament_size` individuals from the population,
/// then returns the one with the highest fitness.
///
/// # Arguments
/// * `population` - Slice of rulesets to select from
/// * `fitness` - Fitness scores corresponding to each ruleset (higher = better)
/// * `tournament_size` - Number of individuals in each tournament
/// * `rng` - Random number generator
///
/// # Returns
/// Reference to the winning ruleset
///
/// # Panics
/// Panics if population is empty or tournament_size is 0
pub fn tournament_select<'a, R: Rng>(
    population: &'a [RuleSet],
    fitness: &[f32],
    tournament_size: usize,
    rng: &mut R,
) -> &'a RuleSet {
    assert!(!population.is_empty(), "Population cannot be empty");
    assert!(tournament_size > 0, "Tournament size must be > 0");
    assert_eq!(population.len(), fitness.len(), "Population and fitness must have same length");

    let tournament_size = tournament_size.min(population.len());

    // Select tournament participants
    let mut best_idx = rng.gen_range(0..population.len());
    let mut best_fitness = fitness[best_idx];

    for _ in 1..tournament_size {
        let idx = rng.gen_range(0..population.len());
        if fitness[idx] > best_fitness {
            best_idx = idx;
            best_fitness = fitness[idx];
        }
    }

    &population[best_idx]
}

/// Select multiple individuals via tournament selection.
///
/// # Arguments
/// * `population` - Slice of rulesets to select from
/// * `fitness` - Fitness scores corresponding to each ruleset
/// * `count` - Number of individuals to select
/// * `tournament_size` - Number of individuals in each tournament
/// * `rng` - Random number generator
///
/// # Returns
/// Vector of references to selected rulesets
pub fn tournament_select_many<'a, R: Rng>(
    population: &'a [RuleSet],
    fitness: &[f32],
    count: usize,
    tournament_size: usize,
    rng: &mut R,
) -> Vec<&'a RuleSet> {
    (0..count)
        .map(|_| tournament_select(population, fitness, tournament_size, rng))
        .collect()
}

/// Select the top N individuals by fitness (elitism).
///
/// Returns indices of the best individuals, sorted by fitness (descending).
///
/// # Arguments
/// * `fitness` - Fitness scores
/// * `n` - Number of elite individuals to select
///
/// # Returns
/// Vector of indices of the top N individuals
pub fn select_elite(fitness: &[f32], n: usize) -> Vec<usize> {
    let mut indices: Vec<usize> = (0..fitness.len()).collect();
    indices.sort_by(|&a, &b| {
        fitness[b].partial_cmp(&fitness[a]).unwrap_or(std::cmp::Ordering::Equal)
    });
    indices.truncate(n);
    indices
}

#[cfg(test)]
mod tests {
    use super::*;
    use hexwar_core::board::Hex;
    use hexwar_core::game::Template;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    fn make_test_ruleset(id: u8) -> RuleSet {
        RuleSet {
            name: format!("test-{}", id),
            white_king: 25,
            white_pieces: vec![id, id, id, id],
            white_positions: vec![
                Hex::new(0, 3),
                Hex::new(-1, 3),
                Hex::new(1, 2),
                Hex::new(-2, 4),
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
                Hex::new(2, -4),
                Hex::new(-2, -1),
            ],
            black_facings: vec![3; 5],
            black_template: Template::E,
        }
    }

    #[test]
    fn test_tournament_select_returns_higher_fitness() {
        let population: Vec<RuleSet> = (0..10).map(|i| make_test_ruleset(i)).collect();
        let fitness: Vec<f32> = (0..10).map(|i| i as f32).collect();
        let mut rng = ChaCha8Rng::seed_from_u64(42);

        // Run many selections, track how often we get high-fitness individuals
        let mut high_count = 0;
        for _ in 0..100 {
            let selected = tournament_select(&population, &fitness, 3, &mut rng);
            // The selected ruleset's "id" is stored in white_pieces[0]
            let id = selected.white_pieces[0];
            if id >= 7 {
                high_count += 1;
            }
        }

        // With tournament size 3, we should heavily favor high-fitness individuals
        assert!(high_count > 50, "Tournament selection should favor high fitness, got {}", high_count);
    }

    #[test]
    fn test_select_elite() {
        let fitness = vec![0.5, 0.9, 0.3, 0.7, 0.1];
        let elite = select_elite(&fitness, 3);

        assert_eq!(elite.len(), 3);
        assert_eq!(elite[0], 1);  // 0.9
        assert_eq!(elite[1], 3);  // 0.7
        assert_eq!(elite[2], 0);  // 0.5
    }

    #[test]
    fn test_select_elite_handles_small_pop() {
        let fitness = vec![0.5, 0.9];
        let elite = select_elite(&fitness, 5);

        assert_eq!(elite.len(), 2);  // Can't select more than population
    }

    #[test]
    fn test_tournament_select_many() {
        let population: Vec<RuleSet> = (0..5).map(|i| make_test_ruleset(i)).collect();
        let fitness = vec![0.1, 0.2, 0.3, 0.4, 0.5];
        let mut rng = ChaCha8Rng::seed_from_u64(42);

        let selected = tournament_select_many(&population, &fitness, 10, 2, &mut rng);
        assert_eq!(selected.len(), 10);
    }
}
