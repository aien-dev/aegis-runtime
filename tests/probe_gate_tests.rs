//! Probe gating is wired into every dispatch path. With the strictest
//! threshold the reference oracle refuses, so a path that skips the gate
//! would run the command. Separate test binary: it sets a process env var.

use aegis::{SkillExecutionRequest, SkillRegistry, WorkspaceCapability};

#[tokio::test]
async fn strict_probe_threshold_refuses_on_every_path() {
    std::env::set_var("AIEN_PROBE_ENFORCE", "1.0");
    let dir = tempfile::tempdir().unwrap();
    let registry = SkillRegistry::with_workspace(WorkspaceCapability::new(dir.path()).unwrap());

    let req = SkillExecutionRequest {
        skill_name: "bash_eval".to_string(),
        arguments: serde_json::json!({ "command": "ls" }),
    };
    let res = registry.execute_gated(&req).await;
    assert!(!res.success, "skill ran past the probe gate");
    assert!(res.error.unwrap_or_default().contains("probe"));

    let shell = registry
        .workspace()
        .dispatch_shell_gated("ls", None, 5)
        .await;
    let err = shell.expect_err("shell ran past the probe gate");
    assert!(err.to_string().contains("probe"), "{}", err);

    let write = SkillExecutionRequest {
        skill_name: "write_file".to_string(),
        arguments: serde_json::json!({ "path": "gated.txt", "content": "x" }),
    };
    assert!(!registry.execute_gated(&write).await.success);
    assert!(!dir.path().join("gated.txt").exists());
}
