//! Gateway hardening: operator token on every non-public route, loopback bind
//! by default, allowlist dispatch. Each test is a bypass attempt or a
//! realistic fault the rule must catch.

use aegis::{
    create_router, AgentEngine, Database, GatewayState, HeartbeatEngine, HttpInferenceBackend,
    OperatorAuth, SkillRegistry, WorkspaceCapability,
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use std::sync::Arc;
use std::time::Instant;
use tower::util::ServiceExt;

const TOKEN: &str = "auth-test-operator-token-0123456789abcdef";

fn state_with(auth: OperatorAuth, workspace: &std::path::Path) -> GatewayState {
    let db = Arc::new(Database::open_in_memory().unwrap());
    // Unreachable inference: no route here should need a model to answer.
    let inference = Arc::new(HttpInferenceBackend::new(
        Some("http://127.0.0.1:9/v1/chat/completions".to_string()),
        None,
    ));
    let heartbeat = Arc::new(HeartbeatEngine::new(60, db.clone()));
    let skills = Arc::new(SkillRegistry::with_workspace(
        WorkspaceCapability::new(workspace).unwrap(),
    ));
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
        operator: auth,
    }
}

fn authed() -> OperatorAuth {
    OperatorAuth::with_token(TOKEN).unwrap()
}

async fn send(
    state: &GatewayState,
    method: &str,
    uri: &str,
    headers: &[(&str, String)],
    body: Value,
) -> (StatusCode, Value) {
    let mut b = Request::builder()
        .method(method)
        .uri(uri)
        .header("content-type", "application/json");
    for (k, v) in headers {
        b = b.header(*k, v);
    }
    let res = create_router(state.clone())
        .oneshot(b.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = res.status();
    let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
        .await
        .unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn bearer(t: &str) -> Vec<(&'static str, String)> {
    vec![("authorization", format!("Bearer {}", t))]
}

/// Every mutating or effectful route, with a body that would act if admitted.
fn protected_routes() -> Vec<(&'static str, &'static str, Value)> {
    vec![
        ("POST", "/api/v1/shell", json!({"command": "ls"})),
        (
            "POST",
            "/api/v1/skills/execute",
            json!({"skill_name": "write_file", "arguments": {"path": "pwned.txt", "content": "x"}}),
        ),
        (
            "POST",
            "/api/v1/tasks",
            json!({"task_type": "shell_exec", "payload": "ls"}),
        ),
        ("POST", "/api/v1/agent/run", json!({"prompt": "hi", "max_turns": 1})),
        ("POST", "/api/v1/chat", json!({"prompt": "hi"})),
        ("POST", "/api/v1/heartbeat/tick", json!({})),
        (
            "POST",
            "/api/v1/discord/relay",
            json!({"channel_id": "c", "author": "a", "content": "hi"}),
        ),
        (
            "POST",
            "/v1/chat/completions",
            json!({"messages": [{"role": "user", "content": "hi"}]}),
        ),
        ("POST", "/v1/probe", json!({"state": {}, "probes": {}})),
        ("GET", "/ws", json!({})),
        ("GET", "/api/v1/ws", json!({})),
    ]
}

#[tokio::test]
async fn protected_routes_refuse_missing_or_wrong_tokens() {
    let ws = tempfile::tempdir().unwrap();
    let state = state_with(authed(), ws.path());
    let attempts: Vec<Vec<(&str, String)>> = vec![
        vec![],
        bearer("wrong-token-wrong-token-wrong-token-xx"),
        bearer(&TOKEN[..TOKEN.len() - 1]),
        bearer(&format!("{}x", TOKEN)),
        bearer(""),
        vec![("authorization", format!("bearer {}", TOKEN))],
        vec![("authorization", format!("Basic {}", TOKEN))],
        vec![("authorization", TOKEN.to_string())],
        vec![("x-forwarded-for", "127.0.0.1".to_string())],
        vec![("x-aegis-operator-token", "nope".to_string())],
    ];
    for (method, uri, body) in protected_routes() {
        for headers in &attempts {
            let (status, _) = send(&state, method, uri, headers, body.clone()).await;
            assert_eq!(
                status,
                StatusCode::UNAUTHORIZED,
                "{} {} admitted with headers {:?}",
                method,
                uri,
                headers
            );
        }
        // Token in the query string is never read.
        let q = format!("{}?token={}&operator_token={}", uri, TOKEN, TOKEN);
        let (status, _) = send(&state, method, &q, &[], body.clone()).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED, "{} via query", uri);
    }
    // The refused write never happened.
    assert!(!ws.path().join("pwned.txt").exists());
    assert!(state.db.list_pending_tasks().unwrap().is_empty());
}

#[tokio::test]
async fn path_tricks_do_not_reach_public_list() {
    let ws = tempfile::tempdir().unwrap();
    let state = state_with(authed(), ws.path());
    for uri in [
        "/health/../api/v1/shell",
        "/api/v1/health/../shell",
        "//api/v1/shell",
        "/api/v1/shell/",
        "/API/V1/SHELL",
    ] {
        let (status, _) = send(&state, "POST", uri, &[], json!({"command": "ls"})).await;
        assert!(
            status == StatusCode::UNAUTHORIZED || status == StatusCode::NOT_FOUND,
            "{} gave {}",
            uri,
            status
        );
        assert_ne!(status, StatusCode::OK, "{}", uri);
    }
    // A POST to a public read path is not public.
    let (status, _) = send(&state, "POST", "/api/v1/tasks", &[], json!({})).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn no_configured_token_means_protected_routes_are_closed() {
    let ws = tempfile::tempdir().unwrap();
    let state = state_with(OperatorAuth::disabled(), ws.path());
    for (method, uri, body) in protected_routes() {
        for headers in [vec![], bearer(""), bearer(TOKEN)] {
            let (status, _) = send(&state, method, uri, &headers, body.clone()).await;
            assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{} {}", method, uri);
        }
    }
    let (status, _) = send(&state, "GET", "/health", &[], json!({})).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn public_reads_stay_open() {
    let ws = tempfile::tempdir().unwrap();
    let state = state_with(authed(), ws.path());
    for uri in ["/health", "/api/v1/health", "/api/v1/skills", "/api/v1/tasks"] {
        let (status, _) = send(&state, "GET", uri, &[], json!({})).await;
        assert_eq!(status, StatusCode::OK, "{}", uri);
    }
}

#[tokio::test]
async fn correct_token_admits_catalogued_shell_only() {
    let ws = tempfile::tempdir().unwrap();
    let state = state_with(authed(), ws.path());
    let (status, body) = send(
        &state,
        "POST",
        "/api/v1/shell",
        &bearer(TOKEN),
        json!({"command": "ls"}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    // Either success or a probe refusal; never an allowlist refusal.
    if body["success"] != true {
        assert!(
            body["stderr"].as_str().unwrap_or("").contains("probe"),
            "{}",
            body
        );
    }
    let alt = vec![("x-aegis-operator-token", TOKEN.to_string())];
    for cmd in [
        "rm -r -f /",
        "find . -delete",
        "curl http://example.invalid",
        "ls; id",
        "ls $(id)",
        "git push",
        "ls -la /",
    ] {
        let (status, body) =
            send(&state, "POST", "/api/v1/shell", &alt, json!({ "command": cmd })).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["success"], false, "{:?} ran", cmd);
    }
}

#[tokio::test]
async fn skill_allowlist_holds_behind_the_token() {
    let ws = tempfile::tempdir().unwrap();
    let state = state_with(authed(), ws.path());
    for name in ["exec", "shell", "Bash_eval", "bash_eval ", "spawn_process"] {
        let (status, body) = send(
            &state,
            "POST",
            "/api/v1/skills/execute",
            &bearer(TOKEN),
            json!({"skill_name": name, "arguments": {"command": "ls"}}),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["success"], false, "{:?} dispatched", name);
        assert!(
            body["error"].as_str().unwrap().contains("allowlist"),
            "{}",
            body
        );
    }
}

#[tokio::test]
async fn websocket_handshake_needs_the_token() {
    let ws = tempfile::tempdir().unwrap();
    let state = state_with(authed(), ws.path());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let app = create_router(state);
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let url = format!("ws://127.0.0.1:{}/ws", port);
    let refused = tokio_tungstenite::connect_async(url.clone()).await;
    assert!(refused.is_err(), "unauthenticated WebSocket was accepted");

    use tokio_tungstenite::tungstenite::client::IntoClientRequest;
    let mut req = url.into_client_request().unwrap();
    req.headers_mut()
        .insert("authorization", format!("Bearer {}", TOKEN).parse().unwrap());
    assert!(tokio_tungstenite::connect_async(req).await.is_ok());
}

/// Runs `aegis serve ...` and expects it to exit on its own (refusal).
/// If it starts listening instead, it is killed and the test fails.
fn serve_must_refuse(args: &[&str], token: Option<&str>) -> String {
    let dir = tempfile::tempdir().unwrap();
    let mut cmd = std::process::Command::new(env!("CARGO_BIN_EXE_aegis"));
    cmd.arg("serve")
        .args(args)
        .args(["--port", "0", "--inference", "http"])
        .current_dir(dir.path())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    match token {
        Some(t) => cmd.env("AEGIS_OPERATOR_TOKEN", t),
        None => cmd.env_remove("AEGIS_OPERATOR_TOKEN"),
    };
    let mut child = cmd.spawn().unwrap();
    let deadline = Instant::now() + std::time::Duration::from_secs(30);
    let status = loop {
        if let Some(s) = child.try_wait().unwrap() {
            break s;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("aegis serve {:?} started instead of refusing", args);
        }
        std::thread::sleep(std::time::Duration::from_millis(50));
    };
    assert!(!status.success());
    let out = child.wait_with_output().unwrap();
    format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
}

#[test]
fn serve_refuses_non_loopback_bind_without_opt_in() {
    for addr in ["0.0.0.0", "::", "192.0.2.10"] {
        let text = serve_must_refuse(&["--bind", addr], None);
        assert!(text.contains("non-loopback"), "{}: {}", addr, text);
    }
}

#[test]
fn serve_refuses_a_short_operator_token() {
    let text = serve_must_refuse(&[], Some("short"));
    assert!(text.contains("at least"), "{}", text);
}
