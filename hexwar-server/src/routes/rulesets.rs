//! Rulesets API endpoint

use axum::Json;
use serde::Serialize;
use std::path::PathBuf;

#[derive(Serialize)]
pub struct RulesetInfo {
    pub id: String,
    pub name: String,
    pub path: String,
}

/// Get list of available rulesets
pub async fn get_rulesets() -> Json<Vec<RulesetInfo>> {
    let base_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    // Match Python server's hardcoded ruleset
    let copper_pass_path = base_dir
        .join("board_sets")
        .join("d7_firstarrow_seeds")
        .join("copper-pass.json");

    let rulesets = vec![RulesetInfo {
        id: "copper-pass".to_string(),
        name: "copper-pass".to_string(),
        path: copper_pass_path.to_string_lossy().to_string(),
    }];

    Json(rulesets)
}
