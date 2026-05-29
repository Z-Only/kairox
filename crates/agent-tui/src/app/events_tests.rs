use super::*;
use agent_core::{
    AgentId, DomainEvent, EventPayload, PrivacyClassification, SessionId, WorkspaceId,
};

fn event(workspace_id: &WorkspaceId, session_id: &SessionId, payload: EventPayload) -> DomainEvent {
    DomainEvent::new(
        workspace_id.clone(),
        session_id.clone(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        payload,
    )
}

#[test]
fn skipped_compaction_event_marks_compaction_not_in_flight() {
    let workspace_id = WorkspaceId::from_string("wrk_test".into());
    let session_id = SessionId::from_string("ses_test".into());
    let mut app = App::new("test", workspace_id.clone());
    app.current_session_id = Some(session_id.clone());
    // Pretend a compaction was in flight so we can observe that the
    // Skipped event clears the in-flight flag rather than leaving
    // the spinner stuck on screen.
    app.compacting = true;

    app.handle_domain_event(&event(
        &workspace_id,
        &session_id,
        EventPayload::ContextCompactionSkipped {
            reason: agent_core::events::CompactionSkipReason::AlreadyCompacting,
            ratio: 0.42,
        },
    ));

    assert!(
        !app.compacting,
        "ContextCompactionSkipped should clear the in-flight compaction flag"
    );
}

#[test]
fn resolving_permission_event_keeps_other_pending_prompts_visible() {
    let workspace_id = WorkspaceId::from_string("wrk_test".into());
    let session_id = SessionId::from_string("ses_test".into());
    let mut app = App::new("test", workspace_id.clone());
    app.current_session_id = Some(session_id.clone());
    app.state.sessions = vec![crate::components::SessionInfo {
        id: session_id.clone(),
        title: "test".into(),
        model_profile: "test".into(),
        state: crate::components::SessionState::Idle,
        pinned: false,
        archived: false,
        project_id: None,
        worktree_path: None,
        branch: None,
        visibility: None,
    }];

    app.handle_domain_event(&event(
        &workspace_id,
        &session_id,
        EventPayload::PermissionRequested {
            request_id: "req1".into(),
            tool_id: "shell.exec".into(),
            preview: "write file".into(),
        },
    ));
    app.handle_domain_event(&event(
        &workspace_id,
        &session_id,
        EventPayload::PermissionRequested {
            request_id: "req2".into(),
            tool_id: "mcp.beta.echo".into(),
            preview: "MCP tool call".into(),
        },
    ));

    assert_eq!(
        app.permission_modal
            .request
            .as_ref()
            .map(|request| request.request_id.as_str()),
        Some("req1")
    );

    app.handle_domain_event(&event(
        &workspace_id,
        &session_id,
        EventPayload::PermissionGranted {
            request_id: "req1".into(),
        },
    ));

    assert!(app.permission_modal.is_visible());
    assert_eq!(
        app.state.sessions[0].state,
        crate::components::SessionState::AwaitingPermission
    );
    assert_eq!(
        app.permission_modal
            .request
            .as_ref()
            .map(|request| request.request_id.as_str()),
        Some("req2")
    );
}
