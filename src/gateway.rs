use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Json, State,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Router,
};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Instant;
use tower_http::cors::{Any, CorsLayer};
use tracing::info;

use crate::heartbeat::HeartbeatEngine;
use crate::inference::InferenceEngine;
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
    pub zero_disk_secrets: bool,
}

pub async fn health_handler(State(state): State<GatewayState>) -> impl IntoResponse {
    let uptime = state.start_time.elapsed().as_secs();
    let ticks = state.heartbeat.tick_count();
    let model = state.inference.model_id();

    Json(HealthResponse {
        status: "healthy",
        engine: "openclaw-rs/0.1.0",
        uptime_seconds: uptime,
        local_model: model,
        heartbeat_interval_seconds: state.heartbeat.interval_secs(),
        total_heartbeat_ticks: ticks,
        database_mode: "sqlite-wal",
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

    // Log turn to database
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
            let start = Instant::now();
            let result = state.inference.generate(&text, None, None).await;
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

pub fn create_router(state: GatewayState) -> Router {
    Router::new()
        .route("/health", get(health_handler))
        .route("/api/v1/health", get(health_handler))
        .route("/api/v1/chat", post(chat_handler))
        .route("/api/v1/heartbeat/tick", post(trigger_heartbeat_handler))
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
