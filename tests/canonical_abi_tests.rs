use aegis::{
    ActionId, AegisEvent, AgentEvent, AgentId, AgentState, ApprovalId, Digest32, EventId,
    ProtocolVersion, RunId, Session, SessionId,
};

#[test]
fn test_aegis_ids_bidirectional_conversion_with_protocol_types() {
    let sess_id = SessionId::new();
    let canon_sess: aien_protocol_types::SessionId = sess_id.clone().into();
    let back_sess: SessionId = canon_sess.into();
    assert!(back_sess.as_str().starts_with("sess_"));

    let run_id = RunId::new();
    let canon_run: aien_protocol_types::RunId = run_id.clone().into();
    let back_run: RunId = canon_run.into();
    assert!(back_run.as_str().starts_with("run_"));

    let act_id = ActionId::new();
    let canon_act: aien_protocol_types::ActionId = act_id.clone().into();
    let back_act: ActionId = canon_act.into();
    assert!(back_act.as_str().starts_with("act_"));

    let appr_id = ApprovalId::new();
    let canon_appr: aien_protocol_types::ApprovalId = appr_id.clone().into();
    let back_appr: ApprovalId = canon_appr.into();
    assert!(back_appr.as_str().starts_with("appr_"));

    let evt_id = EventId::new();
    let canon_evt: aien_protocol_types::EventId = evt_id.clone().into();
    let back_evt: EventId = canon_evt.into();
    assert!(back_evt.as_str().starts_with("evt_"));
}

#[test]
fn test_aegis_session_to_canonical_agent_state_and_digest() {
    let mut session = Session::new(Some("Audit and verify canonical ABI migration".to_string()));
    session.budget.max_total_tokens = 8192;

    let agent_id = AgentId::new_v4();
    let state: AgentState = session.to_agent_state(&agent_id, "GB10-MAX-Qwen");

    assert_eq!(state.abi_version, ProtocolVersion::new(1, 0));
    assert_eq!(state.agent.agent_id, agent_id);
    assert_eq!(state.agent.model_profile, "GB10-MAX-Qwen");
    assert_eq!(state.session.session_id, session.id.clone().into());
    assert_eq!(
        state.objective.as_ref().unwrap().description,
        "Audit and verify canonical ABI migration"
    );
    assert_eq!(state.budget.max_tokens, Some(8192));
    assert!(state.parent.is_none());

    let digest1 = state
        .compute_digest()
        .expect("must compute deterministic digest");
    let digest2 = state
        .compute_digest()
        .expect("must compute deterministic digest");
    assert_eq!(digest1, digest2);
    assert_ne!(digest1, Digest32::ZERO);
}

#[test]
fn test_canonical_agent_state_branching_lineage() {
    let parent = Session::new(Some("Parent primary execution context".to_string()));
    let child = parent.branch();

    let agent_id = AgentId::new_v4();
    let parent_state = parent.to_agent_state(&agent_id, "GB10-MAX-Qwen");
    let child_state = child.to_agent_state(&agent_id, "GB10-MAX-Qwen");

    assert!(parent_state.parent.is_none());
    assert!(child_state.parent.is_some());

    let parent_canon_id: aien_protocol_types::SessionId = parent.id.into();
    assert_eq!(
        child_state.parent.as_ref().unwrap().state_id,
        parent_canon_id.0
    );
}

#[test]
fn test_aegis_event_to_canonical_agent_event_mapping() {
    let created = AegisEvent::SessionCreated {
        session_id: "sess_test1".to_string(),
        objective: Some("Objective".to_string()),
    };
    assert_eq!(
        created.to_canonical_agent_event(),
        Some(AgentEvent::SessionCreated)
    );

    let closed = AegisEvent::SessionClosed {
        session_id: "sess_test1".to_string(),
    };
    assert_eq!(
        closed.to_canonical_agent_event(),
        Some(AgentEvent::SessionClosed)
    );

    let branched = AegisEvent::SessionBranched {
        session_id: "sess_child".to_string(),
        parent_session_id: "sess_parent".to_string(),
    };
    match branched.to_canonical_agent_event() {
        Some(AgentEvent::ContextBranched(_)) => {}
        other => panic!("Expected ContextBranched, got {:?}", other),
    }

    let run_started = AegisEvent::RunStarted {
        run_id: "run_alpha".to_string(),
        session_id: "sess_alpha".to_string(),
        trigger: "UserMessage".to_string(),
    };
    match run_started.to_canonical_agent_event() {
        Some(AgentEvent::RunStarted(_)) => {}
        other => panic!("Expected RunStarted, got {:?}", other),
    }

    let act_req = AegisEvent::ActionRequested {
        action_id: "act_101".to_string(),
        capability: "shell.execute".to_string(),
    };
    match act_req.to_canonical_agent_event() {
        Some(AgentEvent::ActionRequested(_)) => {}
        other => panic!("Expected ActionRequested, got {:?}", other),
    }
}
