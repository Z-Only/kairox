//! TUI App logic integration tests.
//!
//! These tests verify the TUI's core logic (command dispatch, event handling,
//! state transitions) WITHOUT requiring a real terminal. They use the
//! FakeModelClient + in-memory event store to exercise the full
//! LocalRuntime → App event pipeline.

use agent_core::projection::ProjectedRole;
use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_skills::{FileSkillRegistry, SkillRoot, SkillSourceKind};
use agent_store::SqliteEventStore;
use agent_tools::PermissionMode;
use futures::StreamExt;
use std::sync::Arc;

/// Helper: create a runtime with FakeModelClient.
async fn make_runtime() -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["Hello from TUI test!".into()]);
    LocalRuntime::new(store, model).with_permission_mode(PermissionMode::Suggest)
}

#[derive(Default)]
struct TuiMcpFakeFacade {
    calls: std::sync::Mutex<Vec<String>>,
}

impl TuiMcpFakeFacade {
    fn record(&self, call: impl Into<String>) {
        self.calls.lock().expect("calls lock").push(call.into());
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("calls lock").clone()
    }
}

fn agent_settings_view(
    name: &str,
    scope: agent_core::facade::AgentSettingsScope,
) -> agent_core::facade::AgentSettingsView {
    let scope_label = match scope {
        agent_core::facade::AgentSettingsScope::Builtin => "Builtin",
        agent_core::facade::AgentSettingsScope::User => "User",
        agent_core::facade::AgentSettingsScope::Project => "Project",
    };
    agent_core::facade::AgentSettingsView {
        settings_id: format!("{scope_label}:{name}"),
        name: name.into(),
        description: format!("{name} description"),
        scope,
        path: format!("{name}.md"),
        tools: vec!["fs.read".into()],
        model_profile: Some("fast".into()),
        permission_mode: Some("read_only".into()),
        skills: vec!["kairox-dev-workflow".into()],
        nickname_candidates: vec![name.into()],
        enabled: true,
        instructions: format!("{name} instructions"),
        effective: scope != agent_core::facade::AgentSettingsScope::Builtin,
        shadowed_by: None,
        valid: true,
        validation_error: None,
        editable: scope != agent_core::facade::AgentSettingsScope::Builtin,
        deletable: scope != agent_core::facade::AgentSettingsScope::Builtin,
    }
}

#[async_trait::async_trait]
impl agent_core::facade::McpFacade for TuiMcpFakeFacade {
    async fn list_mcp_server_settings(
        &self,
        source_filter: Option<String>,
    ) -> agent_core::Result<Vec<agent_core::facade::McpServerSettingsView>> {
        self.record(format!("list_mcp_server_settings:{source_filter:?}"));
        Ok(vec![agent_core::facade::McpServerSettingsView {
            id: "alpha".into(),
            name: "alpha".into(),
            transport: "stdio".into(),
            enabled: true,
            runtime_status: "stopped".into(),
            trusted: false,
            tool_count: Some(2),
            last_error: None,
            writable: true,
            config_path: Some("/tmp/kairox/config.toml".into()),
            description: Some("Alpha server".into()),
            source: "user".into(),
            verified: false,
        }])
    }

    async fn set_mcp_server_enabled(
        &self,
        server_id: String,
        enabled: bool,
    ) -> agent_core::Result<()> {
        self.record(format!("set_mcp_server_enabled:{server_id}:{enabled}"));
        Ok(())
    }

    async fn delete_mcp_server_settings(&self, server_id: String) -> agent_core::Result<()> {
        self.record(format!("delete_mcp_server_settings:{server_id}"));
        Ok(())
    }

    async fn list_catalog(
        &self,
        query: agent_core::facade::CatalogQuery,
    ) -> agent_core::Result<Vec<agent_core::facade::ServerEntry>> {
        self.record(format!(
            "list_catalog:{:?}:{:?}:{:?}:{:?}:{:?}",
            query.keyword, query.category, query.trust_min, query.source, query.limit
        ));
        Ok(vec![agent_core::facade::ServerEntry {
            id: "filesystem".into(),
            source: "builtin".into(),
            display_name: "Filesystem".into(),
            summary: "File access".into(),
            description: "Filesystem MCP server".into(),
            categories: vec!["dev".into()],
            tags: vec!["files".into()],
            author: Some("Kairox".into()),
            homepage: None,
            version: Some("1.0.0".into()),
            trust: "verified".into(),
            verified: true,
            icon: None,
            install_spec_json: "{}".into(),
            requirements_json: "[]".into(),
            default_env_json: "[]".into(),
        }])
    }

    async fn install_catalog_entry(
        &self,
        request: agent_core::facade::InstallRequest,
    ) -> agent_core::Result<agent_core::facade::InstallOutcomeView> {
        self.record(format!(
            "install_catalog_entry:{}:{}:{}",
            request.catalog_id, request.source, request.auto_start
        ));
        Ok(agent_core::facade::InstallOutcomeView {
            kind: "installed".into(),
            server_id: Some(request.catalog_id),
            started: Some(request.auto_start),
            missing_runtimes: Vec::new(),
            missing_env_keys: Vec::new(),
        })
    }

    async fn uninstall_catalog_entry(&self, server_id: String) -> agent_core::Result<()> {
        self.record(format!("uninstall_catalog_entry:{server_id}"));
        Ok(())
    }

    async fn list_installed_entries(
        &self,
    ) -> agent_core::Result<Vec<agent_core::facade::InstalledEntry>> {
        self.record("list_installed_entries");
        Ok(vec![agent_core::facade::InstalledEntry {
            server_id: "alpha".into(),
            catalog_id: Some("filesystem".into()),
            source: Some("builtin".into()),
            display_name: "Alpha".into(),
            installed_at: "2026-05-21T00:00:00Z".into(),
            running: true,
        }])
    }

    async fn list_catalog_sources(
        &self,
    ) -> agent_core::Result<Vec<agent_core::facade::CatalogSourceView>> {
        self.record("list_catalog_sources");
        Ok(vec![agent_core::facade::CatalogSourceView {
            id: "builtin".into(),
            display_name: "Built-in".into(),
            kind: "builtin".into(),
            url: String::new(),
            api_key_env: None,
            priority: 0,
            default_trust: "verified".into(),
            enabled: true,
            cache_ttl_seconds: None,
            last_error: None,
        }])
    }

    async fn set_catalog_source_enabled(
        &self,
        id: String,
        enabled: bool,
    ) -> agent_core::Result<()> {
        self.record(format!("set_catalog_source_enabled:{id}:{enabled}"));
        Ok(())
    }

    async fn list_profile_settings(
        &self,
        source_filter: Option<String>,
    ) -> agent_core::Result<Vec<agent_core::facade::ProfileSettingsView>> {
        self.record(format!("list_profile_settings:{source_filter:?}"));
        Ok(vec![agent_core::facade::ProfileSettingsView {
            alias: "fast".into(),
            provider: "fake".into(),
            model_id: "fast".into(),
            enabled: true,
            context_window: Some(128000),
            output_limit: Some(4096),
            temperature: None,
            top_p: None,
            top_k: None,
            max_tokens: None,
            base_url: None,
            api_key_env: None,
            has_api_key: true,
            writable: true,
            config_path: Some("/tmp/kairox/profiles.toml".into()),
            source: "profiles_toml".into(),
        }])
    }

    async fn set_profile_enabled(&self, alias: String, enabled: bool) -> agent_core::Result<()> {
        self.record(format!("set_profile_enabled:{alias}:{enabled}"));
        Ok(())
    }

    async fn delete_profile_settings(&self, alias: String) -> agent_core::Result<()> {
        self.record(format!("delete_profile_settings:{alias}"));
        Ok(())
    }

    async fn move_profile_in_order(&self, alias: String, direction: i32) -> agent_core::Result<()> {
        self.record(format!("move_profile_in_order:{alias}:{direction}"));
        Ok(())
    }

    async fn open_profiles_config_file(&self) -> agent_core::Result<Option<String>> {
        self.record("open_profiles_config_file");
        Ok(Some("/tmp/kairox/profiles.toml".into()))
    }
}

#[async_trait::async_trait]
impl agent_core::facade::SessionFacade for TuiMcpFakeFacade {
    async fn open_workspace(
        &self,
        path: String,
    ) -> agent_core::Result<agent_core::facade::WorkspaceInfo> {
        Ok(agent_core::facade::WorkspaceInfo {
            workspace_id: agent_core::WorkspaceId::new(),
            path,
        })
    }

    async fn start_session(
        &self,
        _request: agent_core::facade::StartSessionRequest,
    ) -> agent_core::Result<agent_core::SessionId> {
        Ok(agent_core::SessionId::new())
    }

    async fn send_message(
        &self,
        _request: agent_core::facade::SendMessageRequest,
    ) -> agent_core::Result<()> {
        Ok(())
    }

    async fn decide_permission(
        &self,
        _decision: agent_core::facade::PermissionDecision,
    ) -> agent_core::Result<()> {
        Ok(())
    }

    async fn cancel_session(
        &self,
        _workspace_id: agent_core::WorkspaceId,
        _session_id: agent_core::SessionId,
    ) -> agent_core::Result<()> {
        Ok(())
    }

    async fn get_session_projection(
        &self,
        _session_id: agent_core::SessionId,
    ) -> agent_core::Result<agent_core::projection::SessionProjection> {
        Ok(agent_core::projection::SessionProjection::default())
    }

    async fn get_trace(
        &self,
        _session_id: agent_core::SessionId,
    ) -> agent_core::Result<Vec<agent_core::facade::TraceEntry>> {
        Ok(Vec::new())
    }

    fn subscribe_session(
        &self,
        _session_id: agent_core::SessionId,
    ) -> futures::stream::BoxStream<'static, agent_core::DomainEvent> {
        Box::pin(futures::stream::empty())
    }

    fn subscribe_all(&self) -> futures::stream::BoxStream<'static, agent_core::DomainEvent> {
        Box::pin(futures::stream::empty())
    }

    async fn list_workspaces(&self) -> agent_core::Result<Vec<agent_core::facade::WorkspaceInfo>> {
        Ok(Vec::new())
    }

    async fn list_sessions(
        &self,
        _workspace_id: &agent_core::WorkspaceId,
    ) -> agent_core::Result<Vec<agent_core::facade::SessionMeta>> {
        Ok(Vec::new())
    }

    async fn rename_session(
        &self,
        _session_id: &agent_core::SessionId,
        _title: String,
    ) -> agent_core::Result<()> {
        Ok(())
    }

    async fn soft_delete_session(
        &self,
        _session_id: &agent_core::SessionId,
    ) -> agent_core::Result<()> {
        Ok(())
    }

    async fn cleanup_expired_sessions(
        &self,
        _older_than: std::time::Duration,
    ) -> agent_core::Result<usize> {
        Ok(0)
    }

    async fn get_task_graph(
        &self,
        _session_id: agent_core::SessionId,
    ) -> agent_core::Result<agent_core::facade::TaskGraphSnapshot> {
        Ok(agent_core::facade::TaskGraphSnapshot::default())
    }

    async fn retry_task(
        &self,
        _workspace_id: agent_core::WorkspaceId,
        _session_id: agent_core::SessionId,
        _task_id: agent_core::TaskId,
    ) -> agent_core::Result<()> {
        Ok(())
    }

    async fn cancel_task(
        &self,
        _workspace_id: agent_core::WorkspaceId,
        _session_id: agent_core::SessionId,
        _task_id: agent_core::TaskId,
    ) -> agent_core::Result<()> {
        Ok(())
    }

    async fn get_agent_status(
        &self,
        _session_id: agent_core::SessionId,
    ) -> agent_core::Result<Vec<agent_core::facade::AgentStatusInfo>> {
        Ok(Vec::new())
    }
}

#[async_trait::async_trait]
impl agent_core::facade::SkillsFacade for TuiMcpFakeFacade {}

fn project_meta(
    project_id: &str,
    display_name: &str,
    root_path: &str,
    sort_order: i64,
    expanded: bool,
) -> agent_core::ProjectMeta {
    agent_core::ProjectMeta {
        project_id: agent_core::ProjectId::from_string(project_id.to_string()),
        display_name: display_name.into(),
        root_path: root_path.into(),
        created_at: "2026-05-21T00:00:00Z".into(),
        updated_at: "2026-05-21T00:00:00Z".into(),
        removed_at: None,
        sort_order,
        expanded,
    }
}

#[async_trait::async_trait]
impl agent_core::facade::ProjectFacade for TuiMcpFakeFacade {
    async fn create_blank_project(
        &self,
        workspace_id: agent_core::WorkspaceId,
        display_name: Option<String>,
    ) -> agent_core::Result<agent_core::ProjectMeta> {
        self.record(format!(
            "create_blank_project:{workspace_id}:{display_name:?}"
        ));
        Ok(project_meta(
            "prj_created",
            display_name.as_deref().unwrap_or("New Project"),
            "/tmp/created",
            2,
            true,
        ))
    }

    async fn add_existing_project(
        &self,
        workspace_id: agent_core::WorkspaceId,
        path: String,
    ) -> agent_core::Result<agent_core::ProjectMeta> {
        self.record(format!("add_existing_project:{workspace_id}:{path}"));
        Ok(project_meta("prj_existing", "existing", &path, 3, true))
    }

    async fn rename_project(
        &self,
        project_id: agent_core::ProjectId,
        display_name: String,
    ) -> agent_core::Result<()> {
        self.record(format!("rename_project:{project_id}:{display_name}"));
        Ok(())
    }

    async fn remove_project(&self, project_id: agent_core::ProjectId) -> agent_core::Result<()> {
        self.record(format!("remove_project:{project_id}"));
        Ok(())
    }

    async fn update_project_order(
        &self,
        project_ids: Vec<agent_core::ProjectId>,
    ) -> agent_core::Result<()> {
        let joined = project_ids
            .into_iter()
            .map(|project_id| project_id.to_string())
            .collect::<Vec<_>>()
            .join(",");
        self.record(format!("update_project_order:{joined}"));
        Ok(())
    }

    async fn update_project_expanded(
        &self,
        project_id: agent_core::ProjectId,
        expanded: bool,
    ) -> agent_core::Result<()> {
        self.record(format!("update_project_expanded:{project_id}:{expanded}"));
        Ok(())
    }

    async fn get_project_git_status(
        &self,
        project_id: agent_core::ProjectId,
    ) -> agent_core::Result<agent_core::ProjectGitStatus> {
        self.record(format!("get_project_git_status:{project_id}"));
        Ok(agent_core::ProjectGitStatus {
            kind: agent_core::ProjectGitStatusKind::Clean,
            branch: Some("main".into()),
            worktree_path: "/tmp/project".into(),
            message: None,
        })
    }

    async fn init_project_git(
        &self,
        project_id: agent_core::ProjectId,
    ) -> agent_core::Result<agent_core::ProjectGitStatus> {
        self.record(format!("init_project_git:{project_id}"));
        Ok(agent_core::ProjectGitStatus {
            kind: agent_core::ProjectGitStatusKind::Clean,
            branch: Some("main".into()),
            worktree_path: "/tmp/project".into(),
            message: None,
        })
    }

    async fn get_project_instruction_summary(
        &self,
        project_id: agent_core::ProjectId,
    ) -> agent_core::Result<agent_core::ProjectInstructionSummary> {
        self.record(format!("get_project_instruction_summary:{project_id}"));
        Ok(agent_core::ProjectInstructionSummary {
            source_paths: vec!["/tmp/project/AGENTS.md".into()],
            contents: Some("project instructions".into()),
            warning: None,
        })
    }
}

#[async_trait::async_trait]
impl agent_core::facade::AgentsFacade for TuiMcpFakeFacade {
    async fn list_agent_settings(
        &self,
    ) -> agent_core::Result<Vec<agent_core::facade::AgentSettingsView>> {
        self.record("list_agent_settings");
        Ok(vec![agent_settings_view(
            "worker",
            agent_core::facade::AgentSettingsScope::Builtin,
        )])
    }

    async fn upsert_agent_settings(
        &self,
        input: agent_core::facade::AgentSettingsInput,
    ) -> agent_core::Result<agent_core::facade::AgentSettingsView> {
        self.record(format!(
            "upsert_agent_settings:{:?}:{}",
            input.scope, input.name
        ));
        Ok(agent_settings_view(&input.name, input.scope))
    }

    async fn delete_agent_settings(&self, agent_id: String) -> agent_core::Result<()> {
        self.record(format!("delete_agent_settings:{agent_id}"));
        Ok(())
    }

    async fn copy_agent_settings(
        &self,
        agent_id: String,
        scope: agent_core::facade::AgentSettingsScope,
    ) -> agent_core::Result<agent_core::facade::AgentSettingsView> {
        self.record(format!("copy_agent_settings:{agent_id}:{scope:?}"));
        Ok(agent_settings_view("worker", scope))
    }

    async fn open_agents_dir(&self) -> agent_core::Result<Option<String>> {
        self.record("open_agents_dir");
        Ok(None)
    }
}

#[async_trait::async_trait]
impl agent_core::facade::PluginsFacade for TuiMcpFakeFacade {}

impl AppFacade for TuiMcpFakeFacade {}

// ---------------------------------------------------------------------------
// Test: Workspace → Session → SendMessage → Projection
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tui_send_message_produces_user_and_assistant_messages() {
    let runtime = make_runtime().await;

    let workspace = runtime
        .open_workspace("/tmp/tui-test-workspace".into())
        .await
        .unwrap();

    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "hello from TUI".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    let projection = runtime.get_session_projection(session_id).await.unwrap();
    assert_eq!(
        projection.messages.len(),
        2,
        "Expected user + assistant messages"
    );
    assert_eq!(projection.messages[0].role, ProjectedRole::User);
    assert_eq!(projection.messages[0].content, "hello from TUI");
    assert_eq!(projection.messages[1].role, ProjectedRole::Assistant);
    assert_eq!(projection.messages[1].content, "Hello from TUI test!");
}

// ---------------------------------------------------------------------------
// Test: Event stream mirrors projection data
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tui_event_stream_matches_projection() {
    let runtime = make_runtime().await;

    let workspace = runtime
        .open_workspace("/tmp/tui-event-test".into())
        .await
        .unwrap();

    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
        })
        .await
        .unwrap();

    let mut event_stream = runtime.subscribe_session(session_id.clone());

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "test events".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    // Collect events from stream
    let mut received_events = Vec::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(500);
    loop {
        tokio::select! {
            event = event_stream.next() => {
                match event {
                    Some(e) => received_events.push(e),
                    None => break,
                }
            }
            _ = tokio::time::sleep_until(deadline) => break,
        }
    }

    assert!(
        !received_events.is_empty(),
        "Should receive at least one event"
    );

    let event_types: Vec<&str> = received_events
        .iter()
        .map(|e| e.event_type.as_str())
        .collect();

    assert!(
        event_types.contains(&"UserMessageAdded"),
        "Expected UserMessageAdded in events: {event_types:?}"
    );
    assert!(
        event_types.contains(&"AssistantMessageCompleted"),
        "Expected AssistantMessageCompleted in events: {event_types:?}"
    );
}

// ---------------------------------------------------------------------------
// Test: Multiple sessions, projection isolation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tui_multiple_sessions_have_isolated_projections() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["response".into()]);
    let runtime = LocalRuntime::new(store, model).with_permission_mode(PermissionMode::Suggest);

    let workspace = runtime
        .open_workspace("/tmp/tui-multi-session".into())
        .await
        .unwrap();

    let s1 = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
        })
        .await
        .unwrap();
    let s2 = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
        })
        .await
        .unwrap();

    // Send to s1 only
    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id.clone(),
            session_id: s1.clone(),
            content: "message for s1".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    let proj1 = runtime.get_session_projection(s1).await.unwrap();
    let proj2 = runtime.get_session_projection(s2).await.unwrap();

    assert_eq!(proj1.messages.len(), 2, "Session 1 should have 2 messages");
    assert_eq!(proj2.messages.len(), 0, "Session 2 should have 0 messages");
}

// ---------------------------------------------------------------------------
// Test: Session cancellation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tui_cancel_session_marks_cancelled() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["response".into()]);
    let runtime = LocalRuntime::new(store, model).with_permission_mode(PermissionMode::Suggest);

    let workspace = runtime
        .open_workspace("/tmp/tui-cancel".into())
        .await
        .unwrap();

    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
        })
        .await
        .unwrap();

    runtime
        .cancel_session(workspace.workspace_id, session_id.clone())
        .await
        .unwrap();

    let projection = runtime.get_session_projection(session_id).await.unwrap();
    assert!(
        projection.cancelled,
        "Session should be marked as cancelled"
    );
}

// ---------------------------------------------------------------------------
// Test: Trace entries are populated
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tui_trace_entries_populated_after_message() {
    let runtime = make_runtime().await;

    let workspace = runtime
        .open_workspace("/tmp/tui-trace".into())
        .await
        .unwrap();

    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
        })
        .await
        .unwrap();

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: "trace me".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    let trace = runtime.get_trace(session_id).await.unwrap();
    assert!(
        !trace.is_empty(),
        "Trace should have entries after a message"
    );

    let event_types: Vec<&str> = trace.iter().map(|e| e.event.event_type.as_str()).collect();

    assert!(
        event_types.contains(&"UserMessageAdded"),
        "Trace should contain UserMessageAdded: {event_types:?}"
    );
}

// ---------------------------------------------------------------------------
// Test: Session listing
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tui_session_listing_works() {
    let runtime = make_runtime().await;

    let workspace = runtime
        .open_workspace("/tmp/tui-list".into())
        .await
        .unwrap();

    runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
        })
        .await
        .unwrap();
    runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "smart".into(),

            permission_mode: None,
        })
        .await
        .unwrap();

    let sessions = runtime
        .list_sessions(&workspace.workspace_id)
        .await
        .unwrap();
    assert_eq!(sessions.len(), 2);
}

// ---------------------------------------------------------------------------
// Test: Subscribe-all receives events across sessions
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tui_subscribe_all_receives_events_across_sessions() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["hello".into()]);
    let runtime = LocalRuntime::new(store, model).with_permission_mode(PermissionMode::Suggest);

    let workspace = runtime
        .open_workspace("/tmp/tui-sub-all".into())
        .await
        .unwrap();

    let s1 = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
        })
        .await
        .unwrap();
    let s2 = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),

            permission_mode: None,
        })
        .await
        .unwrap();

    let mut all_stream = runtime.subscribe_all();

    // Send to both sessions
    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id.clone(),
            session_id: s1.clone(),
            content: "msg1".into(),
            attachments: vec![],
        })
        .await
        .unwrap();
    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: s2.clone(),
            content: "msg2".into(),
            attachments: vec![],
        })
        .await
        .unwrap();

    // Collect events
    let mut session_ids = Vec::new();
    let deadline = tokio::time::Instant::now() + std::time::Duration::from_millis(1000);
    loop {
        tokio::select! {
            event = all_stream.next() => {
                match event {
                    Some(e) => {
                        session_ids.push(e.session_id.to_string());
                        if session_ids.len() > 20 { break; }
                    }
                    None => break,
                }
            }
            _ = tokio::time::sleep_until(deadline) => break,
        }
    }

    assert!(
        session_ids.contains(&s1.to_string()),
        "subscribe_all should receive events from session 1"
    );
    assert!(
        session_ids.contains(&s2.to_string()),
        "subscribe_all should receive events from session 2"
    );
}

// ---------------------------------------------------------------------------
// P3 Task 10: `:compact` typed in chat dispatches `Command::CompactSession`
// instead of `Command::SendMessage`.
// ---------------------------------------------------------------------------

#[test]
fn colon_compact_input_dispatches_compact_session_command() {
    use agent_core::projection::SessionProjection;
    use agent_core::{SessionId, WorkspaceId};
    use agent_tui::components::chat::ChatPanel;
    use agent_tui::components::{Command, EventContext, FocusTarget};
    use agent_tui::keybindings::KeyAction;

    let workspace_id = WorkspaceId::new();
    let session_id = Some(SessionId::new());
    let projection = SessionProjection::default();

    let ctx = EventContext {
        focus: FocusTarget::Chat,
        current_session: &projection,
        projects: &[],
        sessions: &[],
        model_profile: "fake",
        permission_mode: PermissionMode::Suggest,
        sidebar_left_visible: true,
        sidebar_right_visible: false,
        workspace_id: &workspace_id,
        current_session_id: &session_id,
    };

    let mut chat = ChatPanel::new();
    for ch in ":compact".chars() {
        let _ = chat.apply_key_action(KeyAction::InputCharacter(ch), &ctx);
    }
    let (_effects, commands) = chat.apply_key_action(KeyAction::SendInput, &ctx);

    assert!(
        commands
            .iter()
            .any(|c| matches!(c, Command::CompactSession { .. })),
        "expected Command::CompactSession; got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|c| matches!(c, Command::SendMessage { .. })),
        "expected NO SendMessage; got {commands:?}"
    );
    // Buffer should be cleared after `:compact` is consumed.
    assert!(
        chat.input_content.is_empty(),
        "expected input cleared, got {:?}",
        chat.input_content
    );
}

// ---------------------------------------------------------------------------
// P4 Task 10: `:model <alias>` typed in chat dispatches `Command::SwitchModel`
// instead of `Command::SendMessage`.
// ---------------------------------------------------------------------------

#[test]
fn colon_model_alias_input_dispatches_switch_model_command() {
    use agent_core::projection::SessionProjection;
    use agent_core::{SessionId, WorkspaceId};
    use agent_tui::components::chat::ChatPanel;
    use agent_tui::components::{Command, EventContext, FocusTarget};
    use agent_tui::keybindings::KeyAction;

    let workspace_id = WorkspaceId::new();
    let session_id = Some(SessionId::new());
    let projection = SessionProjection::default();

    let ctx = EventContext {
        focus: FocusTarget::Chat,
        current_session: &projection,
        projects: &[],
        sessions: &[],
        model_profile: "fake",
        permission_mode: PermissionMode::Suggest,
        sidebar_left_visible: true,
        sidebar_right_visible: false,
        workspace_id: &workspace_id,
        current_session_id: &session_id,
    };

    let mut chat = ChatPanel::new();
    for ch in ":model opus".chars() {
        let _ = chat.apply_key_action(KeyAction::InputCharacter(ch), &ctx);
    }
    let (_effects, commands) = chat.apply_key_action(KeyAction::SendInput, &ctx);

    let found = commands
        .iter()
        .any(|c| matches!(c, Command::SwitchModel { alias, .. } if alias == "opus"));
    assert!(
        found,
        "expected Command::SwitchModel with alias=opus; got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|c| matches!(c, Command::SendMessage { .. })),
        "expected NO SendMessage; got {commands:?}"
    );
    // Buffer should be cleared after `:model <alias>` is consumed.
    assert!(
        chat.input_content.is_empty(),
        "expected input cleared, got {:?}",
        chat.input_content
    );
}

#[test]
fn colon_model_without_alias_falls_through_as_chat_message() {
    use agent_core::projection::SessionProjection;
    use agent_core::{SessionId, WorkspaceId};
    use agent_tui::components::chat::ChatPanel;
    use agent_tui::components::{Command, EventContext, FocusTarget};
    use agent_tui::keybindings::KeyAction;

    let workspace_id = WorkspaceId::new();
    let session_id = Some(SessionId::new());
    let projection = SessionProjection::default();
    let ctx = EventContext {
        focus: FocusTarget::Chat,
        current_session: &projection,
        projects: &[],
        sessions: &[],
        model_profile: "fake",
        permission_mode: PermissionMode::Suggest,
        sidebar_left_visible: true,
        sidebar_right_visible: false,
        workspace_id: &workspace_id,
        current_session_id: &session_id,
    };

    let mut chat = ChatPanel::new();
    for ch in ":model".chars() {
        let _ = chat.apply_key_action(KeyAction::InputCharacter(ch), &ctx);
    }
    let (_effects, commands) = chat.apply_key_action(KeyAction::SendInput, &ctx);

    // `:model` without an alias falls through to SendMessage (user gets
    // feedback the command was malformed — no silent swallow).
    assert!(
        commands
            .iter()
            .any(|c| matches!(c, Command::SendMessage { .. })),
        "expected SendMessage fallback; got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|c| matches!(c, Command::SwitchModel { .. })),
        "expected NO SwitchModel without alias; got {commands:?}"
    );
}

fn chat_commands_for_input(input: &str) -> Vec<agent_tui::components::Command> {
    use agent_core::projection::SessionProjection;
    use agent_core::{SessionId, WorkspaceId};
    use agent_tui::components::chat::ChatPanel;
    use agent_tui::components::{EventContext, FocusTarget};
    use agent_tui::keybindings::KeyAction;

    let workspace_id = WorkspaceId::new();
    let session_id = Some(SessionId::new());
    let projection = SessionProjection::default();
    let ctx = EventContext {
        focus: FocusTarget::Chat,
        current_session: &projection,
        projects: &[],
        sessions: &[],
        model_profile: "fake",
        permission_mode: PermissionMode::Suggest,
        sidebar_left_visible: true,
        sidebar_right_visible: false,
        workspace_id: &workspace_id,
        current_session_id: &session_id,
    };

    let mut chat = ChatPanel::new();
    for character in input.chars() {
        let _ = chat.apply_key_action(KeyAction::InputCharacter(character), &ctx);
    }
    let (_effects, commands) = chat.apply_key_action(KeyAction::SendInput, &ctx);
    commands
}

fn chat_commands_for_project_input(
    input: &str,
) -> (agent_core::ProjectId, Vec<agent_tui::components::Command>) {
    use agent_core::projection::SessionProjection;
    use agent_core::{ProjectId, ProjectSessionVisibility, SessionId, WorkspaceId};
    use agent_tui::components::chat::ChatPanel;
    use agent_tui::components::{EventContext, FocusTarget, SessionInfo, SessionState};
    use agent_tui::keybindings::KeyAction;

    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();
    let project_id = ProjectId::new();
    let projection = SessionProjection::default();
    let sessions = vec![SessionInfo {
        id: session_id.clone(),
        title: "project session".into(),
        model_profile: "fake".into(),
        state: SessionState::Idle,
        pinned: false,
        archived: false,
        project_id: Some(project_id.clone()),
        worktree_path: Some("/tmp/project".into()),
        branch: Some("main".into()),
        visibility: Some(ProjectSessionVisibility::Visible),
    }];
    let current_session_id = Some(session_id);
    let ctx = EventContext {
        focus: FocusTarget::Chat,
        current_session: &projection,
        projects: &[],
        sessions: &sessions,
        model_profile: "fake",
        permission_mode: PermissionMode::Suggest,
        sidebar_left_visible: true,
        sidebar_right_visible: false,
        workspace_id: &workspace_id,
        current_session_id: &current_session_id,
    };

    let mut chat = ChatPanel::new();
    for character in input.chars() {
        let _ = chat.apply_key_action(KeyAction::InputCharacter(character), &ctx);
    }
    let (_effects, commands) = chat.apply_key_action(KeyAction::SendInput, &ctx);
    (project_id, commands)
}

#[test]
fn colon_attach_then_send_carries_attachment_payload() {
    use agent_core::projection::SessionProjection;
    use agent_core::{SessionId, WorkspaceId};
    use agent_tui::components::chat::ChatPanel;
    use agent_tui::components::{Command, EventContext, FocusTarget};
    use agent_tui::keybindings::KeyAction;

    let workspace_id = WorkspaceId::new();
    let session_id = Some(SessionId::new());
    let projection = SessionProjection::default();
    let ctx = EventContext {
        focus: FocusTarget::Chat,
        current_session: &projection,
        projects: &[],
        sessions: &[],
        model_profile: "fake",
        permission_mode: PermissionMode::Suggest,
        sidebar_left_visible: true,
        sidebar_right_visible: false,
        workspace_id: &workspace_id,
        current_session_id: &session_id,
    };

    let manifest = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml");
    let canonical = manifest.canonicalize().unwrap();
    let mut chat = ChatPanel::new();
    for character in format!(":attach {}", manifest.display()).chars() {
        let _ = chat.apply_key_action(KeyAction::InputCharacter(character), &ctx);
    }
    let (_effects, attach_commands) = chat.apply_key_action(KeyAction::SendInput, &ctx);
    assert!(
        attach_commands.is_empty(),
        "attach should only update composer state, got {attach_commands:?}"
    );

    for character in "summarize this".chars() {
        let _ = chat.apply_key_action(KeyAction::InputCharacter(character), &ctx);
    }
    let (_effects, commands) = chat.apply_key_action(KeyAction::SendInput, &ctx);

    assert_eq!(commands.len(), 1);
    assert!(matches!(
        &commands[0],
        Command::SendMessage {
            content,
            attachments,
            ..
        } if content == "summarize this"
            && attachments.len() == 1
            && attachments[0].path == canonical.display().to_string()
            && attachments[0].name == "Cargo.toml"
            && attachments[0].mime_type == "application/toml"
    ));
}

fn write_test_skill(root: &std::path::Path, name: &str, description: &str, body: &str) {
    let skill_directory = root.join(name);
    std::fs::create_dir_all(&skill_directory).expect("skill directory should be created");
    std::fs::write(
        skill_directory.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\n---\n{body}"),
    )
    .expect("skill file should be written");
}

#[test]
fn colon_skills_input_dispatches_list_skills_command() {
    use agent_tui::components::Command;

    let commands = chat_commands_for_input(":skills");

    assert!(
        commands
            .iter()
            .any(|command| matches!(command, Command::ListSkills)),
        "expected Command::ListSkills; got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, Command::SendMessage { .. })),
        "expected NO SendMessage; got {commands:?}"
    );
}

#[test]
fn colon_plugins_input_dispatches_open_plugins_overlay_command() {
    use agent_tui::components::Command;

    let commands = chat_commands_for_input(":plugins");

    assert!(
        commands
            .iter()
            .any(|command| matches!(command, Command::OpenPluginsOverlay)),
        "expected Command::OpenPluginsOverlay; got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, Command::SendMessage { .. })),
        "expected NO SendMessage; got {commands:?}"
    );
}

#[test]
fn colon_agents_input_dispatches_open_agent_settings_overlay_command() {
    use agent_tui::components::Command;

    let commands = chat_commands_for_input(":agents");

    assert!(
        commands
            .iter()
            .any(|command| matches!(command, Command::OpenAgentSettingsOverlay)),
        "expected Command::OpenAgentSettingsOverlay; got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, Command::SendMessage { .. })),
        "expected NO SendMessage; got {commands:?}"
    );
}

#[test]
fn colon_instructions_input_dispatches_open_instructions_overlay_command() {
    use agent_tui::components::Command;

    let commands = chat_commands_for_input(":instructions");

    assert!(
        commands
            .iter()
            .any(|command| matches!(command, Command::OpenInstructionsOverlay)),
        "expected Command::OpenInstructionsOverlay; got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, Command::SendMessage { .. })),
        "expected NO SendMessage; got {commands:?}"
    );
}

#[test]
fn colon_project_draft_input_dispatches_create_project_draft_command() {
    use agent_tui::components::Command;

    let (expected_project_id, commands) = chat_commands_for_project_input(":project draft");

    assert!(
        commands.iter().any(
            |command| matches!(command, Command::CreateProjectDraftSession { project_id } if project_id == &expected_project_id)
        ),
        "expected CreateProjectDraftSession; got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, Command::SendMessage { .. })),
        "expected NO SendMessage; got {commands:?}"
    );
}

#[test]
fn colon_project_create_input_dispatches_create_blank_project_command() {
    use agent_tui::components::Command;

    let commands = chat_commands_for_input(":project create Alpha Workbench");

    assert!(
        commands.iter().any(|command| matches!(
            command,
            Command::CreateBlankProject { display_name: Some(display_name) }
                if display_name == "Alpha Workbench"
        )),
        "expected CreateBlankProject; got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, Command::SendMessage { .. })),
        "expected NO SendMessage; got {commands:?}"
    );
}

#[test]
fn colon_project_import_input_dispatches_add_existing_project_command() {
    use agent_tui::components::Command;

    let commands = chat_commands_for_input(":project import /tmp/kairox-existing");

    assert!(
        commands.iter().any(|command| matches!(
            command,
            Command::AddExistingProject { path } if path == "/tmp/kairox-existing"
        )),
        "expected AddExistingProject; got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, Command::SendMessage { .. })),
        "expected NO SendMessage; got {commands:?}"
    );
}

#[test]
fn colon_project_worktree_input_dispatches_create_worktree_command() {
    use agent_tui::components::Command;

    let (expected_project_id, commands) =
        chat_commands_for_project_input(":project worktree feat/tui");

    assert!(
        commands.iter().any(|command| matches!(
            command,
            Command::CreateProjectWorktreeSession { project_id, branch_name }
                if project_id == &expected_project_id && branch_name == "feat/tui"
        )),
        "expected CreateProjectWorktreeSession; got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, Command::SendMessage { .. })),
        "expected NO SendMessage; got {commands:?}"
    );
}

#[test]
fn colon_skill_show_input_dispatches_show_skill_command() {
    use agent_tui::components::Command;

    let commands = chat_commands_for_input(":skill show test-driven-rust");

    assert!(
        commands.iter().any(
            |command| matches!(command, Command::ShowSkill { skill_id } if skill_id == "test-driven-rust")
        ),
        "expected Command::ShowSkill for test-driven-rust; got {commands:?}"
    );
}

#[test]
fn colon_skill_activate_input_dispatches_activate_skill_command() {
    use agent_tui::components::Command;

    let commands = chat_commands_for_input(":skill activate test-driven-rust");

    assert!(
        commands.iter().any(
            |command| matches!(command, Command::ActivateSkill { skill_id, .. } if skill_id == "test-driven-rust")
        ),
        "expected Command::ActivateSkill for test-driven-rust; got {commands:?}"
    );
}

#[test]
fn colon_skill_deactivate_input_dispatches_deactivate_skill_command() {
    use agent_tui::components::Command;

    let commands = chat_commands_for_input(":skill deactivate test-driven-rust");

    assert!(
        commands.iter().any(
            |command| matches!(command, Command::DeactivateSkill { skill_id, .. } if skill_id == "test-driven-rust")
        ),
        "expected Command::DeactivateSkill for test-driven-rust; got {commands:?}"
    );
}

#[test]
fn colon_skill_catalog_input_dispatches_list_skill_catalog_command() {
    use agent_tui::components::Command;

    let commands = chat_commands_for_input(":skill catalog review");

    assert!(
        commands.iter().any(
            |command| matches!(command, Command::ListSkillCatalog { keyword: Some(keyword) } if keyword == "review")
        ),
        "expected Command::ListSkillCatalog for review; got {commands:?}"
    );
    assert!(
        !commands
            .iter()
            .any(|command| matches!(command, Command::SendMessage { .. })),
        "expected NO SendMessage; got {commands:?}"
    );

    let commands = chat_commands_for_input(":skill catalog");
    assert!(
        commands
            .iter()
            .any(|command| matches!(command, Command::ListSkillCatalog { keyword: None })),
        "expected Command::ListSkillCatalog without keyword; got {commands:?}"
    );
}

#[test]
fn colon_skill_install_github_input_dispatches_github_install_command() {
    use agent_core::facade::SkillInstallTarget;
    use agent_tui::components::Command;

    let commands = chat_commands_for_input(":skill install github owner/review");

    assert!(
        commands.iter().any(|command| matches!(
            command,
            Command::InstallGithubSkill { request }
                if request.source == "owner/review" && request.target == SkillInstallTarget::User
        )),
        "expected Command::InstallGithubSkill for owner/review; got {commands:?}"
    );
}

#[test]
fn skill_catalog_install_update_delete_command_variants_carry_payloads() {
    use agent_core::facade::{
        InstallGithubSkillRequest, InstallRemoteSkillRequest, SkillInstallTarget, SkillUpdateState,
    };
    use agent_tui::components::Command;

    let install = Command::InstallRemoteSkill {
        request: InstallRemoteSkillRequest {
            package: "review".to_string(),
            source: "skillhub".to_string(),
            target: SkillInstallTarget::Project,
            package_url: Some("https://example.test/review.zip".to_string()),
        },
    };
    let github_install = Command::InstallGithubSkill {
        request: InstallGithubSkillRequest {
            source: "owner/review".to_string(),
            target: SkillInstallTarget::User,
        },
    };
    let update = Command::UpdateSkillSettings {
        skill_id: "review".to_string(),
    };
    let delete = Command::DeleteSkillSettings {
        skill_id: "review".to_string(),
    };

    assert!(matches!(
        install,
        Command::InstallRemoteSkill { request }
            if request.package == "review"
                && request.source == "skillhub"
                && request.target == SkillInstallTarget::Project
                && request.package_url.as_deref() == Some("https://example.test/review.zip")
    ));
    assert!(matches!(
        github_install,
        Command::InstallGithubSkill { request }
            if request.source == "owner/review" && request.target == SkillInstallTarget::User
    ));
    assert!(matches!(
        update,
        Command::UpdateSkillSettings { skill_id } if skill_id == "review"
    ));
    assert!(matches!(
        delete,
        Command::DeleteSkillSettings { skill_id } if skill_id == "review"
    ));

    let update_state = SkillUpdateState::UpdateAvailable;
    assert_eq!(update_state, SkillUpdateState::UpdateAvailable);
}

#[tokio::test]
async fn tui_skill_commands_call_facade_and_render_visible_messages() {
    use agent_core::projection::ProjectedRole;
    use agent_core::{EventPayload, SessionId, WorkspaceId};
    use agent_tui::app::App;
    use agent_tui::components::Command;

    let skill_root = std::env::temp_dir().join(format!(
        "kairox-tui-skill-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after UNIX_EPOCH")
            .as_nanos()
    ));
    std::fs::create_dir_all(&skill_root).expect("skill root should be created");
    write_test_skill(
        &skill_root,
        "test-driven-rust",
        "Use when implementing Rust changes with test-first development.",
        "# Test-driven Rust\n\nWrite a failing test first.\n",
    );
    let registry = FileSkillRegistry::discover(vec![SkillRoot::new(
        SkillSourceKind::Workspace,
        &skill_root,
    )])
    .await
    .expect("skill registry should discover test skill");
    let store = SqliteEventStore::in_memory()
        .await
        .expect("in-memory event store");
    let model = FakeModelClient::new(vec!["ok".into()]);
    let runtime = Arc::new(LocalRuntime::new(store, model).with_skill_registry(Arc::new(registry)));

    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();
    let mut app = App::new("fake", PermissionMode::Suggest, workspace_id.clone());
    app.current_session_id = Some(session_id.clone());

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![
            Command::ListSkills,
            Command::ShowSkill {
                skill_id: "test-driven-rust".into(),
            },
            Command::ActivateSkill {
                workspace_id: workspace_id.clone(),
                session_id: session_id.clone(),
                skill_id: "test-driven-rust".into(),
            },
            Command::DeactivateSkill {
                workspace_id,
                session_id: session_id.clone(),
                skill_id: "test-driven-rust".into(),
            },
        ],
    )
    .await;

    let visible_messages: Vec<&str> = app
        .state
        .current_session
        .messages
        .iter()
        .filter(|message| message.role == ProjectedRole::Assistant)
        .map(|message| message.content.as_str())
        .collect();
    assert!(
        visible_messages
            .iter()
            .any(|message| message.contains("test-driven-rust")),
        "expected a visible skill list/detail message; got {visible_messages:?}"
    );
    assert!(
        visible_messages
            .iter()
            .any(|message| message.contains("activated test-driven-rust")),
        "expected visible activation confirmation; got {visible_messages:?}"
    );
    assert!(
        visible_messages
            .iter()
            .any(|message| message.contains("deactivated test-driven-rust")),
        "expected visible deactivation confirmation; got {visible_messages:?}"
    );

    let trace = runtime
        .get_trace(session_id)
        .await
        .expect("skill commands should write trace events");
    assert!(
        trace.iter().any(|entry| {
            matches!(
                &entry.event.payload,
                EventPayload::SkillActivated { skill_id, .. } if skill_id == "test-driven-rust"
            )
        }),
        "expected SkillActivated trace event; got {trace:?}"
    );
    assert!(
        trace.iter().any(|entry| {
            matches!(
                &entry.event.payload,
                EventPayload::SkillDeactivated { skill_id, .. } if skill_id == "test-driven-rust"
            )
        }),
        "expected SkillDeactivated trace event; got {trace:?}"
    );

    std::fs::remove_dir_all(skill_root).expect("test skill root should be cleaned up");
}

#[tokio::test]
async fn tui_mcp_marketplace_commands_call_facade_and_refresh_overlay() {
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::components::Command;
    use std::collections::BTreeMap;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let mut app = App::new("fake", PermissionMode::Suggest, WorkspaceId::new());

    agent_tui::app::dispatch_commands(&runtime, &mut app, vec![Command::OpenMcpOverlay]).await;

    assert!(app.mcp_overlay.is_visible());
    assert_eq!(app.mcp_overlay.settings_len(), 1);
    assert_eq!(app.mcp_overlay.catalog_len(), 1);
    assert_eq!(app.mcp_overlay.sources_len(), 1);
    let calls = runtime.calls();
    assert!(
        calls
            .iter()
            .any(|call| call.starts_with("list_mcp_server_settings")),
        "expected settings list call, got {calls:?}"
    );
    assert!(
        calls.iter().any(|call| call == "list_installed_entries"),
        "expected installed list call, got {calls:?}"
    );
    assert!(
        calls.iter().any(|call| call.starts_with("list_catalog")),
        "expected catalog list call, got {calls:?}"
    );
    assert!(
        calls.iter().any(|call| call == "list_catalog_sources"),
        "expected catalog sources call, got {calls:?}"
    );

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![
            Command::SetMcpServerEnabled {
                server_id: "alpha".into(),
                enabled: false,
            },
            Command::DeleteMcpServerSettings {
                server_id: "alpha".into(),
            },
            Command::InstallMcpServer {
                request: agent_core::facade::InstallRequest {
                    catalog_id: "filesystem".into(),
                    source: "builtin".into(),
                    server_id_override: None,
                    env_overrides: BTreeMap::new(),
                    trust_grant: false,
                    auto_start: true,
                },
            },
            Command::UninstallMcpServer {
                server_id: "alpha".into(),
            },
            Command::SetMcpCatalogSourceEnabled {
                source_id: "builtin".into(),
                enabled: false,
            },
        ],
    )
    .await;

    let calls = runtime.calls();
    for expected in [
        "set_mcp_server_enabled:alpha:false",
        "delete_mcp_server_settings:alpha",
        "install_catalog_entry:filesystem:builtin:true",
        "uninstall_catalog_entry:alpha",
        "set_catalog_source_enabled:builtin:false",
    ] {
        assert!(
            calls.iter().any(|call| call == expected),
            "expected call {expected}, got {calls:?}"
        );
    }
}

#[tokio::test]
async fn tui_model_profile_settings_commands_call_facade_and_report_results() {
    use agent_core::projection::ProjectedRole;
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::components::Command;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let mut app = App::new("fake", PermissionMode::Suggest, WorkspaceId::new());

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![
            Command::SetProfileEnabled {
                alias: "fast".into(),
                enabled: false,
            },
            Command::MoveProfileInOrder {
                alias: "fast".into(),
                direction: 1,
            },
            Command::TestModelProfile {
                alias: "fast".into(),
            },
            Command::DeleteProfileSettings {
                alias: "fast".into(),
            },
        ],
    )
    .await;

    let calls = runtime.calls();
    for expected in [
        "set_profile_enabled:fast:false",
        "move_profile_in_order:fast:1",
        "list_profile_settings:None",
        "delete_profile_settings:fast",
    ] {
        assert!(
            calls.iter().any(|call| call == expected),
            "expected call {expected}, got {calls:?}"
        );
    }

    let visible_messages: Vec<&str> = app
        .state
        .current_session
        .messages
        .iter()
        .filter(|message| message.role == ProjectedRole::Assistant)
        .map(|message| message.content.as_str())
        .collect();
    assert!(
        visible_messages
            .iter()
            .any(|message| message.contains("model profile fast connectivity ok")),
        "expected visible model test result; got {visible_messages:?}"
    );
}

#[tokio::test]
async fn tui_agent_settings_commands_call_facade_and_refresh_overlay() {
    use agent_core::facade::{AgentSettingsInput, AgentSettingsScope};
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::components::Command;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let mut app = App::new("fake", PermissionMode::Suggest, WorkspaceId::new());

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![
            Command::OpenAgentSettingsOverlay,
            Command::SaveAgentSettings {
                input: AgentSettingsInput {
                    scope: AgentSettingsScope::Project,
                    name: "planner".into(),
                    description: "Plans work".into(),
                    tools: vec!["search".into()],
                    model_profile: Some("reasoning".into()),
                    permission_mode: Some("workspace_write".into()),
                    skills: vec!["kairox-dev-workflow".into()],
                    nickname_candidates: vec!["Planner".into()],
                    enabled: true,
                    instructions: "Break work into steps.".into(),
                },
            },
            Command::CopyAgentSettings {
                settings_id: "Builtin:worker".into(),
                scope: AgentSettingsScope::User,
            },
            Command::DeleteAgentSettings {
                settings_id: "User:planner".into(),
            },
        ],
    )
    .await;

    assert!(app.agent_overlay.is_visible());
    let calls = runtime.calls();
    for expected in [
        "list_agent_settings",
        "upsert_agent_settings:Project:planner",
        "copy_agent_settings:Builtin:worker:User",
        "delete_agent_settings:User:planner",
    ] {
        assert!(
            calls.iter().any(|call| call == expected),
            "expected call {expected}, got {calls:?}"
        );
    }
}

#[tokio::test]
async fn tui_project_manager_commands_call_facade_and_update_state() {
    use agent_core::{ProjectId, WorkspaceId};
    use agent_tui::app::App;
    use agent_tui::components::{Command, ProjectInfo};

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let workspace_id = WorkspaceId::new();
    let mut app = App::new("fake", PermissionMode::Suggest, workspace_id.clone());
    let alpha = ProjectId::from_string("prj_alpha".to_string());
    let beta = ProjectId::from_string("prj_beta".to_string());
    app.state.projects = vec![
        ProjectInfo {
            id: alpha.clone(),
            display_name: "alpha".into(),
            root_path: "/tmp/alpha".into(),
            expanded: true,
            git_status: None,
            instruction_summary: None,
        },
        ProjectInfo {
            id: beta.clone(),
            display_name: "beta".into(),
            root_path: "/tmp/beta".into(),
            expanded: true,
            git_status: None,
            instruction_summary: None,
        },
    ];

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![
            Command::CreateBlankProject {
                display_name: Some("Gamma".into()),
            },
            Command::AddExistingProject {
                path: "/tmp/imported".into(),
            },
            Command::RenameProject {
                project_id: alpha.clone(),
                display_name: "Alpha renamed".into(),
            },
            Command::MoveProject {
                project_id: beta.clone(),
                direction: -1,
            },
            Command::SetProjectExpanded {
                project_id: alpha.clone(),
                expanded: false,
            },
            Command::RefreshProjectGitStatus {
                project_id: alpha.clone(),
            },
            Command::InitProjectGit {
                project_id: alpha.clone(),
            },
            Command::ShowProjectInstructions {
                project_id: alpha.clone(),
            },
            Command::RemoveProject {
                project_id: alpha.clone(),
            },
        ],
    )
    .await;

    let calls = runtime.calls();
    for expected in [
        format!("create_blank_project:{workspace_id}:Some(\"Gamma\")"),
        format!("add_existing_project:{workspace_id}:/tmp/imported"),
        format!("rename_project:{alpha}:Alpha renamed"),
        format!("update_project_order:{beta},{alpha},prj_created,prj_existing"),
        format!("update_project_expanded:{alpha}:false"),
        format!("get_project_git_status:{alpha}"),
        format!("init_project_git:{alpha}"),
        format!("get_project_instruction_summary:{alpha}"),
        format!("remove_project:{alpha}"),
    ] {
        assert!(
            calls.iter().any(|call| call == &expected),
            "expected call {expected}, got {calls:?}"
        );
    }

    assert!(
        app.state.projects.iter().all(|project| project.id != alpha),
        "removed project should leave local project list"
    );
    assert!(
        app.state
            .projects
            .iter()
            .any(|project| project.id == ProjectId::from_string("prj_created".to_string())),
        "created project should be inserted"
    );
    assert!(
        app.state
            .current_session
            .messages
            .iter()
            .any(|message| message.content.contains("project instructions")),
        "instruction command should surface summary content"
    );
}

#[tokio::test]
async fn tui_skill_catalog_settings_commands_call_facade_and_render_visible_messages() {
    use agent_core::facade::{InstallRemoteSkillRequest, SkillInstallTarget, SkillUpdateState};
    use agent_core::projection::ProjectedRole;
    use agent_core::WorkspaceId;
    use agent_runtime::skill_package::FakeSkillPackageManager;
    use agent_runtime::skill_settings::SkillSettingsRoots;
    use agent_tui::app::App;
    use agent_tui::components::Command;

    let temp_root = std::env::temp_dir().join(format!(
        "kairox-tui-skill-settings-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system time should be after UNIX_EPOCH")
            .as_nanos()
    ));
    let user_root = temp_root.join("user-skills");
    let catalog_root = temp_root.join("catalog");
    std::fs::create_dir_all(&user_root).expect("user skill root should be created");
    std::fs::create_dir_all(&catalog_root).expect("catalog root should be created");
    std::fs::write(
        catalog_root.join("skill_sources.toml"),
        r#"
[[skill_sources]]
id = "skillhub"
display_name = "SkillHub"
kind = "skillhub"
url = "https://api.skillhub.cn"
search_template = "/api/skills?keyword={{query}}"
download_template = "/api/v1/download?slug={{slug}}"
enabled = false
priority = 1
cache_ttl_seconds = 900
"#,
    )
    .expect("disabled catalog source should be written");
    write_test_skill(
        &user_root,
        "review",
        "Review code changes.",
        "# Review\n\nReview code carefully.\n",
    );

    let manager = Arc::new(FakeSkillPackageManager::default());
    *manager.check_updates_result.lock().await = SkillUpdateState::UpToDate;
    let store = SqliteEventStore::in_memory()
        .await
        .expect("in-memory event store");
    let model = FakeModelClient::new(vec!["ok".into()]);
    let runtime = Arc::new(
        LocalRuntime::new(store, model)
            .with_skill_package_manager(manager.clone())
            .with_skill_settings_roots(SkillSettingsRoots {
                workspace_root: None,
                user_root: Some(user_root.clone()),
                builtin_root: None,
                plugin_roots: Vec::new(),
            })
            .with_skill_catalog(Some(catalog_root)),
    );

    let workspace_id = WorkspaceId::new();
    let mut app = App::new("fake", PermissionMode::Suggest, workspace_id);

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![
            Command::ListSkillCatalog {
                keyword: Some("review".into()),
            },
            Command::InstallRemoteSkill {
                request: InstallRemoteSkillRequest {
                    package: "@skills/review".into(),
                    source: "skillhub".into(),
                    target: SkillInstallTarget::User,
                    package_url: Some("https://example.test/review.zip".into()),
                },
            },
            Command::UpdateSkillSettings {
                skill_id: "review".into(),
            },
            Command::DeleteSkillSettings {
                skill_id: "review".into(),
            },
        ],
    )
    .await;

    let visible_messages: Vec<&str> = app
        .state
        .current_session
        .messages
        .iter()
        .filter(|message| message.role == ProjectedRole::Assistant)
        .map(|message| message.content.as_str())
        .collect();
    assert!(
        visible_messages
            .iter()
            .any(|message| message.contains("No catalog skills found for review")),
        "expected visible catalog empty-state message; got {visible_messages:?}"
    );
    assert!(
        visible_messages
            .iter()
            .any(|message| message.contains("installed skill review")),
        "expected visible install confirmation; got {visible_messages:?}"
    );
    assert!(
        visible_messages
            .iter()
            .any(|message| message.contains("updated skill review")),
        "expected visible update confirmation; got {visible_messages:?}"
    );
    assert!(
        visible_messages
            .iter()
            .any(|message| message.contains("deleted skill review")),
        "expected visible delete confirmation; got {visible_messages:?}"
    );
    assert_eq!(manager.registry_install_requests.lock().await.len(), 1);
    assert_eq!(
        manager.registry_install_requests.lock().await[0].package,
        "@skills/review"
    );
    assert_eq!(manager.update_skill_ids.lock().await.as_slice(), ["review"]);
    assert!(
        !user_root.join("review").exists(),
        "delete command should remove the user skill directory"
    );

    std::fs::remove_dir_all(temp_root).expect("test skill settings root should be cleaned up");
}
