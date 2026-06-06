//! Shared helpers and fakes for `app_logic_*` integration tests.
//!
//! Included via `mod app_logic_common;` in each `tests/app_logic_*.rs`
//! file. Not a test binary itself.

#![allow(dead_code)]

use agent_core::{AppFacade, AutonomousFacade};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;

/// Helper: create a runtime with FakeModelClient.
pub async fn make_runtime() -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec!["Hello from TUI test!".into()]);
    LocalRuntime::new(store, model)
}

#[derive(Default)]
pub struct TuiMcpFakeFacade {
    calls: std::sync::Mutex<Vec<String>>,
    last_install_request: std::sync::Mutex<Option<agent_core::facade::InstallRequest>>,
    install_result: std::sync::Mutex<Option<FakeInstallResult>>,
}

#[derive(Clone)]
pub enum FakeInstallResult {
    Outcome(agent_core::facade::InstallOutcomeView),
    Error(String),
}

impl TuiMcpFakeFacade {
    pub fn with_install_result(install_result: FakeInstallResult) -> Self {
        Self {
            install_result: std::sync::Mutex::new(Some(install_result)),
            ..Self::default()
        }
    }

    fn record(&self, call: impl Into<String>) {
        self.calls.lock().expect("calls lock").push(call.into());
    }

    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("calls lock").clone()
    }

    pub fn last_install_request(&self) -> Option<agent_core::facade::InstallRequest> {
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

pub fn agent_settings_view(
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
        reasoning_effort: Some("medium".into()),
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

pub fn unique_temp_dir(prefix: &str) -> std::path::PathBuf {
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time should be after epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{}-{nonce}", std::process::id()))
}

pub fn test_project(
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

pub fn write_test_skill(root: &std::path::Path, name: &str, description: &str, body: &str) {
    let skill_directory = root.join(name);
    std::fs::create_dir_all(&skill_directory).expect("skill directory should be created");
    std::fs::write(
        skill_directory.join("SKILL.md"),
        format!("---\nname: {name}\ndescription: {description}\n---\n{body}"),
    )
    .expect("skill file should be written");
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
            diagnostic_summary: String::new(),
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
            client_identity: None,
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
            client_identity: input.client_identity,
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

#[async_trait::async_trait]
impl AutonomousFacade for TuiMcpFakeFacade {}
