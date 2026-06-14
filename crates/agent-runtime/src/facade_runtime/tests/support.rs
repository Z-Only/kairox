//! Shared test fixtures used across the split `facade_runtime` test modules.
//!
//! Kept `pub(super)` so the sibling test files under `tests/` can reach them
//! while production code stays unable to depend on test-only helpers.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;

use agent_core::{AgentRole, SessionId, WorkspaceId};
use agent_models::{ModelClient, ModelEvent, ModelMessage, ModelRequest};
use agent_store::EventStore;
use async_trait::async_trait;
use futures::stream::BoxStream;
use tokio::sync::{oneshot, Mutex as TokioMutex};

use crate::facade_runtime::LocalRuntime;
use crate::task_graph::TaskGraph;

pub(super) struct BlockingModelClient {
    started: TokioMutex<Option<oneshot::Sender<()>>>,
    release: TokioMutex<Option<oneshot::Receiver<()>>>,
    stream_calls: Arc<AtomicUsize>,
}

impl BlockingModelClient {
    pub(super) fn new(
        started: oneshot::Sender<()>,
        release: oneshot::Receiver<()>,
        stream_calls: Arc<AtomicUsize>,
    ) -> Self {
        Self {
            started: TokioMutex::new(Some(started)),
            release: TokioMutex::new(Some(release)),
            stream_calls,
        }
    }
}

#[async_trait]
impl ModelClient for BlockingModelClient {
    async fn stream(
        &self,
        _request: ModelRequest,
    ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
        let call_index = self.stream_calls.fetch_add(1, Ordering::SeqCst);
        if call_index == 0 {
            if let Some(started) = self.started.lock().await.take() {
                let _ = started.send(());
            }
            let release = self
                .release
                .lock()
                .await
                .take()
                .expect("blocking stream should be consumed once");
            let stream = async_stream::stream! {
                let _ = release.await;
                yield Ok(ModelEvent::TokenDelta("first".into()));
                yield Ok(ModelEvent::Completed { usage: None });
            };
            return Ok(Box::pin(stream));
        }

        Ok(Box::pin(futures::stream::iter(vec![
            Ok(ModelEvent::TokenDelta("second".into())),
            Ok(ModelEvent::Completed { usage: None }),
        ])))
    }
}

pub(super) struct BlockingStreamGate {
    pub(super) started: oneshot::Sender<()>,
    pub(super) release: oneshot::Receiver<()>,
    pub(super) token: String,
}

pub(super) struct MultiBlockingModelClient {
    gates: TokioMutex<VecDeque<BlockingStreamGate>>,
    stream_calls: Arc<AtomicUsize>,
}

impl MultiBlockingModelClient {
    pub(super) fn new(gates: Vec<BlockingStreamGate>, stream_calls: Arc<AtomicUsize>) -> Self {
        Self {
            gates: TokioMutex::new(VecDeque::from(gates)),
            stream_calls,
        }
    }
}

#[async_trait]
impl ModelClient for MultiBlockingModelClient {
    async fn stream(
        &self,
        _request: ModelRequest,
    ) -> agent_models::Result<BoxStream<'static, agent_models::Result<ModelEvent>>> {
        self.stream_calls.fetch_add(1, Ordering::SeqCst);
        let BlockingStreamGate {
            started,
            release,
            token,
        } = self
            .gates
            .lock()
            .await
            .pop_front()
            .expect("expected a blocking stream gate");
        let _ = started.send(());
        let stream = async_stream::stream! {
            let _ = release.await;
            yield Ok(ModelEvent::TokenDelta(token));
            yield Ok(ModelEvent::Completed { usage: None });
        };
        Ok(Box::pin(stream))
    }
}

pub(super) async fn append_compaction_history<S: EventStore>(
    store: &S,
    workspace_id: &WorkspaceId,
    session_id: &SessionId,
    pairs: usize,
) {
    let base = chrono::Utc::now() - chrono::Duration::hours(1);
    for i in 0..pairs {
        let user = agent_core::DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            agent_core::AgentId::system(),
            agent_core::PrivacyClassification::FullTrace,
            agent_core::EventPayload::UserMessageAdded {
                message_id: format!("seed-user-{i}"),
                content: format!("seed user {i}"),
                display_content: None,
            },
        )
        .with_timestamp(base + chrono::Duration::seconds(i as i64 * 2));
        store.append(&user).await.unwrap();

        let assistant = agent_core::DomainEvent::new(
            workspace_id.clone(),
            session_id.clone(),
            agent_core::AgentId::system(),
            agent_core::PrivacyClassification::FullTrace,
            agent_core::EventPayload::AssistantMessageCompleted {
                message_id: format!("seed-assistant-{i}"),
                content: format!("seed assistant {i}"),
            },
        )
        .with_timestamp(base + chrono::Duration::seconds(i as i64 * 2 + 1));
        store.append(&assistant).await.unwrap();
    }
}

pub(super) struct DecomposingPlannerStrategy;

#[async_trait]
impl crate::agents::AgentStrategy for DecomposingPlannerStrategy {
    fn role(&self) -> AgentRole {
        AgentRole::Planner
    }

    async fn build_context(
        &self,
        _task: &crate::task_graph::AgentTask,
        _graph: &TaskGraph,
        _session_events: &[agent_core::DomainEvent],
    ) -> Vec<ModelMessage> {
        Vec::new()
    }

    async fn decide(
        &self,
        _ctx: &crate::agents::StepContext,
        _messages: Vec<ModelMessage>,
    ) -> crate::agents::AgentDecision {
        crate::agents::AgentDecision::Decompose {
            sub_tasks: vec![crate::agents::SubTaskDef {
                title: "blocked worker".into(),
                role: AgentRole::Worker,
                dependencies: Vec::new(),
                description: "waits for model stream".into(),
            }],
        }
    }

    async fn process_tool_result(
        &self,
        _tool_call: &agent_models::ToolCall,
        _result: &str,
        _iteration: usize,
    ) -> crate::agents::ToolResultAction {
        crate::agents::ToolResultAction::Continue
    }
}

pub(super) struct StreamingWorkerStrategy;

#[async_trait]
impl crate::agents::AgentStrategy for StreamingWorkerStrategy {
    fn role(&self) -> AgentRole {
        AgentRole::Worker
    }

    async fn build_context(
        &self,
        _task: &crate::task_graph::AgentTask,
        _graph: &TaskGraph,
        _session_events: &[agent_core::DomainEvent],
    ) -> Vec<ModelMessage> {
        vec![ModelMessage {
            role: "user".into(),
            content: "stream until cancelled".into(),
            tool_calls: Vec::new(),
            tool_call_id: None,
        }]
    }

    async fn decide(
        &self,
        _ctx: &crate::agents::StepContext,
        _messages: Vec<ModelMessage>,
    ) -> crate::agents::AgentDecision {
        crate::agents::AgentDecision::RequestModel { tools: Vec::new() }
    }

    async fn process_tool_result(
        &self,
        _tool_call: &agent_models::ToolCall,
        _result: &str,
        _iteration: usize,
    ) -> crate::agents::ToolResultAction {
        crate::agents::ToolResultAction::Continue
    }
}

pub(super) async fn install_streaming_dag_executor<S, M>(runtime: &mut LocalRuntime<S, M>)
where
    S: EventStore + 'static,
    M: ModelClient + 'static,
{
    let executor = crate::dag_executor::DagExecutor::new(
        runtime.store.clone(),
        runtime.model.clone(),
        runtime.event_tx.clone(),
        runtime.tool_registry.clone(),
        runtime.permission_engine.clone(),
        runtime.pending_permissions.clone(),
        runtime.memory_store.clone(),
        runtime.config(),
        runtime.dag_config.clone(),
        runtime.agent_settings_roots.clone(),
    )
    .await
    .with_strategy(AgentRole::Planner, Arc::new(DecomposingPlannerStrategy))
    .with_strategy(AgentRole::Worker, Arc::new(StreamingWorkerStrategy));
    runtime.dag_executor = Some(Arc::new(executor));
}

pub(super) fn test_config_with_two_profiles() -> Arc<agent_config::Config> {
    // Field list verified against `crates/agent-config/src/lib.rs`:
    //   ProfileDef { provider, model_id, base_url, api_key, api_key_env,
    //     context_window, output_limit, response }.
    //   Config { profiles, mcp_servers, source, context: ContextPolicy }.
    //   ContextPolicy is `#[derive(Default)]` (line 147) — `::default()` is
    //   safe. ConfigSource::Defaults is the variant used elsewhere in
    //   facade_runtime.rs test fixtures.
    use agent_config::{ConfigSource, ContextPolicy, ProfileDef};
    let fast = ProfileDef {
        provider: "fake".into(),
        model_id: "fake".into(),
        api_key: None,
        api_key_env: None,
        base_url: None,
        context_window: None,
        output_limit: None,
        response: None,
        max_tokens: None,
        temperature: None,
        top_p: None,
        top_k: None,
        headers: None,
        client_identity: None,
        supports_tools: None,
        supports_vision: None,
        supports_reasoning: Some(true),
        server_tool_code_execution: None,
        server_tool_web_search: None,
        extra_params: None,
        enabled: true,
    };
    let opus = ProfileDef {
        provider: "fake".into(),
        model_id: "fake-opus".into(),
        api_key: None,
        api_key_env: None,
        base_url: None,
        context_window: None,
        output_limit: None,
        response: None,
        max_tokens: None,
        temperature: None,
        top_p: None,
        top_k: None,
        headers: None,
        client_identity: None,
        supports_tools: None,
        supports_vision: None,
        supports_reasoning: Some(true),
        server_tool_code_execution: None,
        server_tool_web_search: None,
        extra_params: None,
        enabled: true,
    };
    Arc::new(agent_config::Config {
        profiles: vec![("fast".into(), fast), ("opus".into(), opus)],
        mcp_servers: vec![],
        knowledge_bases: vec![],
        source: ConfigSource::Defaults,
        context: ContextPolicy::default(),
        disabled_mcp_servers: vec![],
        instructions: None,
        features: agent_config::FeatureFlags::default(),
        hooks: vec![],
        lsp_servers: vec![],
        dap_servers: vec![],
        advisor: agent_config::AdvisorConfig::default(),
    })
}

/// Build a `Config` with one enabled `"fake"` profile and a custom
/// auto-compaction threshold. Matches the field list used by
/// `test_config_with_two_profiles` above.
pub(super) fn test_config_with_threshold(threshold: f32) -> Arc<agent_config::Config> {
    use agent_config::{ConfigSource, ContextPolicy, ProfileDef};
    let fake = ProfileDef {
        provider: "fake".into(),
        model_id: "fake".into(),
        api_key: None,
        api_key_env: None,
        base_url: None,
        context_window: None,
        output_limit: None,
        response: None,
        max_tokens: None,
        temperature: None,
        top_p: None,
        top_k: None,
        headers: None,
        client_identity: None,
        supports_tools: None,
        supports_vision: None,
        supports_reasoning: Some(true),
        server_tool_code_execution: None,
        server_tool_web_search: None,
        extra_params: None,
        enabled: true,
    };
    Arc::new(agent_config::Config {
        profiles: vec![("fake".into(), fake)],
        mcp_servers: vec![],
        knowledge_bases: vec![],
        source: ConfigSource::Defaults,
        context: ContextPolicy {
            auto_compact_threshold: threshold,
            compactor_profile: None,
            max_tool_definition_tokens: None,
            max_iterations: None,
        },
        disabled_mcp_servers: vec![],
        instructions: None,
        features: agent_config::FeatureFlags::default(),
        hooks: vec![],
        lsp_servers: vec![],
        dap_servers: vec![],
        advisor: agent_config::AdvisorConfig::default(),
    })
}

/// Planner strategy that resolves the root task by responding directly
/// (no model call, no sub-task decomposition). Lets a `/plan ...`
/// `send_message` complete deterministically so we can observe what
/// `LocalRuntimeTurnExecutor::maybe_schedule_auto_compaction` does at
/// the tail of the DAG path.
pub(super) struct RespondingPlannerStrategy;

#[async_trait]
impl crate::agents::AgentStrategy for RespondingPlannerStrategy {
    fn role(&self) -> AgentRole {
        AgentRole::Planner
    }

    async fn build_context(
        &self,
        _task: &crate::task_graph::AgentTask,
        _graph: &TaskGraph,
        _session_events: &[agent_core::DomainEvent],
    ) -> Vec<ModelMessage> {
        Vec::new()
    }

    async fn decide(
        &self,
        _ctx: &crate::agents::StepContext,
        _messages: Vec<ModelMessage>,
    ) -> crate::agents::AgentDecision {
        crate::agents::AgentDecision::Respond("done".into())
    }

    async fn process_tool_result(
        &self,
        _tool_call: &agent_models::ToolCall,
        _result: &str,
        _iteration: usize,
    ) -> crate::agents::ToolResultAction {
        crate::agents::ToolResultAction::Continue
    }
}

pub(super) async fn install_responding_dag_executor<S, M>(runtime: &mut LocalRuntime<S, M>)
where
    S: EventStore + 'static,
    M: ModelClient + 'static,
{
    let executor = crate::dag_executor::DagExecutor::new(
        runtime.store.clone(),
        runtime.model.clone(),
        runtime.event_tx.clone(),
        runtime.tool_registry.clone(),
        runtime.permission_engine.clone(),
        runtime.pending_permissions.clone(),
        runtime.memory_store.clone(),
        runtime.config(),
        runtime.dag_config.clone(),
        runtime.agent_settings_roots.clone(),
    )
    .await
    .with_strategy(AgentRole::Planner, Arc::new(RespondingPlannerStrategy));
    runtime.dag_executor = Some(Arc::new(executor));
}

/// Poll the event store until `predicate` matches an event or `timeout`
/// elapses. Used for events the scheduler appends from a spawned task.
pub(super) async fn wait_for_event<S: EventStore>(
    store: &S,
    session_id: &SessionId,
    predicate: impl Fn(&agent_core::EventPayload) -> bool,
    timeout: std::time::Duration,
) -> bool {
    let deadline = std::time::Instant::now() + timeout;
    loop {
        let events = store.load_session(session_id).await.unwrap();
        if events.iter().any(|e| predicate(&e.payload)) {
            return true;
        }
        if std::time::Instant::now() >= deadline {
            return false;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
}
