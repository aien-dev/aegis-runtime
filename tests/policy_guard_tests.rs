use aegis::policy_guard::ProbePolicyGuard;

#[tokio::test]
async fn test_probe_policy_guard_reference_execution() {
    // Permissive threshold to verify pipeline execution
    let guard = ProbePolicyGuard::new_reference(0.0);
    let allowed = guard.check_action("read_file", "src/lib.rs").await.unwrap();
    assert!(allowed);
}

#[tokio::test]
async fn test_probe_policy_guard_fails_closed_on_strict_threshold() {
    // Impossibly high threshold forces fail-closed behavior
    let guard = ProbePolicyGuard::new_reference(0.999999);
    let result = guard.check_action("rm_rf", "/").await;
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(err_msg.contains("blocked by sovereign probe policy"));
}
