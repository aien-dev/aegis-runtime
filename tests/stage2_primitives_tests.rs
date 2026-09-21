use aegis::{
    ActionId, ActionReceipt, AegisEvent, ApprovalId, ContainmentLevel, DoctrineDecision, EventBus,
    EventEnvelope, EventId, Message, MessageId, Role, Run, RunBudget, RunId, RunState, Session,
    SessionId, SessionStatus, SystemMessage, TerminationReason, Trigger, UserMessage,
};

#[test]
fn test_domain_ids_and_prefixes() {
    let sess = SessionId::new();
    assert!(sess.as_str().starts_with("sess_"));

    let run = RunId::new();
    assert!(run.as_str().starts_with("run_"));

    let act = ActionId::new();
    assert!(act.as_str().starts_with("act_"));

    let appr = ApprovalId::new();
    assert!(appr.as_str().starts_with("appr_"));

    let evt = EventId::new();
    assert!(evt.as_str().starts_with("evt_"));
}

#[test]
fn test_session_lifecycle_and_branching() {
    let parent = Session::new(Some("Audit repo history".to_string()));
    assert_eq!(parent.status, SessionStatus::Active);
    assert_eq!(parent.objective.as_deref(), Some("Audit repo history"));
    assert!(parent.parent_session_id.is_none());

    let branch = parent.branch();
    assert_eq!(branch.parent_session_id, Some(parent.id.clone()));
    assert_ne!(branch.id, parent.id);
    assert_eq!(branch.objective, parent.objective);
}

#[test]
fn test_message_roles_and_ids() {
    let sys = Message::System(SystemMessage {
        id: MessageId::new(),
        content: "System instructions".to_string(),
        created_at: 1000,
    });
    assert_eq!(sys.role(), Role::System);
    assert!(sys.id().as_str().starts_with("msg_"));

    let usr = Message::User(UserMessage {
        id: MessageId::new(),
        content: "Analyze commits".to_string(),
        created_at: 1001,
    });
    assert_eq!(usr.role(), Role::User);
}

#[tokio::test]
async fn test_event_bus_publish_and_subscribe() {
    let bus = EventBus::new(16);
    let mut rx = bus.subscribe();

    let event = AegisEvent::RunStarted {
        run_id: "run_test1".to_string(),
        session_id: "sess_test1".to_string(),
        trigger: "UserMessage".to_string(),
    };
    let envelope = EventEnvelope::new(
        event,
        Some("sess_test1".to_string()),
        Some("run_test1".to_string()),
        1,
    );

    let sent = bus.publish(envelope.clone()).expect("publish must succeed");
    assert_eq!(sent, 1);

    let received = rx.recv().await.expect("recv must succeed");
    assert_eq!(received.sequence, 1);
    assert_eq!(received.session_id.as_deref(), Some("sess_test1"));
}

#[test]
fn test_doctrine_decision_and_action_receipt() {
    let decision = DoctrineDecision::AllowRestricted(vec!["network:none".to_string()]);
    let receipt = ActionReceipt {
        action_id: "act_123".to_string(),
        session_id: "sess_123".to_string(),
        run_id: "run_123".to_string(),
        capability: "shell.execute".to_string(),
        target: "cargo test".to_string(),
        decision: decision.clone(),
        output: "ok".to_string(),
        success: true,
        duration_ms: 45,
    };

    assert!(receipt.success);
    assert_eq!(receipt.decision, decision);
}

#[test]
fn test_orchestration_run_and_budget() {
    let budget = RunBudget {
        max_steps: 10,
        max_inference_calls: 15,
        max_actions: 20,
        max_wall_time_secs: 60,
    };
    let session_id = SessionId::new();
    let trigger = Trigger::Heartbeat;
    let mut run = Run::new(session_id.clone(), trigger, budget);

    assert_eq!(run.state, RunState::Running);
    assert_eq!(run.steps_used, 0);

    run.steps_used += 1;
    run.state = RunState::Completed;
    run.termination = Some(TerminationReason::Completed);

    assert_eq!(run.steps_used, 1);
    assert_eq!(run.state, RunState::Completed);
}
