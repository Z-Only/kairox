mod app_state;
mod commands;
mod event_forwarder;

#[cfg(not(test))]
use app_state::GuiState;

use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use agent_tools::PermissionMode;

pub fn build_runtime() -> Result<LocalRuntime<SqliteEventStore, FakeModelClient>, String> {
    let tokio_rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("Failed to create tokio runtime: {e}"))?;

    let runtime: Result<LocalRuntime<SqliteEventStore, FakeModelClient>, String> = tokio_rt
        .block_on(async {
            let store = SqliteEventStore::in_memory()
                .await
                .map_err(|e| format!("Failed to create in-memory store: {e}"))?;
            let model = FakeModelClient::new(vec!["hello from Kairox".into()]);
            let cwd =
                std::env::current_dir().map_err(|e| format!("Cannot get current dir: {e}"))?;

            let runtime = LocalRuntime::new(store, model)
                .with_permission_mode(PermissionMode::Suggest)
                .with_context_limit(100_000)
                .with_builtin_tools(cwd)
                .await;

            Ok(runtime)
        });

    runtime
}

#[cfg(not(test))]
pub fn run() {
    let runtime = build_runtime().expect("failed to build runtime");

    tauri::Builder::default()
        .manage(GuiState::new(runtime))
        .invoke_handler(tauri::generate_handler![
            commands::list_profiles,
            commands::initialize_workspace,
            commands::start_session,
            commands::send_message,
            commands::switch_session,
            commands::list_sessions,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run tauri application");
}

#[cfg(test)]
pub fn run() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_profiles_always_includes_fake() {
        assert!(commands::detect_profiles().contains(&"fake".to_string()));
    }

    #[test]
    fn choose_default_profile_prefers_fast() {
        let profiles = vec![
            "fast".to_string(),
            "local-code".to_string(),
            "fake".to_string(),
        ];
        assert_eq!(commands::choose_default_profile(&profiles), "fast");
    }

    #[test]
    fn choose_default_profile_falls_back_to_local_code() {
        let profiles = vec!["local-code".to_string(), "fake".to_string()];
        assert_eq!(commands::choose_default_profile(&profiles), "local-code");
    }

    #[test]
    fn choose_default_profile_falls_back_to_fake() {
        let profiles = vec!["fake".to_string()];
        assert_eq!(commands::choose_default_profile(&profiles), "fake");
    }
}

#[cfg(test)]
mod integration_tests {
    use agent_core::AppFacade;

    async fn create_test_runtime(
    ) -> agent_runtime::LocalRuntime<agent_store::SqliteEventStore, agent_models::FakeModelClient>
    {
        let store = agent_store::SqliteEventStore::in_memory().await.unwrap();
        let model = agent_models::FakeModelClient::new(vec!["test response".into()]);
        agent_runtime::LocalRuntime::new(store, model)
            .with_permission_mode(agent_tools::PermissionMode::Suggest)
            .with_context_limit(100_000)
    }

    #[tokio::test]
    async fn workspace_initialization_creates_session() {
        let runtime = create_test_runtime().await;
        let workspace = runtime.open_workspace("/tmp/test".into()).await.unwrap();

        let session_id = runtime
            .start_session(agent_core::StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fake".into(),
            })
            .await
            .unwrap();

        assert!(!session_id.to_string().is_empty());
    }

    #[tokio::test]
    async fn send_message_produces_user_and_assistant_events() {
        let runtime = create_test_runtime().await;
        let workspace = runtime.open_workspace("/tmp/test".into()).await.unwrap();
        let session_id = runtime
            .start_session(agent_core::StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fake".into(),
            })
            .await
            .unwrap();

        runtime
            .send_message(agent_core::SendMessageRequest {
                workspace_id: workspace.workspace_id,
                session_id: session_id.clone(),
                content: "hello".into(),
            })
            .await
            .unwrap();

        let projection = runtime.get_session_projection(session_id).await.unwrap();
        assert_eq!(projection.messages.len(), 2);
        assert_eq!(projection.messages[0].content, "hello");
        assert_eq!(projection.messages[1].content, "test response");
    }

    #[tokio::test]
    async fn session_projection_serializes_for_frontend() {
        let runtime = create_test_runtime().await;
        let workspace = runtime.open_workspace("/tmp/test".into()).await.unwrap();
        let session_id = runtime
            .start_session(agent_core::StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fake".into(),
            })
            .await
            .unwrap();

        runtime
            .send_message(agent_core::SendMessageRequest {
                workspace_id: workspace.workspace_id,
                session_id: session_id.clone(),
                content: "hi".into(),
            })
            .await
            .unwrap();

        let projection = runtime.get_session_projection(session_id).await.unwrap();
        let json = serde_json::to_value(&projection).unwrap();
        assert!(json["messages"].is_array());
        assert_eq!(json["messages"][0]["role"], "user");
        assert_eq!(json["messages"][1]["role"], "assistant");
    }

    #[tokio::test]
    async fn domain_event_serializes_with_payload_type_tag() {
        let runtime = create_test_runtime().await;
        let workspace = runtime.open_workspace("/tmp/test".into()).await.unwrap();
        let session_id = runtime
            .start_session(agent_core::StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fake".into(),
            })
            .await
            .unwrap();

        // Subscribe BEFORE sending so the stream captures events
        let mut stream = runtime.subscribe_session(session_id.clone());

        runtime
            .send_message(agent_core::SendMessageRequest {
                workspace_id: workspace.workspace_id,
                session_id,
                content: "test".into(),
            })
            .await
            .unwrap();
        let events: Vec<agent_core::DomainEvent> = {
            use futures::StreamExt;
            let mut collected = Vec::new();
            for _ in 0..10 {
                tokio::select! {
                    Some(event) = stream.next() => collected.push(event),
                    _ = tokio::time::sleep(std::time::Duration::from_millis(100)) => break,
                }
            }
            collected
        };

        assert!(!events.is_empty());
        for event in &events {
            let json = serde_json::to_value(event).unwrap();
            assert!(json["payload"]["type"].is_string());
            assert!(json["session_id"].is_string());
        }
    }
}
