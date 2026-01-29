//! Champions and Seeds API endpoints

use axum::{
    extract::Path,
    Json,
};
use serde::Serialize;
use serde_json::{json, Value};
use std::path::PathBuf;

#[derive(Serialize)]
pub struct ChampionInfo {
    pub name: String,
    pub run: String,
    pub id: String,
    pub ucb: f64,
}

/// Get list of champions from the d7 exotics run
pub async fn get_champions_list() -> Json<Vec<ChampionInfo>> {
    let base_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut champions = Vec::new();

    // Look for balance_jan08_d7_exotics_0603 specifically (matching Python server)
    let run_dir = base_dir.join("balance_jan08_d7_exotics_0603");
    let champions_dir = run_dir.join("champions");

    if champions_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&champions_dir) {
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                        let ucb = load_ucb_score(&path);
                        champions.push(ChampionInfo {
                            name: name.to_string(),
                            run: "balance_jan08_d7_exotics_0603".to_string(),
                            id: format!("balance_jan08_d7_exotics_0603/{}", name),
                            ucb,
                        });
                    }
                }
            }
        }
    }

    // Sort by UCB descending
    champions.sort_by(|a, b| b.ucb.partial_cmp(&a.ucb).unwrap_or(std::cmp::Ordering::Equal));

    Json(champions)
}

/// Load UCB score from a champion file
fn load_ucb_score(path: &PathBuf) -> f64 {
    if let Ok(content) = std::fs::read_to_string(path) {
        if let Ok(data) = serde_json::from_str::<Value>(&content) {
            return data.get("ucb_score").and_then(|v| v.as_f64()).unwrap_or(0.0);
        }
    }
    0.0
}

/// Get a specific champion's data
pub async fn get_champion_data(Path(name): Path<String>) -> Json<Value> {
    let base_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));

    // Check if name includes run (e.g., "balance_jan06_0851/some-champion")
    if name.contains('/') {
        let parts: Vec<&str> = name.split('/').collect();
        if parts.len() >= 2 {
            let run = parts[0];
            let champ_name = parts[1..].join("/");
            let path = base_dir.join(run).join("champions").join(format!("{}.json", champ_name));
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(mut data) = serde_json::from_str::<Value>(&content) {
                        if let Some(obj) = data.as_object_mut() {
                            obj.insert("run".to_string(), json!(run));
                        }
                        return Json(data);
                    }
                }
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
        dirs.sort_by(|a, b| b.file_name().cmp(&a.file_name()));

        for dir in dirs {
            let path = dir.path().join("champions").join(format!("{}.json", name));
            if path.exists() {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    if let Ok(mut data) = serde_json::from_str::<Value>(&content) {
                        if let Some(obj) = data.as_object_mut() {
                            obj.insert(
                                "run".to_string(),
                                json!(dir.file_name().to_string_lossy().to_string()),
                            );
                        }
                        return Json(data);
                    }
                }
            }
        }
    }

    // Check board_sets directory
    let board_sets_path = base_dir.join("board_sets").join(format!("{}.json", name));
    if board_sets_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&board_sets_path) {
            if let Ok(mut data) = serde_json::from_str::<Value>(&content) {
                if let Some(obj) = data.as_object_mut() {
                    obj.insert("run".to_string(), json!("board_sets"));
                }
                return Json(data);
            }
        }
    }

    // Check board_sets subdirectories
    if let Ok(entries) = std::fs::read_dir(base_dir.join("board_sets")) {
        for entry in entries.filter_map(|e| e.ok()) {
            if entry.path().is_dir() {
                let path = entry.path().join(format!("{}.json", name));
                if path.exists() {
                    if let Ok(content) = std::fs::read_to_string(&path) {
                        if let Ok(mut data) = serde_json::from_str::<Value>(&content) {
                            if let Some(obj) = data.as_object_mut() {
                                obj.insert(
                                    "run".to_string(),
                                    json!(format!(
                                        "board_sets/{}",
                                        entry.file_name().to_string_lossy()
                                    )),
                                );
                            }
                            return Json(data);
                        }
                    }
                }
            }
        }
    }

    Json(json!({ "error": format!("Champion not found: {}", name) }))
}

/// Get list of seeds
pub async fn get_seeds_list() -> Json<Vec<String>> {
    let base_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let mut seeds = Vec::new();

    let seeds_dir = base_dir.join("seeds_d6_exotics").join("champions");
    if seeds_dir.exists() {
        if let Ok(entries) = std::fs::read_dir(&seeds_dir) {
            let mut seed_entries: Vec<(String, f64)> = Vec::new();
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if path.extension().and_then(|s| s.to_str()) == Some("json") {
                    if let Some(name) = path.file_stem().and_then(|s| s.to_str()) {
                        let ucb = load_ucb_score(&path);
                        seed_entries.push((name.to_string(), ucb));
                    }
                }
            }
            // Sort by UCB descending
            seed_entries.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
            seeds = seed_entries.into_iter().map(|(name, _)| name).collect();
        }
    }

    Json(seeds)
}

/// Get a specific seed's data
pub async fn get_seed_data(Path(name): Path<String>) -> Json<Value> {
    let base_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let seed_path = base_dir
        .join("seeds_d6_exotics")
        .join("champions")
        .join(format!("{}.json", name));

    if seed_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&seed_path) {
            if let Ok(mut data) = serde_json::from_str::<Value>(&content) {
                if let Some(obj) = data.as_object_mut() {
                    obj.insert("name".to_string(), json!(format!("[seed] {}", name)));
                }
                return Json(data);
            }
        }
    }

    Json(json!({ "error": format!("Seed not found: {}", name) }))
}
