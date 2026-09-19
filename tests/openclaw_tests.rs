use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use futures_util::{SinkExt, StreamExt};
use openclaw::{
    create_router, AgentEngine, Database, GatewayState, HeartbeatEngine, InferenceEngine,
    MojoSimdBridge, SkillRegistry,
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
    let heartbeat = Arc::new(HeartbeatEngine::with_inference(
        60,
        db.clone(),
        inference.clone(),
    ));
    let skills = Arc::new(SkillRegistry::new());
    let agent = Arc::new(AgentEngine::new(
        inference.clone(),
        skills.clone(),
        Some(db.clone()),
    ));

    GatewayState {
        start_time: Instant::now(),
        db,
        inference,
        heartbeat,
        skills,
        agent,
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
    assert_eq!(body["engine"], "openclaw-rs/0.2.0");
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
async fn test_gateway_openai_multiturn_completions() {
    let state = create_test_state();
    let app = create_router(state);

    let req_payload = serde_json::json!({
        "model": "atlas-lightning-omni",
        "messages": [
            {"role": "system", "content": "You are a helpful assistant."},
            {"role": "user", "content": "Hello"},
            {"role": "assistant", "content": "Greetings."},
            {"role": "user", "content": "What was my initial query?"}
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
    assert!(body["choices"][0]["message"]["content"]
        .as_str()
        .unwrap()
        .contains("What was my initial query?"));
}

#[tokio::test]
async fn test_gateway_openai_streaming_endpoint() {
    let state = create_test_state();
    let app = create_router(state);

    let req_payload = serde_json::json!({
        "model": "atlas-lightning-omni",
        "messages": [
            {"role": "user", "content": "stream test"}
        ],
        "stream": true
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
    assert_eq!(
        response.headers().get("content-type").unwrap(),
        "text/event-stream"
    );
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let text = String::from_utf8_lossy(&bytes);
    assert!(text.contains("data:"));
}

#[tokio::test]
async fn test_gateway_skills_endpoints() {
    let state = create_test_state();
    let app = create_router(state.clone());

    // 1. List skills
    let list_res = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/skills")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(list_res.status(), StatusCode::OK);
    let bytes = axum::body::to_bytes(list_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let list: Value = serde_json::from_slice(&bytes).unwrap();
    let skills = list.as_array().unwrap();
    assert!(skills.iter().any(|s| s["name"] == "bash_eval"));
    assert!(skills.iter().any(|s| s["name"] == "telemetry_ping"));

    // 2. Execute skill
    let app2 = create_router(state);
    let exec_payload = serde_json::json!({
        "skill_name": "telemetry_ping",
        "arguments": {}
    });

    let exec_res = app2
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/skills/execute")
                .header("Content-Type", "application/json")
                .body(Body::from(exec_payload.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(exec_res.status(), StatusCode::OK);
    let bytes2 = axum::body::to_bytes(exec_res.into_body(), usize::MAX)
        .await
        .unwrap();
    let result: Value = serde_json::from_slice(&bytes2).unwrap();
    assert_eq!(result["success"], true);
    assert!(result["output"].as_str().unwrap().contains("healthy"));
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
        "payload": "echo 'build nominal'"
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
async fn test_gateway_websocket_lifecycle() {
    let state = create_test_state();
    let app = create_router(state);

    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();

    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });

    let ws_url = format!("ws://127.0.0.1:{}/ws", port);
    let (mut ws_stream, _) = tokio_tungstenite::connect_async(ws_url).await.unwrap();

    // 1. Welcome message
    let msg = ws_stream.next().await.unwrap().unwrap();
    let val: Value = serde_json::from_str(&msg.to_string()).unwrap();
    assert_eq!(val["type"], "welcome");
    assert_eq!(val["gateway"], "openclaw-rs");

    // 2. Ping / Pong
    ws_stream
        .send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::json!({"type": "ping"}).to_string(),
        ))
        .await
        .unwrap();

    let msg = ws_stream.next().await.unwrap().unwrap();
    let val: Value = serde_json::from_str(&msg.to_string()).unwrap();
    assert_eq!(val["type"], "pong");

    // 3. Skills list over WebSocket
    ws_stream
        .send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::json!({"type": "skills_list"}).to_string(),
        ))
        .await
        .unwrap();

    let msg = ws_stream.next().await.unwrap().unwrap();
    let val: Value = serde_json::from_str(&msg.to_string()).unwrap();
    assert_eq!(val["type"], "skills_list");
    assert!(!val["skills"].as_array().unwrap().is_empty());

    // 4. Skill execute over WebSocket
    ws_stream
        .send(tokio_tungstenite::tungstenite::Message::Text(
            serde_json::json!({
                "type": "skill_exec",
                "skill_name": "telemetry_ping",
                "arguments": {}
            })
            .to_string(),
        ))
        .await
        .unwrap();

    let msg = ws_stream.next().await.unwrap().unwrap();
    let val: Value = serde_json::from_str(&msg.to_string()).unwrap();
    assert_eq!(val["type"], "skill_result");
    assert_eq!(val["response"]["success"], true);
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

#[tokio::test]
async fn test_gateway_agent_run_endpoint() {
    let state = create_test_state();
    let app = create_router(state);

    let req_payload = serde_json::json!({
        "prompt": "Status and telemetry check",
        "max_turns": 2
    });

    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/agent/run")
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
    assert_eq!(body["status"], "completed");
    assert_eq!(body["prompt"], "Status and telemetry check");
    assert!(body["steps"].is_array());
}
