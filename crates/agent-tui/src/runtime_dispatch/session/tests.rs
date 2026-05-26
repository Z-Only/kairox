use agent_core::{ProjectGitStatus, ProjectSessionVisibility};
use agent_store::SqliteEventStore;

use crate::app::App;
use crate::components::{SessionInfo, SessionState};

use super::lifecycle::restore_session_draft;
use super::state::apply_session_git_status;

#[tokio::test]
async fn restore_session_draft_loads_saved_composer_text() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    store
        .upsert_workspace("wrk_test", "/tmp/kairox")
        .await
        .unwrap();
    let session_id = agent_core::SessionId::from_string("ses_restore".to_string());
    let now = "2026-05-21T00:00:00Z".to_string();
    store
        .upsert_session(&agent_store::SessionRow {
            session_id: session_id.as_str().to_string(),
            workspace_id: "wrk_test".to_string(),
            title: "Restore me".to_string(),
            model_profile: "test".to_string(),
            model_id: None,
            provider: None,
            permission_mode: "suggest".to_string(),
            approval_policy: None,
            sandbox_policy: None,
            deleted_at: None,
            created_at: now.clone(),
            updated_at: now,
        })
        .await
        .unwrap();
    store
        .save_draft(session_id.as_str(), "saved draft")
        .await
        .unwrap();
    let mut app = App::new(
        "test",
        agent_tools::PermissionMode::Suggest,
        agent_core::WorkspaceId::from_string("wrk_test".to_string()),
    );
    app.chat.input_content = "old text".to_string();
    app.chat.input_cursor = app.chat.input_content.len();

    restore_session_draft(&store, &mut app, &session_id).await;

    assert_eq!(app.chat.input_content, "saved draft");
    assert_eq!(app.chat.input_cursor, "saved draft".len());
}

#[test]
fn session_git_meta_applies_refreshed_status_to_session() {
    let session_id = agent_core::SessionId::from_string("ses_git".to_string());
    let mut app = App::new(
        "test",
        agent_tools::PermissionMode::Suggest,
        agent_core::WorkspaceId::from_string("wrk_test".to_string()),
    );
    app.current_session_id = Some(session_id.clone());
    app.state.sessions.push(SessionInfo {
        id: session_id.clone(),
        title: "Worktree".to_string(),
        model_profile: "test".to_string(),
        state: SessionState::Active,
        pinned: false,
        archived: false,
        project_id: None,
        worktree_path: None,
        branch: None,
        visibility: Some(ProjectSessionVisibility::Visible),
    });

    apply_session_git_status(
        &mut app,
        &session_id,
        &ProjectGitStatus {
            kind: agent_core::ProjectGitStatusKind::Clean,
            branch: Some("feat/tui".to_string()),
            worktree_path: "/tmp/project/.kairox/worktrees/feat-tui".to_string(),
            message: None,
        },
    );

    let session = app
        .state
        .sessions
        .iter()
        .find(|session| session.id == session_id)
        .expect("session");
    assert_eq!(session.branch.as_deref(), Some("feat/tui"));
    assert_eq!(
        session.worktree_path.as_deref(),
        Some("/tmp/project/.kairox/worktrees/feat-tui")
    );
    let metadata = app.current_session_git_metadata();
    assert!(metadata.iter().any(|part| part == "worktrees/feat-tui"));
}
