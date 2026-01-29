//! Board geometry endpoint

use axum::Json;
use serde::Serialize;

const BOARD_RADIUS: i8 = 4;

#[derive(Serialize)]
pub struct BoardInfo {
    pub radius: i8,
    pub hexes: Vec<[i8; 2]>,
    pub directions: Vec<[i8; 2]>,
    pub direction_names: Vec<&'static str>,
}

/// Generate all valid hexes on the board
fn all_hexes() -> Vec<[i8; 2]> {
    let mut hexes = Vec::new();
    for q in -BOARD_RADIUS..=BOARD_RADIUS {
        for r in -BOARD_RADIUS..=BOARD_RADIUS {
            if q.abs() + r.abs() + (q + r).abs() <= BOARD_RADIUS * 2 {
                hexes.push([q, r]);
            }
        }
    }
    hexes
}

/// Get board geometry
pub async fn get_board() -> Json<BoardInfo> {
    Json(BoardInfo {
        radius: BOARD_RADIUS,
        hexes: all_hexes(),
        directions: vec![
            [0, -1],  // N
            [1, -1],  // NE
            [1, 0],   // SE
            [0, 1],   // S
            [-1, 1],  // SW
            [-1, 0],  // NW
        ],
        direction_names: vec!["N", "NE", "SE", "S", "SW", "NW"],
    })
}
