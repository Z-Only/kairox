use crate::dag_executor::{DagConfig, DagExecutor};
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
use agent_tools::{PermissionEngine, PermissionMode, ToolRegistry};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

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
    pub(crate) active_cancellation: Arc<Mutex<Option<CancellationToken>>>,
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
            permission_engine: Arc::new(Mutex::new(PermissionEngine::new(PermissionMode::Suggest))),
            tool_registry: Arc::new(Mutex::new(ToolRegistry::new())),
            context_assembler: ContextAssembler::new_standalone(),
            memory_store: None,
            pending_permissions: Arc::new(Mutex::new(HashMap::new())),
            event_tx,
            task_graphs: Arc::new(Mutex::new(HashMap::new())),
            active_cancellation: Arc::new(Mutex::new(None)),
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
    use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest, WorkspaceId};
    use agent_models::FakeModelClient;
    use agent_store::SqliteEventStore;

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

                permission_mode: None,
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

                permission_mode: None,
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

                permission_mode: None,
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

                permission_mode: None,
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

                permission_mode: None,
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

                permission_mode: None,
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
