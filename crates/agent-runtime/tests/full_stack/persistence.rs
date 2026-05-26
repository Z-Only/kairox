//! Persistence: workspaces and sessions survive a fresh `LocalRuntime`
//! reconnecting to the same on-disk SQLite database.

use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use agent_tools::PermissionMode;

#[tokio::test]
async fn full_stack_data_persists_across_reconnection() {
    let db_path = std::env::temp_dir().join(format!(
        "kairox-fullstack-persist-{}.sqlite",
        uuid::Uuid::new_v4()
    ));
    let database_url = format!(
        "sqlite:///{}",
        db_path.display().to_string().trim_start_matches('/')
    );

    let original_ws_id = {
        let store = SqliteEventStore::connect(&database_url).await.unwrap();
        let model = FakeModelClient::new(vec!["persisted response".into()]);
        let runtime = LocalRuntime::new(store, model).with_permission_mode(PermissionMode::Suggest);

        let ws = runtime
            .open_workspace("/tmp/persist-test".into())
            .await
            .unwrap();
        let sid = runtime
            .start_session(StartSessionRequest {
                workspace_id: ws.workspace_id.clone(),
                model_profile: "fake".into(),

                permission_mode: None,
                approval_policy: None,
                sandbox_policy: None,
            })
            .await
            .unwrap();

        runtime
            .send_message(SendMessageRequest {
                workspace_id: ws.workspace_id.clone(),
                session_id: sid,
                content: "persist this".into(),
                attachments: vec![],
            })
            .await
            .unwrap();

        ws.workspace_id.to_string()
    };

    // Reconnect
    {
        let store2 = SqliteEventStore::connect(&database_url).await.unwrap();
        let model2 = FakeModelClient::new(vec!["new response".into()]);
        let runtime2 =
            LocalRuntime::new(store2, model2).with_permission_mode(PermissionMode::Suggest);

        let workspaces = runtime2.list_workspaces().await.unwrap();
        assert_eq!(workspaces.len(), 1, "Should recover workspace");
        assert_eq!(workspaces[0].workspace_id.as_str(), original_ws_id);

        let wid = agent_core::WorkspaceId::from_string(original_ws_id);
        let sessions = runtime2.list_sessions(&wid).await.unwrap();
        assert_eq!(sessions.len(), 1, "Should recover session");
    }

    let _ = std::fs::remove_file(&db_path);
}
