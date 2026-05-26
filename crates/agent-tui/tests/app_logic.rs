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
use futures::StreamExt;
use std::sync::Arc;

/// Helper: create a runtime with FakeModelClient.
async fn make_runtime() -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["Hello from TUI test!".into()]);
    LocalRuntime::new(store, model)
}

#[derive(Default)]
struct TuiMcpFakeFacade {
    calls: std::sync::Mutex<Vec<String>>,
    last_install_request: std::sync::Mutex<Option<agent_core::facade::InstallRequest>>,
    install_result: std::sync::Mutex<Option<FakeInstallResult>>,
}

#[derive(Clone)]
enum FakeInstallResult {
    Outcome(agent_core::facade::InstallOutcomeView),
    Error(String),
}

impl TuiMcpFakeFacade {
    fn with_install_result(install_result: FakeInstallResult) -> Self {
        Self {
            install_result: std::sync::Mutex::new(Some(install_result)),
            ..Self::default()
        }
    }

    fn record(&self, call: impl Into<String>) {
        self.calls.lock().expect("calls lock").push(call.into());
    }

    fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("calls lock").clone()
    }

    fn last_install_request(&self) -> Option<agent_core::facade::InstallRequest> {
        self.last_install_request
            .lock()
            .expect("last install request lock")
            .clone()
    }

    fn install_result(&self) -> Option<FakeInstallResult> {
        self.install_result
            .lock()
            .expect("install result lock")
            .clone()
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

fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{}-{nonce}", std::process::id()))
}

fn test_project(
    project_id: &str,
    root_path: &std::path::Path,
) -> agent_tui::components::ProjectInfo {
    agent_tui::components::ProjectInfo {
        id: agent_core::ProjectId::from_string(project_id.to_string()),
        display_name: project_id.to_string(),
        root_path: root_path.display().to_string(),
        expanded: true,
        git_status: None,
        instruction_summary: None,
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
        *self
            .last_install_request
            .lock()
            .expect("last install request lock") = Some(request.clone());
        self.record(format!(
            "install_catalog_entry:{}:{}:{}",
            request.catalog_id, request.source, request.auto_start
        ));
        match self.install_result() {
            Some(FakeInstallResult::Outcome(outcome)) => Ok(outcome),
            Some(FakeInstallResult::Error(error)) => {
                Err(agent_core::CoreError::InvalidState(error))
            }
            None => Ok(agent_core::facade::InstallOutcomeView {
                kind: "installed".into(),
                server_id: Some(request.catalog_id),
                started: Some(request.auto_start),
                missing_runtimes: Vec::new(),
                missing_env_keys: Vec::new(),
            }),
        }
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

    async fn upsert_profile_settings(
        &self,
        input: agent_core::facade::ProfileSettingsInput,
    ) -> agent_core::Result<agent_core::facade::ProfileSettingsView> {
        self.record(format!(
            "upsert_profile_settings:{}:{}:{}",
            input.alias, input.provider, input.model_id
        ));
        Ok(agent_core::facade::ProfileSettingsView {
            alias: input.alias,
            provider: input.provider,
            model_id: input.model_id,
            enabled: input.enabled,
            context_window: input.context_window,
            output_limit: input.output_limit,
            temperature: input.temperature,
            top_p: input.top_p,
            top_k: input.top_k,
            max_tokens: input.max_tokens,
            base_url: input.base_url,
            api_key_env: input.api_key_env,
            has_api_key: true,
            writable: true,
            config_path: Some("/tmp/kairox/profiles.toml".into()),
            source: "profiles_toml".into(),
        })
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

    async fn open_config_dir(&self) -> agent_core::Result<Option<String>> {
        self.record("open_config_dir");
        Ok(None)
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
impl agent_core::facade::SkillsFacade for TuiMcpFakeFacade {
    async fn list_skill_settings(
        &self,
    ) -> agent_core::Result<Vec<agent_core::facade::SkillSettingsView>> {
        self.record("list_skill_settings");
        Ok(Vec::new())
    }

    async fn list_skill_catalog(
        &self,
        query: agent_core::facade::SkillCatalogQuery,
    ) -> agent_core::Result<Vec<agent_core::facade::SkillCatalogEntry>> {
        self.record(format!(
            "list_skill_catalog:{:?}:{:?}:{:?}",
            query.keyword, query.sources, query.limit
        ));
        Ok(Vec::new())
    }

    async fn list_skill_sources(
        &self,
    ) -> agent_core::Result<Vec<agent_core::facade::SkillSourceView>> {
        self.record("list_skill_sources");
        Ok(vec![agent_core::facade::SkillSourceView {
            id: "skillhub".into(),
            display_name: "SkillHub".into(),
            kind: "skillhub".into(),
            url: "https://api.skillhub.cn".into(),
            search_template: "/api/skills?keyword={{query}}".into(),
            download_template: "/api/v1/download?slug={{slug}}".into(),
            list_template: None,
            detail_template: None,
            field_mapping: agent_core::facade::SkillFieldMappingView::default(),
            enabled: true,
            priority: 1,
            cache_ttl_seconds: 900,
            last_error: None,
        }])
    }

    async fn add_skill_source(
        &self,
        config: agent_core::facade::SkillSourceView,
    ) -> agent_core::Result<()> {
        self.record(format!("add_skill_source:{}", config.id));
        Ok(())
    }

    async fn remove_skill_source(&self, id: String) -> agent_core::Result<()> {
        self.record(format!("remove_skill_source:{id}"));
        Ok(())
    }

    async fn set_skill_source_enabled(&self, id: String, enabled: bool) -> agent_core::Result<()> {
        self.record(format!("set_skill_source_enabled:{id}:{enabled}"));
        Ok(())
    }

    async fn refresh_skill_catalog(&self) -> agent_core::Result<()> {
        self.record("refresh_skill_catalog");
        Ok(())
    }

    async fn open_skills_dir(&self) -> agent_core::Result<Option<String>> {
        self.record("open_skills_dir");
        Ok(None)
    }
}

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
impl agent_core::facade::PluginsFacade for TuiMcpFakeFacade {
    async fn list_plugin_settings(
        &self,
    ) -> agent_core::Result<Vec<agent_core::facade::PluginSettingsView>> {
        self.record("list_plugin_settings");
        Ok(Vec::new())
    }

    async fn list_plugin_marketplace_sources(
        &self,
    ) -> agent_core::Result<Vec<agent_core::facade::PluginMarketplaceSourceView>> {
        self.record("list_plugin_marketplace_sources");
        Ok(vec![agent_core::facade::PluginMarketplaceSourceView {
            id: "local-market".into(),
            display_name: "Local market".into(),
            source: "/tmp/local-market".into(),
            enabled: true,
            builtin: false,
        }])
    }

    async fn list_plugin_catalog(
        &self,
        marketplace_id: Option<String>,
        keyword: Option<String>,
    ) -> agent_core::Result<Vec<agent_core::facade::PluginCatalogEntry>> {
        self.record(format!(
            "list_plugin_catalog:{marketplace_id:?}:{keyword:?}"
        ));
        Ok(vec![agent_core::facade::PluginCatalogEntry {
            marketplace_id: marketplace_id.unwrap_or_else(|| "local-market".into()),
            name: keyword.unwrap_or_else(|| "delta".into()),
            description: "Delta plugin".into(),
            version: Some("0.1.0".into()),
            source: "/tmp/local-market/delta".into(),
        }])
    }
}

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
            approval_policy: None,
            sandbox_policy: None,
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
            approval_policy: None,
            sandbox_policy: None,
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
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime
        .open_workspace("/tmp/tui-multi-session".into())
        .await
        .unwrap();

    let s1 = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();
    let s2 = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
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
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime
        .open_workspace("/tmp/tui-cancel".into())
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
            approval_policy: None,
            sandbox_policy: None,
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
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();
    runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "smart".into(),
            approval_policy: None,
            sandbox_policy: None,
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
    let runtime = LocalRuntime::new(store, model);

    let workspace = runtime
        .open_workspace("/tmp/tui-sub-all".into())
        .await
        .unwrap();

    let s1 = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .unwrap();
    let s2 = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
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
fn overlay_shortcut_smoke_matrix_emits_open_command_and_dismisses_effect() {
    use agent_core::facade::{
        AgentSettingsScope, HooksSettingsView, InstructionsView, PluginInstallTarget,
    };
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::components::{
        AgentOverlaySnapshot, Command, CrossPanelEffect, FocusTarget, McpOverlaySnapshot,
        ModelOverlaySnapshot, ModelProfileEntry, PluginOverlaySnapshot, SkillEntry,
        SkillOverlaySnapshot,
    };
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode, modifiers: KeyModifiers) -> Event {
        Event::Key(KeyEvent::new(code, modifiers))
    }

    fn escape_key() -> Event {
        key(KeyCode::Esc, KeyModifiers::NONE)
    }

    fn is_mcp_open(command: &Command) -> bool {
        matches!(command, Command::OpenMcpOverlay)
    }

    fn is_skills_open(command: &Command) -> bool {
        matches!(command, Command::OpenSkillsOverlay)
    }

    fn is_plugins_open(command: &Command) -> bool {
        matches!(command, Command::OpenPluginsOverlay)
    }

    fn is_model_open(command: &Command) -> bool {
        matches!(command, Command::OpenModelOverlay)
    }

    fn is_hooks_open(command: &Command) -> bool {
        matches!(command, Command::OpenHooksOverlay)
    }

    fn is_instructions_open(command: &Command) -> bool {
        matches!(command, Command::OpenInstructionsOverlay)
    }

    fn mcp_visible(app: &App) -> bool {
        app.mcp_overlay.is_visible()
    }

    fn skills_visible(app: &App) -> bool {
        app.skills_overlay.is_visible()
    }

    fn plugins_visible(app: &App) -> bool {
        app.plugin_overlay.is_visible()
    }

    fn model_visible(app: &App) -> bool {
        app.model_overlay.is_visible()
    }

    fn hooks_visible(app: &App) -> bool {
        app.hooks_overlay.is_visible()
    }

    fn instructions_visible(app: &App) -> bool {
        app.instructions_overlay.is_visible()
    }

    fn show_mcp(app: &mut App) {
        app.dispatch_effects(vec![CrossPanelEffect::ShowMcpOverlay(McpOverlaySnapshot {
            runtime_servers: Vec::new(),
            settings: Vec::new(),
            installed: Vec::new(),
            catalog: Vec::new(),
            sources: Vec::new(),
        })]);
    }

    fn show_skills(app: &mut App) {
        app.dispatch_effects(vec![CrossPanelEffect::ShowSkillsOverlay(
            SkillOverlaySnapshot::from(vec![SkillEntry {
                id: "smoke-skill".into(),
                name: "Smoke Skill".into(),
                description: "Smoke test skill".into(),
                source: "test".into(),
                activation_mode: "manual".into(),
                active: false,
            }]),
        )]);
    }

    fn show_plugins(app: &mut App) {
        app.dispatch_effects(vec![CrossPanelEffect::ShowPluginsOverlay(
            PluginOverlaySnapshot {
                plugins: Vec::new(),
                catalog: Vec::new(),
                sources: Vec::new(),
                install_target: PluginInstallTarget::User,
            },
        )]);
    }

    fn show_model(app: &mut App) {
        app.dispatch_effects(vec![CrossPanelEffect::ShowModelOverlay(
            ModelOverlaySnapshot {
                profiles: vec![ModelProfileEntry {
                    alias: "fast".into(),
                    provider_display: "fake".into(),
                    model_display: "fake-model".into(),
                    context_window: Some(128_000),
                    output_limit: Some(4096),
                    temperature: None,
                    top_p: None,
                    top_k: None,
                    max_tokens: None,
                    base_url: None,
                    api_key_env: None,
                    supports_reasoning: false,
                    enabled: true,
                    writable: true,
                    source: "test".into(),
                    has_api_key: true,
                }],
                current_alias: Some("fast".into()),
                current_effort: None,
            },
        )]);
    }

    fn show_hooks(app: &mut App) {
        app.dispatch_effects(vec![CrossPanelEffect::ShowHooksOverlay(
            HooksSettingsView {
                user: Vec::new(),
                project: Vec::new(),
                templates: Vec::new(),
                user_config_path: "/tmp/kairox-user.toml".into(),
                project_config_path: Some("/tmp/kairox-project.toml".into()),
            },
        )]);
    }

    fn show_instructions(app: &mut App) {
        app.dispatch_effects(vec![CrossPanelEffect::ShowInstructionsOverlay(
            InstructionsView {
                system: "system prompt".into(),
                user: Some("user instructions".into()),
                project: Some("project instructions".into()),
            },
        )]);
    }

    struct OverlaySmokeCase {
        name: &'static str,
        open_key: Event,
        expected_command: fn(&Command) -> bool,
        show: fn(&mut App),
        is_visible: fn(&App) -> bool,
        focus: FocusTarget,
    }

    let cases = [
        OverlaySmokeCase {
            name: "mcp",
            open_key: key(KeyCode::Char('m'), KeyModifiers::CONTROL),
            expected_command: is_mcp_open,
            show: show_mcp,
            is_visible: mcp_visible,
            focus: FocusTarget::McpOverlay,
        },
        OverlaySmokeCase {
            name: "skills",
            open_key: key(KeyCode::Char('s'), KeyModifiers::CONTROL),
            expected_command: is_skills_open,
            show: show_skills,
            is_visible: skills_visible,
            focus: FocusTarget::SkillsOverlay,
        },
        OverlaySmokeCase {
            name: "plugins",
            open_key: key(KeyCode::Char('g'), KeyModifiers::CONTROL),
            expected_command: is_plugins_open,
            show: show_plugins,
            is_visible: plugins_visible,
            focus: FocusTarget::PluginOverlay,
        },
        OverlaySmokeCase {
            name: "model",
            open_key: key(KeyCode::Char('l'), KeyModifiers::CONTROL),
            expected_command: is_model_open,
            show: show_model,
            is_visible: model_visible,
            focus: FocusTarget::ModelOverlay,
        },
        OverlaySmokeCase {
            name: "hooks",
            open_key: key(KeyCode::Char('h'), KeyModifiers::ALT),
            expected_command: is_hooks_open,
            show: show_hooks,
            is_visible: hooks_visible,
            focus: FocusTarget::HooksOverlay,
        },
        OverlaySmokeCase {
            name: "instructions",
            open_key: key(KeyCode::Char('i'), KeyModifiers::ALT),
            expected_command: is_instructions_open,
            show: show_instructions,
            is_visible: instructions_visible,
            focus: FocusTarget::InstructionsOverlay,
        },
    ];

    for case in cases {
        let mut app = App::new("fake", WorkspaceId::new());

        let commands = app.handle_crossterm_event(&case.open_key);
        assert!(
            commands.iter().any(case.expected_command),
            "expected {} open command, got {commands:?}",
            case.name
        );

        (case.show)(&mut app);
        assert!((case.is_visible)(&app), "{} overlay should open", case.name);
        assert_eq!(
            app.state.focus_manager.current(),
            case.focus,
            "{} overlay should take focus",
            case.name
        );

        let commands = app.handle_crossterm_event(&escape_key());
        assert!(
            commands.is_empty(),
            "expected Esc to dismiss {} without commands, got {commands:?}",
            case.name
        );
        assert!(
            !(case.is_visible)(&app),
            "{} overlay should close on Esc",
            case.name
        );
        assert_eq!(
            app.state.focus_manager.current(),
            FocusTarget::Chat,
            "{} overlay should restore chat focus",
            case.name
        );
    }

    let mut app = App::new("fake", WorkspaceId::new());
    app.dispatch_effects(vec![CrossPanelEffect::ShowAgentSettingsOverlay(
        AgentOverlaySnapshot {
            agents: vec![agent_settings_view("worker", AgentSettingsScope::Builtin)],
        },
    )]);
    assert!(app.agent_overlay.is_visible());
    assert_eq!(app.state.focus_manager.current(), FocusTarget::AgentOverlay);
    let commands = app.handle_crossterm_event(&escape_key());
    assert!(
        commands.is_empty(),
        "expected Esc to dismiss agents without commands, got {commands:?}"
    );
    assert!(!app.agent_overlay.is_visible());
    assert_eq!(app.state.focus_manager.current(), FocusTarget::Chat);
}

#[test]
fn archive_overlay_smoke_opens_from_sessions_focus_and_restores_selected_session() {
    use agent_core::{ProjectSessionVisibility, SessionId, WorkspaceId};
    use agent_tui::app::App;
    use agent_tui::components::{Command, FocusTarget, SessionInfo, SessionState};
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    let archived_id = SessionId::from_string("ses_archived".into());
    let app_session = SessionInfo {
        id: archived_id.clone(),
        title: "archived".into(),
        model_profile: "fake".into(),
        state: SessionState::Idle,
        pinned: false,
        archived: true,
        project_id: None,
        worktree_path: None,
        branch: None,
        visibility: Some(ProjectSessionVisibility::Archived),
    };
    let mut app = App::new("fake", WorkspaceId::new());
    app.state.sessions = vec![app_session];
    app.state.focus_manager.set(FocusTarget::Sessions);
    app.sync_component_focus();

    let commands = app.handle_crossterm_event(&key(KeyCode::Char('a')));
    assert!(
        commands.is_empty(),
        "expected archive shortcut to open overlay without runtime commands, got {commands:?}"
    );
    assert!(app.sessions.archive_manager_open);

    let commands = app.handle_crossterm_event(&key(KeyCode::Enter));
    assert_eq!(
        commands,
        vec![Command::RestoreSession {
            session_id: archived_id,
        }]
    );
    assert!(!app.sessions.archive_manager_open);
}

#[test]
fn destructive_tui_actions_require_second_keypress_before_command() {
    use agent_core::facade::{
        HookSettingsView, HooksSettingsView, InstalledEntry, PluginComponentInventoryView,
        PluginInstallTarget, PluginSettingsView, SkillInstallSource, SkillInstallTarget,
        SkillSettingsScope, SkillSettingsView,
    };
    use agent_core::{ConfigScope, ProjectId, ProjectSessionVisibility, SessionId, WorkspaceId};
    use agent_tui::app::App;
    use agent_tui::components::{
        Command, CrossPanelEffect, FocusTarget, McpOverlaySnapshot, ModelOverlaySnapshot,
        ModelProfileEntry, PluginOverlaySnapshot, ProjectInfo, SessionInfo, SessionState,
        SkillOverlaySnapshot,
    };
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn session(session_id: &str, archived: bool) -> SessionInfo {
        SessionInfo {
            id: SessionId::from_string(session_id.to_string()),
            title: session_id.to_string(),
            model_profile: "fake".into(),
            state: SessionState::Idle,
            pinned: false,
            archived,
            project_id: None,
            worktree_path: None,
            branch: None,
            visibility: archived.then_some(ProjectSessionVisibility::Archived),
        }
    }

    fn project(project_id: &str) -> ProjectInfo {
        ProjectInfo {
            id: ProjectId::from_string(project_id.to_string()),
            display_name: project_id.to_string(),
            root_path: format!("/tmp/{project_id}"),
            expanded: true,
            git_status: None,
            instruction_summary: None,
        }
    }

    fn installed_skill(skill_id: &str) -> SkillSettingsView {
        SkillSettingsView {
            settings_id: format!("user:{skill_id}"),
            id: skill_id.into(),
            name: skill_id.into(),
            description: format!("{skill_id} settings"),
            version: Some("1.0.0".into()),
            scope: SkillSettingsScope::User,
            path: format!("/tmp/{skill_id}/SKILL.md"),
            enabled: true,
            activation_mode: "manual".into(),
            install_source: SkillInstallSource::Registry,
            update_state: agent_core::facade::SkillUpdateState::UpToDate,
            effective: true,
            shadowed_by: None,
            valid: true,
            validation_error: None,
            editable: true,
            deletable: true,
        }
    }

    fn installed_plugin(settings_id: &str) -> PluginSettingsView {
        PluginSettingsView {
            settings_id: settings_id.into(),
            id: settings_id.replace(':', "-"),
            name: settings_id.into(),
            description: format!("{settings_id} plugin"),
            version: Some("1.2.3".into()),
            scope: ConfigScope::User,
            path: format!("/tmp/{settings_id}"),
            enabled: true,
            install_source: Some("local".into()),
            marketplace: Some("local-market".into()),
            effective: true,
            shadowed_by: None,
            valid: true,
            validation_error: None,
            inventory: PluginComponentInventoryView {
                skill_count: 1,
                skill_names: vec!["review".into()],
                mcp_server_count: 0,
                app_count: 0,
                agent_count: 0,
                hook_count: 0,
            },
            manifest_kind: "kairox".into(),
        }
    }

    fn hook(id: &str) -> HookSettingsView {
        HookSettingsView {
            id: id.into(),
            event: "Stop".into(),
            matcher: Some("*".into()),
            command: "cargo test".into(),
            status_message: Some("Testing".into()),
            timeout_secs: Some(120),
            enabled: true,
            source: ConfigScope::User,
            config_path: Some(format!("/tmp/{id}.toml")),
        }
    }

    let mut app = App::new("fake", WorkspaceId::new());
    let archive_id = SessionId::from_string("ses_active_confirm".into());
    app.state.sessions = vec![SessionInfo {
        id: archive_id.clone(),
        ..session("ses_active_confirm", false)
    }];
    app.state.focus_manager.set(FocusTarget::Sessions);
    app.sync_component_focus();
    assert!(app.sessions.open_action_menu(&[], &app.state.sessions));
    let commands = app.handle_crossterm_event(&key(KeyCode::Char('a')));
    assert!(
        commands.is_empty(),
        "first archive key should only arm confirmation"
    );
    let commands = app.handle_crossterm_event(&key(KeyCode::Char('a')));
    assert!(matches!(
        &commands[..],
        [Command::ArchiveSession { session_id }] if session_id == &archive_id
    ));

    let mut app = App::new("fake", WorkspaceId::new());
    let archived_id = SessionId::from_string("ses_archived_confirm".into());
    app.state.sessions = vec![SessionInfo {
        id: archived_id.clone(),
        ..session("ses_archived_confirm", true)
    }];
    app.state.focus_manager.set(FocusTarget::Sessions);
    app.sync_component_focus();
    app.sessions.open_archive_manager(&app.state.sessions);
    let commands = app.handle_crossterm_event(&key(KeyCode::Char('d')));
    assert!(
        commands.is_empty(),
        "first archive-manager delete key should only arm confirmation"
    );
    let commands = app.handle_crossterm_event(&key(KeyCode::Char('d')));
    assert!(matches!(
        &commands[..],
        [Command::DeleteSession { session_id }] if session_id == &archived_id
    ));

    let mut app = App::new("fake", WorkspaceId::new());
    let project_id = ProjectId::from_string("prj_confirm".into());
    app.state.projects = vec![ProjectInfo {
        id: project_id.clone(),
        ..project("prj_confirm")
    }];
    app.state.focus_manager.set(FocusTarget::Sessions);
    app.sync_component_focus();
    assert!(app
        .sessions
        .open_action_menu(&app.state.projects, &app.state.sessions));
    let commands = app.handle_crossterm_event(&key(KeyCode::Char('d')));
    assert!(
        commands.is_empty(),
        "first project remove key should only arm confirmation"
    );
    let commands = app.handle_crossterm_event(&key(KeyCode::Char('d')));
    assert!(matches!(
        &commands[..],
        [Command::RemoveProject { project_id: id }] if id == &project_id
    ));

    let mut app = App::new("fake", WorkspaceId::new());
    app.dispatch_effects(vec![CrossPanelEffect::ShowMcpOverlay(McpOverlaySnapshot {
        runtime_servers: Vec::new(),
        settings: Vec::new(),
        installed: vec![InstalledEntry {
            server_id: "alpha".into(),
            catalog_id: Some("filesystem".into()),
            source: Some("builtin".into()),
            display_name: "Alpha".into(),
            installed_at: "2026-05-21T00:00:00Z".into(),
            running: true,
        }],
        catalog: Vec::new(),
        sources: Vec::new(),
    })]);
    let _ = app.handle_crossterm_event(&key(KeyCode::Tab));
    let _ = app.handle_crossterm_event(&key(KeyCode::Tab));
    let commands = app.handle_crossterm_event(&key(KeyCode::Char('x')));
    assert!(
        commands.is_empty(),
        "first MCP uninstall key should only arm confirmation"
    );
    let commands = app.handle_crossterm_event(&key(KeyCode::Char('x')));
    assert!(matches!(
        &commands[..],
        [Command::UninstallMcpServer { server_id }] if server_id == "alpha"
    ));

    let mut app = App::new("fake", WorkspaceId::new());
    app.dispatch_effects(vec![CrossPanelEffect::ShowSkillsOverlay(
        SkillOverlaySnapshot {
            discovered: Vec::new(),
            installed: vec![installed_skill("review")],
            catalog: Vec::new(),
            sources: Vec::new(),
            install_target: SkillInstallTarget::User,
        },
    )]);
    let _ = app.handle_crossterm_event(&key(KeyCode::Tab));
    let commands = app.handle_crossterm_event(&key(KeyCode::Char('x')));
    assert!(
        commands.is_empty(),
        "first skill delete key should only arm confirmation"
    );
    let commands = app.handle_crossterm_event(&key(KeyCode::Char('x')));
    assert!(matches!(
        &commands[..],
        [Command::DeleteSkillSettings { skill_id }] if skill_id == "review"
    ));

    let mut app = App::new("fake", WorkspaceId::new());
    app.dispatch_effects(vec![CrossPanelEffect::ShowPluginsOverlay(
        PluginOverlaySnapshot {
            plugins: vec![installed_plugin("user:alpha")],
            catalog: Vec::new(),
            sources: Vec::new(),
            install_target: PluginInstallTarget::User,
        },
    )]);
    let commands = app.handle_crossterm_event(&key(KeyCode::Char('x')));
    assert!(
        commands.is_empty(),
        "first plugin delete key should only arm confirmation"
    );
    let commands = app.handle_crossterm_event(&key(KeyCode::Char('x')));
    assert!(matches!(
        &commands[..],
        [Command::DeletePluginSettings { settings_id }] if settings_id == "user:alpha"
    ));

    let mut app = App::new("fake", WorkspaceId::new());
    app.dispatch_effects(vec![CrossPanelEffect::ShowModelOverlay(
        ModelOverlaySnapshot {
            profiles: vec![ModelProfileEntry {
                alias: "slow".into(),
                provider_display: "fake".into(),
                model_display: "slow-model".into(),
                context_window: Some(128_000),
                output_limit: Some(4096),
                temperature: None,
                top_p: None,
                top_k: None,
                max_tokens: None,
                base_url: None,
                api_key_env: None,
                supports_reasoning: false,
                enabled: true,
                writable: true,
                source: "test".into(),
                has_api_key: true,
            }],
            current_alias: Some("fast".into()),
            current_effort: None,
        },
    )]);
    let commands = app.handle_crossterm_event(&key(KeyCode::Char('x')));
    assert!(
        commands.is_empty(),
        "first model profile delete key should only arm confirmation"
    );
    let commands = app.handle_crossterm_event(&key(KeyCode::Char('x')));
    assert!(matches!(
        &commands[..],
        [Command::DeleteProfileSettings { alias }] if alias == "slow"
    ));

    let mut app = App::new("fake", WorkspaceId::new());
    app.dispatch_effects(vec![CrossPanelEffect::ShowHooksOverlay(
        HooksSettingsView {
            user: vec![hook("user-verify")],
            project: Vec::new(),
            templates: Vec::new(),
            user_config_path: "/tmp/kairox-user.toml".into(),
            project_config_path: Some("/tmp/kairox-project.toml".into()),
        },
    )]);
    let commands = app.handle_crossterm_event(&key(KeyCode::Char('x')));
    assert!(
        commands.is_empty(),
        "first hook delete key should only arm confirmation"
    );
    let commands = app.handle_crossterm_event(&key(KeyCode::Char('x')));
    assert!(matches!(
        &commands[..],
        [Command::DeleteHookSettings { scope, event, id }]
            if *scope == ConfigScope::User && event == "Stop" && id == "user-verify"
    ));
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
fn colon_hooks_input_dispatches_open_hooks_overlay_command() {
    use agent_tui::components::Command;

    let commands = chat_commands_for_input(":hooks");

    assert!(
        commands
            .iter()
            .any(|command| matches!(command, Command::OpenHooksOverlay)),
        "expected Command::OpenHooksOverlay; got {commands:?}"
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
        commands.iter().any(|command| matches!(
            command,
            Command::ListSkillCatalog {
                keyword: Some(keyword),
                sources: None
            } if keyword == "review"
        )),
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
        commands.iter().any(|command| matches!(
            command,
            Command::ListSkillCatalog {
                keyword: None,
                sources: None
            }
        )),
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
    let mut app = App::new("fake", workspace_id.clone());
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

    let status_messages: Vec<&str> = app
        .state
        .status_log
        .iter()
        .map(|entry| entry.message.as_str())
        .collect();
    assert!(
        status_messages
            .iter()
            .any(|message| message.contains("test-driven-rust")),
        "expected a skill list/detail status; got {status_messages:?}"
    );
    assert!(
        status_messages
            .iter()
            .any(|message| message.contains("activated test-driven-rust")),
        "expected activation confirmation status; got {status_messages:?}"
    );
    assert!(
        status_messages
            .iter()
            .any(|message| message.contains("deactivated test-driven-rust")),
        "expected deactivation confirmation status; got {status_messages:?}"
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
async fn config_source_model_overlay_uses_selected_project_filter() {
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::app_state::SettingsConfigSource;
    use agent_tui::components::Command;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let mut app = App::new("fake", WorkspaceId::new());
    app.state
        .set_settings_config_source(SettingsConfigSource::Project);

    agent_tui::app::dispatch_commands(&runtime, &mut app, vec![Command::OpenModelOverlay]).await;

    let calls = runtime.calls();
    assert!(
        calls
            .iter()
            .any(|call| call == "list_profile_settings:Some(\"project\")"),
        "expected selected project source filter for model overlay, got {calls:?}"
    );
}

#[tokio::test]
async fn config_source_mcp_overlay_uses_selected_project_filter() {
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::app_state::SettingsConfigSource;
    use agent_tui::components::Command;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let mut app = App::new("fake", WorkspaceId::new());
    app.state
        .set_settings_config_source(SettingsConfigSource::Project);

    agent_tui::app::dispatch_commands(&runtime, &mut app, vec![Command::OpenMcpOverlay]).await;

    let calls = runtime.calls();
    assert!(
        calls
            .iter()
            .any(|call| call == "list_mcp_server_settings:Some(\"project\")"),
        "expected selected project source filter for MCP overlay, got {calls:?}"
    );
}

#[tokio::test]
async fn config_source_model_save_uses_selected_project_config_path() {
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::app_state::SettingsConfigSource;
    use agent_tui::components::Command;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let project_root = unique_temp_dir("kairox-tui-model-save-source");
    let mut app = App::new("fake", WorkspaceId::new());
    let project = test_project("prj_model_save", &project_root);
    let project_id = project.id.clone();
    app.state.projects = vec![project];
    app.state
        .set_settings_config_source(SettingsConfigSource::Project);
    app.state.select_settings_project(project_id);

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![Command::SaveProfileSettings {
            input: agent_core::facade::ProfileSettingsInput {
                alias: "project-fast".into(),
                provider: "fake".into(),
                model_id: "project-model".into(),
                enabled: true,
                context_window: Some(64000),
                output_limit: None,
                temperature: Some(0.1),
                top_p: None,
                top_k: None,
                max_tokens: None,
                base_url: None,
                api_key_env: None,
            },
        }],
    )
    .await;

    let config_path = project_root.join(".kairox").join("config.toml");
    let raw = std::fs::read_to_string(&config_path)
        .expect("selected project config should receive model profile");
    assert!(raw.contains("[profiles.project-fast]"));
    assert!(raw.contains("model_id = \"project-model\""));
    assert!(
        !runtime
            .calls()
            .iter()
            .any(|call| call.starts_with("upsert_profile_settings")),
        "project save should not write through the user profile facade"
    );

    std::fs::remove_dir_all(project_root).expect("temp project should be removed");
}

#[tokio::test]
async fn config_source_mcp_save_uses_selected_project_config_path() {
    use agent_core::facade::McpServerSettingsTransport;
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::app_state::SettingsConfigSource;
    use agent_tui::components::Command;
    use std::collections::BTreeMap;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let project_root = unique_temp_dir("kairox-tui-mcp-save-source");
    let mut app = App::new("fake", WorkspaceId::new());
    let project = test_project("prj_mcp_save", &project_root);
    let project_id = project.id.clone();
    app.state.projects = vec![project];
    app.state
        .set_settings_config_source(SettingsConfigSource::Project);
    app.state.select_settings_project(project_id);

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![Command::SaveMcpServerSettings {
            input: agent_core::facade::McpServerSettingsInput {
                name: "project-fs".into(),
                transport: McpServerSettingsTransport::Stdio {
                    command: "kairox-mcp".into(),
                    args: vec!["serve".into()],
                    env: BTreeMap::new(),
                },
                enabled: true,
                description: Some("Project MCP".into()),
            },
        }],
    )
    .await;

    let config_path = project_root.join(".kairox").join("config.toml");
    let raw = std::fs::read_to_string(&config_path)
        .expect("selected project config should receive MCP server");
    assert!(raw.contains("[mcp_servers.project-fs]"));
    assert!(raw.contains("command = \"kairox-mcp\""));
    assert!(
        !runtime
            .calls()
            .iter()
            .any(|call| call.starts_with("upsert_mcp_server_settings")),
        "project save should not write through the user MCP facade"
    );

    std::fs::remove_dir_all(project_root).expect("temp project should be removed");
}

#[tokio::test]
async fn config_source_instructions_overlay_uses_selected_project_config_path() {
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::app_state::SettingsConfigSource;
    use agent_tui::components::Command;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let project_root = unique_temp_dir("kairox-tui-instructions-source");
    let config_dir = project_root.join(".kairox");
    std::fs::create_dir_all(&config_dir).expect("project config dir should be created");
    std::fs::write(
        config_dir.join("config.toml"),
        "instructions = \"Use selected project instructions.\"\n",
    )
    .expect("project config should be written");

    let mut app = App::new("fake", WorkspaceId::new());
    let project = test_project("prj_selected_instructions", &project_root);
    let project_id = project.id.clone();
    app.state.projects = vec![project];
    app.state
        .set_settings_config_source(SettingsConfigSource::Project);
    app.state.select_settings_project(project_id);

    agent_tui::app::dispatch_commands(&runtime, &mut app, vec![Command::OpenInstructionsOverlay])
        .await;

    assert_eq!(
        app.instructions_overlay.project_text(),
        "Use selected project instructions."
    );

    std::fs::remove_dir_all(project_root).expect("temp project should be removed");
}

#[tokio::test]
async fn config_source_hooks_overlay_uses_selected_project_config_path() {
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::app_state::SettingsConfigSource;
    use agent_tui::components::Command;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let project_root = unique_temp_dir("kairox-tui-hooks-source");
    let config_dir = project_root.join(".kairox");
    let config_path = config_dir.join("config.toml");
    std::fs::create_dir_all(&config_dir).expect("project config dir should be created");
    std::fs::write(
        &config_path,
        "[features]\nhooks = true\n\n[hooks.Stop.verify]\ncommand = \"cargo test\"\nenabled = true\n",
    )
    .expect("project config should be written");

    let mut app = App::new("fake", WorkspaceId::new());
    let project = test_project("prj_selected_hooks", &project_root);
    let project_id = project.id.clone();
    app.state.projects = vec![project];
    app.state
        .set_settings_config_source(SettingsConfigSource::Project);
    app.state.select_settings_project(project_id);

    agent_tui::app::dispatch_commands(&runtime, &mut app, vec![Command::OpenHooksOverlay]).await;

    let hooks = app.hooks_overlay.project_hooks();
    assert_eq!(hooks.len(), 1);
    assert_eq!(hooks[0].id, "verify");
    assert_eq!(
        hooks[0].config_path.as_deref(),
        Some(config_path.display().to_string().as_str())
    );

    std::fs::remove_dir_all(project_root).expect("temp project should be removed");
}

#[tokio::test]
async fn tui_mcp_marketplace_commands_call_facade_and_refresh_overlay() {
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::components::Command;
    use std::collections::BTreeMap;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let mut app = App::new("fake", WorkspaceId::new());

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
async fn tui_mcp_install_command_forwards_env_overrides() {
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::components::Command;
    use std::collections::BTreeMap;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let mut app = App::new("fake", WorkspaceId::new());
    let mut env_overrides = BTreeMap::new();
    env_overrides.insert("Authorization".to_string(), "Bearer test-token".to_string());
    env_overrides.insert("GITHUB_ORG".to_string(), "kairox-dev".to_string());

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![Command::InstallMcpServer {
            request: agent_core::facade::InstallRequest {
                catalog_id: "github".into(),
                source: "registry".into(),
                server_id_override: None,
                env_overrides: env_overrides.clone(),
                trust_grant: false,
                auto_start: true,
            },
        }],
    )
    .await;

    let request = runtime
        .last_install_request()
        .expect("install request should reach facade");
    assert_eq!(request.catalog_id, "github");
    assert_eq!(request.source, "registry");
    assert_eq!(request.env_overrides, env_overrides);
}

#[tokio::test]
async fn mcp_overlay_install_outcome_persists_after_command_refresh() {
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::components::{Command, Component};
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
    use std::collections::BTreeMap;

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn rendered_mcp_overlay(app: &App) -> String {
        let backend = ratatui::backend::TestBackend::new(120, 30);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| app.mcp_overlay.render(f.area(), f))
            .expect("render");
        terminal.backend().to_string()
    }

    let cases = [
        (
            FakeInstallResult::Outcome(agent_core::facade::InstallOutcomeView {
                kind: "installed".into(),
                server_id: Some("filesystem".into()),
                started: Some(true),
                missing_runtimes: Vec::new(),
                missing_env_keys: Vec::new(),
            }),
            "install status: installed as filesystem (started)",
        ),
        (
            FakeInstallResult::Outcome(agent_core::facade::InstallOutcomeView {
                kind: "already_installed".into(),
                server_id: Some("filesystem".into()),
                started: None,
                missing_runtimes: Vec::new(),
                missing_env_keys: Vec::new(),
            }),
            "install status: already installed as filesystem",
        ),
        (
            FakeInstallResult::Outcome(agent_core::facade::InstallOutcomeView {
                kind: "invalid_env".into(),
                server_id: None,
                started: None,
                missing_runtimes: Vec::new(),
                missing_env_keys: vec!["Authorization".into()],
            }),
            "install status: missing env Authorization",
        ),
        (
            FakeInstallResult::Outcome(agent_core::facade::InstallOutcomeView {
                kind: "runtime_missing".into(),
                server_id: None,
                started: None,
                missing_runtimes: vec!["node >=18".into()],
                missing_env_keys: Vec::new(),
            }),
            "install status: missing runtime node >=18",
        ),
        (
            FakeInstallResult::Error("write failed".into()),
            "install status: failed invalid state: write failed",
        ),
    ];

    for (install_result, expected) in cases {
        let runtime = Arc::new(TuiMcpFakeFacade::with_install_result(install_result));
        let mut app = App::new("fake", WorkspaceId::new());

        agent_tui::app::dispatch_commands(&runtime, &mut app, vec![Command::OpenMcpOverlay]).await;
        for _ in 0..3 {
            let ctx = app
                .state
                .event_context(&app.workspace_id, &app.current_session_id);
            let _ = app.mcp_overlay.handle_event(&ctx, &key(KeyCode::Tab));
        }

        agent_tui::app::dispatch_commands(
            &runtime,
            &mut app,
            vec![Command::InstallMcpServer {
                request: agent_core::facade::InstallRequest {
                    catalog_id: "filesystem".into(),
                    source: "builtin".into(),
                    server_id_override: None,
                    env_overrides: BTreeMap::new(),
                    trust_grant: false,
                    auto_start: true,
                },
            }],
        )
        .await;

        let rendered = rendered_mcp_overlay(&app);
        assert!(
            rendered.contains(expected),
            "expected {expected:?}, got {rendered}"
        );
    }
}

#[tokio::test]
async fn tui_skill_source_commands_call_facade_and_refresh_overlay() {
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::components::Command;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let mut app = App::new("fake", WorkspaceId::new());

    agent_tui::app::dispatch_commands(&runtime, &mut app, vec![Command::OpenSkillsOverlay]).await;

    assert!(app.skills_overlay.is_visible());
    let calls = runtime.calls();
    assert!(
        calls.iter().any(|call| call == "list_skill_settings"),
        "expected settings list call, got {calls:?}"
    );
    assert!(
        calls
            .iter()
            .any(|call| call.starts_with("list_skill_catalog")),
        "expected skill catalog list call, got {calls:?}"
    );
    assert!(
        calls.iter().any(|call| call == "list_skill_sources"),
        "expected skill sources call, got {calls:?}"
    );

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![
            Command::AddSkillSource {
                config: agent_core::facade::SkillSourceView {
                    id: "corp".into(),
                    display_name: "Corporate Skills".into(),
                    kind: "skillhub".into(),
                    url: "https://skills.example.com".into(),
                    search_template: "/api/skills?keyword={{query}}".into(),
                    download_template: "/api/v1/download?slug={{slug}}".into(),
                    list_template: None,
                    detail_template: None,
                    field_mapping: agent_core::facade::SkillFieldMappingView::default(),
                    enabled: true,
                    priority: 100,
                    cache_ttl_seconds: 900,
                    last_error: None,
                },
            },
            Command::RemoveSkillSource {
                source_id: "corp".into(),
            },
            Command::SetSkillSourceEnabled {
                source_id: "skillhub".into(),
                enabled: false,
            },
        ],
    )
    .await;

    let calls = runtime.calls();
    for expected in [
        "add_skill_source:corp",
        "remove_skill_source:corp",
        "set_skill_source_enabled:skillhub:false",
    ] {
        assert!(
            calls.iter().any(|call| call == expected),
            "expected call {expected}, got {calls:?}"
        );
    }
    assert!(
        calls
            .iter()
            .filter(|call| call.as_str() == "list_skill_sources")
            .count()
            >= 4,
        "expected source mutations to refresh overlay, got {calls:?}"
    );
}

#[tokio::test]
async fn tui_skill_catalog_overlay_queries_include_keyword_and_sources() {
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::components::Command;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let mut app = App::new("fake", WorkspaceId::new());

    agent_tui::app::dispatch_commands(&runtime, &mut app, vec![Command::OpenSkillsOverlay]).await;
    assert!(app.skills_overlay.is_visible());

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![Command::ListSkillCatalog {
            keyword: Some("review".into()),
            sources: Some(vec!["skillhub".into()]),
        }],
    )
    .await;

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![Command::RefreshSkillCatalog {
            keyword: Some("docs".into()),
            sources: Some(vec!["skillhub".into()]),
        }],
    )
    .await;

    let calls = runtime.calls();
    assert!(
        calls.iter().any(
            |call| call
                == "list_skill_catalog:Some(\"review\"):Some([\"skillhub\"]):Some(50)"
        ),
        "expected filtered overlay list call, got {calls:?}"
    );
    assert!(
        calls.iter().any(|call| call == "refresh_skill_catalog"),
        "expected catalog refresh call, got {calls:?}"
    );
    assert!(
        calls
            .iter()
            .any(|call| call == "list_skill_catalog:Some(\"docs\"):Some([\"skillhub\"]):Some(50)"),
        "expected refresh to rerun filtered overlay query, got {calls:?}"
    );
}

#[tokio::test]
async fn tui_model_profile_settings_commands_call_facade_and_report_results() {
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::components::Command;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let mut app = App::new("fake", WorkspaceId::new());

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![
            Command::SaveProfileSettings {
                input: agent_core::facade::ProfileSettingsInput {
                    alias: "local".into(),
                    provider: "fake".into(),
                    model_id: "local-model".into(),
                    enabled: true,
                    context_window: Some(128000),
                    output_limit: Some(8192),
                    temperature: Some(0.2),
                    top_p: Some(0.9),
                    top_k: Some(40),
                    max_tokens: Some(4096),
                    base_url: Some("http://localhost:11434/v1".into()),
                    api_key_env: Some("LOCAL_LLM_API_KEY".into()),
                },
            },
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
        "upsert_profile_settings:local:fake:local-model",
        "set_profile_enabled:fast:false",
        "move_profile_in_order:fast:1",
        "list_profile_settings:Some(\"user\")",
        "delete_profile_settings:fast",
    ] {
        assert!(
            calls.iter().any(|call| call == expected),
            "expected call {expected}, got {calls:?}"
        );
    }

    assert!(
        app.state
            .status_log
            .iter()
            .map(|entry| entry.message.as_str())
            .any(|message| message.contains("model profile fast connectivity ok")),
        "expected model test result in status log; got {:?}",
        app.state.status_log
    );
}

#[tokio::test]
async fn app_logic_command_status_success_does_not_pollute_chat_transcript() {
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::components::Command;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let mut app = App::new("fake", WorkspaceId::new());
    let initial_chat_count = app.state.current_session.messages.len();

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![Command::SetProfileEnabled {
            alias: "fast".into(),
            enabled: false,
        }],
    )
    .await;

    assert_eq!(app.state.current_session.messages.len(), initial_chat_count);
    assert_eq!(
        app.state
            .latest_status_message()
            .map(|entry| entry.message.as_str()),
        Some("disabled model profile fast")
    );
}

#[tokio::test]
async fn app_logic_command_status_failure_does_not_pollute_chat_transcript() {
    use agent_core::{ConfigScope, WorkspaceId};
    use agent_tui::app::App;
    use agent_tui::components::Command;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let mut app = App::new("fake", WorkspaceId::new());
    let initial_chat_count = app.state.current_session.messages.len();

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![Command::DeleteHookSettings {
            scope: ConfigScope::Builtin,
            event: "PreToolUse".into(),
            id: "readonly".into(),
        }],
    )
    .await;

    assert_eq!(app.state.current_session.messages.len(), initial_chat_count);
    let latest = app
        .state
        .latest_status_message()
        .map(|entry| entry.message.as_str())
        .unwrap_or_default();
    assert!(
        latest.contains("[hooks delete error:"),
        "expected hook delete error in status log, got {latest:?}"
    );
}

#[tokio::test]
async fn tui_plugin_overlay_refresh_passes_catalog_filters_to_facade() {
    use agent_core::facade::{PluginInstallTarget, PluginMarketplaceSourceView};
    use agent_core::projection::SessionProjection;
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::components::{
        Command, Component, EventContext, FocusTarget, PluginOverlaySnapshot,
    };
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let workspace_id = WorkspaceId::new();
    let current_session_id = None;
    let projection = SessionProjection::default();
    let ctx = EventContext {
        focus: FocusTarget::PluginOverlay,
        current_session: &projection,
        projects: &[],
        sessions: &[],
        model_profile: "fake",
        sidebar_left_visible: true,
        sidebar_right_visible: true,
        workspace_id: &workspace_id,
        current_session_id: &current_session_id,
    };
    let mut app = App::new("fake", workspace_id.clone());
    app.plugin_overlay.show(PluginOverlaySnapshot {
        plugins: Vec::new(),
        catalog: Vec::new(),
        sources: vec![PluginMarketplaceSourceView {
            id: "local-market".into(),
            display_name: "Local market".into(),
            source: "/tmp/local-market".into(),
            enabled: true,
            builtin: false,
        }],
        install_target: PluginInstallTarget::User,
    });

    let _ = app.plugin_overlay.handle_event(&ctx, &key(KeyCode::Tab));
    let _ = app
        .plugin_overlay
        .handle_event(&ctx, &key(KeyCode::Char('s')));
    let _ = app
        .plugin_overlay
        .handle_event(&ctx, &key(KeyCode::Char('/')));
    for ch in "delta".chars() {
        let _ = app
            .plugin_overlay
            .handle_event(&ctx, &key(KeyCode::Char(ch)));
    }
    let (_, commands) = app.plugin_overlay.handle_event(&ctx, &key(KeyCode::Enter));
    assert!(matches!(&commands[..], [Command::OpenPluginsOverlay]));

    agent_tui::app::dispatch_commands(&runtime, &mut app, commands).await;

    let calls = runtime.calls();
    assert!(
        calls
            .iter()
            .any(|call| call == "list_plugin_catalog:Some(\"local-market\"):Some(\"delta\")"),
        "expected filtered plugin catalog call, got {calls:?}"
    );
}

#[tokio::test]
async fn tui_agent_settings_commands_call_facade_and_refresh_overlay() {
    use agent_core::facade::{AgentSettingsInput, AgentSettingsScope};
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::components::Command;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let mut app = App::new("fake", WorkspaceId::new());

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
    let mut app = App::new("fake", workspace_id.clone());
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
            .status_log
            .iter()
            .any(|entry| entry.message.contains("project instructions")),
        "instruction command should surface summary content in the status log"
    );
}

#[tokio::test]
async fn settings_utility_commands_call_facade_open_dir_methods() {
    use agent_core::WorkspaceId;
    use agent_tui::app::App;
    use agent_tui::components::Command;

    let runtime = Arc::new(TuiMcpFakeFacade::default());
    let mut app = App::new("fake", WorkspaceId::new());

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![Command::OpenConfigDir, Command::OpenSkillsDir],
    )
    .await;

    let calls = runtime.calls();
    assert!(
        calls.contains(&"open_config_dir".to_string()),
        "expected open_config_dir facade call; got {calls:?}"
    );
    assert!(
        calls.contains(&"open_skills_dir".to_string()),
        "expected open_skills_dir facade call; got {calls:?}"
    );
}

#[tokio::test]
async fn tui_skill_catalog_settings_commands_call_facade_and_render_visible_messages() {
    use agent_core::facade::{InstallRemoteSkillRequest, SkillInstallTarget, SkillUpdateState};
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
    let mut app = App::new("fake", workspace_id);

    agent_tui::app::dispatch_commands(
        &runtime,
        &mut app,
        vec![
            Command::ListSkillCatalog {
                keyword: Some("review".into()),
                sources: None,
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

    let status_messages: Vec<&str> = app
        .state
        .status_log
        .iter()
        .map(|entry| entry.message.as_str())
        .collect();
    assert!(
        status_messages
            .iter()
            .any(|message| message.contains("No catalog skills found for review")),
        "expected catalog empty-state status; got {status_messages:?}"
    );
    assert!(
        status_messages
            .iter()
            .any(|message| message.contains("installed skill review")),
        "expected install confirmation status; got {status_messages:?}"
    );
    assert!(
        status_messages
            .iter()
            .any(|message| message.contains("updated skill review")),
        "expected update confirmation status; got {status_messages:?}"
    );
    assert!(
        status_messages
            .iter()
            .any(|message| message.contains("deleted skill review")),
        "expected delete confirmation status; got {status_messages:?}"
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
