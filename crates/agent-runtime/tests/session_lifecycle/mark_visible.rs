//! Guards on `mark_session_visible` for non-draft and unbound-draft sessions.

use agent_core::{AppFacade, CoreError, StartSessionRequest};
use agent_store::{ProjectMetaRepository, SqliteEventStore};

use super::support::make_runtime;

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
            approval_policy: None,
            sandbox_policy: None,
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
            approval_policy: None,
            sandbox_policy: None,
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
