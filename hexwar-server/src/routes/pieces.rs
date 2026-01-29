//! Piece types endpoint
//!
//! Returns all piece type definitions for the UI.

use axum::Json;
use serde::Serialize;
use serde_json::{json, Value};
use std::collections::HashMap;

/// Piece type info for the UI
#[derive(Serialize)]
pub struct PieceTypeInfo {
    pub id: &'static str,
    pub name: &'static str,
    pub move_type: &'static str,
    pub move_range: u8,
    pub directions: Vec<u8>,
    pub special: Option<&'static str>,
    pub is_king: bool,
}

/// Get all piece type definitions
pub async fn get_pieces() -> Json<HashMap<String, Value>> {
    let mut pieces = HashMap::new();

    // Define all piece types matching the Python server
    let piece_defs = [
        // Step-1 (A series)
        ("A1", "Pawn", "STEP", 1, vec![0], None, false),
        ("A2", "Guard", "STEP", 1, vec![0, 1, 2, 3, 4, 5], None, false),
        ("A3", "Scout", "STEP", 1, vec![0, 1, 5], None, false),
        ("A4", "Crab", "STEP", 1, vec![1, 3, 5], None, false),
        ("A5", "Flanker", "STEP", 1, vec![1, 5], None, false),
        // Step-2 (B series)
        ("B1", "Strider", "STEP", 2, vec![0], None, false),
        ("B2", "Dancer", "STEP", 2, vec![1, 5], None, false),
        ("B3", "Ranger", "STEP", 2, vec![0, 1, 2, 3, 4, 5], None, false),
        ("B4", "Hound", "STEP", 2, vec![0, 1, 5], None, false),
        // Step-3 (C series)
        ("C1", "Lancer", "STEP", 3, vec![0], None, false),
        ("C2", "Dragoon", "STEP", 3, vec![0, 1, 5], None, false),
        ("C3", "Courser", "STEP", 3, vec![0, 1, 2, 3, 4, 5], None, false),
        // Slide (D series)
        ("D1", "Pike", "SLIDE", 9, vec![0], None, false),
        ("D2", "Rook", "SLIDE", 9, vec![0, 3], None, false),
        ("D3", "Bishop", "SLIDE", 9, vec![1, 2, 4, 5], None, false),
        ("D4", "Chariot", "SLIDE", 9, vec![0, 1, 5], None, false),
        ("D5", "Queen", "SLIDE", 9, vec![0, 1, 2, 3, 4, 5], None, false),
        // Jump (E/F series)
        ("E1", "Knight", "JUMP", 2, vec![0, 1, 5], None, false),
        ("E2", "Frog", "JUMP", 2, vec![0, 1, 2, 3, 4, 5], None, false),
        ("F1", "Locust", "JUMP", 3, vec![0, 1, 5], None, false),
        ("F2", "Cricket", "JUMP", 3, vec![0, 1, 2, 3, 4, 5], None, false),
        // Special
        ("W1", "Warper", "NONE", 0, vec![], Some("SWAP_MOVE"), false),
        (
            "W2",
            "Shifter",
            "STEP",
            1,
            vec![0, 1, 2, 3, 4, 5],
            Some("SWAP_ROTATE"),
            false,
        ),
        ("P1", "Phoenix", "STEP", 1, vec![0, 1, 5], Some("REBIRTH"), false),
        ("G1", "Ghost", "STEP", 1, vec![0, 1, 2, 3, 4, 5], Some("PHASED"), false),
        // Kings
        ("K1", "King Guard", "STEP", 1, vec![0, 1, 2, 3, 4, 5], None, true),
        ("K2", "King Scout", "STEP", 1, vec![0, 1, 5], None, true),
        ("K3", "King Ranger", "STEP", 2, vec![0, 1, 2, 3, 4, 5], None, true),
        ("K4", "King Frog", "JUMP", 2, vec![0, 1, 2, 3, 4, 5], None, true),
        ("K5", "King Pike", "SLIDE", 9, vec![0], None, true),
    ];

    for (id, name, move_type, move_range, directions, special, is_king) in piece_defs {
        pieces.insert(
            id.to_string(),
            json!({
                "id": id,
                "name": name,
                "move_type": move_type,
                "move_range": move_range,
                "directions": directions,
                "special": special,
                "is_king": is_king,
            }),
        );
    }

    Json(pieces)
}
