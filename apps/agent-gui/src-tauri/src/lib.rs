mod app_state;
pub mod commands;
mod event_forwarder;
pub mod specta;

use agent_config::Config;
#[cfg(not(test))]
use agent_core::AppFacade;
#[cfg(test)]
use agent_models::ModelRouter;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use agent_tools::PermissionMode;

#[cfg(not(test))]
use app_state::GuiState;

#[cfg(not(test))]
pub fn run() {
    use tauri::Manager;

    let specta_builder = specta::create_specta();

    tauri::Builder::default()
        .setup(move |app| {
            let handle = app.handle().clone();
            tauri::async_runtime::block_on(async move {
                // Use a file-backed SQLite database in the system temp directory.
                // In-memory SQLite (`sqlite::memory:` or `sqlite:file:...?mode=memory&cache=shared`)
                // is destroyed when all connections close, which causes "no such table: events"
                // errors when the pool recycles connections. A file-backed DB persists across
                // connection recycling and is cleaned up when the OS reclaims temp files.
                let db_dir = std::env::temp_dir().join("kairox-gui");
                tokio::fs::create_dir_all(&db_dir).await.ok();
                let db_path = db_dir.join("kairox.db");
                let db_url = format!("sqlite://{}?mode=rwc", db_path.display());

                eprintln!("Database: {}", db_url);

                let store = SqliteEventStore::connect(&db_url)
                    .await
                    .expect("Failed to create event store");

                let mem_store = std::sync::Arc::new(
                    agent_memory::SqliteMemoryStore::new(store.pool().clone())
                        .await
                        .expect("Failed to create memory store"),
                ) as std::sync::Arc<dyn agent_memory::MemoryStore>;

                let config = Config::load().unwrap_or_else(|e| {
                    eprintln!("Config warning: {e}, using defaults");
                    Config::defaults()
                });
                let router = config.build_router();

                eprintln!("Available model profiles: {:?}", config.profile_names());
                eprintln!("Default profile: {}", config.default_profile());
                eprintln!("Permission mode: Interactive");

                let cwd = std::env::current_dir().expect("Cannot get current dir");

                let runtime = LocalRuntime::new(store, router)
                    .with_permission_mode(PermissionMode::Interactive)
                    .with_context_limit(100_000)
                    .with_memory_store(mem_store.clone())
                    .with_builtin_tools(cwd)
                    .await;

                handle.manage(GuiState::new(runtime, config, mem_store));

                // Background task: cleanup expired soft-deleted sessions (hourly, 7-day threshold)
                {
                    let runtime = handle.state::<GuiState>().inner().runtime.clone();
                    tokio::spawn(async move {
                        let mut interval =
                            tokio::time::interval(std::time::Duration::from_secs(3600));
                        loop {
                            interval.tick().await;
                            match runtime
                                .cleanup_expired_sessions(std::time::Duration::from_secs(7 * 86400))
                                .await
                            {
                                Ok(count) if count > 0 => {
                                    eprintln!("[cleanup] Removed {count} expired session(s)")
                                }
                                Ok(_) => {}
                                Err(e) => eprintln!("[cleanup] Failed: {e}"),
                            }
                        }
                    });
                }
            });
            Ok(())
        })
        .invoke_handler(specta_builder.invoke_handler())
        .run(tauri::generate_context!())
        .expect("failed to run tauri application");
}

/// Export specta TypeScript bindings to a directory.
#[cfg(test)]
pub fn run() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_defaults_include_fake() {
        let config = Config::defaults();
        assert!(config.profile_names().contains(&"fake".to_string()));
    }

    #[test]
    fn config_default_profile_selection() {
        let config = Config::defaults();
        let default = config.default_profile();
        assert!(!default.is_empty());
    }
}

#[cfg(test)]
mod integration_tests {
    use super::*;
    use agent_core::AppFacade;

    async fn create_test_runtime() -> LocalRuntime<SqliteEventStore, ModelRouter> {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let config = Config::defaults();
        let router = config.build_router();
        LocalRuntime::new(store, router)
            .with_permission_mode(PermissionMode::Suggest)
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
