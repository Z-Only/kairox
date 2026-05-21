use super::*;
use crate::components::{
    EventContext, PermissionRequest, QueuedMessage, SessionInfo, SessionState,
};
use crate::keybindings::KeyAction;
use std::sync::OnceLock;

fn fixture_attachment(name: &str) -> agent_core::AttachmentInfo {
    agent_core::AttachmentInfo {
        path: format!("/tmp/{name}"),
        name: name.to_string(),
        mime_type: "text/plain".to_string(),
    }
}

fn agent_tui_manifest_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")
}

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
                project_id: None,
                worktree_path: None,
                branch: None,
                visibility: None,
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
                project_id: None,
                worktree_path: None,
                branch: None,
                visibility: None,
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
        Command::SendMessage {
            content,
            attachments,
            ..
        } => {
            assert_eq!(content, "hi");
            assert!(attachments.is_empty());
        }
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
fn attach_command_adds_pending_attachment() {
    let manifest = agent_tui_manifest_path();
    let canonical = manifest.canonicalize().unwrap();
    let mut panel = ChatPanel::new();

    for ch in format!(":attach {}", manifest.display()).chars() {
        panel.apply_key_action(KeyAction::InputCharacter(ch), test_ctx_with_session());
    }
    let (effects, cmds) = panel.apply_key_action(KeyAction::SendInput, test_ctx_with_session());

    assert!(effects.is_empty());
    assert!(cmds.is_empty());
    assert_eq!(panel.pending_attachments.len(), 1);
    assert_eq!(
        panel.pending_attachments[0],
        agent_core::AttachmentInfo {
            path: canonical.display().to_string(),
            name: "Cargo.toml".to_string(),
            mime_type: "application/toml".to_string(),
        }
    );
    assert!(panel.input_content.is_empty());
}

#[test]
fn detach_command_clears_pending_attachments() {
    let mut panel = ChatPanel::new();
    panel
        .pending_attachments
        .push(fixture_attachment("first.txt"));
    panel
        .pending_attachments
        .push(fixture_attachment("second.txt"));

    for ch in ":detach".chars() {
        panel.apply_key_action(KeyAction::InputCharacter(ch), test_ctx_with_session());
    }
    let (effects, cmds) = panel.apply_key_action(KeyAction::SendInput, test_ctx_with_session());

    assert!(effects.is_empty());
    assert!(cmds.is_empty());
    assert!(panel.pending_attachments.is_empty());
    assert!(panel.input_content.is_empty());
}

#[test]
fn send_input_emits_attachment_payloads_and_clears_pending() {
    let mut panel = ChatPanel::new();
    panel
        .pending_attachments
        .push(fixture_attachment("notes.txt"));
    for ch in "review this".chars() {
        panel.apply_key_action(KeyAction::InputCharacter(ch), test_ctx_with_session());
    }

    let (effects, cmds) = panel.apply_key_action(KeyAction::SendInput, test_ctx_with_session());

    assert!(effects.is_empty());
    assert_eq!(cmds.len(), 1);
    match &cmds[0] {
        Command::SendMessage {
            content,
            attachments,
            ..
        } => {
            assert_eq!(content, "review this");
            assert_eq!(attachments, &vec![fixture_attachment("notes.txt")]);
        }
        other => panic!("expected SendMessage, got {other:?}"),
    }
    assert!(panel.pending_attachments.is_empty());
}

#[test]
fn attachment_only_send_emits_message() {
    let mut panel = ChatPanel::new();
    panel
        .pending_attachments
        .push(fixture_attachment("screenshot.png"));

    let (_effects, cmds) = panel.apply_key_action(KeyAction::SendInput, test_ctx_with_session());

    assert_eq!(cmds.len(), 1);
    assert!(matches!(
        &cmds[0],
        Command::SendMessage {
            content,
            attachments,
            ..
        } if content.is_empty() && attachments == &vec![fixture_attachment("screenshot.png")]
    ));
    assert!(panel.pending_attachments.is_empty());
}

#[test]
fn attachment_labels_are_compact() {
    assert_eq!(
        format_attachment_labels(&[
            fixture_attachment("one.txt"),
            fixture_attachment("two.txt"),
            fixture_attachment("three.txt"),
        ]),
        "[one.txt] [two.txt] [+1]"
    );
}

#[test]
fn file_mentions_filter_workspace_files_and_attach_selection() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let selected = root.join("src/main.rs").canonicalize().unwrap();
    let mut panel = ChatPanel::new();
    panel.set_workspace_files(
        root,
        vec!["Cargo.toml".to_string(), "src/main.rs".to_string()],
    );

    for ch in "@main".chars() {
        panel.apply_key_action(KeyAction::InputCharacter(ch), test_ctx_with_session());
    }

    assert!(panel.file_mentions_visible());
    assert_eq!(panel.file_mention_matches(), &["src/main.rs".to_string()]);

    let (effects, cmds) = panel.apply_key_action(KeyAction::SendInput, test_ctx_with_session());

    assert!(effects.is_empty());
    assert!(cmds.is_empty());
    assert_eq!(panel.input_content, "@src/main.rs ");
    assert_eq!(panel.input_cursor, panel.input_content.len());
    assert!(!panel.file_mentions_visible());
    assert_eq!(
        panel.pending_attachments,
        vec![agent_core::AttachmentInfo {
            path: selected.display().to_string(),
            name: "main.rs".to_string(),
            mime_type: "text/x-rust".to_string(),
        }]
    );
}

#[test]
fn file_mentions_can_select_next_match_before_accepting() {
    let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let selected = root.join("src/main.rs").canonicalize().unwrap();
    let mut panel = ChatPanel::new();
    panel.set_workspace_files(
        root,
        vec!["Cargo.toml".to_string(), "src/main.rs".to_string()],
    );

    panel.apply_key_action(KeyAction::InputCharacter('@'), test_ctx_with_session());
    assert_eq!(
        panel.file_mention_matches(),
        &["Cargo.toml".to_string(), "src/main.rs".to_string()]
    );

    panel.apply_key_action(KeyAction::InputHistoryDown, test_ctx_with_session());
    let (effects, cmds) = panel.apply_key_action(KeyAction::SendInput, test_ctx_with_session());

    assert!(effects.is_empty());
    assert!(cmds.is_empty());
    assert_eq!(panel.input_content, "@src/main.rs ");
    assert_eq!(
        panel.pending_attachments,
        vec![agent_core::AttachmentInfo {
            path: selected.display().to_string(),
            name: "main.rs".to_string(),
            mime_type: "text/x-rust".to_string(),
        }]
    );
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
    panel
        .pending_attachments
        .push(fixture_attachment("queued.txt"));
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
    assert_eq!(
        panel.message_queue[0].attachments,
        vec![fixture_attachment("queued.txt")]
    );

    // input cleared so user can keep typing
    assert_eq!(panel.input_content, "");
    assert_eq!(panel.input_cursor, 0);
    assert!(panel.pending_attachments.is_empty());
}

#[test]
fn queue_drain_returns_all_pending_in_fifo_order() {
    let mut panel = ChatPanel::new();
    panel.message_queue.push(QueuedMessage {
        content: "first".to_string(),
        attachments: vec![fixture_attachment("first.txt")],
    });
    panel.message_queue.push(QueuedMessage {
        content: "second".to_string(),
        attachments: vec![fixture_attachment("second.txt")],
    });

    let drained = panel.drain_queue();
    assert_eq!(drained.len(), 2);
    assert_eq!(drained[0].content, "first");
    assert_eq!(
        drained[0].attachments,
        vec![fixture_attachment("first.txt")]
    );
    assert_eq!(drained[1].content, "second");
    assert_eq!(
        drained[1].attachments,
        vec![fixture_attachment("second.txt")]
    );
    assert!(panel.message_queue.is_empty());
}

#[test]
fn queue_selection_and_reorder_controls_target_visible_messages() {
    let mut panel = ChatPanel::new();
    for content in ["first", "second", "third"] {
        panel.message_queue.push(QueuedMessage {
            content: content.to_string(),
            attachments: Vec::new(),
        });
    }

    assert_eq!(panel.selected_queue_index(), Some(0));
    assert!(panel.select_next_queued_message());
    assert_eq!(panel.selected_queue_index(), Some(1));

    assert!(panel.move_selected_queued_message_down());
    assert_eq!(
        panel
            .message_queue
            .iter()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>(),
        vec!["first", "third", "second"]
    );
    assert_eq!(panel.selected_queue_index(), Some(2));

    assert!(panel.move_selected_queued_message_up());
    assert_eq!(
        panel
            .message_queue
            .iter()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>(),
        vec!["first", "second", "third"]
    );
    assert_eq!(panel.selected_queue_index(), Some(1));

    assert!(panel.select_previous_queued_message());
    assert_eq!(panel.selected_queue_index(), Some(0));
}

#[test]
fn queue_delete_keeps_current_draft_and_clamps_selection() {
    let mut panel = ChatPanel::new();
    panel.input_content = "current draft".to_string();
    panel.input_cursor = panel.input_content.len();
    for content in ["first", "second"] {
        panel.message_queue.push(QueuedMessage {
            content: content.to_string(),
            attachments: Vec::new(),
        });
    }
    panel.select_next_queued_message();

    let removed = panel.delete_selected_queued_message();

    assert_eq!(
        removed.map(|message| message.content),
        Some("second".to_string())
    );
    assert_eq!(panel.input_content, "current draft");
    assert_eq!(panel.selected_queue_index(), Some(0));
    assert_eq!(panel.message_queue[0].content, "first");
}

#[test]
fn queue_restore_selected_message_moves_it_into_composer() {
    let mut panel = ChatPanel::new();
    panel.input_content = "draft to replace".to_string();
    panel
        .pending_attachments
        .push(fixture_attachment("draft.txt"));
    panel.message_queue.push(QueuedMessage {
        content: "first".to_string(),
        attachments: Vec::new(),
    });
    panel.message_queue.push(QueuedMessage {
        content: "second".to_string(),
        attachments: vec![fixture_attachment("second.txt")],
    });
    panel.select_next_queued_message();

    let restored = panel.restore_selected_queued_message_for_edit();

    assert!(restored);
    assert_eq!(panel.input_content, "second");
    assert_eq!(panel.input_cursor, "second".len());
    assert_eq!(
        panel.pending_attachments,
        vec![fixture_attachment("second.txt")]
    );
    assert_eq!(
        panel
            .message_queue
            .iter()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>(),
        vec!["first"]
    );
}

#[test]
fn queue_strip_renders_multiple_messages_and_selected_row() {
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    let queue = ["first", "second", "third"]
        .into_iter()
        .map(|content| QueuedMessage {
            content: content.to_string(),
            attachments: Vec::new(),
        })
        .collect::<Vec<_>>();
    let backend = TestBackend::new(80, 5);
    let mut terminal = Terminal::new(backend).unwrap();

    terminal
        .draw(|frame| {
            render_queue_strip(frame.area(), frame, &queue, Some(1));
        })
        .expect("render_queue_strip should not panic");

    let output = terminal.backend().to_string();
    assert!(output.contains("Q1 first"), "{output}");
    assert!(output.contains("> Q2 second"), "{output}");
    assert!(output.contains("Q3 third"), "{output}");
    assert!(output.contains("3 queued"), "{output}");
}

#[test]
fn queue_send_now_command_keeps_message_until_runtime_accepts_it() {
    let mut panel = ChatPanel::new();
    panel.message_queue.push(QueuedMessage {
        content: "first".to_string(),
        attachments: Vec::new(),
    });
    panel.message_queue.push(QueuedMessage {
        content: "second".to_string(),
        attachments: vec![fixture_attachment("second.txt")],
    });
    panel.select_next_queued_message();

    let (_, cmds) = panel.apply_key_action(
        KeyAction::ApplyQueueAction(crate::components::QueueAction::SendSelectedNow),
        test_ctx_with_session(),
    );

    assert_eq!(panel.message_queue.len(), 2);
    assert!(matches!(
        cmds.as_slice(),
        [Command::SendQueuedMessageNow { queue_index: 1, .. }]
    ));
}

#[test]
fn local_queue_slash_commands_edit_delete_and_reorder() {
    let mut panel = ChatPanel::new();
    for content in ["first", "second", "third"] {
        panel.message_queue.push(QueuedMessage {
            content: content.to_string(),
            attachments: Vec::new(),
        });
    }

    panel.input_content = ":queue next".to_string();
    panel.input_cursor = panel.input_content.len();
    panel.apply_key_action(KeyAction::SendInput, test_ctx_with_session());
    assert_eq!(panel.selected_queue_index(), Some(1));

    panel.input_content = ":queue up".to_string();
    panel.input_cursor = panel.input_content.len();
    panel.apply_key_action(KeyAction::SendInput, test_ctx_with_session());
    assert_eq!(
        panel
            .message_queue
            .iter()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>(),
        vec!["second", "first", "third"]
    );

    panel.input_content = ":queue delete".to_string();
    panel.input_cursor = panel.input_content.len();
    panel.apply_key_action(KeyAction::SendInput, test_ctx_with_session());
    assert_eq!(
        panel
            .message_queue
            .iter()
            .map(|message| message.content.as_str())
            .collect::<Vec<_>>(),
        vec!["first", "third"]
    );
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
