use super::*;
use agent_core::facade::{
    ActiveSkillView, AgentStatusInfo, McpFacade, SessionFacade, SessionMeta, SkillCatalogEntry,
    SkillCatalogQuery, SkillFieldMappingView, SkillSettingsView, SkillSourceView, SkillView,
    StartSessionRequest, TaskGraphSnapshot, WorkspaceInfo,
};
use agent_core::{SessionId, TaskId, WorkspaceId};
use async_trait::async_trait;
use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use futures::stream::BoxStream;
use std::sync::Mutex;

struct SkillFacade {
    skills: Result<Vec<SkillView>, String>,
    active_skills: Vec<ActiveSkillView>,
    catalog_queries: Mutex<Vec<SkillCatalogQuery>>,
}

impl SkillFacade {
    fn with_skills(skills: Vec<SkillView>) -> Self {
        Self {
            skills: Ok(skills),
            active_skills: Vec::new(),
            catalog_queries: Mutex::new(Vec::new()),
        }
    }

    fn with_skill_error(message: &str) -> Self {
        Self {
            skills: Err(message.to_string()),
            active_skills: Vec::new(),
            catalog_queries: Mutex::new(Vec::new()),
        }
    }

    fn with_active_skills(mut self, active_skills: Vec<ActiveSkillView>) -> Self {
        self.active_skills = active_skills;
        self
    }

    fn catalog_queries(&self) -> Vec<SkillCatalogQuery> {
        self.catalog_queries.lock().unwrap().clone()
    }
}

#[async_trait]
impl SessionFacade for SkillFacade {
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
impl agent_core::facade::SkillsFacade for SkillFacade {
    async fn list_skills(&self) -> agent_core::Result<Vec<SkillView>> {
        self.skills
            .clone()
            .map_err(agent_core::CoreError::InvalidState)
    }

    async fn list_active_skills(
        &self,
        session_id: SessionId,
    ) -> agent_core::Result<Vec<ActiveSkillView>> {
        let _ = session_id;
        Ok(self.active_skills.clone())
    }

    async fn list_skill_catalog(
        &self,
        query: SkillCatalogQuery,
    ) -> agent_core::Result<Vec<SkillCatalogEntry>> {
        self.catalog_queries.lock().unwrap().push(query);
        Ok(Vec::new())
    }

    async fn list_skill_sources(&self) -> agent_core::Result<Vec<SkillSourceView>> {
        Ok(vec![source("skillhub"), source("corp")])
    }
}

#[async_trait]
impl McpFacade for SkillFacade {}

#[async_trait]
impl agent_core::facade::ProjectFacade for SkillFacade {}

#[async_trait]
impl agent_core::facade::AgentsFacade for SkillFacade {}

#[async_trait]
impl agent_core::facade::PluginsFacade for SkillFacade {}

#[async_trait]
impl agent_core::facade::AutonomousFacade for SkillFacade {}

impl AppFacade for SkillFacade {}

fn skill(id: &str) -> SkillView {
    SkillView {
        id: id.to_string(),
        name: format!("{id} skill"),
        description: format!("{id} description"),
        version: Some("1.0.0".to_string()),
        source: "user".to_string(),
        activation_mode: "manual".to_string(),
        keywords: vec![id.to_string()],
        tools: Vec::new(),
        can_request_tools: Vec::new(),
        valid: true,
        validation_error: None,
    }
}

fn active_skill(skill_id: &str) -> ActiveSkillView {
    ActiveSkillView {
        skill_id: skill_id.to_string(),
        name: format!("{skill_id} skill"),
        source: "user".to_string(),
        activation_mode: "manual".to_string(),
    }
}

fn source(id: &str) -> SkillSourceView {
    SkillSourceView {
        id: id.to_string(),
        display_name: id.to_string(),
        kind: "registry".to_string(),
        url: format!("https://{id}.example.test"),
        search_template: "{query}".to_string(),
        download_template: "{package}".to_string(),
        list_template: None,
        detail_template: None,
        field_mapping: SkillFieldMappingView::default(),
        enabled: true,
        priority: 0,
        cache_ttl_seconds: 3600,
        last_error: None,
    }
}

fn app() -> App {
    App::new("default", WorkspaceId::new())
}

fn key(code: KeyCode) -> Event {
    Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
}

fn type_catalog_query(app: &mut App, query: &str) {
    use crate::components::{Component, EventContext, FocusTarget, SessionInfo};

    let projection = agent_core::projection::SessionProjection::default();
    let sessions: Vec<SessionInfo> = Vec::new();
    let workspace_id = WorkspaceId::new();
    let session_id = Some(SessionId::new());
    let ctx = EventContext {
        focus: FocusTarget::SkillsOverlay,
        current_session: &projection,
        projects: &[],
        sessions: &sessions,
        model_profile: "default",
        sidebar_left_visible: true,
        sidebar_right_visible: true,
        workspace_id: &workspace_id,
        current_session_id: &session_id,
    };

    let _ = app.skills_overlay.handle_event(&ctx, &key(KeyCode::Tab));
    let _ = app.skills_overlay.handle_event(&ctx, &key(KeyCode::Tab));
    let _ = app
        .skills_overlay
        .handle_event(&ctx, &key(KeyCode::Char('s')));
    let _ = app
        .skills_overlay
        .handle_event(&ctx, &key(KeyCode::Char('/')));
    for ch in query.chars() {
        let _ = app
            .skills_overlay
            .handle_event(&ctx, &key(KeyCode::Char(ch)));
    }
    let _ = app.skills_overlay.handle_event(&ctx, &key(KeyCode::Enter));
}

#[tokio::test]
async fn load_skill_entries_marks_active_skills_for_current_session() {
    let runtime = std::sync::Arc::new(
        SkillFacade::with_skills(vec![skill("review"), skill("docs")])
            .with_active_skills(vec![active_skill("review")]),
    );
    let mut app = app();
    app.current_session_id = Some(SessionId::new());

    let entries = load_skill_entries(&runtime, &mut app).await.unwrap();

    assert_eq!(entries.len(), 2);
    assert!(
        entries
            .iter()
            .find(|entry| entry.id == "review")
            .unwrap()
            .active
    );
    assert!(
        !entries
            .iter()
            .find(|entry| entry.id == "docs")
            .unwrap()
            .active
    );
}

#[tokio::test]
async fn load_skill_entries_without_current_session_marks_all_inactive() {
    let runtime = std::sync::Arc::new(
        SkillFacade::with_skills(vec![skill("review")])
            .with_active_skills(vec![active_skill("review")]),
    );
    let mut app = app();

    let entries = load_skill_entries(&runtime, &mut app).await.unwrap();

    assert_eq!(entries.len(), 1);
    assert!(!entries[0].active);
}

#[tokio::test]
async fn load_skill_entries_returns_none_and_pushes_status_when_list_skills_fails() {
    let runtime = std::sync::Arc::new(SkillFacade::with_skill_error("skills unavailable"));
    let mut app = app();

    let entries = load_skill_entries(&runtime, &mut app).await;

    assert!(entries.is_none());
    assert_eq!(
        app.state
            .latest_status_message()
            .map(|entry| entry.message.as_str()),
        Some("[skills error: invalid state: skills unavailable]")
    );
}

#[tokio::test]
async fn refresh_skill_catalog_without_command_query_reuses_visible_overlay_query() {
    let runtime = std::sync::Arc::new(SkillFacade::with_skills(vec![skill("review")]));
    let mut app = app();
    app.dispatch_effects(vec![CrossPanelEffect::ShowSkillsOverlay(
        SkillOverlaySnapshot {
            discovered: Vec::new(),
            installed: Vec::<SkillSettingsView>::new(),
            catalog: Vec::new(),
            sources: vec![source("skillhub"), source("corp")],
            install_target: SkillInstallTarget::User,
        },
    )]);
    type_catalog_query(&mut app, "review");

    dispatch(
        &runtime,
        &mut app,
        Command::RefreshSkillCatalog {
            keyword: None,
            sources: None,
        },
    )
    .await;

    let queries = runtime.catalog_queries();
    assert_eq!(queries.len(), 1);
    assert_eq!(queries[0].keyword.as_deref(), Some("review"));
    assert_eq!(queries[0].sources, Some(vec!["skillhub".to_string()]));
    assert_eq!(queries[0].limit, Some(50));
}
