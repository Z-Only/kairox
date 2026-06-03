mod app_state;
pub mod commands;
mod event_forwarder;
pub mod specta;

#[cfg(test)]
use agent_config::Config;
#[cfg(not(test))]
use agent_core::AppFacade;
#[cfg(test)]
use agent_models::ModelRouter;
#[cfg(not(test))]
use agent_runtime::ui_bootstrap::{
    build_ui_runtime, default_data_dir, default_home_dir, load_catalog_sources, load_ui_config,
    sqlite_database_url, UiRuntimeOptions,
};
#[cfg(test)]
use agent_runtime::LocalRuntime;
#[cfg(test)]
use agent_store::SqliteEventStore;
use agent_tools::{ApprovalPolicy, SandboxPolicy};

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
            let home_dir = default_home_dir();
            let db_dir = default_data_dir(&home_dir);
            let gui_settings =
                crate::commands::read_gui_settings(&db_dir, cfg!(debug_assertions), None)
                    .map_err(Box::<dyn std::error::Error>::from)?;
            let devtools_enabled = gui_settings.devtools_enabled;

            let handle = app.handle().clone();
            tauri::async_runtime::block_on(async move {
                let db_url = sqlite_database_url(&db_dir, "kairox-gui.sqlite");

                eprintln!("Database: {}", db_url);

                let config_load = load_ui_config(&db_dir);
                for warning in &config_load.warnings {
                    eprintln!("{warning}");
                }
                let catalog_load = load_catalog_sources(&db_dir);
                for warning in &catalog_load.warnings {
                    eprintln!("{warning}");
                }
                let config = config_load.config;
                eprintln!("Available model profiles: {:?}", config.profile_names());
                eprintln!("Default profile: {}", config.default_profile());
                eprintln!(
                    "Default policy: approval={} sandbox={}",
                    ApprovalPolicy::default(),
                    SandboxPolicy::default().kind_str()
                );
                let cwd = std::env::current_dir().expect("Cannot get current dir");
                eprintln!(
                    "Catalog sources: {} (enabled: {})",
                    catalog_load.sources.len(),
                    catalog_load.sources.iter().filter(|s| s.enabled).count()
                );
                let mcp_server_defs = config.mcp_server_defs();
                eprintln!("MCP server definitions: {}", mcp_server_defs.len());
                let runtime_bootstrap = build_ui_runtime(UiRuntimeOptions::new(
                    home_dir,
                    db_dir.clone(),
                    "kairox-gui.sqlite",
                    cwd,
                    ApprovalPolicy::default(),
                    SandboxPolicy::default(),
                    config,
                    catalog_load.sources,
                ))
                .await
                .expect("Failed to initialize runtime");

                let mut gui_state = GuiState::new(
                    runtime_bootstrap.runtime,
                    runtime_bootstrap.config,
                    runtime_bootstrap.memory_store,
                );
                gui_state.profiles_config_path = Some(runtime_bootstrap.profiles_config_path);
                gui_state.home_dir = runtime_bootstrap.data_dir.clone();
                gui_state.devtools_enabled_at_startup = devtools_enabled;
                handle.manage(gui_state);

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
            create_main_window(app, devtools_enabled)?;
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            crate::commands::list_profiles,
            crate::commands::list_profiles_with_limits,
            crate::commands::get_profile_info,
            crate::commands::refresh_config,
            crate::commands::refresh_config_for_project,
            crate::commands::get_gui_settings,
            crate::commands::set_gui_devtools_enabled,
            crate::commands::initialize_workspace,
            crate::commands::start_session,
            crate::commands::send_message,
            crate::commands::switch_session,
            crate::commands::get_trace,
            crate::commands::export_trace,
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
            crate::commands::list_project_branches,
            crate::commands::get_project_git_status,
            crate::commands::get_session_git_status,
            crate::commands::init_project_git,
            crate::commands::get_project_instruction_summary,
            crate::commands::resolve_permission,
            crate::commands::query_memories,
            crate::commands::accept_memory,
            crate::commands::reject_memory,
            crate::commands::delete_memory,
            crate::commands::list_workspaces,
            crate::commands::rename_session,
            crate::commands::delete_session,
            crate::commands::permanently_delete_session,
            crate::commands::restore_archived_session,
            crate::commands::get_profile_detail,
            crate::commands::restore_workspace,
            crate::commands::get_task_graph,
            crate::commands::retry_task,
            crate::commands::cancel_task,
            crate::commands::cancel_session,
            crate::commands::compact_session,
            crate::commands::switch_model,
            crate::commands::get_session_approval_policy,
            crate::commands::set_session_approval_policy,
            crate::commands::get_session_sandbox_policy,
            crate::commands::set_session_sandbox_policy,
            crate::commands::get_build_info,
            // Skill commands
            crate::commands::list_skills,
            crate::commands::get_skill_detail,
            crate::commands::activate_skill,
            crate::commands::deactivate_skill,
            crate::commands::list_active_skills,
            // Settings commands
            crate::commands::list_mcp_server_settings,
            crate::commands::get_effective_mcp_servers,
            crate::commands::get_effective_skills,
            crate::commands::get_effective_model_profiles,
            crate::commands::upsert_mcp_server_settings,
            crate::commands::set_mcp_server_enabled,
            crate::commands::delete_mcp_server_settings,
            crate::commands::disable_mcp_server_at_scope,
            crate::commands::enable_mcp_server_at_scope,
            crate::commands::open_mcp_config_file,
            crate::commands::get_instructions,
            crate::commands::upsert_instructions,
            crate::commands::get_system_prompt,
            crate::commands::get_hooks_settings,
            crate::commands::upsert_hook_settings,
            crate::commands::delete_hook_settings,
            crate::commands::list_profile_settings,
            crate::commands::upsert_profile_settings,
            crate::commands::set_profile_enabled,
            crate::commands::delete_profile_settings,
            crate::commands::move_profile_in_order,
            crate::commands::test_model_connectivity,
            crate::commands::test_url_connectivity,
            crate::commands::open_config_dir,
            crate::commands::open_profiles_config_file,
            crate::commands::open_agents_dir,
            crate::commands::list_agent_settings,
            crate::commands::upsert_agent_settings,
            crate::commands::delete_agent_settings,
            crate::commands::copy_agent_settings,
            crate::commands::open_skills_dir,
            crate::commands::list_skill_settings,
            crate::commands::get_skill_settings_detail,
            crate::commands::set_skill_enabled,
            crate::commands::delete_skill_settings,
            crate::commands::search_remote_skills,
            crate::commands::install_remote_skill,
            crate::commands::install_github_skill,
            crate::commands::update_skill,
            // Skill catalog commands
            crate::commands::list_skill_catalog,
            crate::commands::list_skill_sources,
            crate::commands::add_skill_source,
            crate::commands::remove_skill_source,
            crate::commands::set_skill_source_enabled,
            crate::commands::refresh_skill_catalog,
            // Plugin commands
            crate::commands::list_plugin_settings,
            crate::commands::get_plugin_detail,
            crate::commands::set_plugin_enabled,
            crate::commands::delete_plugin_settings,
            crate::commands::list_plugin_marketplace_sources,
            crate::commands::set_plugin_marketplace_source_enabled,
            crate::commands::list_plugin_catalog,
            crate::commands::install_plugin,
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
            crate::commands::test_mcp_connectivity,
            crate::commands::check_mcp_health,
            crate::commands::set_mcp_tool_disabled,
            crate::commands::get_mcp_tool_states,
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
            // Monitor commands
            crate::commands::list_monitors,
            crate::commands::stop_monitor,
            // Draft persistence commands
            crate::commands::list_workspace_files,
            crate::commands::save_draft,
            crate::commands::get_draft,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run tauri application");
}

#[cfg(not(test))]
fn create_main_window(app: &mut tauri::App, devtools_enabled: bool) -> tauri::Result<()> {
    let window_config = app
        .config()
        .app
        .windows
        .iter()
        .find(|window| window.label == "main")
        .ok_or(tauri::Error::WindowNotFound)?;
    tauri::WebviewWindowBuilder::from_config(app.handle(), window_config)?
        .devtools(devtools_enabled)
        .build()?;
    Ok(())
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
            .with_approval_and_sandbox(ApprovalPolicy::default(), SandboxPolicy::default())
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
                approval_policy: None,
                sandbox_policy: None,
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
                approval_policy: None,
                sandbox_policy: None,
            })
            .await
            .unwrap();

        runtime
            .send_message(agent_core::SendMessageRequest {
                workspace_id: workspace.workspace_id,
                session_id: session_id.clone(),
                content: "hello".into(),
                display_content: None,
                attachments: vec![],
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
                approval_policy: None,
                sandbox_policy: None,
            })
            .await
            .unwrap();

        runtime
            .send_message(agent_core::SendMessageRequest {
                workspace_id: workspace.workspace_id,
                session_id: session_id.clone(),
                content: "hi".into(),
                display_content: None,
                attachments: vec![],
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
                approval_policy: None,
                sandbox_policy: None,
            })
            .await
            .unwrap();

        let mut stream = runtime.subscribe_session(session_id.clone());

        runtime
            .send_message(agent_core::SendMessageRequest {
                workspace_id: workspace.workspace_id,
                session_id,
                content: "test".into(),
                display_content: None,
                attachments: vec![],
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
