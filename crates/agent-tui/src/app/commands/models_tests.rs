use super::*;
use agent_core::facade::{
    AgentStatusInfo, McpFacade, ProfileSettingsView, SessionFacade, SessionMeta,
    StartSessionRequest, TaskGraphSnapshot, WorkspaceInfo,
};
use agent_core::{SessionId, TaskId, WorkspaceId};
use async_trait::async_trait;
use futures::stream::BoxStream;

struct ModelFacade {
    profiles: Result<Vec<ProfileSettingsView>, String>,
}

impl ModelFacade {
    fn with_profiles(profiles: Vec<ProfileSettingsView>) -> Self {
        Self {
            profiles: Ok(profiles),
        }
    }

    fn with_error(message: &str) -> Self {
        Self {
            profiles: Err(message.to_string()),
        }
    }
}

#[async_trait]
impl SessionFacade for ModelFacade {
    async fn open_workspace(&self, path: String) -> agent_core::Result<WorkspaceInfo> {
        Ok(WorkspaceInfo {
            workspace_id: WorkspaceId::new(),
            path,
        })
    }

    async fn start_session(&self, request: StartSessionRequest) -> agent_core::Result<SessionId> {
        let _ = request;
        Ok(SessionId::new())
    }

    async fn send_message(
        &self,
        request: agent_core::facade::SendMessageRequest,
    ) -> agent_core::Result<()> {
        let _ = request;
        Ok(())
    }

    async fn decide_permission(
        &self,
        decision: agent_core::facade::PermissionDecision,
    ) -> agent_core::Result<()> {
        let _ = decision;
        Ok(())
    }

    async fn cancel_session(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
    ) -> agent_core::Result<()> {
        let _ = (workspace_id, session_id);
        Ok(())
    }

    async fn get_session_projection(
        &self,
        session_id: SessionId,
    ) -> agent_core::Result<agent_core::projection::SessionProjection> {
        let _ = session_id;
        Ok(agent_core::projection::SessionProjection::default())
    }

    async fn get_trace(
        &self,
        session_id: SessionId,
    ) -> agent_core::Result<Vec<agent_core::facade::TraceEntry>> {
        let _ = session_id;
        Ok(Vec::new())
    }

    fn subscribe_session(
        &self,
        session_id: SessionId,
    ) -> BoxStream<'static, agent_core::DomainEvent> {
        let _ = session_id;
        Box::pin(futures::stream::empty())
    }

    fn subscribe_all(&self) -> BoxStream<'static, agent_core::DomainEvent> {
        Box::pin(futures::stream::empty())
    }

    async fn list_workspaces(&self) -> agent_core::Result<Vec<WorkspaceInfo>> {
        Ok(Vec::new())
    }

    async fn list_sessions(
        &self,
        workspace_id: &WorkspaceId,
    ) -> agent_core::Result<Vec<SessionMeta>> {
        let _ = workspace_id;
        Ok(Vec::new())
    }

    async fn rename_session(
        &self,
        session_id: &SessionId,
        title: String,
    ) -> agent_core::Result<()> {
        let _ = (session_id, title);
        Ok(())
    }

    async fn soft_delete_session(&self, session_id: &SessionId) -> agent_core::Result<()> {
        let _ = session_id;
        Ok(())
    }

    async fn cleanup_expired_sessions(
        &self,
        older_than: std::time::Duration,
    ) -> agent_core::Result<usize> {
        let _ = older_than;
        Ok(0)
    }

    async fn get_task_graph(&self, session_id: SessionId) -> agent_core::Result<TaskGraphSnapshot> {
        let _ = session_id;
        Ok(TaskGraphSnapshot::default())
    }

    async fn retry_task(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
        task_id: TaskId,
    ) -> agent_core::Result<()> {
        let _ = (workspace_id, session_id, task_id);
        Ok(())
    }

    async fn cancel_task(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
        task_id: TaskId,
    ) -> agent_core::Result<()> {
        let _ = (workspace_id, session_id, task_id);
        Ok(())
    }

    async fn get_agent_status(
        &self,
        session_id: SessionId,
    ) -> agent_core::Result<Vec<AgentStatusInfo>> {
        let _ = session_id;
        Ok(Vec::new())
    }
}

#[async_trait]
impl McpFacade for ModelFacade {
    async fn list_profile_settings(
        &self,
        source_filter: Option<String>,
    ) -> agent_core::Result<Vec<ProfileSettingsView>> {
        let _ = source_filter;
        self.profiles
            .clone()
            .map_err(agent_core::CoreError::InvalidState)
    }
}

#[async_trait]
impl agent_core::facade::SkillsFacade for ModelFacade {}

#[async_trait]
impl agent_core::facade::ProjectFacade for ModelFacade {}

#[async_trait]
impl agent_core::facade::AgentsFacade for ModelFacade {}

#[async_trait]
impl agent_core::facade::PluginsFacade for ModelFacade {}

#[async_trait]
impl agent_core::facade::AutonomousFacade for ModelFacade {}

impl AppFacade for ModelFacade {}

fn profile(alias: &str, enabled: bool) -> ProfileSettingsView {
    ProfileSettingsView {
        alias: alias.to_string(),
        provider: "openai-compatible".to_string(),
        model_id: format!("{alias}-model"),
        enabled,
        context_window: Some(128_000),
        output_limit: Some(4096),
        temperature: Some(0.2),
        top_p: Some(0.9),
        top_k: Some(40),
        max_tokens: Some(2048),
        base_url: Some(format!("https://{alias}.example.test/v1")),
        api_key: Some("redacted".to_string()),
        api_key_env: Some(format!("{}_API_KEY", alias.to_uppercase())),
        client_identity: Some(format!("{alias}-client")),
        has_api_key: true,
        writable: alias != "builtin",
        config_path: Some(format!("/tmp/{alias}.toml")),
        source: if alias == "builtin" {
            "builtin".to_string()
        } else {
            "user".to_string()
        },
    }
}

fn app() -> App {
    App::new("default", WorkspaceId::new())
}

#[test]
fn model_profile_entry_from_settings_maps_display_and_metadata_fields() {
    let entry = model_profile_entry_from_settings(profile("alpha", true));

    assert_eq!(entry.alias, "alpha");
    assert_eq!(entry.provider_display, "openai-compatible");
    assert_eq!(entry.model_display, "alpha-model");
    assert_eq!(entry.context_window, Some(128_000));
    assert_eq!(entry.output_limit, Some(4096));
    assert_eq!(entry.temperature, Some(0.2));
    assert_eq!(entry.top_p, Some(0.9));
    assert_eq!(entry.top_k, Some(40));
    assert_eq!(entry.max_tokens, Some(2048));
    assert_eq!(
        entry.base_url.as_deref(),
        Some("https://alpha.example.test/v1")
    );
    assert_eq!(entry.api_key_env.as_deref(), Some("ALPHA_API_KEY"));
    assert_eq!(entry.client_identity.as_deref(), Some("alpha-client"));
    assert!(!entry.supports_reasoning);
    assert!(entry.enabled);
    assert!(entry.writable);
    assert_eq!(entry.source, "user");
    assert!(entry.has_api_key);
}

#[tokio::test]
async fn command_palette_model_profiles_returns_enabled_profiles_only() {
    let runtime = std::sync::Arc::new(ModelFacade::with_profiles(vec![
        profile("enabled", true),
        profile("disabled", false),
        profile("builtin", true),
    ]));
    let mut app = app();

    let entries = command_palette_model_profiles(&runtime, &mut app).await;

    let aliases = entries
        .iter()
        .map(|entry| entry.alias.as_str())
        .collect::<Vec<_>>();
    assert_eq!(aliases, vec!["enabled", "builtin"]);
    assert!(entries.iter().all(|entry| entry.enabled));
}

#[tokio::test]
async fn command_palette_model_profiles_pushes_status_on_settings_error() {
    let runtime = std::sync::Arc::new(ModelFacade::with_error("profile load failed"));
    let mut app = app();

    let entries = command_palette_model_profiles(&runtime, &mut app).await;

    assert!(entries.is_empty());
    assert_eq!(
        app.state
            .latest_status_message()
            .map(|entry| entry.message.as_str()),
        Some("[model settings error: invalid state: profile load failed]")
    );
}
