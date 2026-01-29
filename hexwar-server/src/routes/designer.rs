//! Designer API endpoints
//!
//! Manages the designer state for the board editor UI.

use crate::state::{DesignerPiece, DesignerState, Graveyard, ServerState, Templates};
use axum::{
    extract::{Query, State},
    Json,
};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::sync::Arc;

/// Get current designer state
pub async fn get_designer_state(State(state): State<Arc<ServerState>>) -> Json<DesignerState> {
    let designer = state.designer.read().unwrap();
    Json(designer.clone())
}

/// Update request
#[derive(Deserialize)]
pub struct UpdateRequest {
    pub pieces: Option<Vec<DesignerPiece>>,
    pub graveyard: Option<Graveyard>,
    pub templates: Option<Templates>,
    pub name: Option<String>,
}

/// Update designer state
pub async fn update_designer_state(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<UpdateRequest>,
) -> Json<Value> {
    let mut designer = state.designer.write().unwrap();

    if let Some(pieces) = req.pieces {
        designer.pieces = pieces;
    }
    if let Some(graveyard) = req.graveyard {
        designer.graveyard = graveyard;
    }
    if let Some(templates) = req.templates {
        designer.templates = templates;
    }
    if let Some(name) = req.name {
        designer.name = name;
    }
    designer.version += 1;

    Json(json!({
        "success": true,
        "version": designer.version
    }))
}

/// Poll query params
#[derive(Deserialize)]
pub struct PollParams {
    pub version: Option<u64>,
}

/// Long-poll for designer updates
pub async fn poll_designer_state(
    State(state): State<Arc<ServerState>>,
    Query(params): Query<PollParams>,
) -> Json<Value> {
    let client_version = params.version.unwrap_or(0);

    // Check up to 50 times (5 seconds) for updates
    for _ in 0..50 {
        {
            let designer = state.designer.read().unwrap();
            if designer.version != client_version {
                return Json(json!({
                    "reload": true,
                    "pieces": designer.pieces,
                    "graveyard": designer.graveyard,
                    "templates": designer.templates,
                    "version": designer.version,
                    "name": designer.name,
                }));
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    }

    let designer = state.designer.read().unwrap();
    Json(json!({
        "reload": false,
        "version": designer.version
    }))
}

/// Load request
#[derive(Deserialize)]
pub struct LoadRequest {
    pub name: Option<String>,
    pub path: Option<String>,
}

/// Load a champion/seed into the designer
pub async fn load_into_designer(
    State(state): State<Arc<ServerState>>,
    Json(req): Json<LoadRequest>,
) -> Json<Value> {
    let name = req.name.or(req.path).unwrap_or_default();

    // Try to find and load the champion data
    match load_champion_or_seed(&name) {
        Ok((ruleset, loaded_name)) => {
            let pieces = ruleset_to_designer_pieces(&ruleset);
            let templates = Templates {
                white: ruleset
                    .get("white_template")
                    .and_then(|v| v.as_str())
                    .unwrap_or("E")
                    .to_string(),
                black: ruleset
                    .get("black_template")
                    .and_then(|v| v.as_str())
                    .unwrap_or("E")
                    .to_string(),
            };

            let mut designer = state.designer.write().unwrap();
            designer.pieces = pieces;
            designer.graveyard = Graveyard::default();
            designer.templates = templates;
            designer.name = loaded_name.clone();
            designer.version += 1;

            Json(json!({
                "success": true,
                "version": designer.version,
                "loaded": loaded_name
            }))
        }
        Err(e) => Json(json!({ "error": e })),
    }
}

/// Load champion or seed data from disk
fn load_champion_or_seed(name: &str) -> Result<(Value, String), String> {
    use std::path::PathBuf;

    // Get base directory (project root)
    let base_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    // Check if name includes run path (e.g., "balance_jan06_0851/some-champion")
    if name.contains('/') {
        let parts: Vec<&str> = name.split('/').collect();
        if parts.len() >= 2 {
            let run = parts[0];
            let champ_name = parts[1..].join("/");
            let path = base_dir.join(run).join("champions").join(format!("{}.json", champ_name));
            if path.exists() {
                return load_ruleset_from_file(&path, name);
            }
        }
    }

    // Search balance_* directories
    if let Ok(entries) = std::fs::read_dir(&base_dir) {
        let mut dirs: Vec<_> = entries
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_str()
                    .map(|s| s.starts_with("balance_"))
                    .unwrap_or(false)
            })
            .collect();
        dirs.sort_by(|a, b| b.file_name().cmp(&a.file_name())); // Reverse sort

        for dir in dirs {
            let path = dir.path().join("champions").join(format!("{}.json", name));
            if path.exists() {
                return load_ruleset_from_file(&path, name);
            }
        }
    }

    // Check board_sets directory
    let board_sets_path = base_dir.join("board_sets").join(format!("{}.json", name));
    if board_sets_path.exists() {
        return load_ruleset_from_file(&board_sets_path, name);
    }

    // Check board_sets subdirectories
    if let Ok(entries) = std::fs::read_dir(base_dir.join("board_sets")) {
        for entry in entries.filter_map(|e| e.ok()) {
            if entry.path().is_dir() {
                let path = entry.path().join(format!("{}.json", name));
                if path.exists() {
                    return load_ruleset_from_file(&path, name);
                }
            }
        }
    }

    // Check seeds directories
    if let Ok(entries) = std::fs::read_dir(&base_dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let fname = entry.file_name();
            let fname_str = fname.to_string_lossy();
            if fname_str.starts_with("seeds_") {
                let path = entry.path().join("champions").join(format!("{}.json", name));
                if path.exists() {
                    return load_ruleset_from_file(&path, name);
                }
            }
        }
    }

    Err(format!("Champion not found: {}", name))
}

/// Load ruleset from a JSON file
fn load_ruleset_from_file(path: &std::path::Path, name: &str) -> Result<(Value, String), String> {
    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read file: {}", e))?;
    let data: Value = serde_json::from_str(&content)
        .map_err(|e| format!("Failed to parse JSON: {}", e))?;

    // Extract ruleset from wrapper if present
    let ruleset = if data.get("ruleset").is_some() {
        data.get("ruleset").unwrap().clone()
    } else {
        data
    };

    Ok((ruleset, name.to_string()))
}

/// Convert ruleset to designer pieces format
fn ruleset_to_designer_pieces(ruleset: &Value) -> Vec<DesignerPiece> {
    let mut pieces = Vec::new();

    // Extract arrays from ruleset
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
        let facing = white_facings.first().and_then(|v| v.as_u64()).unwrap_or(0) as u8;
        pieces.push(DesignerPiece {
            id: 1,
            piece_id: white_king.to_string(),
            color: "white".to_string(),
            pos: [
                pos.get(0).and_then(|v| v.as_i64()).unwrap_or(0) as i8,
                pos.get(1).and_then(|v| v.as_i64()).unwrap_or(0) as i8,
            ],
            facing,
        });
    }

    // Add white pieces
    for (i, piece) in white_pieces.iter().enumerate() {
        let pos_idx = i + 1;
        if pos_idx < white_positions.len() {
            let pos = &white_positions[pos_idx];
            let facing = white_facings.get(pos_idx).and_then(|v| v.as_u64()).unwrap_or(0) as u8;
            pieces.push(DesignerPiece {
                id: 100 + i as i64,
                piece_id: piece.as_str().unwrap_or("A1").to_string(),
                color: "white".to_string(),
                pos: [
                    pos.get(0).and_then(|v| v.as_i64()).unwrap_or(0) as i8,
                    pos.get(1).and_then(|v| v.as_i64()).unwrap_or(0) as i8,
                ],
                facing,
            });
        }
    }

    // Add black king
    if !black_positions.is_empty() {
        let pos = &black_positions[0];
        let facing = black_facings.first().and_then(|v| v.as_u64()).unwrap_or(3) as u8;
        pieces.push(DesignerPiece {
            id: 2,
            piece_id: black_king.to_string(),
            color: "black".to_string(),
            pos: [
                pos.get(0).and_then(|v| v.as_i64()).unwrap_or(0) as i8,
                pos.get(1).and_then(|v| v.as_i64()).unwrap_or(0) as i8,
            ],
            facing,
        });
    }

    // Add black pieces
    for (i, piece) in black_pieces.iter().enumerate() {
        let pos_idx = i + 1;
        if pos_idx < black_positions.len() {
            let pos = &black_positions[pos_idx];
            let facing = black_facings.get(pos_idx).and_then(|v| v.as_u64()).unwrap_or(3) as u8;
            pieces.push(DesignerPiece {
                id: 200 + i as i64,
                piece_id: piece.as_str().unwrap_or("A1").to_string(),
                color: "black".to_string(),
                pos: [
                    pos.get(0).and_then(|v| v.as_i64()).unwrap_or(0) as i8,
                    pos.get(1).and_then(|v| v.as_i64()).unwrap_or(0) as i8,
                ],
                facing,
            });
        }
    }

    pieces
}
