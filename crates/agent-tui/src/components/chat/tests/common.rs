//! Shared fixtures and helpers used by the chat-panel test modules.
//!
//! Each themed sub-module pairs `use super::super::*;` (to pull `chat::*`
//! internals such as `ChatPanel`, `InputMode`, `Command`, and the render
//! helpers) with `use super::common::*;` for these helpers.

use super::super::*;
use crate::components::{EventContext, SessionInfo, SessionState};
use std::sync::OnceLock;

pub(super) fn fixture_attachment(name: &str) -> agent_core::AttachmentInfo {
    agent_core::AttachmentInfo {
        path: format!("/tmp/{name}"),
        name: name.to_string(),
        mime_type: "text/plain".to_string(),
    }
}

pub(super) fn agent_tui_manifest_path() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")
}

/// Shared static [`EventContext`] for tests. We leak the owned data so
/// that the references inside `EventContext` can be `'static`.
static TEST_CTX: OnceLock<EventContext<'static>> = OnceLock::new();

pub(super) fn test_ctx() -> &'static EventContext<'static> {
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
            projects: &[],
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

pub(super) fn test_ctx_with_session() -> &'static EventContext<'static> {
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
            projects: &[],
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

pub(super) fn test_ctx_busy_session() -> &'static EventContext<'static> {
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
            projects: &[],
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
