//! RuleSet - Army composition definition

use crate::board::Hex;
use crate::game::{GameState, Template};
use crate::pieces::{piece_id_to_index, PieceTypeId};
use rand::{Rng, SeedableRng};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Army composition for evolution
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
    /// Convert to GameState
    pub fn to_game_state(&self) -> GameState {
        let mut white_setup: Vec<(PieceTypeId, Hex, u8)> = Vec::new();
        let mut black_setup: Vec<(PieceTypeId, Hex, u8)> = Vec::new();

        // Add kings
        if !self.white_positions.is_empty() {
            white_setup.push((self.white_king, self.white_positions[0], self.white_facings.get(0).copied().unwrap_or(0)));
        }
        if !self.black_positions.is_empty() {
            black_setup.push((self.black_king, self.black_positions[0], self.black_facings.get(0).copied().unwrap_or(3)));
        }

        // Add other pieces
        for (i, &piece_type) in self.white_pieces.iter().enumerate() {
            if i + 1 < self.white_positions.len() {
                let pos = self.white_positions[i + 1];
                let facing = self.white_facings.get(i + 1).copied().unwrap_or(0);
                white_setup.push((piece_type, pos, facing));
            }
        }
        for (i, &piece_type) in self.black_pieces.iter().enumerate() {
            if i + 1 < self.black_positions.len() {
                let pos = self.black_positions[i + 1];
                let facing = self.black_facings.get(i + 1).copied().unwrap_or(3);
                black_setup.push((piece_type, pos, facing));
            }
        }

        GameState::new(
            &white_setup,
            &black_setup,
            self.white_template,
            self.black_template,
        )
    }

    /// Load from JSON file (handles both flat and wrapped formats, string and numeric IDs)
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;

        // Try parsing as flat RuleSet first (numeric IDs)
        if let Ok(ruleset) = serde_json::from_str::<RuleSet>(&content) {
            return Ok(ruleset);
        }

        // Try parsing as wrapped format with string IDs
        #[derive(Deserialize)]
        struct StringRuleSet {
            name: Option<String>,
            white_king: String,
            white_pieces: Vec<String>,
            white_positions: Vec<Hex>,
            white_facings: Vec<u8>,
            white_template: Template,
            black_king: String,
            black_pieces: Vec<String>,
            black_positions: Vec<Hex>,
            black_facings: Vec<u8>,
            black_template: Template,
        }

        #[derive(Deserialize)]
        struct Wrapped {
            name: Option<String>,
            ruleset: StringRuleSet,
        }

        fn convert_string_ruleset(sr: StringRuleSet, name_override: Option<String>) -> anyhow::Result<RuleSet> {
            let white_king = piece_id_to_index(&sr.white_king)
                .ok_or_else(|| anyhow::anyhow!("Unknown piece ID: {}", sr.white_king))?;
            let black_king = piece_id_to_index(&sr.black_king)
                .ok_or_else(|| anyhow::anyhow!("Unknown piece ID: {}", sr.black_king))?;

            let white_pieces: Result<Vec<_>, _> = sr.white_pieces.iter()
                .map(|id| piece_id_to_index(id).ok_or_else(|| anyhow::anyhow!("Unknown piece ID: {}", id)))
                .collect();
            let black_pieces: Result<Vec<_>, _> = sr.black_pieces.iter()
                .map(|id| piece_id_to_index(id).ok_or_else(|| anyhow::anyhow!("Unknown piece ID: {}", id)))
                .collect();

            Ok(RuleSet {
                name: name_override.or(sr.name).unwrap_or_else(|| "unnamed".to_string()),
                white_king,
                white_pieces: white_pieces?,
                white_positions: sr.white_positions,
                white_facings: sr.white_facings,
                white_template: sr.white_template,
                black_king,
                black_pieces: black_pieces?,
                black_positions: sr.black_positions,
                black_facings: sr.black_facings,
                black_template: sr.black_template,
            })
        }

        // Try wrapped format with string IDs
        if let Ok(wrapped) = serde_json::from_str::<Wrapped>(&content) {
            return convert_string_ruleset(wrapped.ruleset, wrapped.name);
        }

        // Try flat format with string IDs
        if let Ok(sr) = serde_json::from_str::<StringRuleSet>(&content) {
            return convert_string_ruleset(sr, None);
        }

        // Fall back to original error for debugging
        let ruleset: RuleSet = serde_json::from_str(&content)?;
        Ok(ruleset)
    }

    /// Save to JSON file
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
    }

    /// Generate a random symmetric ruleset with the given number of pieces per side
    /// Both sides get identical armies, just mirrored positions
    pub fn random_symmetric<R: Rng>(rng: &mut R, name: &str, num_pieces: usize) -> Self {
        // Non-king pieces are indices 0-24
        const NON_KING_PIECES: usize = 25;
        // Kings are indices 25-29
        const KING_START: u8 = 25;
        const NUM_KINGS: u8 = 5;

        // Pick a random king type
        let king_type = KING_START + (rng.gen::<u8>() % NUM_KINGS);

        // Pick random non-king pieces
        let pieces: Vec<PieceTypeId> = (0..num_pieces)
            .map(|_| rng.gen::<u8>() % NON_KING_PIECES as u8)
            .collect();

        // Generate positions for white (Template D positions)
        // Template D has hex positions in rows 1-4 on white's side
        let white_positions = Self::generate_positions_template_d(num_pieces + 1, true);
        let black_positions = Self::generate_positions_template_d(num_pieces + 1, false);

        // Facings: white faces 0 (up), black faces 3 (down)
        let white_facings = vec![0; num_pieces + 1];
        let black_facings = vec![3; num_pieces + 1];

        Self {
            name: name.to_string(),
            white_king: king_type,
            white_pieces: pieces.clone(),
            white_positions,
            white_facings,
            white_template: Template::E,
            black_king: king_type,
            black_pieces: pieces,
            black_positions,
            black_facings,
            black_template: Template::E,
        }
    }

    /// Generate positions for Template D (standard setup)
    /// King is buried at the back center (row 5), with pieces forming a protective wall
    fn generate_positions_template_d(count: usize, is_white: bool) -> Vec<Hex> {
        // Defensive formation: king at very back, pieces in front
        // Row 5: King (back center)
        // Row 4: Two guards flanking
        // Row 3: Three pieces in front line
        // Row 2: Forward scouts
        let base_positions = vec![
            (0, 5),   // King position (very back center) - PROTECTED
            (-1, 4),  // Guard left of king
            (1, 4),   // Guard right of king
            (0, 4),   // Guard directly in front of king
            (-2, 3),  // Left flank
            (2, 3),   // Right flank
            (0, 3),   // Center front
            (-1, 3),  // Left center front
            (1, 3),   // Right center front
            (-1, 2),  // Forward scout left
            (1, 2),   // Forward scout right
            (0, 2),   // Forward scout center
            (-2, 2),  // Far left scout
        ];

        let sign = if is_white { 1 } else { -1 };
        base_positions
            .iter()
            .take(count)
            .map(|&(q, r)| Hex::new(q, r * sign))
            .collect()
    }

    /// Named chaos ruleset (seed 12345) - 6 random pieces
    pub fn chaos() -> Self {
        let mut rng = rand::rngs::StdRng::seed_from_u64(12345);
        Self::random_symmetric(&mut rng, "chaos", 6)
    }

    /// Named kaos ruleset (seed 54321) - 6 random pieces
    pub fn kaos() -> Self {
        let mut rng = rand::rngs::StdRng::seed_from_u64(54321);
        Self::random_symmetric(&mut rng, "kaos", 6)
    }
}

impl Default for RuleSet {
    fn default() -> Self {
        Self {
            name: "default".to_string(),
            white_king: 25,  // K1
            white_pieces: vec![1, 1, 1, 1],  // 4 Guards
            white_positions: vec![
                Hex::new(0, 3),   // King
                Hex::new(-1, 3),  // Guard 1
                Hex::new(1, 2),   // Guard 2
                Hex::new(-2, 4),  // Guard 3
                Hex::new(2, 1),   // Guard 4
            ],
            white_facings: vec![0; 5],
            white_template: Template::E,
            black_king: 25,  // K1
            black_pieces: vec![1, 1, 1, 1],  // 4 Guards
            black_positions: vec![
                Hex::new(0, -3),  // King
                Hex::new(1, -3),  // Guard 1
                Hex::new(-1, -2), // Guard 2
                Hex::new(2, -4),  // Guard 3
                Hex::new(-2, -1), // Guard 4
            ],
            black_facings: vec![3; 5],
            black_template: Template::E,
        }
    }
}
