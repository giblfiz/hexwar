//! Interactive game API endpoints
//!
//! Start games, make moves, get AI moves.
//!
//! NOTE: Full game logic requires hexwar-core to be implemented.
//! For now, this provides stub responses that match the API contract.

use crate::state::{DesignerPiece, GameSession, ServerState};
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Deserialize)]
pub struct StartGameRequest {
    pub ruleset: Option<String>,
    pub player_side: Option<String>,
    pub ai_depth: Option<u32>,
}

/// Game state response (for future typed responses)
#[derive(Serialize)]
#[allow(dead_code)]
pub struct GameStateResponse {
    pub current_player: u8,
    pub round_number: u32,
    pub current_action: String,
    pub winner: Option<u8>,
    pub pieces: Vec<DesignerPiece>,
}

/// Move response (for future typed responses)
#[derive(Serialize)]
#[allow(dead_code)]
pub struct MoveResponse {
    pub action_type: String,
    pub from_pos: Option<[i8; 2]>,
    pub to_pos: Option<[i8; 2]>,
    pub new_facing: Option<u8>,
}

/// Start a new interactive game
pub async fn start_game(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<StartGameRequest>,
) -> Json<Value> {
    let player_side = req
        .player_side
        .as_deref()
        .map(|s| if s == "black" { 1u8 } else { 0u8 })
        .unwrap_or(0);
    let ai_depth = req.ai_depth.unwrap_or(4);

    // Load ruleset
    let ruleset = if let Some(ruleset_id) = &req.ruleset {
        load_ruleset(ruleset_id)
    } else {
        default_ruleset()
    };

    let pieces = ruleset_to_pieces(&ruleset);

    // Update game state
    {
        let mut game = state.current_game.write().unwrap();
        *game = GameSession {
            active: true,
            current_player: 0,
            round_number: 1,
            current_action: "MOVE".to_string(),
            winner: None,
            pieces: pieces.clone(),
            ai_depth,
            player_side,
            ruleset: Some(ruleset),
        };
    }

    // Generate legal moves (stub - would use hexwar-core)
    let legal_moves = generate_legal_moves_stub(&pieces, 0);

    Json(json!({
        "state": {
            "current_player": 0,
            "round_number": 1,
            "current_action": "MOVE",
            "winner": null,
            "pieces": pieces,
        },
        "legal_moves": legal_moves,
    }))
}

#[derive(Deserialize)]
pub struct MakeMoveRequest {
    #[serde(rename = "move")]
    pub mv: Value,
}

/// Apply a player's move to the game
pub async fn make_player_move(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<MakeMoveRequest>,
) -> Json<Value> {
    let mut game = state.current_game.write().unwrap();

    if !game.active {
        return Json(json!({ "error": "No active game" }));
    }

    // Apply the move
    apply_move_to_pieces(&mut game.pieces, &req.mv);

    // Advance turn (simplified - real logic in hexwar-core)
    game.current_player = 1 - game.current_player;
    if game.current_player == 0 {
        game.round_number += 1;
    }

    // Check for win (simplified - just check if kings exist)
    let white_king_exists = game
        .pieces
        .iter()
        .any(|p| p.color == "white" && p.piece_id.starts_with('K'));
    let black_king_exists = game
        .pieces
        .iter()
        .any(|p| p.color == "black" && p.piece_id.starts_with('K'));

    if !white_king_exists {
        game.winner = Some(1); // Black wins
    } else if !black_king_exists {
        game.winner = Some(0); // White wins
    }

    let legal_moves = if game.winner.is_none() {
        generate_legal_moves_stub(&game.pieces, game.current_player)
    } else {
        vec![]
    };

    Json(json!({
        "state": {
            "current_player": game.current_player,
            "round_number": game.round_number,
            "current_action": game.current_action,
            "winner": game.winner,
            "pieces": game.pieces,
        },
        "legal_moves": legal_moves,
    }))
}

#[derive(Deserialize)]
pub struct AiMoveRequest {
    pub depth: Option<u32>,
}

/// Get the AI's move for the current position
pub async fn get_ai_move(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<AiMoveRequest>,
) -> Json<Value> {
    let mut game = state.current_game.write().unwrap();

    if !game.active {
        return Json(json!({ "error": "No active game" }));
    }

    if game.winner.is_some() {
        return Json(json!({ "error": "Game already finished" }));
    }

    let _depth = req.depth.unwrap_or(game.ai_depth);

    // Generate a simple AI move (stub - real AI in hexwar-core)
    // For now, just pick a random legal-looking move
    let ai_color = if game.current_player == 0 {
        "white"
    } else {
        "black"
    };

    // Find a piece that can move
    let ai_move = game
        .pieces
        .iter()
        .find(|p| p.color == ai_color)
        .map(|piece| {
            // Simple move: try to move forward one hex
            let dir = if ai_color == "white" { -1i8 } else { 1i8 };
            let new_pos = [piece.pos[0], piece.pos[1] + dir];

            // Check if destination is valid and unoccupied by friendly
            let occupied_by_friendly = game
                .pieces
                .iter()
                .any(|p| p.pos == new_pos && p.color == ai_color);

            if !occupied_by_friendly && is_valid_hex(new_pos) {
                json!({
                    "action_type": "MOVE",
                    "from_pos": piece.pos,
                    "to_pos": new_pos,
                    "new_facing": piece.facing,
                })
            } else {
                // Fall back to PASS
                json!({
                    "action_type": "PASS",
                    "from_pos": null,
                    "to_pos": null,
                    "new_facing": null,
                })
            }
        })
        .unwrap_or_else(|| {
            json!({
                "action_type": "PASS",
                "from_pos": null,
                "to_pos": null,
                "new_facing": null,
            })
        });

    // Apply the AI move
    apply_move_to_pieces(&mut game.pieces, &ai_move);

    // Advance turn
    game.current_player = 1 - game.current_player;
    if game.current_player == 0 {
        game.round_number += 1;
    }

    // Check for win
    let white_king_exists = game
        .pieces
        .iter()
        .any(|p| p.color == "white" && p.piece_id.starts_with('K'));
    let black_king_exists = game
        .pieces
        .iter()
        .any(|p| p.color == "black" && p.piece_id.starts_with('K'));

    if !white_king_exists {
        game.winner = Some(1);
    } else if !black_king_exists {
        game.winner = Some(0);
    }

    let legal_moves = if game.winner.is_none() {
        generate_legal_moves_stub(&game.pieces, game.current_player)
    } else {
        vec![]
    };

    Json(json!({
        "state": {
            "current_player": game.current_player,
            "round_number": game.round_number,
            "current_action": game.current_action,
            "winner": game.winner,
            "pieces": game.pieces,
        },
        "legal_moves": legal_moves,
        "move": ai_move,
    }))
}

/// Load ruleset from file or return default
fn load_ruleset(ruleset_id: &str) -> Value {
    let base_dir = std::env::current_dir().unwrap_or_else(|_| std::path::PathBuf::from("."));

    // Check if it's a known ruleset ID
    if ruleset_id == "copper-pass" {
        let path = base_dir
            .join("board_sets")
            .join("d7_firstarrow_seeds")
            .join("copper-pass.json");
        if let Ok(content) = std::fs::read_to_string(&path) {
            if let Ok(data) = serde_json::from_str::<Value>(&content) {
                if let Some(ruleset) = data.get("ruleset") {
                    return ruleset.clone();
                }
                return data;
            }
        }
    }

    // Try as file path
    if let Ok(content) = std::fs::read_to_string(ruleset_id) {
        if let Ok(data) = serde_json::from_str::<Value>(&content) {
            if let Some(ruleset) = data.get("ruleset") {
                return ruleset.clone();
            }
            return data;
        }
    }

    // Return default
    default_ruleset()
}

/// Default ruleset for testing
fn default_ruleset() -> Value {
    json!({
        "white_king": "K1",
        "black_king": "K1",
        "white_pieces": ["D1", "D1", "E1", "A1", "A1"],
        "black_pieces": ["D1", "D1", "E1", "A1", "A1"],
        "white_positions": [[0, 3], [-1, 3], [1, 3], [0, 4], [-2, 4], [2, 4]],
        "black_positions": [[0, -3], [-1, -3], [1, -3], [0, -4], [-2, -4], [2, -4]],
        "white_facings": [0, 0, 0, 0, 0, 0],
        "black_facings": [3, 3, 3, 3, 3, 3],
        "white_template": "E",
        "black_template": "E",
    })
}

/// Convert ruleset to pieces
fn ruleset_to_pieces(ruleset: &Value) -> Vec<DesignerPiece> {
    let mut pieces = Vec::new();

    let white_positions = ruleset
        .get("white_positions")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let black_positions = ruleset
        .get("black_positions")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let white_pieces = ruleset
        .get("white_pieces")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let black_pieces = ruleset
        .get("black_pieces")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let white_facings = ruleset
        .get("white_facings")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let black_facings = ruleset
        .get("black_facings")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let white_king = ruleset
        .get("white_king")
        .and_then(|v| v.as_str())
        .unwrap_or("K1");
    let black_king = ruleset
        .get("black_king")
        .and_then(|v| v.as_str())
        .unwrap_or("K1");

    // Add white king
    if !white_positions.is_empty() {
        pieces.push(DesignerPiece {
            id: 1,
            piece_id: white_king.to_string(),
            color: "white".to_string(),
            pos: parse_pos(&white_positions[0]),
            facing: white_facings
                .first()
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as u8,
        });
    }

    // Add white pieces
    for (i, piece) in white_pieces.iter().enumerate() {
        if i + 1 < white_positions.len() {
            pieces.push(DesignerPiece {
                id: 100 + i as i64,
                piece_id: piece.as_str().unwrap_or("A1").to_string(),
                color: "white".to_string(),
                pos: parse_pos(&white_positions[i + 1]),
                facing: white_facings
                    .get(i + 1)
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u8,
            });
        }
    }

    // Add black king
    if !black_positions.is_empty() {
        pieces.push(DesignerPiece {
            id: 2,
            piece_id: black_king.to_string(),
            color: "black".to_string(),
            pos: parse_pos(&black_positions[0]),
            facing: black_facings
                .first()
                .and_then(|v| v.as_u64())
                .unwrap_or(3) as u8,
        });
    }

    // Add black pieces
    for (i, piece) in black_pieces.iter().enumerate() {
        if i + 1 < black_positions.len() {
            pieces.push(DesignerPiece {
                id: 200 + i as i64,
                piece_id: piece.as_str().unwrap_or("A1").to_string(),
                color: "black".to_string(),
                pos: parse_pos(&black_positions[i + 1]),
                facing: black_facings
                    .get(i + 1)
                    .and_then(|v| v.as_u64())
                    .unwrap_or(3) as u8,
            });
        }
    }

    pieces
}

/// Parse position from JSON value
fn parse_pos(pos: &Value) -> [i8; 2] {
    if let Some(arr) = pos.as_array() {
        [
            arr.get(0).and_then(|v| v.as_i64()).unwrap_or(0) as i8,
            arr.get(1).and_then(|v| v.as_i64()).unwrap_or(0) as i8,
        ]
    } else {
        [0, 0]
    }
}

/// Check if a hex position is valid
fn is_valid_hex(pos: [i8; 2]) -> bool {
    let q = pos[0];
    let r = pos[1];
    q.abs() <= 4 && r.abs() <= 4 && (q + r).abs() <= 4
}

/// Generate legal moves (stub implementation)
fn generate_legal_moves_stub(pieces: &[DesignerPiece], current_player: u8) -> Vec<Value> {
    let color = if current_player == 0 {
        "white"
    } else {
        "black"
    };
    let mut moves = Vec::new();

    // For each piece of current player, generate simple moves
    for piece in pieces.iter().filter(|p| p.color == color) {
        // Direction vectors
        let dirs: [(i8, i8); 6] = [
            (0, -1),  // N
            (1, -1),  // NE
            (1, 0),   // SE
            (0, 1),   // S
            (-1, 1),  // SW
            (-1, 0),  // NW
        ];

        // Generate move in each direction
        for (dq, dr) in dirs.iter() {
            let new_pos = [piece.pos[0] + dq, piece.pos[1] + dr];

            if !is_valid_hex(new_pos) {
                continue;
            }

            // Check if occupied by friendly
            let blocked = pieces.iter().any(|p| p.pos == new_pos && p.color == color);
            if blocked {
                continue;
            }

            // Check if capture
            let is_capture = pieces.iter().any(|p| p.pos == new_pos && p.color != color);

            moves.push(json!({
                "action_type": "MOVE",
                "from_pos": piece.pos,
                "to_pos": new_pos,
                "new_facing": piece.facing,
                "is_capture": is_capture,
            }));
        }

        // Generate rotation moves
        for new_facing in 0..6u8 {
            if new_facing != piece.facing {
                moves.push(json!({
                    "action_type": "ROTATE",
                    "from_pos": piece.pos,
                    "to_pos": null,
                    "new_facing": new_facing,
                }));
            }
        }
    }

    // Always include PASS
    moves.push(json!({
        "action_type": "PASS",
        "from_pos": null,
        "to_pos": null,
        "new_facing": null,
    }));

    moves
}

/// Apply a move to the pieces list
fn apply_move_to_pieces(pieces: &mut Vec<DesignerPiece>, mv: &Value) {
    let action_type = mv.get("action_type").and_then(|v| v.as_str()).unwrap_or("");

    match action_type {
        "MOVE" | "MOVEMENT" => {
            let from_pos = mv.get("from_pos").map(parse_pos).unwrap_or([0, 0]);
            let to_pos = mv.get("to_pos").map(parse_pos).unwrap_or([0, 0]);
            let new_facing = mv.get("new_facing").and_then(|v| v.as_u64());

            // Remove any piece at destination
            pieces.retain(|p| p.pos != to_pos);

            // Move piece from source to destination
            for piece in pieces.iter_mut() {
                if piece.pos == from_pos {
                    piece.pos = to_pos;
                    if let Some(f) = new_facing {
                        piece.facing = f as u8;
                    }
                    break;
                }
            }
        }
        "ROTATE" => {
            let pos = mv.get("from_pos").map(parse_pos).unwrap_or([0, 0]);
            let new_facing = mv.get("new_facing").and_then(|v| v.as_u64()).unwrap_or(0) as u8;

            for piece in pieces.iter_mut() {
                if piece.pos == pos {
                    piece.facing = new_facing;
                    break;
                }
            }
        }
        "SWAP" => {
            let from_pos = mv.get("from_pos").map(parse_pos).unwrap_or([0, 0]);
            let target_pos = mv.get("to_pos").map(parse_pos).unwrap_or([0, 0]);

            let mut from_idx = None;
            let mut target_idx = None;
            for (i, piece) in pieces.iter().enumerate() {
                if piece.pos == from_pos {
                    from_idx = Some(i);
                }
                if piece.pos == target_pos {
                    target_idx = Some(i);
                }
            }

            if let (Some(fi), Some(ti)) = (from_idx, target_idx) {
                let temp = pieces[fi].pos;
                pieces[fi].pos = pieces[ti].pos;
                pieces[ti].pos = temp;
            }
        }
        "PASS" | "SURRENDER" => {
            // No board change
        }
        _ => {}
    }
}
