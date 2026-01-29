//! Game API routes - stub for Agent 7 to complete
//!
//! TODO: Implement game creation, move application, state retrieval

use axum::{extract::State, Json};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use crate::state::AppState;

/// Placeholder for game routes
/// Agent 7 will implement full game API
pub async fn placeholder() -> &'static str {
    "Game API - TODO"
}
