use super::*;
use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_store::SqliteEventStore;

#[tokio::test]
async fn send_message_records_user_and_assistant_events() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hello".into()]);
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime
        .open_workspace("/tmp/workspace".into())
        .await
        .unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "hi".into(),
            display_content: None,
            attachments: vec![],
        })
        .await
        .unwrap();

    let projection = runtime.get_session_projection(session_id).await.unwrap();
    assert_eq!(projection.messages.len(), 2);
    assert_eq!(projection.messages[0].content, "hi");
    assert_eq!(projection.messages[1].content, "hello");
}

#[tokio::test]
async fn open_workspace_persists_metadata() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hi".into()]);
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime.open_workspace("/tmp/project".into()).await.unwrap();

    let workspaces = runtime.list_workspaces().await.unwrap();
    assert_eq!(workspaces.len(), 1);
    assert_eq!(workspaces[0].workspace_id, workspace.workspace_id);
    assert_eq!(workspaces[0].path, "/tmp/project");
}

#[tokio::test]
async fn start_session_persists_metadata() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hi".into()]);
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime.open_workspace("/tmp/project".into()).await.unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    let sessions = runtime
        .list_sessions(&workspace.workspace_id)
        .await
        .unwrap();
    assert_eq!(sessions.len(), 1);
    assert_eq!(sessions[0].session_id, session_id);
    assert_eq!(sessions[0].title, "New conversation");
}

#[tokio::test]
async fn rename_session_updates_metadata() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hi".into()]);
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime.open_workspace("/tmp/project".into()).await.unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    runtime
        .rename_session(&session_id, "My Custom Title".into())
        .await
        .unwrap();

    let sessions = runtime
        .list_sessions(&workspace.workspace_id)
        .await
        .unwrap();
    assert_eq!(sessions[0].title, "My Custom Title");
}

#[tokio::test]
async fn soft_delete_hides_session() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hi".into()]);
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime.open_workspace("/tmp/project".into()).await.unwrap();
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();

    runtime.soft_delete_session(&session_id).await.unwrap();

    let sessions = runtime
        .list_sessions(&workspace.workspace_id)
        .await
        .unwrap();
    assert!(sessions.is_empty());
}
