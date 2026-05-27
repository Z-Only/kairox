use crate::dag_executor::{DagConfig, DagExecutor};
use crate::execution_runtime::SessionExecutionRuntime;
use crate::skill_package::{DirectDownloadPackageManager, SkillPackageManager};
use crate::task_graph::TaskGraph;
use crate::McpServerManager;
use agent_core::{AppFacade, DomainEvent, PermissionDecision, SessionId};
use agent_mcp::catalog::skills::aggregate::AggregateSkillCatalogProvider;
use agent_mcp::catalog::{AggregateCatalogProvider, CatalogProvider};
use agent_mcp::installer::Installer;
use agent_mcp::{HttpResponseCache, SharedHttpClient};
use agent_memory::{ContextAssembler, MemoryStore};
use agent_store::EventStore;
use agent_tools::{PermissionEngine, ToolRegistry};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

#[path = "facade_session_ops.rs"]
mod facade_session_ops;

const EVENT_CHANNEL_CAPACITY: usize = 1024;

/// Execution mode determines how the agent processes requests.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExecutionMode {
    /// Default: current single-step agent loop behavior.
    SingleStep,
    /// DAG-driven: Planner decomposes, Workers execute in parallel, Reviewer evaluates.
    DagExecution,
}

pub struct LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    pub(crate) store: Arc<S>,
    pub(crate) model: Arc<M>,
    pub(crate) permission_engine: Arc<Mutex<PermissionEngine>>,
    pub(crate) mcp_manager: Option<Arc<Mutex<McpServerManager>>>,
    pub(crate) tool_registry: Arc<Mutex<ToolRegistry>>,
    pub(crate) context_assembler: ContextAssembler,
    pub(crate) memory_store: Option<Arc<dyn MemoryStore>>,
    pub(crate) pending_permissions:
        Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<PermissionDecision>>>>,
    pub(crate) event_tx: tokio::sync::broadcast::Sender<DomainEvent>,
    pub(crate) task_graphs: Arc<Mutex<HashMap<String, TaskGraph>>>,
    pub(crate) session_execution: SessionExecutionRuntime,
    pub(crate) dag_executor: Option<Arc<DagExecutor<S, M>>>,
    pub(crate) dag_config: DagConfig,
    /// Catalog provider (built-in + future remote sources). `None` when the
    /// marketplace has not been wired via [`Self::with_marketplace`].
    pub(crate) catalog: Option<Arc<dyn CatalogProvider>>,
    /// Installer for marketplace entries. `None` when the marketplace has not
    /// been wired via [`Self::with_marketplace`].
    pub(crate) installer: Option<Arc<Installer>>,
    /// Directory containing `config.toml` (used for atomic
    /// catalog source mutations + reloads). `None` when no marketplace has
    /// been wired.
    pub(crate) marketplace_dir: Option<PathBuf>,
    /// Phase 2: concrete handle to the aggregate provider for `reload`
    /// after toml mutations. `None` when no marketplace has been wired.
    pub(crate) aggregate_handle: Option<Arc<AggregateCatalogProvider>>,
    /// Phase 2: shared HTTP client + cache for remote catalog providers.
    pub(crate) catalog_http: Option<SharedHttpClient>,
    pub(crate) catalog_cache: Option<Arc<HttpResponseCache>>,
    pub(crate) skill_registry: Option<Arc<dyn agent_skills::SkillRegistry>>,
    pub(crate) skill_settings_roots: crate::skill_settings::SkillSettingsRoots,
    pub(crate) agent_settings_roots: crate::agent_settings::AgentSettingsRoots,
    pub(crate) plugin_settings_roots: crate::plugin_settings::PluginSettingsRoots,
    pub(crate) skill_package_manager: Arc<dyn SkillPackageManager>,
    pub(crate) active_skills: Arc<Mutex<HashMap<String, Vec<String>>>>,
    /// Per-session in-memory state. Inserted lazily on first access.
    pub(crate) session_states: Arc<Mutex<HashMap<String, crate::session::SessionState>>>,
    /// Loaded TOML config (`Config::load()` in production, in-line in tests).
    /// Required by Tasks 9-10 to look up `ProfileDef` by alias and call
    /// `agent_config::resolve_limits`.
    pub(crate) config: Arc<agent_config::Config>,
    /// Profile-alias → typed Ollama client. Populated by `with_ollama_clients`
    /// at wiring time so Task 10 can fire `probe_context_window`. Empty when
    /// no Ollama profiles are configured.
    pub(crate) ollama_clients: HashMap<String, Arc<agent_models::OllamaClient>>,
    // Skill catalog
    pub(crate) skill_catalog: std::sync::OnceLock<Arc<AggregateSkillCatalogProvider>>,
    pub(crate) skill_sources_toml: Option<crate::skill_sources_toml::SkillSourcesToml>,
    pub(crate) skill_catalog_http: Option<SharedHttpClient>,
    pub(crate) skill_catalog_cache_dir: Option<PathBuf>,
}

impl<S, M> LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    pub fn new(store: S, model: M) -> Self {
        let (event_tx, _) = tokio::sync::broadcast::channel(EVENT_CHANNEL_CAPACITY);
        Self {
            store: Arc::new(store),
            model: Arc::new(model),
            permission_engine: Arc::new(Mutex::new(PermissionEngine::default())),
            tool_registry: Arc::new(Mutex::new(ToolRegistry::new())),
            context_assembler: ContextAssembler::new_standalone(),
            memory_store: None,
            pending_permissions: Arc::new(Mutex::new(HashMap::new())),
            event_tx,
            task_graphs: Arc::new(Mutex::new(HashMap::new())),
            session_execution: SessionExecutionRuntime::new(),
            mcp_manager: None,
            dag_executor: None,
            dag_config: DagConfig::default(),
            catalog: None,
            installer: None,
            marketplace_dir: None,
            aggregate_handle: None,
            catalog_http: None,
            catalog_cache: None,
            skill_registry: None,
            skill_settings_roots: crate::skill_settings::SkillSettingsRoots::default(),
            agent_settings_roots: crate::agent_settings::AgentSettingsRoots::default(),
            plugin_settings_roots: crate::plugin_settings::PluginSettingsRoots::default(),
            skill_package_manager: Arc::new(DirectDownloadPackageManager),
            active_skills: Arc::new(Mutex::new(HashMap::new())),
            session_states: Arc::new(Mutex::new(HashMap::new())),
            config: Arc::new(agent_config::Config {
                profiles: vec![],
                mcp_servers: vec![],
                source: agent_config::ConfigSource::Defaults,
                context: agent_config::ContextPolicy::default(),
                disabled_mcp_servers: vec![],
                instructions: None,
                features: agent_config::FeatureFlags::default(),
                hooks: vec![],
            }),
            ollama_clients: HashMap::new(),
            skill_catalog: std::sync::OnceLock::new(),
            skill_sources_toml: None,
            skill_catalog_http: None,
            skill_catalog_cache_dir: None,
        }
    }

    /// Public accessor for the underlying event store.
    pub fn store(&self) -> &S {
        &self.store
    }

    /// Public accessor for the loaded `Config`. Used by UI dispatchers (TUI
    /// model overlay, GUI settings) that need to snapshot profile metadata.
    pub fn config(&self) -> &Arc<agent_config::Config> {
        &self.config
    }

    /// Test-only accessor for the underlying event store. Gated so production
    /// code can never read it.
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn event_store_for_test(&self) -> &S {
        &self.store
    }

    /// Test-only accessor for the per-session state map. Used by the P2
    /// compaction integration test to flip the busy gate deterministically.
    #[cfg(any(test, feature = "test-helpers"))]
    pub fn session_states_for_test(
        &self,
    ) -> &Arc<Mutex<HashMap<String, crate::session::SessionState>>> {
        &self.session_states
    }
}

impl<S, M> LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    /// Trigger a compaction pass for `session_id`. Blocks until the chain
    /// completes (success or fallback). Returns `Err(SessionBusy)` if a
    /// compaction is already running for the same session.
    ///
    /// This is the inherent method; P3 will surface it via the `AppFacade`
    /// trait once the GUI/TUI commands wire to it.
    pub async fn compact_session(
        &self,
        session_id: SessionId,
        reason: agent_core::CompactionReason,
    ) -> agent_core::Result<()> {
        // Resolve the workspace_id from the first event of the session.
        let events = self
            .store
            .load_session(&session_id)
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;
        let workspace_id = events
            .first()
            .map(|e| e.workspace_id.clone())
            .ok_or_else(|| agent_core::CoreError::InvalidState("session has no events".into()))?;

        // Pre-check the busy gate so we can surface SessionBusy upfront
        // (the orchestrator silently no-ops when already compacting).
        {
            let states = self.session_states.lock().await;
            if let Some(entry) = states.get(&session_id.to_string()) {
                if entry.compacting {
                    return Err(agent_core::CoreError::SessionBusy {
                        session_id: session_id.to_string(),
                        reason: "compaction already running".into(),
                    });
                }
            }
        }

        // Pick the profile alias for the summarisation call:
        // ContextPolicy.compactor_profile takes priority; otherwise fall
        // back to the session's current profile (from SessionInitialized).
        let profile_alias = self
            .config
            .context
            .compactor_profile
            .clone()
            .unwrap_or_else(|| {
                events
                    .iter()
                    .find_map(|e| match &e.payload {
                        agent_core::EventPayload::SessionInitialized { model_profile } => {
                            Some(model_profile.clone())
                        }
                        _ => None,
                    })
                    .unwrap_or_else(|| "fake".to_string())
            });

        crate::compaction::compact_session(
            &*self.store,
            &self.event_tx,
            &*self.model,
            &profile_alias,
            &self.session_states,
            workspace_id,
            session_id,
            reason,
        )
        .await
    }
}

impl<S, M> AppFacade for LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::{
        AgentRole, AppFacade, SendMessageRequest, StartSessionRequest, TaskState, WorkspaceId,
    };
    use agent_models::{FakeModelClient, ModelClient, ModelEvent, ModelMessage, ModelRequest};
    use agent_store::SqliteEventStore;
    use async_trait::async_trait;
    use futures::stream::BoxStream;
    use std::collections::VecDeque;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::{oneshot, Mutex as TokioMutex};

    struct BlockingModelClient {
        started: TokioMutex<Option<oneshot::Sender<()>>>,
        release: TokioMutex<Option<oneshot::Receiver<()>>>,
        stream_calls: Arc<AtomicUsize>,
    }

    impl BlockingModelClient {
        fn new(
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

    struct BlockingStreamGate {
        started: oneshot::Sender<()>,
        release: oneshot::Receiver<()>,
        token: String,
    }

    struct MultiBlockingModelClient {
        gates: TokioMutex<VecDeque<BlockingStreamGate>>,
        stream_calls: Arc<AtomicUsize>,
    }

    impl MultiBlockingModelClient {
        fn new(gates: Vec<BlockingStreamGate>, stream_calls: Arc<AtomicUsize>) -> Self {
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

    struct DecomposingPlannerStrategy;

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

    struct StreamingWorkerStrategy;

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

    async fn install_streaming_dag_executor<S, M>(runtime: &mut LocalRuntime<S, M>)
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
            runtime.dag_config.clone(),
            runtime.agent_settings_roots.clone(),
        )
        .await
        .with_strategy(AgentRole::Planner, Arc::new(DecomposingPlannerStrategy))
        .with_strategy(AgentRole::Worker, Arc::new(StreamingWorkerStrategy));
        runtime.dag_executor = Some(Arc::new(executor));
    }

    #[tokio::test]
    async fn default_execution_mode_is_single_step() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hi".into()]);
        let runtime = LocalRuntime::new(store, model);

        let request = SendMessageRequest {
            workspace_id: WorkspaceId::new(),
            session_id: SessionId::new(),
            content: "hello".into(),
            attachments: vec![],
        };
        assert_eq!(runtime.execution_mode(&request), ExecutionMode::SingleStep);
    }

    #[tokio::test]
    async fn plan_prefix_triggers_dag_mode() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hi".into()]);
        let runtime = LocalRuntime::new(store, model).with_dag_execution().await;

        let request = SendMessageRequest {
            workspace_id: WorkspaceId::new(),
            session_id: SessionId::new(),
            content: "/plan implement feature X".into(),
            attachments: vec![],
        };
        assert_eq!(
            runtime.execution_mode(&request),
            ExecutionMode::DagExecution
        );
    }

    #[tokio::test]
    async fn start_session_registers_idle_session_actor() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hi".into()]);
        let runtime = LocalRuntime::new(store, model);

        let workspace = runtime
            .open_workspace("/tmp/workspace".into())
            .await
            .unwrap();
        assert_eq!(runtime.session_execution.actor_count().await, 0);

        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id,
                model_profile: "fake".into(),
                approval_policy: None,
                sandbox_policy: None,
            })
            .await
            .unwrap();

        assert_eq!(runtime.session_execution.actor_count().await, 1);
        assert_eq!(
            runtime.session_execution.session_state(&session_id).await,
            Some(crate::execution_runtime::ExecutionState::Idle)
        );
    }

    #[tokio::test]
    async fn soft_delete_session_stops_session_actor() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["done".into()]);
        let runtime = LocalRuntime::new(store, model);

        let workspace = runtime
            .open_workspace("/tmp/workspace".into())
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
                content: "hello".into(),
                attachments: vec![],
            })
            .await
            .unwrap();
        assert_eq!(runtime.session_execution.actor_count().await, 1);

        runtime.soft_delete_session(&session_id).await.unwrap();

        assert_eq!(
            runtime.session_execution.session_state(&session_id).await,
            None
        );
        assert_eq!(runtime.session_execution.actor_count().await, 0);
    }

    #[tokio::test]
    async fn permanently_delete_session_stops_session_actor() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["done".into()]);
        let runtime = LocalRuntime::new(store, model);

        let workspace = runtime
            .open_workspace("/tmp/workspace".into())
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
                content: "hello".into(),
                attachments: vec![],
            })
            .await
            .unwrap();
        assert_eq!(runtime.session_execution.actor_count().await, 1);

        runtime
            .permanently_delete_session(&session_id)
            .await
            .unwrap();

        assert_eq!(
            runtime.session_execution.session_state(&session_id).await,
            None
        );
        assert_eq!(runtime.session_execution.actor_count().await, 0);
    }

    #[tokio::test]
    async fn restore_archived_session_restarts_session_actor() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["done".into()]);
        let runtime = LocalRuntime::new(store, model);

        let workspace = runtime
            .open_workspace("/tmp/workspace".into())
            .await
            .unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id,
                model_profile: "fake".into(),
                approval_policy: None,
                sandbox_policy: None,
            })
            .await
            .unwrap();
        assert_eq!(runtime.session_execution.actor_count().await, 1);

        runtime.soft_delete_session(&session_id).await.unwrap();
        assert_eq!(runtime.session_execution.actor_count().await, 0);
        assert_eq!(
            runtime.session_execution.session_state(&session_id).await,
            None
        );

        runtime.restore_archived_session(&session_id).await.unwrap();

        assert_eq!(runtime.session_execution.actor_count().await, 1);
        assert_eq!(
            runtime.session_execution.session_state(&session_id).await,
            Some(crate::execution_runtime::ExecutionState::Idle)
        );
    }

    #[tokio::test]
    async fn send_message_returns_session_busy_when_compacting() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hello".into()]);
        let runtime = LocalRuntime::new(store, model);
        let rt = &runtime as &dyn AppFacade;

        let workspace = AppFacade::open_workspace(rt, "/tmp/workspace".into())
            .await
            .unwrap();
        let session_id = AppFacade::start_session(
            rt,
            StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fake".into(),
                approval_policy: None,
                sandbox_policy: None,
            },
        )
        .await
        .unwrap();

        // Force the session into compacting state.
        {
            let mut states = runtime.session_states.lock().await;
            states
                .entry(session_id.to_string())
                .or_insert_with(crate::session::SessionState::default)
                .compacting = true;
        }

        let result = AppFacade::send_message(
            rt,
            SendMessageRequest {
                workspace_id: workspace.workspace_id,
                session_id: session_id.clone(),
                content: "hello".into(),
                attachments: vec![],
            },
        )
        .await;
        match result {
            Err(agent_core::CoreError::SessionBusy { session_id: id, .. }) => {
                assert_eq!(id, session_id.to_string());
            }
            other => panic!("expected SessionBusy, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn send_message_queues_same_session_turn_when_actor_turn_running() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let (started_tx, started_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let stream_calls = Arc::new(AtomicUsize::new(0));
        let model = BlockingModelClient::new(started_tx, release_rx, stream_calls.clone());
        let runtime = Arc::new(LocalRuntime::new(store, model));

        let workspace = runtime
            .open_workspace("/tmp/workspace".into())
            .await
            .unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "blocking".into(),
                approval_policy: None,
                sandbox_policy: None,
            })
            .await
            .unwrap();

        let first_runtime = runtime.clone();
        let first_workspace_id = workspace.workspace_id.clone();
        let first_session_id = session_id.clone();
        let first = tokio::spawn(async move {
            first_runtime
                .send_message(SendMessageRequest {
                    workspace_id: first_workspace_id,
                    session_id: first_session_id,
                    content: "first".into(),
                    attachments: vec![],
                })
                .await
        });
        started_rx.await.unwrap();

        let second_runtime = runtime.clone();
        let second_workspace_id = workspace.workspace_id;
        let second_session_id = session_id.clone();
        let second = tokio::spawn(async move {
            second_runtime
                .send_message(SendMessageRequest {
                    workspace_id: second_workspace_id,
                    session_id: second_session_id,
                    content: "second".into(),
                    attachments: vec![],
                })
                .await
        });

        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        assert!(!second.is_finished());
        assert_eq!(stream_calls.load(Ordering::SeqCst), 1);

        release_tx.send(()).unwrap();
        first.await.unwrap().unwrap();
        second.await.unwrap().unwrap();
        assert_eq!(stream_calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn send_message_returns_session_busy_when_compacting_during_actor_turn() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let (started_tx, started_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let stream_calls = Arc::new(AtomicUsize::new(0));
        let model = BlockingModelClient::new(started_tx, release_rx, stream_calls.clone());
        let runtime = Arc::new(LocalRuntime::new(store, model));

        let workspace = runtime
            .open_workspace("/tmp/workspace".into())
            .await
            .unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "blocking".into(),
                approval_policy: None,
                sandbox_policy: None,
            })
            .await
            .unwrap();

        let first_runtime = runtime.clone();
        let first_workspace_id = workspace.workspace_id.clone();
        let first_session_id = session_id.clone();
        let first = tokio::spawn(async move {
            first_runtime
                .send_message(SendMessageRequest {
                    workspace_id: first_workspace_id,
                    session_id: first_session_id,
                    content: "first".into(),
                    attachments: vec![],
                })
                .await
        });
        started_rx.await.unwrap();

        {
            let mut states = runtime.session_states.lock().await;
            states
                .entry(session_id.to_string())
                .or_insert_with(crate::session::SessionState::default)
                .compacting = true;
        }

        let result = runtime
            .send_message(SendMessageRequest {
                workspace_id: workspace.workspace_id,
                session_id: session_id.clone(),
                content: "second".into(),
                attachments: vec![],
            })
            .await;

        match result {
            Err(agent_core::CoreError::SessionBusy { session_id: id, .. }) => {
                assert_eq!(id, session_id.to_string());
            }
            other => panic!("expected SessionBusy, got {other:?}"),
        }
        assert_eq!(stream_calls.load(Ordering::SeqCst), 1);

        release_tx.send(()).unwrap();
        first.await.unwrap().unwrap();
    }

    #[tokio::test]
    async fn cancel_session_interrupts_running_single_step_turn() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let (started_tx, started_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let stream_calls = Arc::new(AtomicUsize::new(0));
        let model = BlockingModelClient::new(started_tx, release_rx, stream_calls.clone());
        let runtime = Arc::new(LocalRuntime::new(store, model));

        let workspace = runtime
            .open_workspace("/tmp/workspace".into())
            .await
            .unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "blocking".into(),
                approval_policy: None,
                sandbox_policy: None,
            })
            .await
            .unwrap();

        let turn_runtime = runtime.clone();
        let turn_workspace_id = workspace.workspace_id.clone();
        let turn_session_id = session_id.clone();
        let mut turn = tokio::spawn(async move {
            turn_runtime
                .send_message(SendMessageRequest {
                    workspace_id: turn_workspace_id,
                    session_id: turn_session_id,
                    content: "blocked single-step".into(),
                    attachments: vec![],
                })
                .await
        });
        started_rx.await.unwrap();

        runtime
            .cancel_session(workspace.workspace_id, session_id.clone())
            .await
            .unwrap();

        let completed =
            tokio::time::timeout(std::time::Duration::from_millis(250), &mut turn).await;
        if completed.is_err() {
            drop(release_tx);
            let _ = turn.await;
            panic!(
                "single-step turn should finish after session cancellation without stream release"
            );
        }

        completed.unwrap().unwrap().unwrap();
        assert_eq!(stream_calls.load(Ordering::SeqCst), 1);

        let graphs = runtime.task_graphs.lock().await;
        let graph = graphs.get(&session_id.to_string()).unwrap();
        let counts = graph.state_counts();
        assert_eq!(counts.running, 0);
        assert!(counts.failed > 0);
    }

    #[tokio::test]
    async fn cancel_session_rejects_queued_same_session_turn() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let (started_tx, started_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let stream_calls = Arc::new(AtomicUsize::new(0));
        let model = BlockingModelClient::new(started_tx, release_rx, stream_calls.clone());
        let runtime = Arc::new(LocalRuntime::new(store, model));

        let workspace = runtime
            .open_workspace("/tmp/workspace".into())
            .await
            .unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "blocking".into(),
                approval_policy: None,
                sandbox_policy: None,
            })
            .await
            .unwrap();

        let first_runtime = runtime.clone();
        let first_workspace_id = workspace.workspace_id.clone();
        let first_session_id = session_id.clone();
        let mut first = tokio::spawn(async move {
            first_runtime
                .send_message(SendMessageRequest {
                    workspace_id: first_workspace_id,
                    session_id: first_session_id,
                    content: "first".into(),
                    attachments: vec![],
                })
                .await
        });
        started_rx.await.unwrap();

        let second_runtime = runtime.clone();
        let second_workspace_id = workspace.workspace_id.clone();
        let second_session_id = session_id.clone();
        let second = tokio::spawn(async move {
            second_runtime
                .send_message(SendMessageRequest {
                    workspace_id: second_workspace_id,
                    session_id: second_session_id,
                    content: "second".into(),
                    attachments: vec![],
                })
                .await
        });

        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        assert!(!second.is_finished());
        assert_eq!(stream_calls.load(Ordering::SeqCst), 1);

        runtime
            .cancel_session(workspace.workspace_id, session_id.clone())
            .await
            .unwrap();

        let first_completed =
            tokio::time::timeout(std::time::Duration::from_millis(250), &mut first).await;
        if first_completed.is_err() {
            drop(release_tx);
            let _ = first.await;
            let _ = second.await;
            panic!("first turn should finish after session cancellation");
        }

        first_completed.unwrap().unwrap().unwrap();
        let second_result = second.await.unwrap();
        assert!(
            matches!(second_result, Err(agent_core::CoreError::InvalidState(ref message)) if message.contains("session execution cancelled")),
            "expected queued turn cancellation error, got {second_result:?}"
        );
        assert_eq!(stream_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn cancel_session_does_not_cancel_other_running_session() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let (first_started_tx, first_started_rx) = oneshot::channel();
        let (first_release_tx, first_release_rx) = oneshot::channel();
        let (second_started_tx, second_started_rx) = oneshot::channel();
        let (second_release_tx, second_release_rx) = oneshot::channel();
        let stream_calls = Arc::new(AtomicUsize::new(0));
        let model = MultiBlockingModelClient::new(
            vec![
                BlockingStreamGate {
                    started: first_started_tx,
                    release: first_release_rx,
                    token: "first".into(),
                },
                BlockingStreamGate {
                    started: second_started_tx,
                    release: second_release_rx,
                    token: "second".into(),
                },
            ],
            stream_calls.clone(),
        );
        let runtime = Arc::new(LocalRuntime::new(store, model));

        let workspace = runtime
            .open_workspace("/tmp/workspace".into())
            .await
            .unwrap();
        let first_session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "blocking".into(),
                approval_policy: None,
                sandbox_policy: None,
            })
            .await
            .unwrap();
        let second_session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "blocking".into(),
                approval_policy: None,
                sandbox_policy: None,
            })
            .await
            .unwrap();

        let first_runtime = runtime.clone();
        let first_workspace_id = workspace.workspace_id.clone();
        let first_turn_session = first_session_id.clone();
        let mut first = tokio::spawn(async move {
            first_runtime
                .send_message(SendMessageRequest {
                    workspace_id: first_workspace_id,
                    session_id: first_turn_session,
                    content: "first blocked turn".into(),
                    attachments: vec![],
                })
                .await
        });
        first_started_rx.await.unwrap();

        let second_runtime = runtime.clone();
        let second_workspace_id = workspace.workspace_id.clone();
        let second_turn_session = second_session_id.clone();
        let second = tokio::spawn(async move {
            second_runtime
                .send_message(SendMessageRequest {
                    workspace_id: second_workspace_id,
                    session_id: second_turn_session,
                    content: "second blocked turn".into(),
                    attachments: vec![],
                })
                .await
        });
        second_started_rx.await.unwrap();

        runtime
            .cancel_session(workspace.workspace_id, first_session_id)
            .await
            .unwrap();

        let completed =
            tokio::time::timeout(std::time::Duration::from_millis(250), &mut first).await;
        if completed.is_err() {
            drop(first_release_tx);
            drop(second_release_tx);
            let _ = first.await;
            let _ = second.await;
            panic!("cancelled session should finish without releasing its model stream");
        }
        completed.unwrap().unwrap().unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        assert!(
            !second.is_finished(),
            "cancelling one session must not cancel another running session"
        );

        second_release_tx.send(()).unwrap();
        second.await.unwrap().unwrap();
        assert_eq!(stream_calls.load(Ordering::SeqCst), 2);
        drop(first_release_tx);
    }

    #[tokio::test]
    async fn cancel_session_interrupts_running_dag_turn() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let (started_tx, started_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let stream_calls = Arc::new(AtomicUsize::new(0));
        let model = BlockingModelClient::new(started_tx, release_rx, stream_calls.clone());
        let mut runtime = LocalRuntime::new(store, model);
        install_streaming_dag_executor(&mut runtime).await;
        let runtime = Arc::new(runtime);

        let workspace = runtime
            .open_workspace("/tmp/workspace".into())
            .await
            .unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "blocking".into(),
                approval_policy: None,
                sandbox_policy: None,
            })
            .await
            .unwrap();

        let turn_runtime = runtime.clone();
        let turn_workspace_id = workspace.workspace_id.clone();
        let turn_session_id = session_id.clone();
        let mut turn = tokio::spawn(async move {
            turn_runtime
                .send_message(SendMessageRequest {
                    workspace_id: turn_workspace_id,
                    session_id: turn_session_id,
                    content: "/plan blocked dag".into(),
                    attachments: vec![],
                })
                .await
        });
        started_rx.await.unwrap();

        runtime
            .cancel_session(workspace.workspace_id, session_id.clone())
            .await
            .unwrap();

        let completed =
            tokio::time::timeout(std::time::Duration::from_millis(250), &mut turn).await;
        if completed.is_err() {
            drop(release_tx);
            let _ = turn.await;
            panic!("DAG turn should finish after session cancellation without stream release");
        }

        completed.unwrap().unwrap().unwrap();
        assert_eq!(stream_calls.load(Ordering::SeqCst), 1);

        let graphs = runtime.task_graphs.lock().await;
        let graph = graphs.get(&session_id.to_string()).unwrap();
        let counts = graph.state_counts();
        assert_eq!(counts.running, 0);
        assert!(counts.cancelled > 0);
    }

    #[tokio::test]
    async fn retry_task_queues_behind_active_actor_turn() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let (started_tx, started_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let stream_calls = Arc::new(AtomicUsize::new(0));
        let model = BlockingModelClient::new(started_tx, release_rx, stream_calls.clone());
        let runtime = Arc::new(LocalRuntime::new(store, model).with_dag_execution().await);

        let workspace = runtime
            .open_workspace("/tmp/workspace".into())
            .await
            .unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "blocking".into(),
                approval_policy: None,
                sandbox_policy: None,
            })
            .await
            .unwrap();

        let failed_task_id = {
            let mut graph = TaskGraph::default();
            let task_id = graph.add_task("failed task", AgentRole::Worker, vec![]);
            graph.mark_running(&task_id).unwrap();
            graph.mark_failed(&task_id, "boom".into()).unwrap();
            runtime
                .task_graphs
                .lock()
                .await
                .insert(session_id.to_string(), graph);
            task_id
        };

        let first_runtime = runtime.clone();
        let first_workspace_id = workspace.workspace_id.clone();
        let first_session_id = session_id.clone();
        let first = tokio::spawn(async move {
            first_runtime
                .send_message(SendMessageRequest {
                    workspace_id: first_workspace_id,
                    session_id: first_session_id,
                    content: "first".into(),
                    attachments: vec![],
                })
                .await
        });
        started_rx.await.unwrap();

        let retry_runtime = runtime.clone();
        let retry_workspace_id = workspace.workspace_id;
        let retry_session_id = session_id.clone();
        let retry_task_id = failed_task_id.clone();
        let retry = tokio::spawn(async move {
            retry_runtime
                .retry_task(retry_workspace_id, retry_session_id, retry_task_id)
                .await
        });

        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        assert!(
            !retry.is_finished(),
            "retry_task should wait for the actor turn"
        );
        {
            let graphs = runtime.task_graphs.lock().await;
            let graph = graphs.get(&session_id.to_string()).unwrap();
            let task = graph.get_task(&failed_task_id).unwrap();
            assert_eq!(task.state, TaskState::Failed);
            assert_eq!(task.retry_count, 0);
        }

        release_tx.send(()).unwrap();
        first.await.unwrap().unwrap();
        retry.await.unwrap().unwrap();

        let graphs = runtime.task_graphs.lock().await;
        let graph = graphs.get(&session_id.to_string()).unwrap();
        let task = graph.get_task(&failed_task_id).unwrap();
        assert_eq!(task.state, TaskState::Pending);
        assert_eq!(task.retry_count, 1);
    }

    #[tokio::test]
    async fn cancel_task_queues_behind_active_actor_turn() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let (started_tx, started_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let stream_calls = Arc::new(AtomicUsize::new(0));
        let model = BlockingModelClient::new(started_tx, release_rx, stream_calls.clone());
        let runtime = Arc::new(LocalRuntime::new(store, model).with_dag_execution().await);

        let workspace = runtime
            .open_workspace("/tmp/workspace".into())
            .await
            .unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "blocking".into(),
                approval_policy: None,
                sandbox_policy: None,
            })
            .await
            .unwrap();

        let pending_task_id = {
            let mut graph = TaskGraph::default();
            let task_id = graph.add_task("pending task", AgentRole::Worker, vec![]);
            runtime
                .task_graphs
                .lock()
                .await
                .insert(session_id.to_string(), graph);
            task_id
        };

        let first_runtime = runtime.clone();
        let first_workspace_id = workspace.workspace_id.clone();
        let first_session_id = session_id.clone();
        let first = tokio::spawn(async move {
            first_runtime
                .send_message(SendMessageRequest {
                    workspace_id: first_workspace_id,
                    session_id: first_session_id,
                    content: "first".into(),
                    attachments: vec![],
                })
                .await
        });
        started_rx.await.unwrap();

        let cancel_runtime = runtime.clone();
        let cancel_workspace_id = workspace.workspace_id;
        let cancel_session_id = session_id.clone();
        let cancel_task_id = pending_task_id.clone();
        let cancel = tokio::spawn(async move {
            cancel_runtime
                .cancel_task(cancel_workspace_id, cancel_session_id, cancel_task_id)
                .await
        });

        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        assert!(
            !cancel.is_finished(),
            "cancel_task should wait for the actor turn"
        );
        {
            let graphs = runtime.task_graphs.lock().await;
            let graph = graphs.get(&session_id.to_string()).unwrap();
            let task = graph.get_task(&pending_task_id).unwrap();
            assert_eq!(task.state, TaskState::Pending);
        }

        release_tx.send(()).unwrap();
        first.await.unwrap().unwrap();
        cancel.await.unwrap().unwrap();

        let graphs = runtime.task_graphs.lock().await;
        let graph = graphs.get(&session_id.to_string()).unwrap();
        let task = graph.get_task(&pending_task_id).unwrap();
        assert_eq!(task.state, TaskState::Cancelled);
    }

    // ------------------------------------------------------------------
    // P4: mid-session model switch
    // ------------------------------------------------------------------

    fn test_config_with_two_profiles() -> Arc<agent_config::Config> {
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
            supports_tools: None,
            supports_vision: None,
            supports_reasoning: Some(true),
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
            supports_tools: None,
            supports_vision: None,
            supports_reasoning: Some(true),
            extra_params: None,
            enabled: true,
        };
        Arc::new(agent_config::Config {
            profiles: vec![("fast".into(), fast), ("opus".into(), opus)],
            mcp_servers: vec![],
            source: ConfigSource::Defaults,
            context: ContextPolicy::default(),
            disabled_mcp_servers: vec![],
            instructions: None,
            features: agent_config::FeatureFlags::default(),
            hooks: vec![],
        })
    }

    #[tokio::test]
    async fn switch_model_appends_event_and_updates_session_limits() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hi".into()]);
        let runtime = LocalRuntime::new(store, model).with_config(test_config_with_two_profiles());
        let rt = &runtime as &dyn AppFacade;

        let workspace = AppFacade::open_workspace(rt, "/tmp/ws".into())
            .await
            .unwrap();
        let session_id = AppFacade::start_session(
            rt,
            StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fast".into(),
                approval_policy: None,
                sandbox_policy: None,
            },
        )
        .await
        .unwrap();

        runtime
            .switch_model(session_id.clone(), "opus".into(), None)
            .await
            .expect("switch should succeed");

        let events = runtime
            .event_store_for_test()
            .load_session(&session_id)
            .await
            .unwrap();
        let switched = events
            .iter()
            .find(|e| {
                matches!(
                    &e.payload,
                    agent_core::EventPayload::ModelProfileSwitched { .. }
                )
            })
            .expect("ModelProfileSwitched event present");
        match &switched.payload {
            agent_core::EventPayload::ModelProfileSwitched {
                from_profile,
                to_profile,
                ..
            } => {
                assert_eq!(from_profile, "fast");
                assert_eq!(to_profile, "opus");
            }
            _ => unreachable!(),
        }

        let states = runtime.session_states_for_test().lock().await;
        let entry = states.get(session_id.as_str()).unwrap();
        let limits = entry
            .model_limits
            .as_ref()
            .expect("limits set after switch");
        assert!(limits.context_window > 0);
    }

    #[tokio::test]
    async fn switch_model_rejects_unknown_alias() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hi".into()]);
        let runtime = LocalRuntime::new(store, model).with_config(test_config_with_two_profiles());
        let rt = &runtime as &dyn AppFacade;

        let workspace = AppFacade::open_workspace(rt, "/tmp/ws".into())
            .await
            .unwrap();
        let session_id = AppFacade::start_session(
            rt,
            StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fast".into(),
                approval_policy: None,
                sandbox_policy: None,
            },
        )
        .await
        .unwrap();

        let result = runtime
            .switch_model(session_id, "nonexistent".into(), None)
            .await;
        assert!(matches!(
            result,
            Err(agent_core::CoreError::InvalidState(ref msg)) if msg.contains("nonexistent")
        ));
    }

    #[tokio::test]
    async fn switch_model_appends_event_for_reasoning_only_change() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hi".into()]);
        let runtime = LocalRuntime::new(store, model).with_config(test_config_with_two_profiles());
        let rt = &runtime as &dyn AppFacade;

        let workspace = AppFacade::open_workspace(rt, "/tmp/ws".into())
            .await
            .unwrap();
        let session_id = AppFacade::start_session(
            rt,
            StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "opus".into(),
                approval_policy: None,
                sandbox_policy: None,
            },
        )
        .await
        .unwrap();

        runtime
            .switch_model(session_id.clone(), "opus".into(), Some("xhigh".into()))
            .await
            .expect("reasoning-only switch should succeed");

        let events = runtime
            .event_store_for_test()
            .load_session(&session_id)
            .await
            .unwrap();
        let switched = events
            .iter()
            .find_map(|event| match &event.payload {
                agent_core::EventPayload::ModelProfileSwitched {
                    reasoning_effort, ..
                } => reasoning_effort.as_deref(),
                _ => None,
            })
            .expect("reasoning switch event present");
        assert_eq!(switched, "xhigh");
    }

    #[tokio::test]
    async fn switch_model_is_noop_when_alias_matches_current_profile() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hi".into()]);
        let runtime = LocalRuntime::new(store, model).with_config(test_config_with_two_profiles());
        let rt = &runtime as &dyn AppFacade;

        let workspace = AppFacade::open_workspace(rt, "/tmp/ws".into())
            .await
            .unwrap();
        let session_id = AppFacade::start_session(
            rt,
            StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fast".into(),
                approval_policy: None,
                sandbox_policy: None,
            },
        )
        .await
        .unwrap();

        runtime
            .switch_model(session_id.clone(), "fast".into(), None)
            .await
            .expect("same-profile switch is a no-op, not an error");

        let events = runtime
            .event_store_for_test()
            .load_session(&session_id)
            .await
            .unwrap();
        let count = events
            .iter()
            .filter(|e| {
                matches!(
                    &e.payload,
                    agent_core::EventPayload::ModelProfileSwitched { .. }
                )
            })
            .count();
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn switch_model_returns_session_busy_when_compacting() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hi".into()]);
        let runtime = LocalRuntime::new(store, model).with_config(test_config_with_two_profiles());
        let rt = &runtime as &dyn AppFacade;

        let workspace = AppFacade::open_workspace(rt, "/tmp/ws".into())
            .await
            .unwrap();
        let session_id = AppFacade::start_session(
            rt,
            StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fast".into(),
                approval_policy: None,
                sandbox_policy: None,
            },
        )
        .await
        .unwrap();

        {
            let mut states = runtime.session_states.lock().await;
            states
                .entry(session_id.to_string())
                .or_insert_with(crate::session::SessionState::default)
                .compacting = true;
        }

        let result = runtime
            .switch_model(session_id.clone(), "opus".into(), None)
            .await;
        match result {
            Err(agent_core::CoreError::SessionBusy { session_id: id, .. }) => {
                assert_eq!(id, session_id.to_string());
            }
            other => panic!("expected SessionBusy, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn switch_model_queues_behind_active_actor_turn() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let (started_tx, started_rx) = oneshot::channel();
        let (release_tx, release_rx) = oneshot::channel();
        let stream_calls = Arc::new(AtomicUsize::new(0));
        let model = BlockingModelClient::new(started_tx, release_rx, stream_calls.clone());
        let runtime =
            Arc::new(LocalRuntime::new(store, model).with_config(test_config_with_two_profiles()));

        let workspace = runtime.open_workspace("/tmp/ws".into()).await.unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fast".into(),
                approval_policy: None,
                sandbox_policy: None,
            })
            .await
            .unwrap();

        let turn_runtime = runtime.clone();
        let turn_workspace_id = workspace.workspace_id;
        let turn_session_id = session_id.clone();
        let turn = tokio::spawn(async move {
            turn_runtime
                .send_message(SendMessageRequest {
                    workspace_id: turn_workspace_id,
                    session_id: turn_session_id,
                    content: "first".into(),
                    attachments: vec![],
                })
                .await
        });
        started_rx.await.unwrap();

        let switch_runtime = runtime.clone();
        let switch_session_id = session_id.clone();
        let switch = tokio::spawn(async move {
            switch_runtime
                .switch_model(switch_session_id, "opus".into(), None)
                .await
        });

        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
        assert!(!switch.is_finished());
        let events = runtime
            .event_store_for_test()
            .load_session(&session_id)
            .await
            .unwrap();
        assert!(!events.iter().any(|event| matches!(
            &event.payload,
            agent_core::EventPayload::ModelProfileSwitched { .. }
        )));

        release_tx.send(()).unwrap();
        turn.await.unwrap().unwrap();
        switch.await.unwrap().unwrap();

        let events = runtime
            .event_store_for_test()
            .load_session(&session_id)
            .await
            .unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(
                    &event.payload,
                    agent_core::EventPayload::ModelProfileSwitched { .. }
                ))
                .count(),
            1
        );
        assert_eq!(stream_calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn no_plan_prefix_uses_single_step_even_with_dag() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hi".into()]);
        let runtime = LocalRuntime::new(store, model).with_dag_execution().await;

        let request = SendMessageRequest {
            workspace_id: WorkspaceId::new(),
            session_id: SessionId::new(),
            content: "just a question".into(),
            attachments: vec![],
        };
        assert_eq!(runtime.execution_mode(&request), ExecutionMode::SingleStep);
    }
}
