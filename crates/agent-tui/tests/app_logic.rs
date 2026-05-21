//! TUI App logic integration tests.
//!
//! These tests verify the TUI's core logic (command dispatch, event handling,
//! state transitions) WITHOUT requiring a real terminal. They use the
//! FakeModelClient + in-memory event store to exercise the full
//! LocalRuntime → App event pipeline.

use agent_core::projection::ProjectedRole;
use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_skills::{FileSkillRegistry, SkillRoot, SkillSourceKind};
use agent_store::SqliteEventStore;
use agent_tools::PermissionMode;
use futures::StreamExt;
use std::sync::Arc;

/// Helper: create a runtime with FakeModelClient.
async fn make_runtime() -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["Hello from TUI test!".into()]);
    LocalRuntime::new(store, model).with_permission_mode(PermissionMode::Suggest)
}

// ---------------------------------------------------------------------------
// Test: Workspace → Session → SendMessage → Projection
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tui_send_message_produces_user_and_assistant_messages() {
    let runtime = make_runtime().await;

    let workspace = runtime
        .open_workspace("/tmp/tui-test-workspace".into())
        .await
        .unwrap();

    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "hello from TUI".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    let projection = runtime.get_session_projection(session_id).await.unwrap();
    assert_eq!(
        projection.messages.len(),
        2,
        "Expected user + assistant messages"
    );
    assert_eq!(projection.messages[0].role, ProjectedRole::User);
    assert_eq!(projection.messages[0].content, "hello from TUI");
    assert_eq!(projection.messages[1].role, ProjectedRole::Assistant);
    assert_eq!(projection.messages[1].content, "Hello from TUI test!");
}

// ---------------------------------------------------------------------------
// Test: Event stream mirrors projection data
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tui_event_stream_matches_projection() {
    let runtime = make_runtime().await;

    let workspace = runtime
        .open_workspace("/tmp/tui-event-test".into())
        .await
        .unwrap();

    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
        })
        .await
        .unwrap();

    let mut event_stream = runtime.subscribe_session(session_id.clone());

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "test events".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    // Collect events from stream
    let mut received_events = Vec::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(500);
    loop {
        tokio::select! {
            event = event_stream.next() => {
                match event {
                    Some(e) => received_events.push(e),
                    None => break,
                }
            }
            _ = tokio::time::sleep_until(deadline) => break,
        }
    }

    assert!(
        !received_events.is_empty(),
        "Should receive at least one event"
    );

    let event_types: Vec<&str> = received_events
        .iter()
        .map(|e| e.event_type.as_str())
        .collect();

    assert!(
        event_types.contains(&"UserMessageAdded"),
        "Expected UserMessageAdded in events: {event_types:?}"
    );
    assert!(
        event_types.contains(&"AssistantMessageCompleted"),
        "Expected AssistantMessageCompleted in events: {event_types:?}"
    );
}

// ---------------------------------------------------------------------------
// Test: Multiple sessions, projection isolation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tui_multiple_sessions_have_isolated_projections() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["response".into()]);
    let runtime = LocalRuntime::new(store, model).with_permission_mode(PermissionMode::Suggest);

    let workspace = runtime
        .open_workspace("/tmp/tui-multi-session".into())
        .await
        .unwrap();

    let s1 = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
        })
        .await
        .unwrap();
    let s2 = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
        })
        .await
        .unwrap();

    // Send to s1 only
    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id.clone(),
            session_id: s1.clone(),
            content: "message for s1".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    let proj1 = runtime.get_session_projection(s1).await.unwrap();
    let proj2 = runtime.get_session_projection(s2).await.unwrap();

    assert_eq!(proj1.messages.len(), 2, "Session 1 should have 2 messages");
    assert_eq!(proj2.messages.len(), 0, "Session 2 should have 0 messages");
}

// ---------------------------------------------------------------------------
// Test: Session cancellation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tui_cancel_session_marks_cancelled() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["response".into()]);
    let runtime = LocalRuntime::new(store, model).with_permission_mode(PermissionMode::Suggest);

    let workspace = runtime
        .open_workspace("/tmp/tui-cancel".into())
        .await
        .unwrap();

    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
        })
        .await
        .unwrap();

    runtime
        .cancel_session(workspace.workspace_id, session_id.clone())
        .await
        .unwrap();

    let projection = runtime.get_session_projection(session_id).await.unwrap();
    assert!(
        projection.cancelled,
        "Session should be marked as cancelled"
    );
}

// ---------------------------------------------------------------------------
// Test: Trace entries are populated
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tui_trace_entries_populated_after_message() {
    let runtime = make_runtime().await;

    let workspace = runtime
        .open_workspace("/tmp/tui-trace".into())
        .await
        .unwrap();

    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "trace me".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    let trace = runtime.get_trace(session_id).await.unwrap();
    assert!(
        !trace.is_empty(),
        "Trace should have entries after a message"
    );

    let event_types: Vec<&str> = trace.iter().map(|e| e.event.event_type.as_str()).collect();

    assert!(
        event_types.contains(&"UserMessageAdded"),
        "Trace should contain UserMessageAdded: {event_types:?}"
    );
}

// ---------------------------------------------------------------------------
// Test: Session listing
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tui_session_listing_works() {
    let runtime = make_runtime().await;

    let workspace = runtime
        .open_workspace("/tmp/tui-list".into())
        .await
        .unwrap();

    runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
        })
        .await
        .unwrap();
    runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "smart".into(),

            permission_mode: None,
        })
        .await
        .unwrap();

    let sessions = runtime
        .list_sessions(&workspace.workspace_id)
        .await
        .unwrap();
    assert_eq!(sessions.len(), 2);
}

// ---------------------------------------------------------------------------
// Test: Subscribe-all receives events across sessions
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tui_subscribe_all_receives_events_across_sessions() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hello".into()]);
    let runtime = LocalRuntime::new(store, model).with_permission_mode(PermissionMode::Suggest);

    let workspace = runtime
        .open_workspace("/tmp/tui-sub-all".into())
        .await
        .unwrap();

    let s1 = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
        })
        .await
        .unwrap();
    let s2 = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
        })
        .await
        .unwrap();

    let mut all_stream = runtime.subscribe_all();

    // Send to both sessions
    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id.clone(),
            session_id: s1.clone(),
            content: "msg1".into(),
            attachments: vec![],
        })
        .await
        .unwrap();
    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: s2.clone(),
            content: "msg2".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    // Collect events
    let mut session_ids = Vec::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(1000);
    loop {
        tokio::select! {
            event = all_stream.next() => {
                match event {
                    Some(e) => {
                        session_ids.push(e.session_id.to_string());
                        if session_ids.len() > 20 { break; }
                    }
                    None => break,
                }
            }
            _ = tokio::time::sleep_until(deadline) => break,
        }
    }

    assert!(
        session_ids.contains(&s1.to_string()),
        "subscribe_all should receive events from session 1"
    );
    assert!(
        session_ids.contains(&s2.to_string()),
        "subscribe_all should receive events from session 2"
    );
}

// ---------------------------------------------------------------------------
// P3 Task 10: `:compact` typed in chat dispatches `Command::CompactSession`
// instead of `Command::SendMessage`.
// ---------------------------------------------------------------------------

#[test]
fn colon_compact_input_dispatches_compact_session_command() {
    use agent_core::projection::SessionProjection;
    use agent_core::{SessionId, WorkspaceId};
    use agent_tui::components::chat::ChatPanel;
    use agent_tui::components::{Command, EventContext, FocusTarget};
    use agent_tui::keybindings::KeyAction;

    let workspace_id = WorkspaceId::new();
    let session_id = Some(SessionId::new());
    let projection = SessionProjection::default();

    let ctx = EventContext {
        focus: FocusTarget::Chat,
        current_session: &projection,
        sessions: &[],
        model_profile: "fake",
        permission_mode: PermissionMode::Suggest,
        sidebar_left_visible: true,
        sidebar_right_visible: false,
        workspace_id: &workspace_id,
        current_session_id: &session_id,
    };

    let mut chat = ChatPanel::new();
    for ch in ":compact".chars() {
        let _ = chat.apply_key_action(KeyAction::InputCharacter(ch), &ctx);
    }
    let (_effects, commands) = chat.apply_key_action(KeyAction::SendInput, &ctx);

    assert!(
        commands
            .iter()
            .any(|c| matches!(c, Command::CompactSession { .. })),
        "expected Command::CompactSession; got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|c| matches!(c, Command::SendMessage { .. })),
        "expected NO SendMessage; got {commands:?}"
    );
    // Buffer should be cleared after `:compact` is consumed.
    assert!(
        chat.input_content.is_empty(),
        "expected input cleared, got {:?}",
        chat.input_content
    );
}

// ---------------------------------------------------------------------------
// P4 Task 10: `:model <alias>` typed in chat dispatches `Command::SwitchModel`
// instead of `Command::SendMessage`.
// ---------------------------------------------------------------------------

#[test]
fn colon_model_alias_input_dispatches_switch_model_command() {
    use agent_core::projection::SessionProjection;
    use agent_core::{SessionId, WorkspaceId};
    use agent_tui::components::chat::ChatPanel;
    use agent_tui::components::{Command, EventContext, FocusTarget};
    use agent_tui::keybindings::KeyAction;

    let workspace_id = WorkspaceId::new();
    let session_id = Some(SessionId::new());
    let projection = SessionProjection::default();

    let ctx = EventContext {
        focus: FocusTarget::Chat,
        current_session: &projection,
        sessions: &[],
        model_profile: "fake",
        permission_mode: PermissionMode::Suggest,
        sidebar_left_visible: true,
        sidebar_right_visible: false,
        workspace_id: &workspace_id,
        current_session_id: &session_id,
    };

    let mut chat = ChatPanel::new();
    for ch in ":model opus".chars() {
        let _ = chat.apply_key_action(KeyAction::InputCharacter(ch), &ctx);
    }
    let (_effects, commands) = chat.apply_key_action(KeyAction::SendInput, &ctx);

    let found = commands
        .iter()
        .any(|c| matches!(c, Command::SwitchModel { alias, .. } if alias == "opus"));
    assert!(
        found,
        "expected Command::SwitchModel with alias=opus; got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|c| matches!(c, Command::SendMessage { .. })),
        "expected NO SendMessage; got {commands:?}"
    );
    // Buffer should be cleared after `:model <alias>` is consumed.
    assert!(
        chat.input_content.is_empty(),
        "expected input cleared, got {:?}",
        chat.input_content
    );
}

#[test]
fn colon_model_without_alias_falls_through_as_chat_message() {
    use agent_core::projection::SessionProjection;
    use agent_core::{SessionId, WorkspaceId};
    use agent_tui::components::chat::ChatPanel;
    use agent_tui::components::{Command, EventContext, FocusTarget};
    use agent_tui::keybindings::KeyAction;

    let workspace_id = WorkspaceId::new();
    let session_id = Some(SessionId::new());
    let projection = SessionProjection::default();
    let ctx = EventContext {
        focus: FocusTarget::Chat,
        current_session: &projection,
        sessions: &[],
        model_profile: "fake",
        permission_mode: PermissionMode::Suggest,
        sidebar_left_visible: true,
        sidebar_right_visible: false,
        workspace_id: &workspace_id,
        current_session_id: &session_id,
    };

    let mut chat = ChatPanel::new();
    for ch in ":model".chars() {
        let _ = chat.apply_key_action(KeyAction::InputCharacter(ch), &ctx);
    }
    let (_effects, commands) = chat.apply_key_action(KeyAction::SendInput, &ctx);

    // `:model` without an alias falls through to SendMessage (user gets
    // feedback the command was malformed — no silent swallow).
    assert!(
        commands
            .iter()
            .any(|c| matches!(c, Command::SendMessage { .. })),
        "expected SendMessage fallback; got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|c| matches!(c, Command::SwitchModel { .. })),
        "expected NO SwitchModel without alias; got {commands:?}"
    );
}

fn chat_commands_for_input(input: &str) -> Vec<agent_tui::components::Command> {
    use agent_core::projection::SessionProjection;
    use agent_core::{SessionId, WorkspaceId};
    use agent_tui::components::chat::ChatPanel;
    use agent_tui::components::{EventContext, FocusTarget};
    use agent_tui::keybindings::KeyAction;

    let workspace_id = WorkspaceId::new();
    let session_id = Some(SessionId::new());
    let projection = SessionProjection::default();
    let ctx = EventContext {
        focus: FocusTarget::Chat,
        current_session: &projection,
        sessions: &[],
        model_profile: "fake",
        permission_mode: PermissionMode::Suggest,
        sidebar_left_visible: true,
        sidebar_right_visible: false,
        workspace_id: &workspace_id,
        current_session_id: &session_id,
    };

    let mut chat = ChatPanel::new();
    for character in input.chars() {
        let _ = chat.apply_key_action(KeyAction::InputCharacter(character), &ctx);
    }
    let (_effects, commands) = chat.apply_key_action(KeyAction::SendInput, &ctx);
    commands
}

#[test]
fn colon_attach_then_send_carries_attachment_payload() {
    use agent_core::projection::SessionProjection;
    use agent_core::{SessionId, WorkspaceId};
    use agent_tui::components::chat::ChatPanel;
    use agent_tui::components::{Command, EventContext, FocusTarget};
    use agent_tui::keybindings::KeyAction;

    let workspace_id = WorkspaceId::new();
    let session_id = Some(SessionId::new());
    let projection = SessionProjection::default();
    let ctx = EventContext {
        focus: FocusTarget::Chat,
        current_session: &projection,
        sessions: &[],
        model_profile: "fake",
        permission_mode: PermissionMode::Suggest,
        sidebar_left_visible: true,
        sidebar_right_visible: false,
        workspace_id: &workspace_id,
        current_session_id: &session_id,
    };

    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let canonical = manifest.canonicalize().unwrap();
    let mut chat = ChatPanel::new();
    for character in format!(":attach {}", manifest.display()).chars() {
        let _ = chat.apply_key_action(KeyAction::InputCharacter(character), &ctx);
    }
    let (_effects, attach_commands) = chat.apply_key_action(KeyAction::SendInput, &ctx);
    assert!(
        attach_commands.is_empty(),
        "attach should only update composer state, got {attach_commands:?}"
    );

    for character in "summarize this".chars() {
        let _ = chat.apply_key_action(KeyAction::InputCharacter(character), &ctx);
    }
    let (_effects, commands) = chat.apply_key_action(KeyAction::SendInput, &ctx);

    assert_eq!(commands.len(), 1);
    assert!(matches!(
        &commands[0],
        Command::SendMessage {
            content,
            attachments,
            ..
        } if content == "summarize this"
            && attachments.len() == 1
            && attachments[0].path == canonical.display().to_string()
            && attachments[0].name == "Cargo.toml"
            && attachments[0].mime_type == "application/toml"
    ));
}

fn write_test_skill(root: &std::path::Path, name: &str, description: &str, body: &str) {
    let skill_directory = root.join(name);
    std::fs::create_dir_all(&skill_directory).expect("skill directory should be created");
    std::fs::write(
        skill_directory.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\n---\n{body}"),
    )
    .expect("skill file should be written");
}

#[test]
fn colon_skills_input_dispatches_list_skills_command() {
    use agent_tui::components::Command;

    let commands = chat_commands_for_input(":skills");

    assert!(
        commands
            .iter()
            .any(|command| matches!(command, Command::ListSkills)),
        "expected Command::ListSkills; got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, Command::SendMessage { .. })),
        "expected NO SendMessage; got {commands:?}"
    );
}

#[test]
fn colon_plugins_input_dispatches_open_plugins_overlay_command() {
    use agent_tui::components::Command;

    let commands = chat_commands_for_input(":plugins");

    assert!(
        commands
            .iter()
            .any(|command| matches!(command, Command::OpenPluginsOverlay)),
        "expected Command::OpenPluginsOverlay; got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, Command::SendMessage { .. })),
        "expected NO SendMessage; got {commands:?}"
    );
}

#[test]
fn colon_skill_show_input_dispatches_show_skill_command() {
    use agent_tui::components::Command;

    let commands = chat_commands_for_input(":skill show test-driven-rust");

    assert!(
        commands.iter().any(
            |command| matches!(command, Command::ShowSkill { skill_id } if skill_id == "test-driven-rust")
        ),
        "expected Command::ShowSkill for test-driven-rust; got {commands:?}"
    );
}

#[test]
fn colon_skill_activate_input_dispatches_activate_skill_command() {
    use agent_tui::components::Command;

    let commands = chat_commands_for_input(":skill activate test-driven-rust");

    assert!(
        commands.iter().any(
            |command| matches!(command, Command::ActivateSkill { skill_id, .. } if skill_id == "test-driven-rust")
        ),
        "expected Command::ActivateSkill for test-driven-rust; got {commands:?}"
    );
}

#[test]
fn colon_skill_deactivate_input_dispatches_deactivate_skill_command() {
    use agent_tui::components::Command;

    let commands = chat_commands_for_input(":skill deactivate test-driven-rust");

    assert!(
        commands.iter().any(
            |command| matches!(command, Command::DeactivateSkill { skill_id, .. } if skill_id == "test-driven-rust")
        ),
        "expected Command::DeactivateSkill for test-driven-rust; got {commands:?}"
    );
}

#[tokio::test]
async fn tui_skill_commands_call_facade_and_render_visible_messages() {
    use agent_core::projection::ProjectedRole;
    use agent_core::{EventPayload, SessionId, WorkspaceId};
    use agent_tui::app::App;
    use agent_tui::components::Command;

    let skill_root = std::env::temp_dir().join(format!(
        "kairox-tui-skill-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after UNIX_EPOCH")
            .as_nanos()
    ));
    std::fs::create_dir_all(&skill_root).expect("skill root should be created");
    write_test_skill(
        &skill_root,
        "test-driven-rust",
        "Use when implementing Rust changes with test-first development.",
        "# Test-driven Rust\n\nWrite a failing test first.\n",
    );
    let registry = FileSkillRegistry::discover(vec![SkillRoot::new(
        SkillSourceKind::Workspace,
        &skill_root,
    )])
    .await
    .expect("skill registry should discover test skill");
    let store = SqliteEventStore::in_memory()
        .await
        .expect("in-memory event store");
    let model = FakeModelClient::new(vec!["ok".into()]);
    let runtime = Arc::new(LocalRuntime::new(store, model).with_skill_registry(Arc::new(registry)));

    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();
    let mut app = App::new("fake", PermissionMode::Suggest, workspace_id.clone());
    app.current_session_id = Some(session_id.clone());

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![
            Command::ListSkills,
            Command::ShowSkill {
                skill_id: "test-driven-rust".into(),
            },
            Command::ActivateSkill {
                workspace_id: workspace_id.clone(),
                session_id: session_id.clone(),
                skill_id: "test-driven-rust".into(),
            },
            Command::DeactivateSkill {
                workspace_id,
                session_id: session_id.clone(),
                skill_id: "test-driven-rust".into(),
            },
        ],
    )
    .await;

    let visible_messages: Vec<&str> = app
        .state
        .current_session
        .messages
        .iter()
        .filter(|message| message.role == ProjectedRole::Assistant)
        .map(|message| message.content.as_str())
        .collect();
    assert!(
        visible_messages
            .iter()
            .any(|message| message.contains("test-driven-rust")),
        "expected a visible skill list/detail message; got {visible_messages:?}"
    );
    assert!(
        visible_messages
            .iter()
            .any(|message| message.contains("activated test-driven-rust")),
        "expected visible activation confirmation; got {visible_messages:?}"
    );
    assert!(
        visible_messages
            .iter()
            .any(|message| message.contains("deactivated test-driven-rust")),
        "expected visible deactivation confirmation; got {visible_messages:?}"
    );

    let trace = runtime
        .get_trace(session_id)
        .await
        .expect("skill commands should write trace events");
    assert!(
        trace.iter().any(|entry| {
            matches!(
                &entry.event.payload,
                EventPayload::SkillActivated { skill_id, .. } if skill_id == "test-driven-rust"
            )
        }),
        "expected SkillActivated trace event; got {trace:?}"
    );
    assert!(
        trace.iter().any(|entry| {
            matches!(
                &entry.event.payload,
                EventPayload::SkillDeactivated { skill_id, .. } if skill_id == "test-driven-rust"
            )
        }),
        "expected SkillDeactivated trace event; got {trace:?}"
    );

    std::fs::remove_dir_all(skill_root).expect("test skill root should be cleaned up");
}
