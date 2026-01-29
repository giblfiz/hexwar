//! Game playback API endpoints
//!
//! Allows loading game records and stepping through moves.

use crate::state::{DesignerPiece, PlaybackState, ServerState};
use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

#[derive(Serialize)]
pub struct PlaybackStateResponse {
    pub active: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub move_index: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub total_moves: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at_start: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub at_end: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub current_player: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub round_number: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub winner: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pieces: Option<Vec<DesignerPiece>>,
}

/// Get current playback state
pub async fn get_playback_state(State(state): State<Arc<ServerState>>) -> Json<PlaybackStateResponse> {
    let playback = state.playback.read().unwrap();

    if !playback.active {
        return Json(PlaybackStateResponse {
            active: false,
            move_index: None,
            total_moves: None,
            at_start: None,
            at_end: None,
            current_player: None,
            round_number: None,
            winner: None,
            pieces: None,
        });
    }

    Json(PlaybackStateResponse {
        active: true,
        move_index: Some(playback.move_index),
        total_moves: Some(playback.total_moves),
        at_start: Some(playback.move_index == 0),
        at_end: Some(playback.move_index >= playback.total_moves),
        current_player: Some(playback.current_player),
        round_number: Some(playback.round_number),
        winner: if playback.move_index >= playback.total_moves {
            playback.winner
        } else {
            None
        },
        pieces: Some(playback.pieces.clone()),
    })
}

#[derive(Deserialize)]
pub struct LoadGameRequest {
    pub path: Option<String>,
    pub record: Option<Value>,
    pub moves: Option<Vec<Value>>,
}

/// Load a game record for playback
pub async fn load_game_record(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<Value>,
) -> Json<Value> {
    // Try to get game record from various formats
    let record = if let Some(path) = req.get("path").and_then(|v| v.as_str()) {
        // Load from file
        match std::fs::read_to_string(path) {
            Ok(content) => match serde_json::from_str::<Value>(&content) {
                Ok(data) => data,
                Err(e) => return Json(json!({ "error": format!("Failed to parse JSON: {}", e) })),
            },
            Err(e) => return Json(json!({ "error": format!("Failed to read file: {}", e) })),
        }
    } else if let Some(record) = req.get("record") {
        record.clone()
    } else if req.get("moves").is_some() {
        // Direct game record format
        req.clone()
    } else {
        return Json(json!({
            "error": "Must provide \"path\", \"record\", or direct game record with \"moves\""
        }));
    };

    // Extract moves and initial state
    let moves = record
        .get("moves")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let total_moves = moves.len();
    let winner = record.get("winner").and_then(|v| v.as_u64()).map(|v| v as u8);
    let end_reason = record
        .get("end_reason")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // Extract initial pieces from ruleset
    let initial_pieces = if let Some(ruleset) = record.get("ruleset") {
        ruleset_to_pieces(ruleset)
    } else if let Some(initial_state) = record.get("initial_state") {
        state_to_pieces(initial_state)
    } else {
        Vec::new()
    };

    // Update playback state
    {
        let mut playback = state.playback.write().unwrap();
        playback.active = true;
        playback.move_index = 0;
        playback.total_moves = total_moves;
        playback.current_player = 0;
        playback.round_number = 1;
        playback.winner = winner;
        playback.end_reason = end_reason.clone();
        playback.pieces = initial_pieces.clone();
        playback.moves = moves;
        playback.initial_pieces = initial_pieces;
    }

    Json(json!({
        "success": true,
        "total_moves": total_moves,
        "winner": winner,
        "end_reason": end_reason,
    }))
}

/// Step forward one move
pub async fn playback_forward(State(state): State<Arc<ServerState>>) -> Json<Value> {
    let mut playback = state.playback.write().unwrap();

    if !playback.active {
        return Json(json!({ "error": "No active playback" }));
    }

    if playback.move_index >= playback.total_moves {
        return Json(json!({ "error": "Already at end", "at_end": true }));
    }

    // Apply the move at current index
    let move_data = playback.moves.get(playback.move_index).cloned();
    if let Some(mv) = move_data {
        apply_move_to_pieces(&mut playback.pieces, &mv);
    }

    playback.move_index += 1;

    // Update player and round
    // In a 2-action-per-turn game, we alternate every action
    playback.current_player = (playback.move_index % 2) as u8;
    playback.round_number = (playback.move_index / 2) as u32 + 1;

    drop(playback);
    get_playback_state_internal(&state)
}

/// Step backward one move
pub async fn playback_backward(State(state): State<Arc<ServerState>>) -> Json<Value> {
    // To go backward, we need to rebuild from initial state
    let (target_index, total_moves, moves, initial_pieces) = {
        let playback = state.playback.read().unwrap();
        if !playback.active {
            return Json(json!({ "error": "No active playback" }));
        }
        if playback.move_index == 0 {
            return Json(json!({ "error": "Already at start", "at_start": true }));
        }
        (
            playback.move_index - 1,
            playback.total_moves,
            playback.moves.clone(),
            playback.initial_pieces.clone(),
        )
    };

    // Rebuild state from scratch
    let mut pieces = initial_pieces;
    for i in 0..target_index {
        if let Some(mv) = moves.get(i) {
            apply_move_to_pieces(&mut pieces, mv);
        }
    }

    {
        let mut playback = state.playback.write().unwrap();
        playback.move_index = target_index;
        playback.pieces = pieces;
        playback.current_player = (target_index % 2) as u8;
        playback.round_number = (target_index / 2) as u32 + 1;
    }

    get_playback_state_internal(&state)
}

#[derive(Deserialize)]
pub struct GotoRequest {
    pub index: usize,
}

/// Jump to specific move index
pub async fn playback_goto(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<GotoRequest>,
) -> Json<Value> {
    let (total_moves, moves, initial_pieces) = {
        let playback = state.playback.read().unwrap();
        if !playback.active {
            return Json(json!({ "error": "No active playback" }));
        }
        (
            playback.total_moves,
            playback.moves.clone(),
            playback.initial_pieces.clone(),
        )
    };

    let target_index = req.index.min(total_moves);

    // Rebuild state from scratch
    let mut pieces = initial_pieces;
    for i in 0..target_index {
        if let Some(mv) = moves.get(i) {
            apply_move_to_pieces(&mut pieces, mv);
        }
    }

    {
        let mut playback = state.playback.write().unwrap();
        playback.move_index = target_index;
        playback.pieces = pieces;
        playback.current_player = (target_index % 2) as u8;
        playback.round_number = (target_index / 2) as u32 + 1;
    }

    get_playback_state_internal(&state)
}

/// Stop playback mode
pub async fn playback_stop(State(state): State<Arc<ServerState>>) -> Json<Value> {
    let mut playback = state.playback.write().unwrap();
    *playback = PlaybackState::default();

    Json(json!({ "success": true }))
}

/// Internal helper to get playback state as JSON
fn get_playback_state_internal(state: &Arc<ServerState>) -> Json<Value> {
    let playback = state.playback.read().unwrap();

    if !playback.active {
        return Json(json!({ "active": false }));
    }

    Json(json!({
        "active": true,
        "move_index": playback.move_index,
        "total_moves": playback.total_moves,
        "at_start": playback.move_index == 0,
        "at_end": playback.move_index >= playback.total_moves,
        "current_player": playback.current_player,
        "round_number": playback.round_number,
        "winner": if playback.move_index >= playback.total_moves { playback.winner } else { None },
        "pieces": playback.pieces,
    }))
}

/// Convert ruleset to initial pieces
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
        let pos = &white_positions[0];
        pieces.push(DesignerPiece {
            id: 1,
            piece_id: white_king.to_string(),
            color: "white".to_string(),
            pos: parse_pos(pos),
            facing: white_facings.first().and_then(|v| v.as_u64()).unwrap_or(0) as u8,
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
                facing: white_facings.get(i + 1).and_then(|v| v.as_u64()).unwrap_or(0) as u8,
            });
        }
    }

    // Add black king
    if !black_positions.is_empty() {
        let pos = &black_positions[0];
        pieces.push(DesignerPiece {
            id: 2,
            piece_id: black_king.to_string(),
            color: "black".to_string(),
            pos: parse_pos(pos),
            facing: black_facings.first().and_then(|v| v.as_u64()).unwrap_or(3) as u8,
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
                facing: black_facings.get(i + 1).and_then(|v| v.as_u64()).unwrap_or(3) as u8,
            });
        }
    }

    pieces
}

/// Convert game state to pieces
fn state_to_pieces(state: &Value) -> Vec<DesignerPiece> {
    let mut pieces = Vec::new();

    if let Some(board) = state.get("board").and_then(|v| v.as_object()) {
        for (pos_str, piece) in board {
            // Parse position from string like "(0, 1)"
            let pos = parse_pos_string(pos_str);
            pieces.push(DesignerPiece {
                id: pieces.len() as i64,
                piece_id: piece
                    .get("type_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("A1")
                    .to_string(),
                color: if piece.get("owner").and_then(|v| v.as_u64()).unwrap_or(0) == 0 {
                    "white".to_string()
                } else {
                    "black".to_string()
                },
                pos,
                facing: piece.get("facing").and_then(|v| v.as_u64()).unwrap_or(0) as u8,
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

/// Parse position from string like "(0, 1)"
fn parse_pos_string(s: &str) -> [i8; 2] {
    let s = s.trim_matches(|c| c == '(' || c == ')');
    let parts: Vec<&str> = s.split(',').collect();
    if parts.len() == 2 {
        [
            parts[0].trim().parse().unwrap_or(0),
            parts[1].trim().parse().unwrap_or(0),
        ]
    } else {
        [0, 0]
    }
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
            let pos = mv.get("pos").or_else(|| mv.get("from_pos")).map(parse_pos).unwrap_or([0, 0]);
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
            let target_pos = mv.get("to_pos").or_else(|| mv.get("target")).map(parse_pos).unwrap_or([0, 0]);

            // Find both pieces and swap positions
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
        "REBIRTH" => {
            // Phoenix returns from graveyard
            // This would need graveyard state to be properly implemented
            let dest = mv.get("dest").or_else(|| mv.get("to_pos")).map(parse_pos).unwrap_or([0, 0]);
            let new_facing = mv.get("new_facing").and_then(|v| v.as_u64()).unwrap_or(0) as u8;

            // For now, just note that a phoenix would appear
            // Full implementation needs graveyard tracking
            pieces.push(DesignerPiece {
                id: pieces.len() as i64 + 1000,
                piece_id: "P1".to_string(),
                color: "white".to_string(), // Would need to determine from context
                pos: dest,
                facing: new_facing,
            });
        }
        "PASS" | "SURRENDER" => {
            // No board change
        }
        _ => {}
    }
}
