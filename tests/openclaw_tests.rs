use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use openclaw::{
    create_router, Database, GatewayState, HeartbeatEngine, InferenceEngine, MojoSimdBridge,
    SkillRegistry,
};
use serde_json::Value;
use std::sync::Arc;
use std::time::Instant;
use tower::util::ServiceExt;

fn create_test_state() -> GatewayState {
    let db = Arc::new(Database::open_in_memory().unwrap());
    let inference = Arc::new(InferenceEngine::new(
        Some("http://127.0.0.1:9999/v1/chat/completions".to_string()),
        None,
    ));
    let heartbeat = Arc::new(HeartbeatEngine::new(60, db.clone()));
    let skills = Arc::new(SkillRegistry::new());

    GatewayState {
        start_time: Instant::now(),
        db,
        inference,
        heartbeat,
        skills,
    }
}

#[tokio::test]
async fn test_gateway_health_endpoint() {
    let state = create_test_state();
    let app = create_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["status"], "healthy");
    assert_eq!(body["engine"], "openclaw-rs/0.1.0");
    assert_eq!(body["database_mode"], "sqlite-wal");
    assert_eq!(body["zero_disk_secrets"], true);
}

#[tokio::test]
async fn test_gateway_openai_completions_endpoint() {
    let state = create_test_state();
    let app = create_router(state);

    let req_payload = serde_json::json!({
        "model": "atlas-lightning-omni",
        "messages": [
            {"role": "system", "content": "You are OpenClaw."},
            {"role": "user", "content": "ping"}
        ]
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/v1/chat/completions")
                .header("Content-Type", "application/json")
                .body(Body::from(req_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["object"], "chat.completion");
    assert_eq!(body["choices"][0]["message"]["role"], "assistant");
}

#[tokio::test]
async fn test_gateway_shell_execution() {
    let state = create_test_state();
    let app = create_router(state);

    let req_payload = serde_json::json!({
        "command": "echo 'Sovereign OpenClaw native execution'"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/shell")
                .header("Content-Type", "application/json")
                .body(Body::from(req_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(body["success"], true);
    assert!(body["stdout"]
        .as_str()
        .unwrap()
        .contains("Sovereign OpenClaw"));
}

#[tokio::test]
async fn test_gateway_task_creation_and_listing() {
    let state = create_test_state();
    let app = create_router(state.clone());

    let req_payload = serde_json::json!({
        "task_type": "build_monitor",
        "payload": "cargo check --workspace"
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/tasks")
                .header("Content-Type", "application/json")
                .body(Body::from(req_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let app2 = create_router(state);
    let list_response = app2
        .oneshot(
            Request::builder()
                .uri("/api/v1/tasks")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(list_response.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(list_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let tasks: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(tasks.as_array().unwrap().len(), 1);
    assert_eq!(tasks[0]["task_type"], "build_monitor");
}

#[tokio::test]
async fn test_mojo_simd_bridge_operations() {
    let v1 = [1.0f32, 0.0, 0.0, 0.0];
    let v2 = [1.0f32, 0.0, 0.0, 0.0];
    let sim = MojoSimdBridge::cosine_similarity_4d(v1, v2);
    assert!((sim - 1.0).abs() < 1e-4);

    let tokens = [2.0f32, 4.0, 6.0, 8.0];
    let weights = [0.25f32, 0.25, 0.25, 0.25];
    let proj = MojoSimdBridge::token_projection(tokens, weights, 2.0);
    assert!((proj - 7.0).abs() < 1e-4);

    let entropy = MojoSimdBridge::token_entropy([0.5, 0.5, 0.0, 0.0]);
    assert!(entropy < 0.0);
}
