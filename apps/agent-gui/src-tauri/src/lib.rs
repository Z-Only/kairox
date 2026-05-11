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

    let _specta_builder = specta::create_specta();

    // `mut` is only used when the `pilot` feature is enabled in a debug build;
    // suppress the lint when the `#[cfg]` block below is compiled out.
    #[cfg_attr(not(all(debug_assertions, feature = "pilot")), allow(unused_mut))]
    let mut builder = tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init());

    #[cfg(all(debug_assertions, feature = "pilot"))]
    {
        builder = builder.plugin(tauri_plugin_pilot::init());
    }

    builder
        .setup(move |app| {
            let handle = app.handle().clone();
            tauri::async_runtime::block_on(async move {
                // Use a file-backed SQLite database in the user's .kairox directory
                // for persistent storage across app restarts.
                let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
                let home_dir = std::path::PathBuf::from(home);
                let db_dir = home_dir.join(".kairox");
                tokio::fs::create_dir_all(&db_dir).await.ok();
                let db_path = db_dir.join("kairox-gui.sqlite");
                let db_url = format!(
                    "sqlite:///{}?mode=rwc",
                    db_path.display().to_string().trim_start_matches('/')
                );

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
                let skill_roots = agent_runtime::skills::build_default_skill_roots(&home_dir, &cwd);
                let skill_registry = agent_skills::FileSkillRegistry::discover(skill_roots)
                    .await
                    .expect("Failed to discover skills");

                // Read catalog sources from disk so that remote providers
                // (e.g. MCP Registry) configured in mcp_servers.toml are
                // registered in the aggregate at startup — not only after
                // the first explicit refresh.
                let catalog_sources = {
                    let toml_path = db_dir.join("mcp_servers.toml");
                    let user_sources = match std::fs::read_to_string(&toml_path) {
                        Ok(raw) => agent_config::parse_catalog_sources(&raw).unwrap_or_else(|e| {
                            eprintln!("Catalog sources warning: {e}, using defaults");
                            Vec::new()
                        }),
                        Err(_) => Vec::new(),
                    };
                    agent_config::merge_with_defaults(user_sources)
                };
                eprintln!(
                    "Catalog sources: {} (enabled: {})",
                    catalog_sources.len(),
                    catalog_sources.iter().filter(|s| s.enabled).count()
                );

                let ollama_clients = agent_config::build_ollama_clients(&config);
                let config_arc = std::sync::Arc::new(config.clone());
                let runtime = LocalRuntime::new(store, router)
                    .with_permission_mode(PermissionMode::Interactive)
                    .with_context_limit(100_000)
                    .with_memory_store(mem_store.clone())
                    .with_config(config_arc)
                    .with_ollama_clients(ollama_clients)
                    .with_marketplace_loaded(db_dir.clone(), &catalog_sources)
                    .expect("Failed to initialize marketplace")
                    .with_skill_registry(std::sync::Arc::new(skill_registry))
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
        .invoke_handler(tauri::generate_handler![
            crate::commands::list_profiles,
            crate::commands::list_profiles_with_limits,
            crate::commands::get_profile_info,
            crate::commands::refresh_config_for_project,
            crate::commands::initialize_workspace,
            crate::commands::start_session,
            crate::commands::send_message,
            crate::commands::switch_session,
            crate::commands::get_trace,
            crate::commands::list_sessions,
            // Project workspace commands
            crate::commands::list_projects,
            crate::commands::create_blank_project,
            crate::commands::add_existing_project,
            crate::commands::rename_project,
            crate::commands::remove_project,
            crate::commands::restore_project_session,
            crate::commands::update_project_order,
            crate::commands::update_project_expanded,
            crate::commands::create_project_draft_session,
            crate::commands::list_project_sessions,
            crate::commands::list_archived_sessions,
            crate::commands::create_project_worktree_session,
            crate::commands::get_project_git_status,
            crate::commands::get_session_git_status,
            crate::commands::init_project_git,
            crate::commands::get_project_instruction_summary,
            crate::commands::resolve_permission,
            crate::commands::query_memories,
            crate::commands::delete_memory,
            crate::commands::list_workspaces,
            crate::commands::rename_session,
            crate::commands::delete_session,
            crate::commands::get_profile_detail,
            crate::commands::restore_workspace,
            crate::commands::get_task_graph,
            crate::commands::cancel_session,
            crate::commands::compact_session,
            crate::commands::switch_model,
            crate::commands::get_permission_mode,
            crate::commands::get_build_info,
            // Skill commands
            crate::commands::list_skills,
            crate::commands::get_skill_detail,
            crate::commands::activate_skill,
            crate::commands::deactivate_skill,
            crate::commands::list_active_skills,
            // Settings commands
            crate::commands::list_mcp_server_settings,
            crate::commands::upsert_mcp_server_settings,
            crate::commands::set_mcp_server_enabled,
            crate::commands::delete_mcp_server_settings,
            crate::commands::open_mcp_config_file,
            crate::commands::list_skill_settings,
            crate::commands::get_skill_settings_detail,
            crate::commands::set_skill_enabled,
            crate::commands::delete_skill_settings,
            crate::commands::search_remote_skills,
            crate::commands::install_remote_skill,
            crate::commands::install_github_skill,
            crate::commands::update_skill,
            // MCP commands
            crate::commands::list_mcp_servers,
            crate::commands::start_mcp_server,
            crate::commands::stop_mcp_server,
            crate::commands::refresh_mcp_tools,
            crate::commands::trust_mcp_server,
            crate::commands::revoke_mcp_trust,
            crate::commands::list_mcp_resources,
            crate::commands::list_mcp_prompts,
            crate::commands::read_mcp_resource,
            // Marketplace commands
            crate::commands::list_catalog,
            crate::commands::get_catalog_entry,
            crate::commands::refresh_catalog,
            crate::commands::install_catalog_entry,
            crate::commands::uninstall_catalog_entry,
            crate::commands::list_installed_entries,
            // Phase 2: catalog source commands
            crate::commands::list_catalog_sources,
            crate::commands::add_catalog_source,
            crate::commands::remove_catalog_source,
            crate::commands::set_catalog_source_enabled,
        ])
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
        let ollama_clients = agent_config::build_ollama_clients(&config);
        let config_arc = std::sync::Arc::new(config);
        LocalRuntime::new(store, router)
            .with_permission_mode(PermissionMode::Suggest)
            .with_context_limit(100_000)
            .with_config(config_arc)
            .with_ollama_clients(ollama_clients)
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
