//! Integration tests for hexwar-server API

use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use hexwar_server::{create_router, ServerConfig, ServerState};
use serde_json::Value;
use std::sync::Arc;
use tower::ServiceExt;

fn test_app() -> axum::Router {
    let config = ServerConfig::default();
    let state = Arc::new(ServerState::new());
    create_router(&config, state)
}

#[tokio::test]
async fn test_status_endpoint() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/status")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["status"], "ok");
    assert_eq!(json["engine"], "rust");
}

#[tokio::test]
async fn test_pieces_endpoint() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/pieces")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // Check that piece types are returned
    assert!(json.get("K1").is_some(), "Should have K1 king");
    assert!(json.get("A1").is_some(), "Should have A1 pawn");
    assert!(json.get("D5").is_some(), "Should have D5 queen");

    // Check piece properties
    let k1 = &json["K1"];
    assert_eq!(k1["is_king"], true);
    assert_eq!(k1["name"], "King Guard");
}

#[tokio::test]
async fn test_board_endpoint() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/board")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["radius"], 4);
    assert!(json["hexes"].as_array().is_some());
    assert!(json["directions"].as_array().is_some());
    assert!(json["direction_names"].as_array().is_some());
}

#[tokio::test]
async fn test_designer_state() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/designer")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // Default state should have empty pieces
    assert!(json["pieces"].as_array().is_some());
    assert_eq!(json["version"], 0);
}

#[tokio::test]
async fn test_playback_state_inactive() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/playback/state")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    assert_eq!(json["active"], false);
}

#[tokio::test]
async fn test_rulesets_endpoint() {
    let app = test_app();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/rulesets")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let json: Value = serde_json::from_slice(&body).unwrap();

    // Should return an array of rulesets
    assert!(json.as_array().is_some());
}
