//! RuleSet - Army composition definition

use crate::board::Hex;
use crate::game::{GameState, Template};
use crate::pieces::PieceTypeId;
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

    /// Load from JSON file
    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let content = std::fs::read_to_string(path)?;
        let ruleset: RuleSet = serde_json::from_str(&content)?;
        Ok(ruleset)
    }

    /// Save to JSON file
    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let content = serde_json::to_string_pretty(self)?;
        std::fs::write(path, content)?;
        Ok(())
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
            white_template: Template::D,
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
            black_template: Template::D,
        }
    }
}
