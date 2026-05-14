use crate::dag_executor::{DagConfig, DagExecutor};
use crate::skill_package::{DirectDownloadPackageManager, SkillPackageManager};
use crate::task_graph::TaskGraph;
use crate::McpServerManager;
use agent_core::facade::SessionFacade;
use agent_core::{
    AgentStatusInfo, AppFacade, DomainEvent, PermissionDecision, SendMessageRequest, SessionId,
    StartSessionRequest, TaskId, TraceEntry, WorkspaceId, WorkspaceInfo,
};
use agent_mcp::catalog::skills::{
    aggregate::AggregateSkillCatalogProvider,
    remote::{build_skill_provider, RemoteSkillSourceConfig, SkillSourceKind},
    SkillCatalogProvider,
};
use agent_mcp::catalog::{
    AggregateCatalogProvider, BuiltinCatalogProvider, CatalogProvider, TrustLevel,
};
use agent_mcp::{
    build_remote_catalog_provider, HttpResponseCache, RemoteSourceConfig, RemoteSourceKind,
    SharedHttpClient,
};

use crate::catalog_sink::CatalogEventSink;
use agent_mcp::installer::{Installer, OsRuntimeProbe};
use agent_mcp::types::McpServerDef;
use agent_memory::{ContextAssembler, MemoryStore};
use agent_store::{EventStore, ProjectMetaRepository};
use agent_tools::{BuiltinProvider, PermissionEngine, PermissionMode, ToolProvider, ToolRegistry};
use async_trait::async_trait;
use futures::stream::BoxStream;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

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
    context_assembler: ContextAssembler,
    pub(crate) memory_store: Option<Arc<dyn MemoryStore>>,
    pub(crate) pending_permissions:
        Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<PermissionDecision>>>>,
    pub(crate) event_tx: tokio::sync::broadcast::Sender<DomainEvent>,
    pub(crate) task_graphs: Arc<Mutex<HashMap<String, TaskGraph>>>,
    pub(crate) active_cancellation: Arc<Mutex<Option<CancellationToken>>>,
    pub(crate) dag_executor: Option<Arc<DagExecutor<S, M>>>,
    dag_config: DagConfig,
    /// Catalog provider (built-in + future remote sources). `None` when the
    /// marketplace has not been wired via [`Self::with_marketplace`].
    pub(crate) catalog: Option<Arc<dyn CatalogProvider>>,
    /// Installer for marketplace entries. `None` when the marketplace has not
    /// been wired via [`Self::with_marketplace`].
    pub(crate) installer: Option<Arc<Installer>>,
    /// Phase 2: directory containing `mcp_servers.toml` (used for atomic
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
    skill_catalog_http: Option<SharedHttpClient>,
    skill_catalog_cache_dir: Option<PathBuf>,
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
            skill_package_manager: Arc::new(DirectDownloadPackageManager),
            active_skills: Arc::new(Mutex::new(HashMap::new())),
            session_states: Arc::new(Mutex::new(HashMap::new())),
            config: Arc::new(agent_config::Config {
                profiles: vec![],
                mcp_servers: vec![],
                source: agent_config::ConfigSource::Defaults,
                context: agent_config::ContextPolicy::default(),
            }),
            ollama_clients: HashMap::new(),
            skill_catalog: std::sync::OnceLock::new(),
            skill_sources_toml: None,
            skill_catalog_http: None,
            skill_catalog_cache_dir: None,
        }
    }

    /// Inject the loaded `Config` so the runtime can resolve `ModelLimits`
    /// per session. Called by every production wiring site after `Config::load()`.
    pub fn with_config(mut self, config: Arc<agent_config::Config>) -> Self {
        self.config = config;
        self
    }

    /// Register typed Ollama clients per profile alias. Called by the wiring
    /// code AFTER `build_router` so we retain the typed handle needed for
    /// `probe_context_window` (which `Arc<dyn ModelClient>` cannot expose).
    /// Idempotent — calling twice replaces the entries.
    pub fn with_ollama_clients(
        mut self,
        clients: HashMap<String, Arc<agent_models::OllamaClient>>,
    ) -> Self {
        self.ollama_clients = clients;
        self
    }

    /// Configure the skill catalog with a cache directory. Creates an internal
    /// HTTP client automatically.
    pub fn with_skill_catalog(mut self, dir: Option<PathBuf>) -> Self {
        if let Some(ref d) = dir {
            self.skill_sources_toml = Some(crate::skill_sources_toml::SkillSourcesToml::new(d));
        }
        self.skill_catalog_http = SharedHttpClient::new().ok();
        self.skill_catalog_cache_dir = dir;
        self
    }

    /// Update the in-memory `SessionState` for `session_id` with newly
    /// resolved model limits. Inserts a default `SessionState` if missing.
    pub(crate) async fn set_session_limits(
        &self,
        session_id: &SessionId,
        limits: agent_models::ModelLimits,
    ) {
        let mut states = self.session_states.lock().await;
        let entry = states
            .entry(session_id.to_string())
            .or_insert_with(crate::session::SessionState::default);
        entry.model_limits = Some(limits);
    }

    /// Public accessor for the underlying event store.
    pub fn store(&self) -> &S {
        &self.store
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

    /// Wire the MCP marketplace: built-in catalog provider + on-disk installer
    /// targeting `<config_dir>/mcp_servers.toml`.
    ///
    /// Without this, the catalog-related [`AppFacade`] methods return errors
    /// (or empty results) because they have nowhere to read from or write to.
    pub fn with_marketplace(self, config_dir: PathBuf) -> crate::Result<Self> {
        self.with_marketplace_loaded(config_dir, &[])
    }

    /// Phase 2: like [`with_marketplace`] but also registers user-configured
    /// remote catalog sources. The runtime stores the marketplace directory
    /// for future atomic toml mutations + reloads.
    pub fn with_marketplace_loaded(
        mut self,
        config_dir: PathBuf,
        sources: &[agent_config::CatalogSourceConfig],
    ) -> crate::Result<Self> {
        let cache_dir = config_dir.join("catalog-cache");
        let event_tx = self.event_tx.clone();
        let aggregate = build_catalog_provider(sources, cache_dir.clone(), event_tx)
            .map_err(|e| crate::RuntimeError::Other(format!("catalog provider: {e}")))?;
        let aggregate_arc = Arc::new(aggregate);
        let dyn_arc: Arc<dyn CatalogProvider> = aggregate_arc.clone();
        self.aggregate_handle = Some(aggregate_arc);
        self.catalog = Some(dyn_arc);

        let toml_path = config_dir.join("mcp_servers.toml");
        self.installer = Some(Arc::new(Installer::new(
            toml_path,
            Arc::new(OsRuntimeProbe),
        )));
        self.catalog_http = Some(
            SharedHttpClient::new()
                .map_err(|e| crate::RuntimeError::Other(format!("http client: {e}")))?,
        );
        self.catalog_cache = Some(Arc::new(HttpResponseCache::new(cache_dir)));
        self.marketplace_dir = Some(config_dir);
        Ok(self)
    }

    pub fn with_permission_mode(mut self, mode: PermissionMode) -> Self {
        self.permission_engine = Arc::new(Mutex::new(PermissionEngine::new(mode)));
        self
    }

    pub fn with_skill_registry(mut self, registry: Arc<dyn agent_skills::SkillRegistry>) -> Self {
        self.skill_registry = Some(registry);
        self
    }

    pub fn with_skill_package_manager(mut self, manager: Arc<dyn SkillPackageManager>) -> Self {
        self.skill_package_manager = manager;
        self
    }

    pub fn with_skill_settings_roots(
        mut self,
        roots: crate::skill_settings::SkillSettingsRoots,
    ) -> Self {
        self.skill_settings_roots = roots;
        self
    }

    pub(crate) fn skill_settings_roots(&self) -> crate::skill_settings::SkillSettingsRoots {
        self.skill_settings_roots.clone()
    }

    /// Legacy builder kept for compatibility. The `max_tokens` argument is
    /// ignored — Task 8 will replace this with per-session `ContextBudget`
    /// configuration. Until then call sites can keep passing their old value.
    pub fn with_context_limit(mut self, _max_tokens: usize) -> Self {
        self.context_assembler = ContextAssembler::new_standalone();
        self
    }

    pub fn tool_registry(&self) -> Arc<Mutex<ToolRegistry>> {
        self.tool_registry.clone()
    }

    pub(crate) fn project_repository(&self) -> agent_core::Result<ProjectMetaRepository> {
        self.store
            .sqlite_pool()
            .map(ProjectMetaRepository::new)
            .ok_or_else(crate::project::invalid_project_store_error)
    }

    /// Get the current permission mode.
    pub async fn permission_mode(&self) -> PermissionMode {
        *self.permission_engine.lock().await.mode()
    }

    /// Set the memory store for persistent memory.
    pub fn with_memory_store(mut self, store: Arc<dyn MemoryStore>) -> Self {
        self.memory_store = Some(store.clone());
        self.context_assembler = ContextAssembler::new(store);
        self
    }

    /// Get a reference to the memory store (if configured).
    pub fn memory_store(&self) -> Option<Arc<dyn MemoryStore>> {
        self.memory_store.clone()
    }

    /// Register builtin tools (shell.exec, search.ripgrep, patch.apply, fs.read)
    pub async fn with_builtin_tools(mut self, workspace_root: PathBuf) -> Self {
        if self.skill_settings_roots.workspace_root.is_none()
            && self.skill_settings_roots.user_root.is_none()
        {
            let home_dir = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            self.skill_settings_roots =
                crate::skills::build_default_skill_settings_roots(&home_dir, &workspace_root);
        }
        let provider = BuiltinProvider::with_defaults(workspace_root);
        self.tool_registry
            .lock()
            .await
            .add_provider(Box::new(provider))
            .await;
        self
    }

    /// Register a custom tool provider
    pub async fn with_provider(self, provider: Box<dyn ToolProvider>) -> Self {
        self.tool_registry.lock().await.add_provider(provider).await;
        self
    }

    /// Configure MCP servers from parsed config definitions.
    pub async fn with_mcp_servers(mut self, configs: Vec<McpServerDef>) -> Self {
        if configs.is_empty() {
            return self;
        }
        let mut manager = McpServerManager::from_config(
            configs,
            self.tool_registry.clone(),
            self.permission_engine.clone(),
            Some(self.event_tx.clone()),
        );
        let results = manager.start_persistent_servers().await;
        for result in &results {
            if let Err(e) = result {
                tracing::warn!("MCP server startup warning: {}", e);
            }
        }
        self.mcp_manager = Some(Arc::new(Mutex::new(manager)));
        self
    }

    /// Get a reference to the MCP server manager (if configured).
    pub fn mcp_manager(&self) -> Option<Arc<Mutex<McpServerManager>>> {
        self.mcp_manager.clone()
    }

    /// Enable DAG execution mode with the default configuration.
    pub fn with_dag_execution(mut self) -> Self {
        self.dag_config = DagConfig::default();
        self.dag_executor = Some(Arc::new(DagExecutor::new(
            self.store.clone(),
            self.model.clone(),
            self.event_tx.clone(),
            self.tool_registry.clone(),
            self.permission_engine.clone(),
            self.pending_permissions.clone(),
            self.memory_store.clone(),
            self.dag_config.clone(),
        )));
        self
    }

    /// Enable DAG execution mode with a custom configuration.
    pub fn with_dag_config(mut self, config: DagConfig) -> Self {
        self.dag_config = config.clone();
        self.dag_executor = Some(Arc::new(DagExecutor::new(
            self.store.clone(),
            self.model.clone(),
            self.event_tx.clone(),
            self.tool_registry.clone(),
            self.permission_engine.clone(),
            self.pending_permissions.clone(),
            self.memory_store.clone(),
            config,
        )));
        self
    }

    /// Determine the execution mode for a given request.
    pub(crate) fn execution_mode(&self, request: &SendMessageRequest) -> ExecutionMode {
        if request.content.starts_with("/plan ") && self.dag_executor.is_some() {
            ExecutionMode::DagExecution
        } else {
            ExecutionMode::SingleStep
        }
    }
}

/// Resolve a pending permission request (used by GUI Interactive mode).
impl<S, M> LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    pub async fn resolve_permission(
        &self,
        request_id: &str,
        decision: PermissionDecision,
    ) -> agent_core::Result<()> {
        crate::permission::resolve_permission(&self.pending_permissions, request_id, decision).await
    }

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

    /// Switch the active model profile for an ongoing session.
    ///
    /// The switch takes effect at the next `send_message` call — any
    /// in-flight agent loop completes on the old profile end-to-end so
    /// provider-specific tool-call formats (Anthropic `tool_use` vs.
    /// OpenAI function-calling) don't get mixed mid-stream.
    ///
    /// Errors:
    /// - `CoreError::InvalidState` if the alias is unknown.
    /// - `CoreError::SessionBusy` if the session is currently compacting.
    ///
    /// Same-profile switches (alias equals the current profile) are a
    /// silent no-op — they return `Ok(())` without appending an event.
    pub async fn switch_model(
        &self,
        session_id: agent_core::SessionId,
        profile_alias: String,
    ) -> agent_core::Result<()> {
        // Validate alias exists in the loaded Config.
        let profile_def = self
            .config
            .profiles
            .iter()
            .find(|(alias, def)| alias == &profile_alias && def.enabled)
            .map(|(_, def)| def.clone())
            .ok_or_else(|| {
                agent_core::CoreError::InvalidState(format!("unknown model: {profile_alias}"))
            })?;

        // Resolve the session's current profile using the same helper
        // the agent loop uses — the two resolvers must never drift.
        let events = self
            .store
            .load_session(&session_id)
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;
        let from_profile = crate::agent_loop::latest_model_profile_for(&events);

        // Same-profile switch → silent no-op.
        if from_profile == profile_alias {
            return Ok(());
        }

        let workspace_id = events
            .first()
            .map(|e| e.workspace_id.clone())
            .ok_or_else(|| agent_core::CoreError::InvalidState("session has no events".into()))?;

        // Busy-gate — refuse when compacting (mirrors compact_session
        // lines 374-388 of this file).
        {
            let states = self.session_states.lock().await;
            if let Some(entry) = states.get(&session_id.to_string()) {
                if entry.compacting {
                    return Err(agent_core::CoreError::SessionBusy {
                        session_id: session_id.to_string(),
                        reason: "context compaction in progress".into(),
                    });
                }
            }
        }

        // Resolve the new profile's limits (registry + user overrides).
        let new_limits = agent_config::resolve_limits(&profile_def);
        let limit_source_str = match new_limits.source {
            agent_models::LimitSource::UserConfig => "user_config",
            agent_models::LimitSource::BuiltinRegistry => "builtin_registry",
            agent_models::LimitSource::RuntimeProbe => "runtime_probe",
            agent_models::LimitSource::Fallback => "fallback",
        };

        let event = agent_core::DomainEvent::new(
            workspace_id,
            session_id.clone(),
            agent_core::AgentId::system(),
            agent_core::PrivacyClassification::MinimalTrace,
            agent_core::EventPayload::ModelProfileSwitched {
                from_profile,
                to_profile: profile_alias.clone(),
                effective_at: chrono::Utc::now(),
                context_window: new_limits.context_window,
                output_limit: new_limits.output_limit,
                limit_source: limit_source_str.into(),
            },
        );
        crate::event_emitter::append_and_broadcast(&*self.store, &self.event_tx, &event).await?;

        // Refresh cached limits so the next send_message's agent loop
        // doesn't re-derive from the old profile.
        self.set_session_limits(&session_id, new_limits.clone())
            .await;

        // Ollama probe for the new profile (fire-and-forget, 3s timeout) —
        // mirrors the probe spawned by start_session around line 515.
        if profile_def.provider == "ollama" {
            if let Some(client) = self.ollama_clients.get(&profile_alias).cloned() {
                let model_id = profile_def.model_id.clone();
                let session_id_for_probe = session_id.clone();
                let session_states = self.session_states.clone();
                tokio::spawn(async move {
                    let probe = tokio::time::timeout(
                        std::time::Duration::from_secs(3),
                        client.probe_context_window(&model_id),
                    )
                    .await;
                    if let Ok(Some(window)) = probe {
                        let mut states = session_states.lock().await;
                        if let Some(entry) = states.get_mut(session_id_for_probe.as_str()) {
                            if let Some(ref mut l) = entry.model_limits {
                                l.context_window = window;
                                l.source = agent_models::LimitSource::RuntimeProbe;
                            }
                        }
                    }
                });
            }
        }

        Ok(())
    }

    /// Rebuild the skill catalog aggregate from `skill_sources.toml` and
    /// re-create providers. Called after every toml mutation so the runtime
    /// always reflects the latest persisted configuration.
    pub(crate) fn rebuild_skill_aggregate(&self) -> agent_core::Result<()> {
        let Some(toml) = &self.skill_sources_toml else {
            return Ok(());
        };
        let http = self.skill_catalog_http.clone().ok_or_else(|| {
            agent_core::CoreError::InvalidState("skill catalog http not configured".into())
        })?;
        let sources = toml.merge_with_defaults(&toml.read());
        let providers: Vec<(u32, Arc<dyn SkillCatalogProvider>)> = sources
            .into_iter()
            .filter(|s| s.enabled)
            .filter_map(|s| {
                let kind = SkillSourceKind::from_str(&s.kind)?;
                let cfg = RemoteSkillSourceConfig {
                    id: s.id.clone(),
                    display_name: s.display_name.clone(),
                    kind,
                    url: s.url.clone(),
                    search_template: s.search_template.clone(),
                    list_template: s.list_template.clone(),
                    enabled: s.enabled,
                    priority: s.priority,
                    cache_ttl_seconds: s.cache_ttl_seconds,
                };
                Some((s.priority, build_skill_provider(cfg, http.clone())))
            })
            .collect();
        if let Some(catalog) = self.skill_catalog.get() {
            catalog.reload(providers);
        } else {
            let agg = Arc::new(AggregateSkillCatalogProvider::new(providers));
            let _ = self.skill_catalog.set(agg);
        }
        Ok(())
    }

    /// Get (or lazily build) the skill catalog aggregate. Returns `None`
    /// only when the catalog has never been configured.
    pub(crate) fn ensure_skill_catalog(&self) -> Option<Arc<AggregateSkillCatalogProvider>> {
        if let Some(c) = self.skill_catalog.get() {
            return Some(c.clone());
        }
        let _ = self.rebuild_skill_aggregate();
        self.skill_catalog.get().cloned()
    }
}

#[async_trait]
impl<S, M> SessionFacade for LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    async fn open_workspace(&self, path: String) -> agent_core::Result<WorkspaceInfo> {
        crate::session::open_workspace(&*self.store, &self.event_tx, path).await
    }

    async fn start_session(&self, request: StartSessionRequest) -> agent_core::Result<SessionId> {
        let model_profile_alias = request.model_profile.clone();
        let session_id = crate::session::start_session(
            &*self.store,
            &self.event_tx,
            request.workspace_id,
            request.model_profile,
        )
        .await?;

        // Resolve initial limits from config + builtin registry. If the
        // session uses an Ollama profile and we have a typed client for it,
        // spawn a bounded probe to refine `context_window` from the live
        // server.
        let profile_def = self
            .config
            .profiles
            .iter()
            .find(|(alias, _)| alias == &model_profile_alias)
            .map(|(_, def)| def.clone());
        if let Some(def) = profile_def {
            let initial_limits = agent_config::resolve_limits(&def);
            self.set_session_limits(&session_id, initial_limits.clone())
                .await;

            if def.provider == "ollama" {
                if let Some(client) = self.ollama_clients.get(&model_profile_alias).cloned() {
                    let model_id = def.model_id.clone();
                    let session_id_for_probe = session_id.clone();
                    let session_states = self.session_states.clone();
                    tokio::spawn(async move {
                        let probe = tokio::time::timeout(
                            std::time::Duration::from_secs(3),
                            client.probe_context_window(&model_id),
                        )
                        .await;
                        if let Ok(Some(window)) = probe {
                            let mut states = session_states.lock().await;
                            if let Some(entry) = states.get_mut(session_id_for_probe.as_str()) {
                                if let Some(ref mut l) = entry.model_limits {
                                    l.context_window = window;
                                    l.source = agent_models::LimitSource::RuntimeProbe;
                                }
                            }
                        }
                    });
                }
            }
        }

        Ok(session_id)
    }

    async fn send_message(&self, request: SendMessageRequest) -> agent_core::Result<()> {
        // Reject sends while a compaction is in flight (P2 busy gate).
        // The state is cleared by `compaction::compact_session` on exit.
        {
            let states = self.session_states.lock().await;
            if let Some(entry) = states.get(&request.session_id.to_string()) {
                if entry.compacting {
                    return Err(agent_core::CoreError::SessionBusy {
                        session_id: request.session_id.to_string(),
                        reason: "context compaction in progress".into(),
                    });
                }
            }
        }

        if let Ok(repository) = self.project_repository() {
            if let Ok(Some(_binding)) = repository
                .get_session_binding(request.session_id.as_str())
                .await
            {
                let visibility = repository
                    .get_session_visibility(request.session_id.as_str())
                    .await
                    .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
                if visibility.as_deref() == Some("draft_hidden") {
                    self.mark_session_visible(&request.session_id, request.content.clone())
                        .await?;
                }
            }
        }

        match self.execution_mode(&request) {
            ExecutionMode::DagExecution => {
                let executor = self.dag_executor.as_ref().ok_or_else(|| {
                    agent_core::CoreError::InvalidState("DAG executor not available".into())
                })?;
                let result = executor.execute(&request, &self.task_graphs).await?;
                tracing::info!(
                    "DAG execution completed: {} tasks, {} completed, {} failed, {} skipped",
                    result.total_tasks,
                    result.completed,
                    result.failed,
                    result.skipped,
                );
                Ok(())
            }
            ExecutionMode::SingleStep => {
                let root_path = match self.project_repository() {
                    Ok(repo) => match repo.get_session_binding(request.session_id.as_str()).await {
                        Ok(Some(binding)) => repo
                            .get_project(&binding.project_id)
                            .await
                            .ok()
                            .map(|project| std::path::PathBuf::from(project.root_path)),
                        _ => None,
                    },
                    Err(_) => None,
                };

                crate::agent_loop::run_agent_loop(
                    crate::agent_loop::AgentLoopDeps {
                        store: &self.store,
                        model: &self.model,
                        event_tx: &self.event_tx,
                        tool_registry: &self.tool_registry,
                        permission_engine: &self.permission_engine,
                        pending_permissions: &self.pending_permissions,
                        memory_store: &self.memory_store,
                        task_graphs: &self.task_graphs,
                        active_cancellation: &self.active_cancellation,
                        config: &self.config,
                        session_states: &self.session_states,
                        skill_registry: &self.skill_registry,
                        active_skills: &self.active_skills,
                        root_path,
                    },
                    &request,
                )
                .await
            }
        }
    }

    async fn decide_permission(&self, decision: PermissionDecision) -> agent_core::Result<()> {
        let _ = decision;
        Ok(())
    }

    async fn cancel_session(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
    ) -> agent_core::Result<()> {
        crate::session::cancel_session(
            &*self.store,
            &self.event_tx,
            &self.active_cancellation,
            workspace_id,
            session_id,
        )
        .await
    }

    async fn get_session_projection(
        &self,
        session_id: SessionId,
    ) -> agent_core::Result<agent_core::projection::SessionProjection> {
        crate::session::get_session_projection(&*self.store, session_id).await
    }

    async fn get_trace(&self, session_id: SessionId) -> agent_core::Result<Vec<TraceEntry>> {
        crate::session::get_trace(&*self.store, session_id).await
    }

    fn subscribe_session(&self, session_id: SessionId) -> BoxStream<'static, DomainEvent> {
        crate::session::subscribe_session(&self.event_tx, session_id)
    }

    fn subscribe_all(&self) -> BoxStream<'static, DomainEvent> {
        crate::session::subscribe_all(&self.event_tx)
    }

    async fn list_workspaces(&self) -> agent_core::Result<Vec<WorkspaceInfo>> {
        crate::session::list_workspaces(&*self.store).await
    }

    async fn list_sessions(
        &self,
        workspace_id: &WorkspaceId,
    ) -> agent_core::Result<Vec<agent_core::SessionMeta>> {
        crate::session::list_sessions(&*self.store, workspace_id).await
    }

    async fn rename_session(
        &self,
        session_id: &SessionId,
        title: String,
    ) -> agent_core::Result<()> {
        crate::session::rename_session(&*self.store, session_id, title).await
    }

    async fn soft_delete_session(&self, session_id: &SessionId) -> agent_core::Result<()> {
        crate::session::soft_delete_session(&*self.store, session_id).await
    }

    async fn permanently_delete_session(&self, session_id: &SessionId) -> agent_core::Result<()> {
        crate::session::permanently_delete_session(&*self.store, session_id.as_str()).await
    }

    async fn restore_archived_session(&self, session_id: &SessionId) -> agent_core::Result<()> {
        crate::session::restore_archived_session(&*self.store, session_id.as_str()).await
    }

    async fn cleanup_expired_sessions(
        &self,
        older_than: std::time::Duration,
    ) -> agent_core::Result<usize> {
        crate::session::cleanup_expired_sessions(&*self.store, older_than).await
    }

    async fn get_task_graph(
        &self,
        session_id: SessionId,
    ) -> agent_core::Result<agent_core::TaskGraphSnapshot> {
        crate::session::get_task_graph(&self.task_graphs, session_id).await
    }

    async fn retry_task(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
        task_id: TaskId,
    ) -> agent_core::Result<()> {
        if let Some(executor) = &self.dag_executor {
            let mut graphs = self.task_graphs.lock().await;
            let graph = graphs.get_mut(&session_id.to_string()).ok_or_else(|| {
                agent_core::CoreError::InvalidState(format!(
                    "No task graph found for session {}",
                    session_id
                ))
            })?;
            executor
                .retry_task(&workspace_id, &session_id, graph, &task_id)
                .await
        } else {
            Err(agent_core::CoreError::InvalidState(
                "DAG executor not available".into(),
            ))
        }
    }

    async fn cancel_task(
        &self,
        workspace_id: WorkspaceId,
        session_id: SessionId,
        task_id: TaskId,
    ) -> agent_core::Result<()> {
        if let Some(executor) = &self.dag_executor {
            let mut graphs = self.task_graphs.lock().await;
            let graph = graphs.get_mut(&session_id.to_string()).ok_or_else(|| {
                agent_core::CoreError::InvalidState(format!(
                    "No task graph found for session {}",
                    session_id
                ))
            })?;
            executor
                .cancel_task(&workspace_id, &session_id, graph, &task_id)
                .await
        } else {
            Err(agent_core::CoreError::InvalidState(
                "DAG executor not available".into(),
            ))
        }
    }

    async fn get_agent_status(
        &self,
        session_id: SessionId,
    ) -> agent_core::Result<Vec<AgentStatusInfo>> {
        let graphs = self.task_graphs.lock().await;
        match graphs.get(&session_id.to_string()) {
            Some(graph) => {
                if let Some(executor) = &self.dag_executor {
                    let statuses = executor.get_agent_status(graph);
                    Ok(statuses
                        .into_iter()
                        .map(|s| AgentStatusInfo {
                            agent_id: s.agent_id,
                            role: s.role,
                            task_id: s.task_id,
                            status: s.status,
                        })
                        .collect())
                } else {
                    Ok(Vec::new())
                }
            }
            None => Ok(Vec::new()),
        }
    }
}

impl<S, M> AppFacade for LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
}

pub(crate) fn parse_trust_str(s: &str) -> TrustLevel {
    match s {
        "verified" => TrustLevel::Verified,
        "unverified" => TrustLevel::Unverified,
        _ => TrustLevel::Community,
    }
}

/// Build the aggregate catalog provider: builtin (priority 0) plus every
/// enabled remote source. Wires a [`CatalogEventSink`] for failure
/// observability.
fn build_catalog_provider(
    sources: &[agent_config::CatalogSourceConfig],
    cache_dir: PathBuf,
    event_tx: tokio::sync::broadcast::Sender<DomainEvent>,
) -> anyhow::Result<AggregateCatalogProvider> {
    let http = SharedHttpClient::new()?;
    let cache = Arc::new(HttpResponseCache::new(cache_dir));

    let mut providers: Vec<(u32, Arc<dyn CatalogProvider>)> = Vec::new();
    let builtin = Arc::new(BuiltinCatalogProvider::new()?);
    providers.push((0, builtin));

    for s in sources.iter().filter(|s| s.enabled) {
        let cfg = RemoteSourceConfig {
            id: s.id.clone(),
            display_name: s.display_name.clone(),
            kind: match s.kind {
                agent_config::CatalogSourceKind::McpRegistry => RemoteSourceKind::McpRegistry,
            },
            url: s.url.clone(),
            api_key_env: s.api_key_env.clone(),
            priority: s.priority,
            default_trust: parse_trust_str(&s.default_trust),
            enabled: true,
            cache_ttl_seconds: s.cache_ttl_seconds,
        };
        let provider = build_remote_catalog_provider(cfg, http.clone(), cache.clone());
        providers.push((s.priority, provider));
    }

    let sink: Arc<dyn agent_mcp::DomainEventSink> = CatalogEventSink::new(event_tx);
    Ok(AggregateCatalogProvider::new_with_priority(
        providers,
        Some(sink),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
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
        let runtime = LocalRuntime::new(store, model).with_dag_execution();

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
            supports_reasoning: None,
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
            supports_reasoning: None,
            extra_params: None,
            enabled: true,
        };
        Arc::new(agent_config::Config {
            profiles: vec![("fast".into(), fast), ("opus".into(), opus)],
            mcp_servers: vec![],
            source: ConfigSource::Defaults,
            context: ContextPolicy::default(),
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
            },
        )
        .await
        .unwrap();

        runtime
            .switch_model(session_id.clone(), "opus".into())
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
            },
        )
        .await
        .unwrap();

        let result = runtime.switch_model(session_id, "nonexistent".into()).await;
        assert!(matches!(
            result,
            Err(agent_core::CoreError::InvalidState(ref msg)) if msg.contains("nonexistent")
        ));
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
            },
        )
        .await
        .unwrap();

        runtime
            .switch_model(session_id.clone(), "fast".into())
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
            .switch_model(session_id.clone(), "opus".into())
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
        let runtime = LocalRuntime::new(store, model).with_dag_execution();

        let request = SendMessageRequest {
            workspace_id: WorkspaceId::new(),
            session_id: SessionId::new(),
            content: "just a question".into(),
            attachments: vec![],
        };
        assert_eq!(runtime.execution_mode(&request), ExecutionMode::SingleStep);
    }
}
