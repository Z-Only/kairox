use super::*;
use agent_config::{Config, ProfileDef};
use agent_core::{DomainEvent, EventPayload, SendMessageRequest, SessionId, WorkspaceId};
use agent_models::FakeModelClient;
use agent_store::SqliteEventStore;
use agent_tools::{ApprovalPolicy, PermissionEngine, SandboxPolicy, ToolRegistry};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Build a minimal `Config` with the given profiles vec.
fn config_with_profiles(profiles: Vec<(String, ProfileDef)>) -> Config {
    Config {
        profiles,
        ..Config::defaults()
    }
}

/// Build a `ProfileDef` with the specified server-tool toggles.
fn profile_def(
    enabled: bool,
    code_execution: Option<bool>,
    web_search: Option<bool>,
) -> ProfileDef {
    ProfileDef {
        provider: "fake".into(),
        model_id: "fake".into(),
        base_url: None,
        api_key: None,
        api_key_env: None,
        context_window: Some(4096),
        output_limit: Some(2048),
        response: None,
        max_tokens: None,
        temperature: None,
        top_p: None,
        top_k: None,
        headers: None,
        client_identity: None,
        supports_tools: None,
        supports_vision: None,
        supports_reasoning: None,
        extra_params: None,
        server_tool_code_execution: code_execution,
        server_tool_web_search: web_search,
        enabled,
    }
}

/// Shorthand: create all the plumbing values at once.
struct TestHarness {
    store: Arc<SqliteEventStore>,
    model: Arc<FakeModelClient>,
    event_tx: tokio::sync::broadcast::Sender<DomainEvent>,
    tool_registry: Arc<Mutex<ToolRegistry>>,
    permission_engine: Arc<Mutex<PermissionEngine>>,
    pending: crate::permission::PendingPermissionsMap,
    task_graphs: Arc<Mutex<HashMap<String, crate::task_graph::TaskGraph>>>,
    config: Arc<Config>,
    session_states: Arc<Mutex<HashMap<String, crate::session::SessionState>>>,
    active_skills: Arc<Mutex<HashMap<String, Vec<String>>>>,
    // Owned `Option` fields whose references are lent to `AgentLoopDeps`.
    memory_store: Option<Arc<dyn agent_memory::MemoryStore>>,
    skill_registry: Option<Arc<dyn agent_skills::SkillRegistry>>,
    workspace_scoped: Option<Arc<agent_tools::WorkspaceScopedBuiltinTools>>,
    trajectory_store: Option<Arc<dyn agent_store::TrajectoryStore>>,
}

impl TestHarness {
    async fn new(config: Config) -> Self {
        let store = Arc::new(SqliteEventStore::in_memory().await.unwrap());
        let model = Arc::new(FakeModelClient::new(vec!["ok".into()]));
        let (event_tx, _) = tokio::sync::broadcast::channel(1024);
        let tool_registry = Arc::new(Mutex::new(ToolRegistry::new()));
        let permission_engine = Arc::new(Mutex::new(PermissionEngine::new(
            ApprovalPolicy::OnRequest,
            SandboxPolicy::WorkspaceWrite {
                network_access: false,
                writable_roots: vec![],
            },
        )));
        let pending: crate::permission::PendingPermissionsMap =
            Arc::new(Mutex::new(HashMap::new()));
        let task_graphs = Arc::new(Mutex::new(HashMap::new()));
        let config = Arc::new(config);
        let session_states = Arc::new(Mutex::new(HashMap::new()));
        let active_skills = Arc::new(Mutex::new(HashMap::new()));
        Self {
            store,
            model,
            event_tx,
            tool_registry,
            permission_engine,
            pending,
            task_graphs,
            config,
            session_states,
            active_skills,
            memory_store: None,
            skill_registry: None,
            workspace_scoped: None,
            trajectory_store: None,
        }
    }

    fn deps(&self) -> AgentLoopDeps<'_, SqliteEventStore, FakeModelClient> {
        AgentLoopDeps {
            store: &self.store,
            model: &self.model,
            event_tx: &self.event_tx,
            tool_registry: &self.tool_registry,
            permission_engine: &self.permission_engine,
            pending_permissions: &self.pending,
            memory_store: &self.memory_store,
            task_graphs: &self.task_graphs,
            config: &self.config,
            session_states: &self.session_states,
            skill_registry: &self.skill_registry,
            active_skills: &self.active_skills,
            workspace_scoped_builtin_tools: &self.workspace_scoped,
            trajectory_store: &self.trajectory_store,
            turn_cancellation: CancellationToken::new(),
            root_path: None,
        }
    }
}

fn make_request() -> SendMessageRequest {
    SendMessageRequest {
        workspace_id: WorkspaceId::new(),
        session_id: SessionId::new(),
        content: "hello".into(),
        display_content: None,
        attachments: vec![],
    }
}

// ===========================================================================
// server_tools_for_profile tests
// ===========================================================================

#[test]
fn server_tools_returns_tools_for_matching_enabled_profile() {
    let config = config_with_profiles(vec![(
        "test-profile".into(),
        profile_def(true, Some(true), Some(true)),
    )]);

    let tools = server_tools_for_profile(&config, "test-profile");

    assert_eq!(tools.len(), 2);
    assert!(tools.iter().any(|t| matches!(t, ServerTool::CodeExecution)));
    assert!(tools
        .iter()
        .any(|t| matches!(t, ServerTool::WebSearch { .. })));
}

#[test]
fn server_tools_returns_empty_for_nonexistent_profile() {
    let config = config_with_profiles(vec![(
        "existing".into(),
        profile_def(true, Some(true), Some(true)),
    )]);

    let tools = server_tools_for_profile(&config, "nonexistent");

    assert!(tools.is_empty());
}

#[test]
fn server_tools_returns_empty_for_disabled_profile() {
    let config = config_with_profiles(vec![(
        "disabled-profile".into(),
        profile_def(false, Some(true), Some(true)),
    )]);

    let tools = server_tools_for_profile(&config, "disabled-profile");

    assert!(tools.is_empty());
}

#[test]
fn server_tools_only_code_execution_when_web_search_false() {
    let config = config_with_profiles(vec![(
        "code-only".into(),
        profile_def(true, Some(true), Some(false)),
    )]);

    let tools = server_tools_for_profile(&config, "code-only");

    assert_eq!(tools.len(), 1);
    assert!(matches!(tools[0], ServerTool::CodeExecution));
}

#[test]
fn server_tools_only_web_search_when_code_execution_false() {
    let config = config_with_profiles(vec![(
        "search-only".into(),
        profile_def(true, Some(false), Some(true)),
    )]);

    let tools = server_tools_for_profile(&config, "search-only");

    assert_eq!(tools.len(), 1);
    assert!(matches!(tools[0], ServerTool::WebSearch { .. }));
}

// ===========================================================================
// prepare_turn_context tests
// ===========================================================================

#[tokio::test]
async fn prepare_turn_context_returns_reasonable_defaults() {
    let harness = TestHarness::new(Config::defaults()).await;
    let deps = harness.deps();
    let request = make_request();
    let session_events: Vec<DomainEvent> = vec![];

    let turn_ctx = prepare_turn_context(&deps, &request, &session_events)
        .await
        .expect("prepare_turn_context should succeed");

    // With no session events the profile falls back to "fake".
    assert_eq!(turn_ctx.model_profile_alias, "fake");
    // System prompt must contain the Kairox identity marker.
    assert!(turn_ctx.system_prompt.contains("Kairox"));
    // Budget should have positive context window.
    assert!(turn_ctx.budget.context_window > 0);
}

#[tokio::test]
async fn prepare_turn_context_uses_switched_profile() {
    let config = config_with_profiles(vec![
        ("default".into(), profile_def(true, None, None)),
        ("custom".into(), profile_def(true, None, None)),
    ]);
    let harness = TestHarness::new(config).await;
    let deps = harness.deps();
    let request = make_request();

    // Simulate a ModelProfileSwitched event so the resolver picks "custom".
    let switch_event = DomainEvent::new(
        request.workspace_id.clone(),
        request.session_id.clone(),
        agent_core::AgentId::system(),
        agent_core::PrivacyClassification::MinimalTrace,
        EventPayload::ModelProfileSwitched {
            from_profile: "default".into(),
            to_profile: "custom".into(),
            effective_at: Utc::now(),
            context_window: 4096,
            output_limit: 2048,
            limit_source: "builtin_registry".into(),
            reasoning_effort: None,
        },
    );

    let session_events = vec![switch_event];
    let turn_ctx = prepare_turn_context(&deps, &request, &session_events)
        .await
        .expect("prepare_turn_context should succeed with switched profile");

    assert_eq!(turn_ctx.model_profile_alias, "custom");
}

#[tokio::test]
async fn prepare_turn_context_includes_instructions_in_system_prompt() {
    let mut config = Config::defaults();
    config.instructions = Some("Always respond in JSON.".into());
    let harness = TestHarness::new(config).await;
    let deps = harness.deps();
    let request = make_request();
    let session_events: Vec<DomainEvent> = vec![];

    let turn_ctx = prepare_turn_context(&deps, &request, &session_events)
        .await
        .expect("prepare_turn_context should succeed");

    assert!(
        turn_ctx.system_prompt.contains("Always respond in JSON."),
        "system prompt should include custom instructions"
    );
}
