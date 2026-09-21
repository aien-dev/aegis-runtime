//! End-to-End Test Suite for In-Process Inference in AEGIS.
//! Verifies:
//! 1. In-process EmbeddedInferenceBackend initialization.
//! 2. ProtocolInferenceBackend contract execution.
//! 3. Pure-Rust TinyLlama chat template formatting.
//! 4. Structured tool calling (model -> structured tool call -> OpenClaw tool -> model).
//! 5. AgentEngine integration using Arc<dyn InferenceEngine>.
//! 6. Gateway streaming chat completions over SSE.

use aegis::inference::{
    format_messages_to_prompt, parse_structured_tool_calls, EmbeddedInferenceBackend,
    InferenceEngine,
};
use aegis::{AgentEngine, SkillExecutionRequest, SkillRegistry};
use futures_util::StreamExt;
use serde_json::json;
use std::sync::Arc;

#[tokio::test]
async fn test_embedded_model_reference_creation() {
    let config = json!({
        "num_layers": 2,
        "num_heads": 4,
        "vocab_size": 256
    });

    let backend = EmbeddedInferenceBackend::with_reference_weights(&config)
        .expect("EmbeddedInferenceBackend must construct with reference weights");

    assert_eq!(backend.endpoint(), "protocol://aien-inference-service");
    assert!(backend.check_health().await);
}

#[tokio::test]
async fn test_pure_rust_chat_template_formatting() {
    let messages = vec![
        json!({"role": "system", "content": "You are a helpful assistant."}),
        json!({"role": "user", "content": "Hello!"}),
        json!({"role": "assistant", "content": "Greetings! How can I assist?"}),
        json!({"role": "user", "content": "What is 2+2?"}),
    ];

    let prompt = format_messages_to_prompt(&messages, None);
    assert!(prompt.starts_with("<|system|>\nYou are a helpful assistant.</s>\n"));
    assert!(prompt.contains("<|user|>\nHello!</s>\n"));
    assert!(prompt.contains("<|assistant|>\nGreetings! How can I assist?</s>\n"));
    assert!(prompt.ends_with("<|user|>\nWhat is 2+2?</s>\n<|assistant|>\n"));
}

#[tokio::test]
async fn test_embedded_structured_tool_calling_flow() {
    let registry = SkillRegistry::new();
    let tools = registry.to_openai_tools();
    assert!(!tools.is_empty());

    let simulated_model_output = r#"I need to check system health.
```json
{
  "name": "telemetry_ping",
  "arguments": {}
}
```"#;

    let (content, tool_calls) = parse_structured_tool_calls(simulated_model_output);
    assert_eq!(content.as_deref(), Some("I need to check system health."));
    assert!(
        tool_calls.is_some(),
        "Tool call must be extracted from model output"
    );

    let calls = tool_calls.unwrap();
    assert_eq!(calls.len(), 1);
    let call = &calls[0];
    assert_eq!(call.function.name, "telemetry_ping");

    let args: serde_json::Value = serde_json::from_str(&call.function.arguments).unwrap();
    let req = SkillExecutionRequest {
        skill_name: call.function.name.clone(),
        arguments: args,
    };
    let exec_res = registry.execute(&req);
    assert!(
        exec_res.success,
        "Tool execution must succeed: {:?}",
        exec_res.error
    );
    let tool_output = exec_res.output;
    assert!(tool_output.contains("healthy"));

    let multi_turn_messages = vec![
        json!({"role": "user", "content": "Ping the system"}),
        json!({
            "role": "assistant",
            "content": content.unwrap_or_default(),
            "tool_calls": [{
                "id": call.id,
                "type": "function",
                "function": {
                    "name": call.function.name,
                    "arguments": call.function.arguments
                }
            }]
        }),
        json!({"role": "tool", "content": tool_output}),
    ];

    let second_turn_prompt = format_messages_to_prompt(&multi_turn_messages, Some(&tools));
    assert!(second_turn_prompt.contains("[Tool Output]:"));
    assert!(second_turn_prompt.contains("healthy"));
    assert!(second_turn_prompt.ends_with("<|assistant|>\n"));
}

#[tokio::test]
async fn test_agent_engine_with_embedded_backend_trait() {
    let config = json!({});
    let embedded_backend = EmbeddedInferenceBackend::with_reference_weights(&config).unwrap();
    let inference: Arc<dyn InferenceEngine> = Arc::new(embedded_backend);
    let skills = Arc::new(SkillRegistry::new());

    let _agent = AgentEngine::new(inference.clone(), skills, None);
    assert_eq!(inference.endpoint(), "protocol://aien-inference-service");

    let mut history = vec![
        json!({"role": "system", "content": "System prompt"}),
        json!({"role": "user", "content": "User goal"}),
        json!({"role": "assistant", "content": "Thought"}),
        json!({"role": "tool", "content": "Tool output"}),
    ];
    let pruned = AgentEngine::prune_context_window(&mut history, 5);
    assert!(pruned);
    assert_eq!(history.len(), 2);
}

#[tokio::test]
async fn test_real_model_embedded_execution_if_present() {
    let backend = EmbeddedInferenceBackend::load_or_fallback(None, None);
    assert!(
        backend.is_ok(),
        "EmbeddedInferenceBackend must load successfully: {:?}",
        backend.err()
    );

    let backend = backend.unwrap();
    let res = backend.generate("Hello world", None, None).await;
    assert!(res.is_ok(), "Generation must succeed: {:?}", res.err());
    let text = res.unwrap();
    assert!(!text.is_empty());
}

#[tokio::test]
async fn test_embedded_inference_chat_streaming() {
    let config = json!({});
    let backend = EmbeddedInferenceBackend::with_reference_weights(&config).unwrap();
    let messages = vec![
        json!({"role": "system", "content": "You are a helpful sovereign agent."}),
        json!({"role": "user", "content": "Status check"}),
    ];

    let stream_res = backend.stream_chat(&messages, Some(0.0), Some(4)).await;
    assert!(
        stream_res.is_ok(),
        "Streaming failed: {:?}",
        stream_res.err()
    );
    let mut stream = stream_res.unwrap();

    let mut full_sse = String::new();
    while let Some(chunk) = stream.next().await {
        assert!(chunk.is_ok(), "Chunk read failed: {:?}", chunk.err());
        let bytes = chunk.unwrap();
        let chunk_str = String::from_utf8_lossy(&bytes);
        full_sse.push_str(&chunk_str);
    }

    assert!(
        full_sse.contains("data: "),
        "SSE payload must contain data: prefix"
    );
    assert!(
        full_sse.contains("chat.completion.chunk"),
        "SSE payload must contain chat.completion.chunk"
    );
    assert!(
        full_sse.contains("data: [DONE]"),
        "SSE payload must terminate with [DONE]"
    );
}

#[tokio::test]
async fn test_gateway_with_embedded_inference_streaming() {
    use aegis::create_router;
    use aegis::heartbeat::HeartbeatEngine;
    use aegis::persistence::Database;
    use aegis::GatewayState;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    let config = json!({});
    let embedded_backend =
        Arc::new(EmbeddedInferenceBackend::with_reference_weights(&config).unwrap());
    let db = Arc::new(Database::open_in_memory().unwrap());
    let heartbeat = Arc::new(HeartbeatEngine::with_inference(
        60,
        db.clone(),
        embedded_backend.clone(),
    ));
    let skills = Arc::new(SkillRegistry::new());
    let agent = Arc::new(AgentEngine::new(
        embedded_backend.clone(),
        skills.clone(),
        None,
    ));

    let state = GatewayState {
        start_time: std::time::Instant::now(),
        inference: embedded_backend,
        db,
        heartbeat,
        skills,
        agent,
    };
    let app = create_router(state);

    let req_payload = serde_json::json!({
        "model": "atlas-lightning-omni",
        "messages": [
            {"role": "user", "content": "hello"}
        ],
        "stream": true,
        "max_tokens": 4
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
    assert!(
        text.contains("data: "),
        "Gateway SSE output must contain data: prefix"
    );
    assert!(
        text.contains("chat.completion.chunk"),
        "Gateway SSE output must contain chat.completion.chunk"
    );
    assert!(
        text.contains("data: [DONE]"),
        "Gateway SSE output must terminate with [DONE]"
    );
}
