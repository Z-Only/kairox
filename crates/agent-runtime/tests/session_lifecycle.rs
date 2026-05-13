//! Session lifecycle integration tests for CRUD, persistence, and cleanup.

use agent_core::{
    AppFacade, CoreError, DomainEvent, ProjectId, SendMessageRequest, SessionId,
    StartSessionRequest, WorkspaceId,
};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::{
    event_store::ProjectSessionMetaRow, EventStore, ProjectMetaRepository, SessionRow,
    SqliteEventStore, WorkspaceRow,
};
use async_trait::async_trait;
use std::time::Duration;

static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

struct NonSqliteEventStore;

#[async_trait]
impl EventStore for NonSqliteEventStore {
    async fn append(&self, _event: &DomainEvent) -> agent_store::Result<()> {
        Ok(())
    }

    async fn load_session(&self, _session_id: &SessionId) -> agent_store::Result<Vec<DomainEvent>> {
        Ok(Vec::new())
    }

    async fn upsert_workspace(&self, _workspace_id: &str, _path: &str) -> agent_store::Result<()> {
        Ok(())
    }

    async fn upsert_session(&self, _meta: &SessionRow) -> agent_store::Result<()> {
        Ok(())
    }

    async fn list_workspaces(&self) -> agent_store::Result<Vec<WorkspaceRow>> {
        Ok(Vec::new())
    }

    async fn list_active_sessions(
        &self,
        _workspace_id: &str,
    ) -> agent_store::Result<Vec<SessionRow>> {
        Ok(Vec::new())
    }

    async fn rename_session(&self, _session_id: &str, _title: &str) -> agent_store::Result<()> {
        Ok(())
    }

    async fn soft_delete_session(&self, _session_id: &str) -> agent_store::Result<()> {
        Ok(())
    }

    async fn permanently_delete_session(&self, _session_id: &str) -> agent_store::Result<()> {
        Ok(())
    }

    async fn restore_archived_session(&self, _session_id: &str) -> agent_store::Result<()> {
        Ok(())
    }

    async fn cleanup_expired_sessions(&self, _older_than: Duration) -> agent_store::Result<usize> {
        Ok(0)
    }

    async fn list_visible_project_sessions(
        &self,
        _project_id: &str,
    ) -> agent_store::Result<Vec<ProjectSessionMetaRow>> {
        Ok(Vec::new())
    }

    async fn list_archived_project_session_metas(
        &self,
        _workspace_id: &str,
    ) -> agent_store::Result<Vec<ProjectSessionMetaRow>> {
        Ok(Vec::new())
    }
}

/// Helper: create an in-memory runtime for quick tests.
fn make_runtime(store: SqliteEventStore) -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    LocalRuntime::new(store, FakeModelClient::new(vec!["response".into()]))
}

/// Helper: create a file-backed SQLite store for persistence tests.
async fn make_file_backed_store() -> (SqliteEventStore, std::path::PathBuf) {
    let db_path = std::env::temp_dir().join(format!(
        "kairox-session-lifecycle-{}.sqlite",
        uuid::Uuid::new_v4()
    ));
    let database_url = format!("sqlite://{}", db_path.display());
    let store = SqliteEventStore::connect(&database_url)
        .await
        .expect("failed to connect to file-backed SQLite");
    (store, db_path)
}

// ---------------------------------------------------------------------------
// Test 1: Full workspace → session → message → projection → cancel → trace
// ---------------------------------------------------------------------------

#[tokio::test]
async fn full_workspace_session_round_trip() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let runtime = make_runtime(store);

    // Open workspace
    let workspace = runtime
        .open_workspace("/tmp/kairox-round-trip".into())
        .await
        .unwrap();

    // Start session
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    // Send message
    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id.clone(),
            session_id: session_id.clone(),
            content: "hello agent".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    // Get projection — should have 2 messages (user + assistant)
    let projection = runtime
        .get_session_projection(session_id.clone())
        .await
        .unwrap();
    assert_eq!(
        projection.messages.len(),
        2,
        "expected 2 messages (user + assistant), got {:?}",
        projection
            .messages
            .iter()
            .map(|m| format!("{:?}: {}", m.role, m.content))
            .collect::<Vec<_>>()
    );
    assert_eq!(projection.messages[0].content, "hello agent");
    assert_eq!(projection.messages[1].content, "response");

    // Cancel session
    runtime
        .cancel_session(workspace.workspace_id, session_id.clone())
        .await
        .unwrap();

    // Verify cancelled flag
    let projection_after_cancel = runtime
        .get_session_projection(session_id.clone())
        .await
        .unwrap();
    assert!(
        projection_after_cancel.cancelled,
        "session should be marked as cancelled"
    );

    // Get trace — should be non-empty
    let trace = runtime.get_trace(session_id).await.unwrap();
    assert!(
        !trace.is_empty(),
        "trace should contain events after the round trip"
    );
}

// ---------------------------------------------------------------------------
// Test 2: Session metadata persists across store reconnection
// ---------------------------------------------------------------------------

#[tokio::test]
async fn session_metadata_persists_across_reopen() {
    let (store, db_path) = make_file_backed_store().await;
    let runtime = make_runtime(store);

    // Open workspace + start session
    let workspace = runtime
        .open_workspace("/tmp/kairox-persist".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    // Keep copies of IDs for verification after reconnection
    let original_workspace_id = workspace.workspace_id.to_string();
    let original_session_id = session_id.to_string();

    // Drop the store (and runtime) — data is on disk
    drop(runtime);

    // Reconnect to the same database
    let database_url = format!("sqlite://{}", db_path.display());
    let store2 = SqliteEventStore::connect(&database_url)
        .await
        .expect("failed to reconnect to file-backed SQLite");
    let runtime2 = make_runtime(store2);

    // Verify workspace data recovered
    let workspaces = runtime2.list_workspaces().await.unwrap();
    assert_eq!(workspaces.len(), 1, "should recover 1 workspace");
    assert_eq!(workspaces[0].workspace_id.as_str(), original_workspace_id);

    // Verify session data recovered
    let wid = agent_core::WorkspaceId::from_string(original_workspace_id);
    let sessions = runtime2.list_sessions(&wid).await.unwrap();
    assert_eq!(sessions.len(), 1, "should recover 1 session");
    assert_eq!(sessions[0].session_id.as_str(), original_session_id);

    // Clean up temp file
    let _ = std::fs::remove_file(&db_path);
}

// ---------------------------------------------------------------------------
// Test 3: Rename and soft-delete flow
// ---------------------------------------------------------------------------

#[tokio::test]
async fn rename_and_soft_delete_flow() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let runtime = make_runtime(store);

    // Open workspace → start session
    let workspace = runtime
        .open_workspace("/tmp/kairox-rename-delete".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    // Rename
    runtime
        .rename_session(&session_id, "Renamed Session".into())
        .await
        .unwrap();

    // Verify new title in list
    let sessions = runtime
        .list_sessions(&workspace.workspace_id)
        .await
        .unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].title, "Renamed Session");

    // Soft-delete
    runtime.soft_delete_session(&session_id).await.unwrap();

    // Verify list is empty (soft-deleted sessions are excluded from active list)
    let sessions_after_delete = runtime
        .list_sessions(&workspace.workspace_id)
        .await
        .unwrap();
    assert!(
        sessions_after_delete.is_empty(),
        "soft-deleted session should not appear in active list"
    );
}

// ---------------------------------------------------------------------------
// Test 4: Multiple sessions in the same workspace
// ---------------------------------------------------------------------------

#[tokio::test]
async fn multiple_sessions_in_same_workspace() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let runtime = make_runtime(store);

    // Open workspace
    let workspace = runtime
        .open_workspace("/tmp/kairox-multi-session".into())
        .await
        .unwrap();

    // Create 3 sessions with different profiles
    let s1 = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "gpt-4".into(),
        })
        .await
        .unwrap();
    let s2 = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "claude".into(),
        })
        .await
        .unwrap();
    let s3 = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "ollama".into(),
        })
        .await
        .unwrap();

    // List returns 3
    let sessions = runtime
        .list_sessions(&workspace.workspace_id)
        .await
        .unwrap();
    assert_eq!(sessions.len(), 3, "should have 3 sessions");

    // Delete one
    runtime.soft_delete_session(&s2).await.unwrap();

    // List returns 2
    let sessions_after = runtime
        .list_sessions(&workspace.workspace_id)
        .await
        .unwrap();
    assert_eq!(
        sessions_after.len(),
        2,
        "should have 2 sessions after delete"
    );

    // Verify remaining IDs are correct
    let remaining_ids: Vec<String> = sessions_after
        .iter()
        .map(|s| s.session_id.as_str().to_string())
        .collect();
    assert!(
        remaining_ids.contains(&s1.to_string()),
        "s1 should still be present"
    );
    assert!(
        remaining_ids.contains(&s3.to_string()),
        "s3 should still be present"
    );
    assert!(
        !remaining_ids.contains(&s2.to_string()),
        "s2 should not be present after soft delete"
    );
}

// ---------------------------------------------------------------------------
// Test 5: Cleanup expired removes old sessions and events
// ---------------------------------------------------------------------------

#[tokio::test]
async fn cleanup_expired_removes_old_sessions_and_events() {
    let (store, db_path) = make_file_backed_store().await;
    let runtime = make_runtime(store);

    // Open workspace + start session
    let workspace = runtime
        .open_workspace("/tmp/kairox-cleanup".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    // Send a message (creates events)
    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id.clone(),
            session_id: session_id.clone(),
            content: "hello".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    // Verify trace is non-empty before delete
    let trace = runtime.get_trace(session_id.clone()).await.unwrap();
    assert!(!trace.is_empty(), "trace should have events before cleanup");

    // Soft-delete the session
    runtime.soft_delete_session(&session_id).await.unwrap();

    // Drop the runtime, reopen the store to call cleanup
    drop(runtime);

    let database_url = format!("sqlite://{}", db_path.display());
    let store2 = SqliteEventStore::connect(&database_url)
        .await
        .expect("failed to reconnect for cleanup");
    let runtime2 = make_runtime(store2);

    // cleanup_expired_sessions with Duration::from_secs(0) forces cleanup
    // of all soft-deleted sessions (deleted_at is always in the past)
    let deleted = runtime2
        .cleanup_expired_sessions(std::time::Duration::from_secs(0))
        .await
        .unwrap();
    assert!(
        deleted >= 1,
        "should have cleaned up at least 1 session, got {}",
        deleted
    );

    // Verify the session is gone from the store
    let sessions = runtime2
        .list_sessions(&workspace.workspace_id)
        .await
        .unwrap();
    assert!(
        sessions.is_empty(),
        "session should be fully removed after cleanup"
    );

    // Clean up temp file
    let _ = std::fs::remove_file(&db_path);
}

#[tokio::test]
async fn project_removal_archives_visible_project_sessions() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let runtime = make_runtime(store);
    let workspace = runtime
        .open_workspace("/tmp/kairox-workspace".into())
        .await
        .unwrap();

    let project = runtime
        .add_existing_project(workspace.workspace_id.clone(), "/tmp/kairox-project".into())
        .await
        .unwrap();
    let session_id = runtime
        .create_project_draft_session(project.project_id.clone())
        .await
        .unwrap();

    runtime
        .mark_session_visible(&session_id, "hello project".into())
        .await
        .unwrap();
    runtime
        .remove_project(project.project_id.clone())
        .await
        .unwrap();

    let visible = runtime
        .list_project_sessions(project.project_id.clone())
        .await
        .unwrap();
    let archived = runtime
        .list_archived_sessions(&workspace.workspace_id)
        .await
        .unwrap();

    assert!(visible.is_empty());
    assert!(archived
        .iter()
        .any(|session| session.session_id == session_id));
}

#[tokio::test]
async fn project_session_lists_require_sqlite_metadata_store() {
    let runtime = LocalRuntime::new(
        NonSqliteEventStore,
        FakeModelClient::new(vec!["response".into()]),
    );

    let visible_error = runtime
        .list_project_sessions(ProjectId::from_string("prj_non_sqlite".into()))
        .await
        .expect_err("non-SQLite project session listing should fail");
    let archived_error = runtime
        .list_archived_sessions(&WorkspaceId::from_string("wrk_non_sqlite".into()))
        .await
        .expect_err("non-SQLite archived session listing should fail");

    for error in [visible_error, archived_error] {
        match error {
            CoreError::InvalidState(message) => {
                assert_eq!(message, "project metadata requires sqlite event store")
            }
            other => panic!("expected InvalidState, got {other:?}"),
        }
    }
}

#[tokio::test]
async fn create_blank_project_uses_new_project_default_name() {
    let _environment_guard = ENV_LOCK.lock().await;
    let previous_home = std::env::var_os("HOME");
    let home_dir = tempfile::tempdir().expect("temp home");

    std::env::set_var("HOME", home_dir.path());

    let store = SqliteEventStore::in_memory().await.unwrap();
    let runtime = make_runtime(store);
    let workspace = runtime
        .open_workspace("/tmp/kairox-blank-project-default-name".into())
        .await
        .unwrap();
    let project = runtime
        .create_blank_project(workspace.workspace_id, None)
        .await
        .unwrap();

    match previous_home {
        Some(value) => std::env::set_var("HOME", value),
        None => std::env::remove_var("HOME"),
    }

    assert_eq!(project.display_name, "New Project");
}

#[tokio::test]
async fn create_blank_project_reports_git_init_failure() {
    let _environment_guard = ENV_LOCK.lock().await;
    let previous_home = std::env::var_os("HOME");
    let previous_path = std::env::var_os("PATH");
    let home_dir = tempfile::tempdir().expect("temp home");

    std::env::set_var("HOME", home_dir.path());
    std::env::set_var("PATH", "");

    let store = SqliteEventStore::in_memory().await.unwrap();
    let runtime = make_runtime(store);
    let workspace = runtime
        .open_workspace("/tmp/kairox-blank-project-git-failure".into())
        .await
        .unwrap();
    let result = runtime
        .create_blank_project(workspace.workspace_id, Some("No Git Available".into()))
        .await;

    match previous_home {
        Some(value) => std::env::set_var("HOME", value),
        None => std::env::remove_var("HOME"),
    }
    match previous_path {
        Some(value) => std::env::set_var("PATH", value),
        None => std::env::remove_var("PATH"),
    }

    let error = result.expect_err("missing git executable should fail blank project creation");
    assert!(
        matches!(error, CoreError::InvalidState(_)),
        "expected InvalidState, got {error:?}"
    );
}

#[tokio::test]
async fn mark_session_visible_rejects_non_draft_sessions() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let runtime = make_runtime(store);
    let workspace = runtime
        .open_workspace("/tmp/kairox-non-draft-visible".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id,
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    let error = runtime
        .mark_session_visible(&session_id, "should not rename".into())
        .await
        .expect_err("normal sessions should not be promoted as project drafts");
    assert!(
        matches!(error, CoreError::InvalidState(_)),
        "expected InvalidState, got {error:?}"
    );
}

#[tokio::test]
async fn mark_session_visible_rejects_draft_visibility_without_project_binding() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let repository = ProjectMetaRepository::new(store.pool().clone());
    let runtime = make_runtime(store);
    let workspace = runtime
        .open_workspace("/tmp/kairox-stray-draft-visible".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id,
            model_profile: "fake".into(),
        })
        .await
        .unwrap();

    repository
        .set_session_visibility(session_id.as_str(), "draft_hidden")
        .await
        .unwrap();

    let error = runtime
        .mark_session_visible(&session_id, "should still fail".into())
        .await
        .expect_err("draft visibility without project binding should not be promoted");
    assert!(
        matches!(error, CoreError::InvalidState(_)),
        "expected InvalidState, got {error:?}"
    );
}
