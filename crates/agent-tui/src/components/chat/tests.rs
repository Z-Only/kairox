use super::*;
use crate::components::{
    EventContext, PermissionRequest, QueuedMessage, SessionInfo, SessionState,
};
use crate::keybindings::KeyAction;
use std::sync::OnceLock;

/// Shared static [`EventContext`] for tests. We leak the owned data so
/// that the references inside `EventContext` can be `'static`.
static TEST_CTX: OnceLock<EventContext<'static>> = OnceLock::new();

fn test_ctx() -> &'static EventContext<'static> {
    TEST_CTX.get_or_init(|| {
        let projection = Box::leak(Box::new(
            agent_core::projection::SessionProjection::default(),
        ));
        let sessions: &[SessionInfo] = Box::leak(Vec::<SessionInfo>::new().into_boxed_slice());
        let workspace_id = Box::leak(Box::new(agent_core::WorkspaceId::new()));
        let current_session_id = Box::leak(Box::new(None));
        EventContext {
            focus: FocusTarget::Chat,
            current_session: projection,
            sessions,
            model_profile: "test",
            permission_mode: agent_tools::PermissionMode::Suggest,
            sidebar_left_visible: true,
            sidebar_right_visible: false,
            workspace_id,
            current_session_id,
        }
    })
}

// A variant with sessions (Idle) so SendMessage can be emitted.
static TEST_CTX_WITH_SESSION: OnceLock<EventContext<'static>> = OnceLock::new();

fn test_ctx_with_session() -> &'static EventContext<'static> {
    TEST_CTX_WITH_SESSION.get_or_init(|| {
        let projection = Box::leak(Box::new(
            agent_core::projection::SessionProjection::default(),
        ));
        let session_id = agent_core::SessionId::new();
        let sessions: &[SessionInfo] = Box::leak(
            vec![SessionInfo {
                id: session_id.clone(),
                title: "test session".to_string(),
                model_profile: "fast".to_string(),
                state: SessionState::Idle,
                pinned: false,
                archived: false,
            }]
            .into_boxed_slice(),
        );
        let workspace_id = Box::leak(Box::new(agent_core::WorkspaceId::new()));
        let current_session_id = Box::leak(Box::new(Some(session_id)));
        EventContext {
            focus: FocusTarget::Chat,
            current_session: projection,
            sessions,
            model_profile: "test",
            permission_mode: agent_tools::PermissionMode::Suggest,
            sidebar_left_visible: true,
            sidebar_right_visible: false,
            workspace_id,
            current_session_id,
        }
    })
}

// A variant with a busy (Active) session so Enter must enqueue instead of send.
static TEST_CTX_BUSY_SESSION: OnceLock<EventContext<'static>> = OnceLock::new();

fn test_ctx_busy_session() -> &'static EventContext<'static> {
    TEST_CTX_BUSY_SESSION.get_or_init(|| {
        let projection = Box::leak(Box::new(
            agent_core::projection::SessionProjection::default(),
        ));
        let session_id = agent_core::SessionId::new();
        let sessions: &[SessionInfo] = Box::leak(
            vec![SessionInfo {
                id: session_id.clone(),
                title: "busy session".to_string(),
                model_profile: "fast".to_string(),
                state: SessionState::Active,
                pinned: false,
                archived: false,
            }]
            .into_boxed_slice(),
        );
        let workspace_id = Box::leak(Box::new(agent_core::WorkspaceId::new()));
        let current_session_id = Box::leak(Box::new(Some(session_id)));
        EventContext {
            focus: FocusTarget::Chat,
            current_session: projection,
            sessions,
            model_profile: "test",
            permission_mode: agent_tools::PermissionMode::Suggest,
            sidebar_left_visible: true,
            sidebar_right_visible: false,
            workspace_id,
            current_session_id,
        }
    })
}

#[test]
fn input_character_appends_to_content() {
    let mut panel = ChatPanel::new();
    let (effects, cmds) = panel.apply_key_action(KeyAction::InputCharacter('a'), test_ctx());
    assert!(effects.is_empty());
    assert!(cmds.is_empty());
    assert_eq!(panel.input_content, "a");
    assert_eq!(panel.input_cursor, 1);

    panel.apply_key_action(KeyAction::InputCharacter('b'), test_ctx());
    assert_eq!(panel.input_content, "ab");
    assert_eq!(panel.input_cursor, 2);
}

#[test]
fn backspace_removes_character() {
    let mut panel = ChatPanel::new();
    panel.apply_key_action(KeyAction::InputCharacter('x'), test_ctx());
    panel.apply_key_action(KeyAction::InputCharacter('y'), test_ctx());
    assert_eq!(panel.input_content, "xy");
    assert_eq!(panel.input_cursor, 2);

    let (effects, cmds) = panel.apply_key_action(KeyAction::InputBackspace, test_ctx());
    assert!(effects.is_empty());
    assert!(cmds.is_empty());
    assert_eq!(panel.input_content, "x");
    assert_eq!(panel.input_cursor, 1);

    // Backspace at start does nothing
    panel.apply_key_action(KeyAction::InputBackspace, test_ctx());
    assert_eq!(panel.input_content, "");
    assert_eq!(panel.input_cursor, 0);
    panel.apply_key_action(KeyAction::InputBackspace, test_ctx());
    assert_eq!(panel.input_content, "");
    assert_eq!(panel.input_cursor, 0);
}

#[test]
fn toggle_input_mode_switches() {
    let mut panel = ChatPanel::new();
    assert_eq!(panel.input_mode, InputMode::SingleLine);

    let (effects, cmds) = panel.apply_key_action(KeyAction::ToggleInputMode, test_ctx());
    assert!(effects.is_empty());
    assert!(cmds.is_empty());
    assert_eq!(panel.input_mode, InputMode::MultiLine);

    panel.apply_key_action(KeyAction::ToggleInputMode, test_ctx());
    assert_eq!(panel.input_mode, InputMode::SingleLine);
}

#[test]
fn permission_wait_state_allows_deny() {
    let mut panel = ChatPanel::new();
    panel.input_state = InputState::PermissionWait {
        request_id: "req-1".to_string(),
        pending_prompt: "rm file?".to_string(),
    };

    let (effects, cmds) = panel.apply_key_action(KeyAction::DenyPermission, test_ctx());
    assert_eq!(effects, vec![CrossPanelEffect::DismissPermissionPrompt]);
    assert_eq!(
        cmds,
        vec![Command::DecidePermission {
            request_id: "req-1".to_string(),
            approved: false,
        }]
    );
    assert_eq!(panel.input_state, InputState::Normal);

    // Allow also works
    panel.input_state = InputState::PermissionWait {
        request_id: "req-2".to_string(),
        pending_prompt: "write file?".to_string(),
    };
    let (effects2, cmds2) = panel.apply_key_action(KeyAction::AllowPermission, test_ctx());
    assert_eq!(effects2, vec![CrossPanelEffect::DismissPermissionPrompt]);
    assert_eq!(
        cmds2,
        vec![Command::DecidePermission {
            request_id: "req-2".to_string(),
            approved: true,
        }]
    );
    assert_eq!(panel.input_state, InputState::Normal);

    // DenyAllPermission also resolves to deny
    panel.input_state = InputState::PermissionWait {
        request_id: "req-3".to_string(),
        pending_prompt: "run cmd?".to_string(),
    };
    let (effects3, cmds3) = panel.apply_key_action(KeyAction::DenyAllPermission, test_ctx());
    assert_eq!(effects3, vec![CrossPanelEffect::DismissPermissionPrompt]);
    assert_eq!(
        cmds3,
        vec![Command::DecidePermission {
            request_id: "req-3".to_string(),
            approved: false,
        }]
    );
    assert_eq!(panel.input_state, InputState::Normal);
}

#[test]
fn history_navigation_works() {
    let mut panel = ChatPanel::new();

    // Pre-populate history
    panel.input_history = vec![
        "first message".to_string(),
        "second message".to_string(),
        "third message".to_string(),
    ];

    // HistoryUp from live position -> goes to most recent (index 2)
    let (effects, cmds) = panel.apply_key_action(KeyAction::InputHistoryUp, test_ctx());
    assert!(effects.is_empty());
    assert!(cmds.is_empty());
    assert_eq!(panel.input_history_index, Some(2));
    assert_eq!(panel.input_content, "third message");
    assert_eq!(panel.input_cursor, "third message".len());

    // HistoryUp again -> index 1
    panel.apply_key_action(KeyAction::InputHistoryUp, test_ctx());
    assert_eq!(panel.input_history_index, Some(1));
    assert_eq!(panel.input_content, "second message");

    // HistoryUp again -> index 0 (oldest)
    panel.apply_key_action(KeyAction::InputHistoryUp, test_ctx());
    assert_eq!(panel.input_history_index, Some(0));
    assert_eq!(panel.input_content, "first message");

    // HistoryUp at oldest -> stays at index 0
    panel.apply_key_action(KeyAction::InputHistoryUp, test_ctx());
    assert_eq!(panel.input_history_index, Some(0));
    assert_eq!(panel.input_content, "first message");

    // HistoryDown -> index 1
    panel.apply_key_action(KeyAction::InputHistoryDown, test_ctx());
    assert_eq!(panel.input_history_index, Some(1));
    assert_eq!(panel.input_content, "second message");

    // HistoryDown to index 2
    panel.apply_key_action(KeyAction::InputHistoryDown, test_ctx());
    assert_eq!(panel.input_history_index, Some(2));
    assert_eq!(panel.input_content, "third message");

    // HistoryDown from index 2 -> back to live position
    panel.apply_key_action(KeyAction::InputHistoryDown, test_ctx());
    assert_eq!(panel.input_history_index, None);
    assert_eq!(panel.input_content, "");
}

#[test]
fn send_input_clears_content_and_emits_command() {
    let mut panel = ChatPanel::new();
    // Type some content
    panel.apply_key_action(KeyAction::InputCharacter('h'), test_ctx());
    panel.apply_key_action(KeyAction::InputCharacter('i'), test_ctx());
    assert_eq!(panel.input_content, "hi");

    // Send
    let (effects, cmds) = panel.apply_key_action(KeyAction::SendInput, test_ctx_with_session());
    assert!(effects.is_empty());
    assert_eq!(cmds.len(), 1);

    match &cmds[0] {
        Command::SendMessage { content, .. } => assert_eq!(content, "hi"),
        other => panic!("expected SendMessage, got {:?}", other),
    }

    // Content should be cleared
    assert_eq!(panel.input_content, "");
    assert_eq!(panel.input_cursor, 0);

    // History should contain the sent message
    assert_eq!(panel.input_history, vec!["hi"]);

    // History index should be reset
    assert_eq!(panel.input_history_index, None);
}

#[test]
fn send_input_empty_does_nothing() {
    let mut panel = ChatPanel::new();
    let (effects, cmds) = panel.apply_key_action(KeyAction::SendInput, test_ctx_with_session());
    assert!(effects.is_empty());
    assert!(cmds.is_empty());
}

#[test]
fn send_input_no_sessions_no_command() {
    let mut panel = ChatPanel::new();
    panel.apply_key_action(KeyAction::InputCharacter('x'), test_ctx());
    let (effects, cmds) = panel.apply_key_action(KeyAction::SendInput, test_ctx());
    // No sessions -> no command emitted, but content is still consumed
    assert!(effects.is_empty());
    assert!(cmds.is_empty());
    // Content is still pushed to history and cleared
    assert_eq!(panel.input_content, "");
    assert_eq!(panel.input_history, vec!["x"]);
}

#[test]
fn escape_multiline_empty_switches_to_singleline() {
    let mut panel = ChatPanel::new();
    panel.input_mode = InputMode::MultiLine;
    assert!(panel.input_content.is_empty());

    let (effects, cmds) = panel.apply_key_action(KeyAction::Escape, test_ctx());
    assert!(effects.is_empty());
    assert!(cmds.is_empty());
    assert_eq!(panel.input_mode, InputMode::SingleLine);
}

#[test]
fn escape_multiline_nonempty_does_not_switch() {
    let mut panel = ChatPanel::new();
    panel.input_mode = InputMode::MultiLine;
    panel.apply_key_action(KeyAction::InputCharacter('a'), test_ctx());

    panel.apply_key_action(KeyAction::Escape, test_ctx());
    assert_eq!(panel.input_mode, InputMode::MultiLine);
}

#[test]
fn newline_only_in_multiline() {
    let mut panel = ChatPanel::new();
    // SingleLine: newline is a no-op
    panel.apply_key_action(KeyAction::InputNewline, test_ctx());
    assert_eq!(panel.input_content, "");

    // Switch to MultiLine
    panel.apply_key_action(KeyAction::ToggleInputMode, test_ctx());
    panel.apply_key_action(KeyAction::InputNewline, test_ctx());
    assert_eq!(panel.input_content, "\n");
    assert_eq!(panel.input_cursor, 1);
}

#[test]
fn handle_effect_show_permission_prompt_write_level() {
    let mut panel = ChatPanel::new();
    let req = PermissionRequest {
        request_id: "r1".to_string(),
        tool_id: "write_file".to_string(),
        tool_preview: "write to foo.txt".to_string(),
        risk_level: RiskLevel::Write,
    };
    panel.handle_effect(&CrossPanelEffect::ShowPermissionPrompt(req));
    assert!(matches!(
        panel.input_state,
        InputState::PermissionWait { ref request_id, .. } if request_id == "r1"
    ));
}

#[test]
fn handle_effect_show_permission_prompt_destructive_ignored() {
    let mut panel = ChatPanel::new();
    let req = PermissionRequest {
        request_id: "r2".to_string(),
        tool_id: "delete_file".to_string(),
        tool_preview: "rm -rf /".to_string(),
        risk_level: RiskLevel::Destructive,
    };
    panel.handle_effect(&CrossPanelEffect::ShowPermissionPrompt(req));
    assert_eq!(panel.input_state, InputState::Normal);
}

#[test]
fn handle_effect_dismiss_permission_prompt() {
    let mut panel = ChatPanel::new();
    panel.input_state = InputState::PermissionWait {
        request_id: "r1".to_string(),
        pending_prompt: "test".to_string(),
    };
    panel.handle_effect(&CrossPanelEffect::DismissPermissionPrompt);
    assert_eq!(panel.input_state, InputState::Normal);
}

#[test]
fn handle_effect_start_stop_streaming_noop() {
    let mut panel = ChatPanel::new();
    panel.handle_effect(&CrossPanelEffect::StartStreaming);
    panel.handle_effect(&CrossPanelEffect::StopStreaming);
    // Just verifying no panic and state unchanged.
    assert_eq!(panel.input_state, InputState::Normal);
}

#[test]
fn render_messages_basic() {
    use agent_core::facade::TaskGraphSnapshot;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let projection = agent_core::projection::SessionProjection {
        messages: vec![
            agent_core::projection::ProjectedMessage {
                role: agent_core::projection::ProjectedRole::User,
                content: "hello".to_string(),
            },
            agent_core::projection::ProjectedMessage {
                role: agent_core::projection::ProjectedRole::Assistant,
                content: "world".to_string(),
            },
        ],
        task_titles: vec![],
        task_graph: TaskGraphSnapshot::default(),
        token_stream: String::new(),
        cancelled: false,
        last_context_usage: None,
        model_limits: None,
        compaction: agent_core::projection::CompactionStatus::Idle,
    };

    let backend = TestBackend::new(80, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            render_messages(frame.area(), frame, &projection);
        })
        .expect("render_messages should not panic");
}

#[test]
fn render_messages_with_streaming_and_cancelled() {
    use agent_core::facade::TaskGraphSnapshot;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let projection = agent_core::projection::SessionProjection {
        messages: vec![agent_core::projection::ProjectedMessage {
            role: agent_core::projection::ProjectedRole::User,
            content: "go".to_string(),
        }],
        task_titles: vec![],
        task_graph: TaskGraphSnapshot::default(),
        token_stream: "thinking".to_string(),
        cancelled: true,
        last_context_usage: None,
        model_limits: None,
        compaction: agent_core::projection::CompactionStatus::Idle,
    };

    let backend = TestBackend::new(80, 10);
    let mut terminal = Terminal::new(backend).unwrap();
    terminal
        .draw(|frame| {
            render_messages(frame.area(), frame, &projection);
        })
        .expect("render_messages should not panic");
}

#[test]
fn queues_message_while_session_running() {
    let mut panel = ChatPanel::new();
    panel.apply_key_action(KeyAction::InputCharacter('h'), test_ctx_busy_session());
    panel.apply_key_action(KeyAction::InputCharacter('i'), test_ctx_busy_session());
    assert_eq!(panel.input_content, "hi");

    let (effects, cmds) = panel.apply_key_action(KeyAction::SendInput, test_ctx_busy_session());

    assert!(effects.is_empty());
    assert!(
        !cmds
            .iter()
            .any(|c| matches!(c, Command::SendMessage { .. })),
        "must not emit SendMessage while session busy, got {:?}",
        cmds
    );

    assert_eq!(panel.message_queue.len(), 1);
    assert_eq!(panel.message_queue[0].content, "hi");

    // input cleared so user can keep typing
    assert_eq!(panel.input_content, "");
    assert_eq!(panel.input_cursor, 0);
}

#[test]
fn queue_drain_returns_all_pending_in_fifo_order() {
    let mut panel = ChatPanel::new();
    panel.message_queue.push(QueuedMessage {
        content: "first".to_string(),
    });
    panel.message_queue.push(QueuedMessage {
        content: "second".to_string(),
    });

    let drained = panel.drain_queue();
    assert_eq!(drained.len(), 2);
    assert_eq!(drained[0].content, "first");
    assert_eq!(drained[1].content, "second");
    assert!(panel.message_queue.is_empty());
}

#[test]
fn send_input_idle_session_emits_send_not_queue() {
    let mut panel = ChatPanel::new();
    panel.apply_key_action(KeyAction::InputCharacter('h'), test_ctx_with_session());
    let (_, cmds) = panel.apply_key_action(KeyAction::SendInput, test_ctx_with_session());
    assert!(cmds
        .iter()
        .any(|c| matches!(c, Command::SendMessage { .. })));
    assert!(panel.message_queue.is_empty());
}

#[test]
fn focused_and_set_focused() {
    let mut panel = ChatPanel::new();
    assert!(!panel.focused());
    panel.set_focused(true);
    assert!(panel.focused());
    panel.set_focused(false);
    assert!(!panel.focused());
}
