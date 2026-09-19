use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Json, State,
    },
    http::StatusCode,
    response::{
        sse::{Event, KeepAlive, Sse},
        IntoResponse,
    },
    routing::{get, post},
    Router,
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::convert::Infallible;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tokio::process::Command;
use tokio_stream::wrappers::BroadcastStream;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

use crate::heartbeat::HeartbeatEngine;
use crate::inference::InferenceEngine;
use crate::mojo_bridge::MojoSimdBridge;
use crate::persistence::Database;
use crate::skills::SkillRegistry;

#[derive(Clone)]
pub struct GatewayState {
    pub start_time: Instant,
    pub db: Arc<Database>,
    pub inference: Arc<InferenceEngine>,
    pub heartbeat: Arc<HeartbeatEngine>,
    pub skills: Arc<SkillRegistry>,
}

#[derive(Serialize)]
pub struct HealthResponse {
    pub status: &'static str,
    pub engine: &'static str,
    pub uptime_seconds: u64,
    pub local_model: String,
    pub heartbeat_interval_seconds: u64,
    pub total_heartbeat_ticks: u64,
    pub database_mode: &'static str,
    pub mojo_simd_accelerated: bool,
    pub mojo_simd_version: i32,
    pub zero_disk_secrets: bool,
}

pub async fn health_handler(State(state): State<GatewayState>) -> impl IntoResponse {
    let uptime = state.start_time.elapsed().as_secs();
    let ticks = state.heartbeat.tick_count();
    let model = state.inference.model_id();
    let mojo_active = MojoSimdBridge::is_mojo_accelerated();
    let mojo_ver = MojoSimdBridge::version();

    Json(HealthResponse {
        status: "healthy",
        engine: "openclaw-rs/0.1.0",
        uptime_seconds: uptime,
        local_model: model,
        heartbeat_interval_seconds: state.heartbeat.interval_secs(),
        total_heartbeat_ticks: ticks,
        database_mode: "sqlite-wal",
        mojo_simd_accelerated: mojo_active,
        mojo_simd_version: mojo_ver,
        zero_disk_secrets: true,
    })
}

#[derive(Deserialize)]
pub struct ChatRequest {
    pub prompt: String,
    pub system: Option<String>,
    pub temperature: Option<f32>,
}

#[derive(Serialize)]
pub struct ChatResponse {
    pub role: &'static str,
    pub content: String,
    pub model: String,
    pub duration_ms: u128,
}

pub async fn chat_handler(
    State(state): State<GatewayState>,
    Json(payload): Json<ChatRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let start = Instant::now();
    let response = state
        .inference
        .generate(&payload.prompt, payload.system.as_deref(), payload.temperature)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let duration = start.elapsed().as_millis();
    let model = state.inference.model_id();

    let _ = state
        .db
        .record_turn("user", &payload.prompt, &response, duration as u64);

    Ok(Json(ChatResponse {
        role: "assistant",
        content: response,
        model,
        duration_ms: duration,
    }))
}

// OpenAI-compatible Chat Completions API
#[derive(Deserialize)]
pub struct OpenAiMessage {
    pub role: String,
    pub content: String,
}

#[derive(Deserialize)]
pub struct OpenAiChatRequest {
    pub model: Option<String>,
    pub messages: Vec<OpenAiMessage>,
    pub temperature: Option<f32>,
}

pub async fn openai_completions_handler(
    State(state): State<GatewayState>,
    Json(payload): Json<OpenAiChatRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let start = Instant::now();
    let mut system_prompt = None;
    let mut last_user_prompt = String::new();

    for m in &payload.messages {
        if m.role == "system" {
            system_prompt = Some(m.content.clone());
        } else if m.role == "user" {
            last_user_prompt = m.content.clone();
        }
    }

    let response = state
        .inference
        .generate(
            &last_user_prompt,
            system_prompt.as_deref(),
            payload.temperature,
        )
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let duration = start.elapsed().as_millis();
    let model = state.inference.model_id();

    let _ = state
        .db
        .record_turn("user", &last_user_prompt, &response, duration as u64);

    Ok(Json(json!({
        "id": format!("chatcmpl-{}", uuid::Uuid::new_v4()),
        "object": "chat.completion",
        "created": chrono::Utc::now().timestamp(),
        "model": model,
        "choices": [{
            "index": 0,
            "message": {
                "role": "assistant",
                "content": response,
            },
            "finish_reason": "stop"
        }],
        "usage": {
            "prompt_tokens": last_user_prompt.split_whitespace().count(),
            "completion_tokens": response.split_whitespace().count(),
            "total_tokens": last_user_prompt.split_whitespace().count() + response.split_whitespace().count(),
        }
    })))
}

pub async fn openai_models_handler(State(state): State<GatewayState>) -> impl IntoResponse {
    let model = state.inference.model_id();
    Json(json!({
        "object": "list",
        "data": [{
            "id": model,
            "object": "model",
            "created": 1789783086,
            "owned_by": "openclaw-rs",
        }]
    }))
}

// SSE Events Endpoint
pub async fn sse_events_handler(
    State(state): State<GatewayState>,
) -> Sse<impl futures_util::Stream<Item = Result<Event, Infallible>>> {
    let rx = state.heartbeat.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|item| async move {
        match item {
            Ok(receipt) => {
                let data = serde_json::to_string(&receipt).unwrap_or_default();
                Some(Ok(Event::default().event("heartbeat").data(data)))
            }
            Err(_) => None,
        }
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}

// Discord Hub Relay
#[derive(Deserialize)]
pub struct DiscordRelayRequest {
    pub channel_id: String,
    pub author: String,
    pub content: String,
}

pub async fn discord_relay_handler(
    State(state): State<GatewayState>,
    Json(payload): Json<DiscordRelayRequest>,
) -> impl IntoResponse {
    let prompt = format!("[Discord from {} in {}]: {}", payload.author, payload.channel_id, payload.content);
    let reply = state.inference.generate(&prompt, None, None).await.unwrap_or_else(|e| format!("Error: {}", e));

    let _ = state.db.record_turn(&payload.author, &payload.content, &reply, 10);

    Json(json!({
        "status": "relayed",
        "reply": reply,
        "timestamp": chrono::Utc::now().to_rfc3339(),
    }))
}

// Local Shell Executor
#[derive(Deserialize)]
pub struct ShellRequest {
    pub command: String,
}

#[derive(Serialize)]
pub struct ShellResponse {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
    pub success: bool,
}

pub async fn shell_handler(
    Json(payload): Json<ShellRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let output = Command::new("sh")
        .arg("-c")
        .arg(&payload.command)
        .output()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("Failed to execute command: {}", e)))?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let exit_code = output.status.code().unwrap_or(-1);

    Ok(Json(ShellResponse {
        stdout,
        stderr,
        exit_code,
        success: output.status.success(),
    }))
}

// Task endpoints
#[derive(Deserialize)]
pub struct CreateTaskRequest {
    pub task_type: String,
    pub payload: String,
}

pub async fn create_task_handler(
    State(state): State<GatewayState>,
    Json(req): Json<CreateTaskRequest>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let id = format!("task-{}", uuid::Uuid::new_v4());
    state
        .db
        .create_task(&id, &req.task_type, &req.payload)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(json!({
        "status": "created",
        "id": id,
        "task_type": req.task_type,
    })))
}

pub async fn list_tasks_handler(
    State(state): State<GatewayState>,
) -> Result<impl IntoResponse, (StatusCode, String)> {
    let tasks = state
        .db
        .list_pending_tasks()
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(tasks))
}

pub async fn trigger_heartbeat_handler(State(state): State<GatewayState>) -> impl IntoResponse {
    let receipt = state.heartbeat.pulse_once().await;
    Json(receipt)
}

pub async fn ws_handler(
    ws: WebSocketUpgrade,
    State(state): State<GatewayState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: GatewayState) {
    let welcome = json!({
        "type": "welcome",
        "gateway": "openclaw-rs",
        "version": "0.1.0",
        "timestamp": chrono::Utc::now().to_rfc3339(),
    });

    if socket
        .send(Message::Text(welcome.to_string()))
        .await
        .is_err()
    {
        return;
    }

    while let Some(Ok(msg)) = socket.next().await {
        if let Message::Text(text) = msg {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&text) {
                let msg_type = parsed.get("type").and_then(|v| v.as_str()).unwrap_or("chat");
                match msg_type {
                    "ping" => {
                        let pong = json!({"type": "pong", "timestamp": chrono::Utc::now().to_rfc3339()});
                        let _ = socket.send(Message::Text(pong.to_string())).await;
                    }
                    "heartbeat" => {
                        let receipt = state.heartbeat.pulse_once().await;
                        let out = json!({"type": "heartbeat_receipt", "receipt": receipt});
                        let _ = socket.send(Message::Text(out.to_string())).await;
                    }
                    "shell" => {
                        let cmd_str = parsed.get("command").and_then(|v| v.as_str()).unwrap_or("echo shell ready");
                        let res = Command::new("sh").arg("-c").arg(cmd_str).output().await;
                        let out = match res {
                            Ok(o) => json!({
                                "type": "shell_output",
                                "stdout": String::from_utf8_lossy(&o.stdout),
                                "stderr": String::from_utf8_lossy(&o.stderr),
                                "exit_code": o.status.code().unwrap_or(-1),
                            }),
                            Err(e) => json!({
                                "type": "shell_error",
                                "error": e.to_string(),
                            }),
                        };
                        let _ = socket.send(Message::Text(out.to_string())).await;
                    }
                    "simd_eval" => {
                        let sim = MojoSimdBridge::cosine_similarity(&[1.0, 0.0, 0.0, 0.0], &[1.0, 0.0, 0.0, 0.0]);
                        let out = json!({
                            "type": "simd_result",
                            "cosine_similarity": sim,
                            "accelerated": MojoSimdBridge::is_mojo_accelerated(),
                            "version": MojoSimdBridge::version(),
                        });
                        let _ = socket.send(Message::Text(out.to_string())).await;
                    }
                    _ => {
                        let prompt = parsed.get("content").and_then(|v| v.as_str()).unwrap_or(&text);
                        let start = Instant::now();
                        let result = state.inference.generate(prompt, None, None).await;
                        let duration = start.elapsed().as_millis();

                        match result {
                            Ok(reply) => {
                                let out = json!({
                                    "type": "response",
                                    "content": reply,
                                    "duration_ms": duration,
                                    "timestamp": chrono::Utc::now().to_rfc3339(),
                                });
                                if socket.send(Message::Text(out.to_string())).await.is_err() {
                                    break;
                                }
                            }
                            Err(err) => {
                                let err_out = json!({
                                    "type": "error",
                                    "message": err.to_string(),
                                });
                                if socket.send(Message::Text(err_out.to_string())).await.is_err() {
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn create_router(state: GatewayState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/api/v1/health", get(health_handler))
        .route("/api/v1/chat", post(chat_handler))
        .route("/api/v1/heartbeat/tick", post(trigger_heartbeat_handler))
        .route("/api/v1/events", get(sse_events_handler))
        .route("/events", get(sse_events_handler))
        .route("/api/v1/discord/relay", post(discord_relay_handler))
        .route("/api/v1/shell", post(shell_handler))
        .route("/api/v1/tasks", post(create_task_handler).get(list_tasks_handler))
        .route("/v1/chat/completions", post(openai_completions_handler))
        .route("/v1/models", get(openai_models_handler))
        .route("/ws", get(ws_handler))
        .route("/api/v1/ws", get(ws_handler))
        .layer(
            CorsLayer::new()
                .allow_origin(Any)
                .allow_methods(Any)
                .allow_headers(Any),
        )
        .with_state(state)
}

pub async fn start_gateway(state: GatewayState, port: u16) -> Result<(), Box<dyn std::error::Error>> {
    let app = create_router(state);
    let addr = SocketAddr::from(([0, 0, 0, 0], port));
    info!("OpenClaw Gateway listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}
