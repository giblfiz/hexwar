//! Server state management
//!
//! Shared state for designer, playback, and game sessions.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

/// A piece in the designer format
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DesignerPiece {
    pub id: i64,
    #[serde(rename = "pieceId")]
    pub piece_id: String,
    pub color: String,
    pub pos: [i8; 2],
    pub facing: u8,
}

/// Designer graveyard state
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Graveyard {
    pub white: Vec<String>,
    pub black: Vec<String>,
}

/// Designer templates
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Templates {
    pub white: String,
    pub black: String,
}

impl Default for Templates {
    fn default() -> Self {
        Self {
            white: "E".to_string(),
            black: "E".to_string(),
        }
    }
}

/// Designer state
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct DesignerState {
    pub pieces: Vec<DesignerPiece>,
    pub graveyard: Graveyard,
    pub templates: Templates,
    pub version: u64,
    pub name: String,
}

/// Playback state
#[derive(Clone, Debug, Default)]
pub struct PlaybackState {
    pub active: bool,
    pub move_index: usize,
    pub total_moves: usize,
    pub current_player: u8,
    pub round_number: u32,
    pub winner: Option<u8>,
    pub end_reason: Option<String>,
    pub pieces: Vec<DesignerPiece>,
    pub moves: Vec<serde_json::Value>,
    pub initial_pieces: Vec<DesignerPiece>,
}

/// Interactive game state
#[derive(Clone, Debug, Default)]
pub struct GameSession {
    pub active: bool,
    pub current_player: u8,
    pub round_number: u32,
    pub current_action: String,
    pub winner: Option<u8>,
    pub pieces: Vec<DesignerPiece>,
    pub ai_depth: u32,
    pub player_side: u8,
    pub ruleset: Option<serde_json::Value>,
}

/// Server-wide shared state
pub struct ServerState {
    pub designer: RwLock<DesignerState>,
    pub playback: RwLock<PlaybackState>,
    pub games: RwLock<HashMap<String, GameSession>>,
    pub current_game: RwLock<GameSession>,
}

impl ServerState {
    pub fn new() -> Self {
        Self {
            designer: RwLock::new(DesignerState::default()),
            playback: RwLock::new(PlaybackState::default()),
            games: RwLock::new(HashMap::new()),
            current_game: RwLock::new(GameSession::default()),
        }
    }
}

impl Default for ServerState {
    fn default() -> Self {
        Self::new()
    }
}
