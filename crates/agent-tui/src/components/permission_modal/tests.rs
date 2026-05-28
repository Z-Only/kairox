use super::*;
use crate::components::{
    Command, Component, CrossPanelEffect, FocusTarget, PermissionRequest, RiskLevel,
};
use crossterm::event::{Event, KeyCode};

fn test_ctx() -> EventContext<'static> {
    use agent_core::projection::SessionProjection;
    static PROJECTION: std::sync::OnceLock<SessionProjection> = std::sync::OnceLock::new();
    let projection = PROJECTION.get_or_init(SessionProjection::default);
    static SESSIONS: std::sync::OnceLock<Vec<crate::components::SessionInfo>> =
        std::sync::OnceLock::new();
    let sessions = SESSIONS.get_or_init(Vec::new);
    EventContext {
        focus: FocusTarget::Chat,
        current_session: projection,
        projects: &[],
        sessions,
        model_profile: "fake",
        sidebar_left_visible: true,
        sidebar_right_visible: true,
        workspace_id: Box::leak(Box::new(agent_core::WorkspaceId::new())),
        current_session_id: Box::leak(Box::new(None)),
    }
}

#[test]
fn modal_invisible_when_no_request() {
    let modal = PermissionModal::new();
    assert!(!modal.is_visible());
}

#[test]
fn modal_visible_on_destructive_effect() {
    let mut modal = PermissionModal::new();
    modal.handle_effect(&CrossPanelEffect::ShowPermissionPrompt(PermissionRequest {
        request_id: "req1".into(),
        tool_id: "shell.exec".into(),
        tool_preview: "rm -rf target/".into(),
        risk_level: RiskLevel::Destructive,
    }));
    assert!(modal.is_visible());
}

#[test]
fn modal_visible_on_write_risk() {
    let mut modal = PermissionModal::new();
    modal.handle_effect(&CrossPanelEffect::ShowPermissionPrompt(PermissionRequest {
        request_id: "req2".into(),
        tool_id: "shell.exec".into(),
        tool_preview: "cargo build".into(),
        risk_level: RiskLevel::Write,
    }));
    assert!(modal.is_visible());
}

#[test]
fn modal_visible_on_mcp_tool_risk() {
    let mut modal = PermissionModal::new();
    modal.handle_effect(&CrossPanelEffect::ShowPermissionPrompt(PermissionRequest {
        request_id: "req3".into(),
        tool_id: "echo".into(),
        tool_preview: "MCP tool invocation".into(),
        risk_level: RiskLevel::McpTool {
            server_id: "my-server".into(),
        },
    }));
    assert!(modal.is_visible());
}

#[test]
fn allow_sends_decide_and_dismisses() {
    let mut modal = PermissionModal::new();
    modal.request = Some(PermissionRequest {
        request_id: "req1".into(),
        tool_id: "shell.exec".into(),
        tool_preview: "rm -rf target/".into(),
        risk_level: RiskLevel::Destructive,
    });
    let key = Event::Key(crossterm::event::KeyEvent::new(
        KeyCode::Char('y'),
        crossterm::event::KeyModifiers::NONE,
    ));
    let (effects, commands) = modal.handle_event(&test_ctx(), &key);
    assert_eq!(commands.len(), 1);
    assert!(matches!(
        &commands[0],
        Command::DecidePermission { approved: true, .. }
    ));
    assert!(effects.contains(&CrossPanelEffect::DismissPermissionPrompt));
    assert!(!modal.is_visible());
}

#[test]
fn deny_sends_decide_false_and_dismisses() {
    let mut modal = PermissionModal::new();
    modal.request = Some(PermissionRequest {
        request_id: "req1".into(),
        tool_id: "shell.exec".into(),
        tool_preview: "rm -rf target/".into(),
        risk_level: RiskLevel::Destructive,
    });
    let key = Event::Key(crossterm::event::KeyEvent::new(
        KeyCode::Char('n'),
        crossterm::event::KeyModifiers::NONE,
    ));
    let (effects, commands) = modal.handle_event(&test_ctx(), &key);
    assert_eq!(commands.len(), 1);
    assert!(matches!(
        &commands[0],
        Command::DecidePermission {
            approved: false,
            ..
        }
    ));
    assert!(effects.contains(&CrossPanelEffect::DismissPermissionPrompt));
    assert!(!modal.is_visible());
}

#[test]
fn escape_denies_and_dismisses() {
    let mut modal = PermissionModal::new();
    modal.request = Some(PermissionRequest {
        request_id: "req1".into(),
        tool_id: "shell.exec".into(),
        tool_preview: "rm -rf target/".into(),
        risk_level: RiskLevel::Destructive,
    });
    let key = Event::Key(crossterm::event::KeyEvent::new(
        KeyCode::Esc,
        crossterm::event::KeyModifiers::NONE,
    ));
    let (_, commands) = modal.handle_event(&test_ctx(), &key);
    assert!(matches!(
        &commands[0],
        Command::DecidePermission {
            approved: false,
            ..
        }
    ));
    assert!(!modal.is_visible());
}

#[test]
fn trust_key_trusts_mcp_server_and_approves() {
    let mut modal = PermissionModal::new();
    modal.request = Some(PermissionRequest {
        request_id: "req1".into(),
        tool_id: "echo".into(),
        tool_preview: "MCP tool call".into(),
        risk_level: RiskLevel::McpTool {
            server_id: "my-server".into(),
        },
    });
    let key = Event::Key(crossterm::event::KeyEvent::new(
        KeyCode::Char('t'),
        crossterm::event::KeyModifiers::NONE,
    ));
    let (effects, commands) = modal.handle_event(&test_ctx(), &key);
    // Should produce TrustMcpServer + DecidePermission(approved=true)
    assert_eq!(commands.len(), 2);
    assert!(matches!(
        &commands[0],
        Command::TrustMcpServer { server_id } if server_id == "my-server"
    ));
    assert!(matches!(
        &commands[1],
        Command::DecidePermission { approved: true, .. }
    ));
    assert!(effects.contains(&CrossPanelEffect::DismissPermissionPrompt));
    assert!(!modal.is_visible());
}

#[test]
fn trust_key_ignored_for_non_mcp_risk() {
    let mut modal = PermissionModal::new();
    modal.request = Some(PermissionRequest {
        request_id: "req1".into(),
        tool_id: "shell.exec".into(),
        tool_preview: "rm -rf target/".into(),
        risk_level: RiskLevel::Destructive,
    });
    let key = Event::Key(crossterm::event::KeyEvent::new(
        KeyCode::Char('t'),
        crossterm::event::KeyModifiers::NONE,
    ));
    let (_, commands) = modal.handle_event(&test_ctx(), &key);
    assert!(commands.is_empty());
    assert!(modal.is_visible()); // Modal stays visible
}

#[test]
fn pending_prompts_queue_and_resolve_one_at_a_time() {
    let mut modal = PermissionModal::new();
    modal.handle_effect(&CrossPanelEffect::ShowPermissionPrompt(PermissionRequest {
        request_id: "req1".into(),
        tool_id: "shell.exec".into(),
        tool_preview: "write file".into(),
        risk_level: RiskLevel::Write,
    }));
    modal.handle_effect(&CrossPanelEffect::ShowPermissionPrompt(PermissionRequest {
        request_id: "req2".into(),
        tool_id: "shell.exec".into(),
        tool_preview: "delete file".into(),
        risk_level: RiskLevel::Destructive,
    }));

    assert_eq!(
        modal.request.as_ref().map(|req| req.request_id.as_str()),
        Some("req1")
    );

    let allow = Event::Key(crossterm::event::KeyEvent::new(
        KeyCode::Char('y'),
        crossterm::event::KeyModifiers::NONE,
    ));
    let (_, commands) = modal.handle_event(&test_ctx(), &allow);
    assert_eq!(
        commands,
        vec![Command::DecidePermission {
            request_id: "req1".into(),
            approved: true,
        }]
    );
    assert!(modal.is_visible());
    assert_eq!(
        modal.request.as_ref().map(|req| req.request_id.as_str()),
        Some("req2")
    );

    let deny = Event::Key(crossterm::event::KeyEvent::new(
        KeyCode::Char('n'),
        crossterm::event::KeyModifiers::NONE,
    ));
    let (_, commands) = modal.handle_event(&test_ctx(), &deny);
    assert_eq!(
        commands,
        vec![Command::DecidePermission {
            request_id: "req2".into(),
            approved: false,
        }]
    );
    assert!(!modal.is_visible());
}
