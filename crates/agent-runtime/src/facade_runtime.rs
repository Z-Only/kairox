use crate::dag_executor::{DagConfig, DagExecutor};
use crate::skill_package::{NpxSkillsPackageManager, SkillPackageManager};
use crate::skills::{
    skill_document_to_detail, skill_metadata_to_active_view, skill_metadata_to_view,
};
use crate::task_graph::TaskGraph;
use crate::McpServerManager;
use agent_core::facade::{
    InstallGithubSkillRequest, InstallRemoteSkillRequest, McpServerSettingsInput,
    McpServerSettingsView, RemoteSkillSearchResult, SkillSettingsDetail, SkillSettingsView,
};
use agent_core::{
    ActivateSkillRequest, ActiveSkillView, AddCatalogSourceRequest, AgentId, AgentStatusInfo,
    AppFacade, CatalogQuery as CoreCatalogQuery, CatalogSourceView, DeactivateSkillRequest,
    DomainEvent, EventPayload, InstallOutcomeView as CoreInstallOutcomeView,
    InstallRequest as CoreInstallRequest, InstalledEntry as CoreInstalledEntry, PermissionDecision,
    PrivacyClassification, ProjectGitStatus, ProjectId, ProjectInstructionSummary, ProjectMeta,
    ProjectSessionVisibility, SendMessageRequest, ServerEntry as CoreServerEntry, SessionId,
    SessionMeta, SkillDetail, SkillView, StartSessionRequest, TaskId, TraceEntry, WorkspaceId,
    WorkspaceInfo,
};
use agent_mcp::catalog::{
    AggregateCatalogProvider, BuiltinCatalogProvider, CatalogProvider, CatalogQuery,
    InstallRequest as McpInstallRequest, InstallSpec, ServerEntry, TrustLevel,
};
use agent_mcp::{
    build_remote_catalog_provider, HttpResponseCache, RemoteSourceConfig, RemoteSourceKind,
    SharedHttpClient,
};

use crate::catalog_sink::CatalogEventSink;
use agent_mcp::installer::{InstallOutcomeView, Installer, OsRuntimeProbe};
use agent_mcp::types::{McpServerDef, McpTransportDef};
use agent_memory::{ContextAssembler, MemoryStore};
use agent_store::{EventStore, ProjectMetaRepository};
use agent_tools::{BuiltinProvider, PermissionEngine, PermissionMode, ToolProvider, ToolRegistry};
use async_trait::async_trait;
use futures::stream::BoxStream;
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{Arc, OnceLock};
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
    store: Arc<S>,
    model: Arc<M>,
    permission_engine: Arc<Mutex<PermissionEngine>>,
    mcp_manager: Option<Arc<Mutex<McpServerManager>>>,
    tool_registry: Arc<Mutex<ToolRegistry>>,
    context_assembler: ContextAssembler,
    memory_store: Option<Arc<dyn MemoryStore>>,
    pending_permissions:
        Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<PermissionDecision>>>>,
    event_tx: tokio::sync::broadcast::Sender<DomainEvent>,
    task_graphs: Arc<Mutex<HashMap<String, TaskGraph>>>,
    active_cancellation: Arc<Mutex<Option<CancellationToken>>>,
    dag_executor: Option<Arc<DagExecutor<S, M>>>,
    dag_config: DagConfig,
    /// Catalog provider (built-in + future remote sources). `None` when the
    /// marketplace has not been wired via [`Self::with_marketplace`].
    catalog: Option<Arc<dyn CatalogProvider>>,
    /// Installer for marketplace entries. `None` when the marketplace has not
    /// been wired via [`Self::with_marketplace`].
    installer: Option<Arc<Installer>>,
    /// Phase 2: directory containing `mcp_servers.toml` (used for atomic
    /// catalog source mutations + reloads). `None` when no marketplace has
    /// been wired.
    marketplace_dir: Option<PathBuf>,
    /// Phase 2: concrete handle to the aggregate provider for `reload`
    /// after toml mutations. `None` when no marketplace has been wired.
    aggregate_handle: Option<Arc<AggregateCatalogProvider>>,
    /// Phase 2: shared HTTP client + cache for remote catalog providers.
    catalog_http: Option<SharedHttpClient>,
    catalog_cache: Option<Arc<HttpResponseCache>>,
    skill_registry: Option<Arc<dyn agent_skills::SkillRegistry>>,
    skill_settings_roots: crate::skill_settings::SkillSettingsRoots,
    skill_package_manager: Arc<dyn SkillPackageManager>,
    active_skills: Arc<Mutex<HashMap<String, Vec<String>>>>,
    /// Per-session in-memory state. Inserted lazily on first access.
    session_states: Arc<Mutex<HashMap<String, crate::session::SessionState>>>,
    /// Loaded TOML config (`Config::load()` in production, in-line in tests).
    /// Required by Tasks 9-10 to look up `ProfileDef` by alias and call
    /// `agent_config::resolve_limits`.
    config: Arc<agent_config::Config>,
    /// Profile-alias → typed Ollama client. Populated by `with_ollama_clients`
    /// at wiring time so Task 10 can fire `probe_context_window`. Empty when
    /// no Ollama profiles are configured.
    ollama_clients: HashMap<String, Arc<agent_models::OllamaClient>>,
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
            skill_package_manager: Arc::new(NpxSkillsPackageManager),
            active_skills: Arc::new(Mutex::new(HashMap::new())),
            session_states: Arc::new(Mutex::new(HashMap::new())),
            config: Arc::new(agent_config::Config {
                profiles: vec![],
                mcp_servers: vec![],
                source: agent_config::ConfigSource::Defaults,
                context: agent_config::ContextPolicy::default(),
            }),
            ollama_clients: HashMap::new(),
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

    fn skill_settings_roots(&self) -> crate::skill_settings::SkillSettingsRoots {
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

    fn project_repository(&self) -> agent_core::Result<ProjectMetaRepository> {
        self.store
            .sqlite_pool()
            .map(ProjectMetaRepository::new)
            .ok_or_else(crate::project::invalid_project_store_error)
    }

    pub async fn mark_session_visible(
        &self,
        session_id: &SessionId,
        first_message: String,
    ) -> agent_core::Result<()> {
        let repository = self.project_repository()?;
        let draft_hidden =
            crate::project::visibility_to_storage(ProjectSessionVisibility::DraftHidden);
        let binding = repository
            .get_session_binding(session_id.as_str())
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        let visibility = repository
            .get_session_visibility(session_id.as_str())
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        if binding.is_none() || visibility.as_deref() != Some(draft_hidden) {
            return Err(agent_core::CoreError::InvalidState(
                "only draft_hidden project sessions can be marked visible".into(),
            ));
        }

        repository
            .set_session_visibility(
                session_id.as_str(),
                crate::project::visibility_to_storage(ProjectSessionVisibility::Visible),
            )
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        let title = crate::session::temporary_title_from_first_message(&first_message);
        self.store
            .rename_session(session_id.as_str(), &title)
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))
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
    fn execution_mode(&self, request: &SendMessageRequest) -> ExecutionMode {
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
            .find(|(alias, _)| alias == &profile_alias)
            .map(|(_, def)| def.clone())
            .ok_or_else(|| {
                agent_core::CoreError::InvalidState(format!(
                    "unknown model profile: {profile_alias}"
                ))
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

    /// Phase 2: rebuild the aggregate's remote provider list from
    /// `<marketplace_dir>/mcp_servers.toml`, calling
    /// [`AggregateCatalogProvider::reload`]. The builtin provider is
    /// always re-added at priority 0.
    async fn rebuild_aggregate_from_disk(&self) -> agent_core::Result<()> {
        let marketplace_dir = self.marketplace_dir.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState("marketplace not configured".into())
        })?;
        let aggregate = self.aggregate_handle.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState("marketplace not configured".into())
        })?;
        let http = self.catalog_http.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState("marketplace not configured".into())
        })?;
        let cache = self.catalog_cache.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState("marketplace not configured".into())
        })?;

        let mt = crate::marketplace_toml::MarketplaceToml::new(marketplace_dir);
        let user_sources = mt
            .read_sources()
            .map_err(|e| agent_core::CoreError::InvalidState(format!("marketplace toml: {e}")))?;
        // Merge in the shipped defaults so that a user who toggled a
        // default to enabled (without ever adding a custom override) still
        // gets a real remote provider built. Defaults whose enabled flag
        // remains false are filtered out below — the merge is purely a
        // "make the candidate set complete" step, not auto-fetching.
        let sources = agent_config::merge_with_defaults(user_sources);

        let mut providers: Vec<(u32, Arc<dyn CatalogProvider>)> = Vec::new();
        let builtin =
            Arc::new(BuiltinCatalogProvider::new().map_err(|e| {
                agent_core::CoreError::InvalidState(format!("builtin catalog: {e}"))
            })?);
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
            providers.push((
                s.priority,
                build_remote_catalog_provider(cfg, http.clone(), cache.clone()),
            ));
        }
        aggregate.reload(providers);
        Ok(())
    }
}

#[async_trait]
impl<S, M> AppFacade for LocalRuntime<S, M>
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

    async fn list_skills(&self) -> agent_core::Result<Vec<SkillView>> {
        let Some(registry) = &self.skill_registry else {
            return Ok(Vec::new());
        };
        Ok(registry.list().iter().map(skill_metadata_to_view).collect())
    }

    async fn get_skill(&self, skill_id: String) -> agent_core::Result<Option<SkillDetail>> {
        let Some(registry) = &self.skill_registry else {
            return Ok(None);
        };
        let skill_id = agent_skills::SkillId::new(skill_id);
        if registry.get(&skill_id).is_none() {
            return Ok(None);
        }
        let document = registry
            .load_document(&skill_id)
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        Ok(Some(skill_document_to_detail(document)))
    }

    async fn activate_skill(
        &self,
        request: ActivateSkillRequest,
    ) -> agent_core::Result<ActiveSkillView> {
        let registry = self.skill_registry.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState("skill registry not configured".into())
        })?;
        let skill_id = agent_skills::SkillId::new(request.skill_id.clone());
        let metadata = registry.get(&skill_id).ok_or_else(|| {
            agent_core::CoreError::InvalidState(format!("skill not found: {}", request.skill_id))
        })?;
        let active_view = skill_metadata_to_active_view(&metadata);

        let activated = {
            let mut active_skills = self.active_skills.lock().await;
            let session_skills = active_skills
                .entry(request.session_id.to_string())
                .or_insert_with(Vec::new);
            if session_skills.iter().any(|id| id == &request.skill_id) {
                false
            } else {
                session_skills.push(request.skill_id.clone());
                true
            }
        };

        if activated {
            let event = DomainEvent::new(
                request.workspace_id,
                request.session_id,
                AgentId::system(),
                PrivacyClassification::MinimalTrace,
                EventPayload::SkillActivated {
                    skill_id: active_view.skill_id.clone(),
                    name: active_view.name.clone(),
                    source: active_view.source.clone(),
                    activation_mode: active_view.activation_mode.clone(),
                },
            );
            crate::event_emitter::append_and_broadcast(&*self.store, &self.event_tx, &event)
                .await?;
        }

        Ok(active_view)
    }

    async fn deactivate_skill(&self, request: DeactivateSkillRequest) -> agent_core::Result<()> {
        let registry = self.skill_registry.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState("skill registry not configured".into())
        })?;
        let skill_id = agent_skills::SkillId::new(request.skill_id.clone());
        let metadata = registry.get(&skill_id).ok_or_else(|| {
            agent_core::CoreError::InvalidState(format!("skill not found: {}", request.skill_id))
        })?;
        let active_view = skill_metadata_to_active_view(&metadata);

        let removed = {
            let mut active_skills = self.active_skills.lock().await;
            let Some(session_skills) = active_skills.get_mut(&request.session_id.to_string())
            else {
                return Ok(());
            };
            let original_len = session_skills.len();
            session_skills.retain(|id| id != &request.skill_id);
            session_skills.len() != original_len
        };
        if !removed {
            return Ok(());
        }

        let event = DomainEvent::new(
            request.workspace_id,
            request.session_id,
            AgentId::system(),
            PrivacyClassification::MinimalTrace,
            EventPayload::SkillDeactivated {
                skill_id: active_view.skill_id,
                name: active_view.name,
                source: active_view.source,
            },
        );
        crate::event_emitter::append_and_broadcast(&*self.store, &self.event_tx, &event).await
    }

    async fn list_active_skills(
        &self,
        session_id: SessionId,
    ) -> agent_core::Result<Vec<ActiveSkillView>> {
        let Some(registry) = &self.skill_registry else {
            return Ok(Vec::new());
        };
        let skill_ids = {
            let active_skills = self.active_skills.lock().await;
            active_skills
                .get(&session_id.to_string())
                .cloned()
                .unwrap_or_default()
        };
        Ok(skill_ids
            .into_iter()
            .filter_map(|skill_id| registry.get(&agent_skills::SkillId::new(skill_id)))
            .map(|metadata| skill_metadata_to_active_view(&metadata))
            .collect())
    }

    async fn list_mcp_server_settings(&self) -> agent_core::Result<Vec<McpServerSettingsView>> {
        let config_path =
            crate::mcp_settings::writable_mcp_config_path(self.marketplace_dir.as_deref())?;
        crate::mcp_settings::list_mcp_server_settings(
            &self.config,
            config_path.as_deref(),
            self.mcp_manager(),
        )
        .await
    }

    async fn upsert_mcp_server_settings(
        &self,
        input: McpServerSettingsInput,
    ) -> agent_core::Result<McpServerSettingsView> {
        let config_path =
            crate::mcp_settings::writable_mcp_config_path(self.marketplace_dir.as_deref())?
                .ok_or_else(|| {
                    agent_core::CoreError::InvalidState(
                        "marketplace install dir not configured; cannot write MCP settings".into(),
                    )
                })?;
        crate::mcp_settings::upsert_mcp_server_settings(&config_path, input).await
    }

    async fn delete_mcp_server_settings(&self, server_id: String) -> agent_core::Result<()> {
        let config_path =
            crate::mcp_settings::writable_mcp_config_path(self.marketplace_dir.as_deref())?
                .ok_or_else(|| {
                    agent_core::CoreError::InvalidState(
                        "marketplace install dir not configured; cannot write MCP settings".into(),
                    )
                })?;
        crate::mcp_settings::delete_mcp_server_settings(
            &config_path,
            self.mcp_manager(),
            &server_id,
        )
        .await
    }

    async fn set_mcp_server_enabled(
        &self,
        server_id: String,
        enabled: bool,
    ) -> agent_core::Result<()> {
        let config_path =
            crate::mcp_settings::writable_mcp_config_path(self.marketplace_dir.as_deref())?
                .ok_or_else(|| {
                    agent_core::CoreError::InvalidState(
                        "marketplace install dir not configured; cannot write MCP settings".into(),
                    )
                })?;
        crate::mcp_settings::set_mcp_server_enabled(
            &config_path,
            self.mcp_manager(),
            &server_id,
            enabled,
        )
        .await
    }

    async fn open_mcp_config_file(&self) -> agent_core::Result<Option<String>> {
        Ok(
            crate::mcp_settings::writable_mcp_config_path(self.marketplace_dir.as_deref())?
                .map(|path| path.display().to_string()),
        )
    }

    async fn list_skill_settings(&self) -> agent_core::Result<Vec<SkillSettingsView>> {
        crate::skill_settings::list_skill_settings(self.skill_settings_roots()).await
    }

    async fn get_skill_settings_detail(
        &self,
        skill_id: String,
    ) -> agent_core::Result<Option<SkillSettingsDetail>> {
        crate::skill_settings::get_skill_settings_detail(self.skill_settings_roots(), &skill_id)
            .await
    }

    async fn set_skill_enabled(&self, skill_id: String, enabled: bool) -> agent_core::Result<()> {
        crate::skill_settings::set_skill_enabled(self.skill_settings_roots(), &skill_id, enabled)
            .await
    }

    async fn delete_skill_settings(&self, skill_id: String) -> agent_core::Result<()> {
        crate::skill_settings::delete_skill(self.skill_settings_roots(), &skill_id).await
    }

    async fn search_remote_skills(
        &self,
        query: String,
    ) -> agent_core::Result<Vec<RemoteSkillSearchResult>> {
        self.skill_package_manager.search(&query).await
    }

    async fn install_remote_skill(
        &self,
        request: InstallRemoteSkillRequest,
    ) -> agent_core::Result<SkillSettingsView> {
        crate::skill_settings::install_remote_skill(
            self.skill_settings_roots(),
            self.skill_package_manager.as_ref(),
            request,
        )
        .await
    }

    async fn install_github_skill(
        &self,
        request: InstallGithubSkillRequest,
    ) -> agent_core::Result<SkillSettingsView> {
        crate::skill_settings::install_github_skill(
            self.skill_settings_roots(),
            self.skill_package_manager.as_ref(),
            request,
        )
        .await
    }

    async fn update_skill(&self, skill_id: String) -> agent_core::Result<SkillSettingsView> {
        crate::skill_settings::update_skill(
            self.skill_settings_roots(),
            self.skill_package_manager.as_ref(),
            &skill_id,
        )
        .await
    }

    // -----------------------------------------------------------------------
    // Marketplace catalog
    // -----------------------------------------------------------------------
    async fn list_catalog(
        &self,
        query: CoreCatalogQuery,
    ) -> agent_core::Result<Vec<CoreServerEntry>> {
        let inner_query = map_query(query);
        let entries = match self.catalog.as_ref() {
            Some(catalog) => catalog
                .list(&inner_query)
                .await
                .map_err(|e| agent_core::CoreError::InvalidState(format!("catalog list: {e}")))?,
            None => {
                // Marketplace not configured: degrade to a builtin-only
                // aggregator so the GUI can still render its catalog out of
                // the box. See `catalog_resilience` integration tests.
                let builtin = builtin_only_provider()?;
                builtin.list(&inner_query).await.map_err(|e| {
                    agent_core::CoreError::InvalidState(format!("catalog list: {e}"))
                })?
            }
        };
        Ok(entries.into_iter().map(map_entry_to_core).collect())
    }

    async fn get_catalog_entry(
        &self,
        id: String,
        _source: Option<String>,
    ) -> agent_core::Result<Option<CoreServerEntry>> {
        let entry =
            match self.catalog.as_ref() {
                Some(catalog) => catalog.get(&id).await.map_err(|e| {
                    agent_core::CoreError::InvalidState(format!("catalog get: {e}"))
                })?,
                None => {
                    // Marketplace not configured: degrade to builtin-only lookup.
                    let builtin = builtin_only_provider()?;
                    builtin.get(&id).await.map_err(|e| {
                        agent_core::CoreError::InvalidState(format!("catalog get: {e}"))
                    })?
                }
            };
        Ok(entry.map(map_entry_to_core))
    }

    async fn refresh_catalog(&self, _source: Option<String>) -> agent_core::Result<()> {
        let Some(catalog) = self.catalog.as_ref() else {
            // No remote sources to refresh — noop. The builtin catalog is
            // statically compiled so there is nothing to fetch.
            return Ok(());
        };

        // Rebuild the aggregate from disk before refreshing so that sources
        // configured in `mcp_servers.toml` (but not present at startup
        // because `with_marketplace` passes `&[]`) are loaded into the
        // aggregate. Without this, only the builtin provider is refreshed
        // and remote entries never appear.
        if self.marketplace_dir.is_some() {
            self.rebuild_aggregate_from_disk().await?;
        }

        catalog
            .refresh()
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(format!("catalog refresh: {e}")))?;
        let entry_count = catalog
            .list(&CatalogQuery::default())
            .await
            .map(|v| v.len())
            .unwrap_or(0);
        emit_marketplace_event(
            &self.event_tx,
            EventPayload::CatalogRefreshed {
                source: "aggregate".into(),
                entry_count,
            },
        );
        Ok(())
    }

    async fn install_catalog_entry(
        &self,
        request: CoreInstallRequest,
    ) -> agent_core::Result<CoreInstallOutcomeView> {
        // Install genuinely needs disk + catalog state to write to. If the
        // marketplace was never wired, fail with a clearer message instead
        // of the generic "marketplace not configured".
        let catalog = self.catalog.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState(
                "marketplace install dir not configured; cannot install".into(),
            )
        })?;
        let installer = self.installer.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState(
                "marketplace install dir not configured; cannot install".into(),
            )
        })?;

        let inner_req = map_install_request(request);
        let entry = catalog
            .get(&inner_req.catalog_id)
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(format!("catalog: {e}")))?
            .ok_or_else(|| {
                agent_core::CoreError::InvalidState(format!(
                    "entry not found: {}",
                    inner_req.catalog_id
                ))
            })?;

        emit_marketplace_event(
            &self.event_tx,
            EventPayload::CatalogEntryInstalling {
                catalog_id: inner_req.catalog_id.clone(),
                source: inner_req.source.clone(),
            },
        );

        let outcome = installer
            .install(&entry, &inner_req)
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(format!("installer: {e}")))?;

        match &outcome {
            InstallOutcomeView::RuntimeMissing { missing } => {
                emit_marketplace_event(
                    &self.event_tx,
                    EventPayload::CatalogRuntimeMissing {
                        catalog_id: inner_req.catalog_id.clone(),
                        missing: missing.iter().map(|r| r.kind.as_str().into()).collect(),
                    },
                );
            }
            InstallOutcomeView::Installed { server_id, started } => {
                if let Some(manager) = &self.mcp_manager {
                    let def = build_server_def(&entry, &inner_req);
                    let mut mgr = manager.lock().await;
                    if !mgr.is_registered(server_id) {
                        if let Err(e) = mgr.register_dynamic(def) {
                            tracing::warn!(
                                "marketplace install: register_dynamic({server_id}) failed: {e}"
                            );
                        }
                    }
                    if *started {
                        if let Err(e) = mgr.ensure_server(server_id).await {
                            tracing::warn!(
                                "marketplace install: ensure_server({server_id}) failed: {e}"
                            );
                        }
                    }
                }
                emit_marketplace_event(
                    &self.event_tx,
                    EventPayload::CatalogEntryInstalled {
                        catalog_id: inner_req.catalog_id.clone(),
                        source: inner_req.source.clone(),
                        server_id: server_id.clone(),
                    },
                );
            }
            _ => {}
        }
        Ok(map_outcome_to_core(outcome))
    }

    async fn uninstall_catalog_entry(&self, server_id: String) -> agent_core::Result<()> {
        let installer = self.installer.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState(
                "marketplace install dir not configured; cannot uninstall".into(),
            )
        })?;
        installer
            .uninstall(&server_id)
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(format!("installer: {e}")))?;
        if let Some(manager) = &self.mcp_manager {
            if let Err(e) = manager.lock().await.unregister_dynamic(&server_id).await {
                tracing::warn!(
                    "marketplace uninstall: unregister_dynamic({server_id}) failed: {e}"
                );
            }
        }
        emit_marketplace_event(
            &self.event_tx,
            EventPayload::CatalogEntryUninstalled {
                server_id: server_id.clone(),
            },
        );
        Ok(())
    }

    async fn list_installed_entries(&self) -> agent_core::Result<Vec<CoreInstalledEntry>> {
        let Some(installer) = self.installer.as_ref() else {
            // No installer wired (marketplace unconfigured) → nothing can
            // possibly be installed. Return empty rather than erroring so
            // the GUI's "Installed" tab renders as empty state.
            return Ok(Vec::new());
        };
        let ids = installer
            .list_installed_ids()
            .map_err(|e| agent_core::CoreError::InvalidState(format!("installer: {e}")))?;

        // Best-effort: enrich each id with catalog metadata + running status.
        let mut out = Vec::with_capacity(ids.len());
        for id in ids {
            let entry = if let Some(c) = &self.catalog {
                c.get(&id).await.ok().flatten()
            } else {
                None
            };
            let running = if let Some(manager) = &self.mcp_manager {
                manager.lock().await.is_running(&id).unwrap_or(false)
            } else {
                false
            };
            let display_name = entry
                .as_ref()
                .map(|e| e.display_name.clone())
                .unwrap_or_else(|| id.clone());
            out.push(CoreInstalledEntry {
                server_id: id,
                catalog_id: entry.as_ref().map(|e| e.id.clone()),
                source: entry.as_ref().map(|e| e.source.clone()),
                display_name,
                installed_at: chrono::Utc::now().to_rfc3339(),
                running,
            });
        }
        Ok(out)
    }

    // -----------------------------------------------------------------------
    // Phase 2: catalog source mutations
    // -----------------------------------------------------------------------

    async fn list_catalog_sources(&self) -> agent_core::Result<Vec<CatalogSourceView>> {
        // The implicit builtin source is always present, even when no
        // marketplace dir has been configured (GUI cold-start with no
        // [mcp_marketplace] section in kairox.toml).
        let builtin_view = builtin_source_view();

        // Even before any marketplace dir is wired, surface the shipped
        // default remote sources so the GUI marketplace tab has visible
        // subscriptions out of the box. All defaults are enabled=false,
        // so this is purely informational until the user opts in.
        let user_sources = match self.marketplace_dir.as_ref() {
            Some(dir) => {
                let mt = crate::marketplace_toml::MarketplaceToml::new(dir);
                mt.read_sources().map_err(|e| {
                    agent_core::CoreError::InvalidState(format!("marketplace toml: {e}"))
                })?
            }
            None => Vec::new(),
        };
        let merged = agent_config::merge_with_defaults(user_sources);

        let mut out = Vec::with_capacity(merged.len() + 1);
        out.push(builtin_view);
        for s in merged {
            out.push(catalog_source_to_view(s));
        }
        Ok(out)
    }

    async fn add_catalog_source(&self, request: AddCatalogSourceRequest) -> agent_core::Result<()> {
        let marketplace_dir = self.marketplace_dir.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState(
                "catalog source registry not initialized; cannot modify sources".into(),
            )
        })?;
        let cfg = request_to_source_config(request)?;
        let id = cfg.id.clone();
        let mt = crate::marketplace_toml::MarketplaceToml::new(marketplace_dir);
        mt.add_source(cfg)
            .map_err(|e| agent_core::CoreError::InvalidState(format!("marketplace toml: {e}")))?;
        self.rebuild_aggregate_from_disk().await?;
        emit_marketplace_event(
            &self.event_tx,
            EventPayload::CatalogSourceAdded { source: id },
        );
        Ok(())
    }

    async fn remove_catalog_source(&self, id: String) -> agent_core::Result<()> {
        if id == "builtin" {
            return Ok(());
        }
        let marketplace_dir = self.marketplace_dir.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState(
                "catalog source registry not initialized; cannot modify sources".into(),
            )
        })?;
        let mt = crate::marketplace_toml::MarketplaceToml::new(marketplace_dir);
        mt.remove_source(&id)
            .map_err(|e| agent_core::CoreError::InvalidState(format!("marketplace toml: {e}")))?;
        self.rebuild_aggregate_from_disk().await
    }

    async fn set_catalog_source_enabled(
        &self,
        id: String,
        enabled: bool,
    ) -> agent_core::Result<()> {
        if id == "builtin" {
            return Ok(());
        }
        let marketplace_dir = self.marketplace_dir.as_ref().ok_or_else(|| {
            agent_core::CoreError::InvalidState(
                "catalog source registry not initialized; cannot modify sources".into(),
            )
        })?;
        let mt = crate::marketplace_toml::MarketplaceToml::new(marketplace_dir);
        mt.set_enabled(&id, enabled)
            .map_err(|e| agent_core::CoreError::InvalidState(format!("marketplace toml: {e}")))?;
        self.rebuild_aggregate_from_disk().await
    }

    async fn list_projects(
        &self,
        workspace_id: &WorkspaceId,
    ) -> agent_core::Result<Vec<ProjectMeta>> {
        let repository = self.project_repository()?;
        let rows = repository
            .list_active_projects(workspace_id.as_str())
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        Ok(rows
            .into_iter()
            .map(crate::project::project_row_to_meta)
            .collect())
    }

    async fn create_blank_project(
        &self,
        workspace_id: WorkspaceId,
        display_name: Option<String>,
    ) -> agent_core::Result<ProjectMeta> {
        let repository = self.project_repository()?;
        let display_name = display_name.unwrap_or_else(|| "New Project".into());
        let root_path = crate::project::unique_blank_project_path(&display_name);
        tokio::fs::create_dir_all(&root_path)
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        let root_path_string = root_path.display().to_string();
        let git_init_output = Command::new("git")
            .args(["-C", &root_path_string, "init"])
            .output()
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        if !git_init_output.status.success() {
            let stderr = String::from_utf8_lossy(&git_init_output.stderr)
                .trim()
                .to_string();
            let stdout = String::from_utf8_lossy(&git_init_output.stdout)
                .trim()
                .to_string();
            let message = if stderr.is_empty() { stdout } else { stderr };
            return Err(agent_core::CoreError::InvalidState(format!(
                "git init failed: {message}"
            )));
        }

        let project = repository
            .create_project(workspace_id.as_str(), &display_name, &root_path_string, 0)
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        Ok(crate::project::project_row_to_meta(project))
    }

    async fn add_existing_project(
        &self,
        workspace_id: WorkspaceId,
        path: String,
    ) -> agent_core::Result<ProjectMeta> {
        let repository = self.project_repository()?;
        let display_name = crate::project::display_name_from_path(&path);
        let project = repository
            .create_project(workspace_id.as_str(), &display_name, &path, 0)
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        Ok(crate::project::project_row_to_meta(project))
    }

    async fn rename_project(
        &self,
        project_id: ProjectId,
        display_name: String,
    ) -> agent_core::Result<()> {
        self.project_repository()?
            .rename_project(project_id.as_str(), &display_name)
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))
    }

    async fn remove_project(&self, project_id: ProjectId) -> agent_core::Result<()> {
        self.project_repository()?
            .remove_project(project_id.as_str())
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))
    }

    async fn restore_project_session(
        &self,
        session_id: SessionId,
    ) -> agent_core::Result<ProjectMeta> {
        let repository = self.project_repository()?;
        let binding = repository
            .get_session_binding(session_id.as_str())
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?
            .ok_or_else(|| {
                agent_core::CoreError::InvalidState("session is not bound to a project".into())
            })?;
        let project = repository
            .restore_project(&binding.project_id)
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        repository
            .set_session_visibility(session_id.as_str(), "visible")
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        Ok(crate::project::project_row_to_meta(project))
    }

    async fn update_project_order(&self, project_ids: Vec<ProjectId>) -> agent_core::Result<()> {
        let project_id_strings: Vec<String> = project_ids
            .into_iter()
            .map(|project_id| project_id.to_string())
            .collect();
        self.project_repository()?
            .update_project_order(&project_id_strings)
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))
    }

    async fn update_project_expanded(
        &self,
        project_id: ProjectId,
        expanded: bool,
    ) -> agent_core::Result<()> {
        self.project_repository()?
            .update_project_expanded(project_id.as_str(), expanded)
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))
    }

    async fn create_project_draft_session(
        &self,
        project_id: ProjectId,
    ) -> agent_core::Result<SessionId> {
        let repository = self.project_repository()?;
        let project = repository
            .get_project(project_id.as_str())
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        let session_id = crate::session::start_session(
            &*self.store,
            &self.event_tx,
            WorkspaceId::from_string(project.workspace_id.clone()),
            "default".into(),
        )
        .await?;
        repository
            .bind_session(session_id.as_str(), project_id.as_str(), &project.root_path)
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        repository
            .set_session_visibility(session_id.as_str(), "draft_hidden")
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        Ok(session_id)
    }

    async fn create_project_worktree_session(
        &self,
        project_id: ProjectId,
        branch_name: String,
    ) -> agent_core::Result<SessionId> {
        let repository = self.project_repository()?;
        let project = repository
            .get_project(project_id.as_str())
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        let session_id = crate::session::start_session(
            &*self.store,
            &self.event_tx,
            WorkspaceId::from_string(project.workspace_id.clone()),
            branch_name,
        )
        .await?;
        repository
            .bind_session(session_id.as_str(), project_id.as_str(), &project.root_path)
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        Ok(session_id)
    }

    async fn list_project_sessions(
        &self,
        project_id: ProjectId,
    ) -> agent_core::Result<Vec<SessionMeta>> {
        let _repository = self.project_repository()?;
        let rows = self
            .store
            .list_visible_project_sessions(project_id.as_str())
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        Ok(rows
            .into_iter()
            .map(crate::project::project_session_row_to_meta)
            .collect())
    }

    async fn list_archived_sessions(
        &self,
        workspace_id: &WorkspaceId,
    ) -> agent_core::Result<Vec<SessionMeta>> {
        let _repository = self.project_repository()?;
        let rows = self
            .store
            .list_archived_project_session_metas(workspace_id.as_str())
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        Ok(rows
            .into_iter()
            .map(crate::project::project_session_row_to_meta)
            .collect())
    }

    async fn get_project_git_status(
        &self,
        project_id: ProjectId,
    ) -> agent_core::Result<ProjectGitStatus> {
        let project = self
            .project_repository()?
            .get_project(project_id.as_str())
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        Ok(crate::project::get_git_status(&project.root_path))
    }

    async fn get_session_git_status(
        &self,
        session_id: SessionId,
    ) -> agent_core::Result<ProjectGitStatus> {
        let binding = self
            .project_repository()?
            .get_session_binding(session_id.as_str())
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?
            .ok_or_else(|| {
                agent_core::CoreError::InvalidState("session is not bound to a project".into())
            })?;
        Ok(crate::project::get_git_status(&binding.worktree_path))
    }

    async fn init_project_git(
        &self,
        project_id: ProjectId,
    ) -> agent_core::Result<ProjectGitStatus> {
        let project = self
            .project_repository()?
            .get_project(project_id.as_str())
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        let output = Command::new("git")
            .args(["-C", &project.root_path, "init"])
            .output()
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        if !output.status.success() {
            return Ok(ProjectGitStatus {
                kind: agent_core::ProjectGitStatusKind::Error,
                branch: None,
                worktree_path: project.root_path,
                message: Some(String::from_utf8_lossy(&output.stderr).trim().to_string()),
            });
        }
        Ok(crate::project::get_git_status(&project.root_path))
    }

    async fn get_project_instruction_summary(
        &self,
        project_id: ProjectId,
    ) -> agent_core::Result<ProjectInstructionSummary> {
        let project = self
            .project_repository()?
            .get_project(project_id.as_str())
            .await
            .map_err(|error| agent_core::CoreError::InvalidState(error.to_string()))?;
        Ok(
            crate::project::read_project_instruction_summary(std::path::Path::new(
                &project.root_path,
            ))
            .await,
        )
    }
}

/// View descriptor for the always-present implicit "builtin" catalog
/// source. Returned by [`AppFacade::list_catalog_sources`] both when the
/// marketplace is fully wired and when it is not configured at all.
fn builtin_source_view() -> CatalogSourceView {
    CatalogSourceView {
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
    }
}

/// Build a builtin-only `AggregateCatalogProvider` for the degraded path
/// where the user has no `[mcp_marketplace]` section in `kairox.toml`.
/// The GUI marketplace tab still works because it sees the curated
/// `BuiltinCatalogProvider` entries.
///
/// The aggregator is cached in a process-wide `OnceLock` so the GUI
/// marketplace hot path (`list_catalog` / `get_catalog_entry`) does not
/// re-parse the static built-in JSON on every poll. `BUILTIN_JSON` is
/// `include_str!`'d at compile time, so a parse failure here means the
/// shipped binary itself is broken — `expect` is the correct response.
fn builtin_only_provider() -> agent_core::Result<Arc<AggregateCatalogProvider>> {
    static BUILTIN_AGGREGATE: OnceLock<Arc<AggregateCatalogProvider>> = OnceLock::new();
    let agg = BUILTIN_AGGREGATE.get_or_init(|| {
        let builtin = Arc::new(
            BuiltinCatalogProvider::new()
                .expect("BUILTIN_JSON must parse; this is a build-time invariant"),
        );
        let providers: Vec<Arc<dyn CatalogProvider>> = vec![builtin];
        Arc::new(AggregateCatalogProvider::new(providers))
    });
    Ok(Arc::clone(agg))
}

fn catalog_source_to_view(s: agent_config::CatalogSourceConfig) -> CatalogSourceView {
    CatalogSourceView {
        id: s.id,
        display_name: s.display_name,
        kind: match s.kind {
            agent_config::CatalogSourceKind::McpRegistry => "mcp_registry".into(),
        },
        url: s.url,
        api_key_env: s.api_key_env,
        priority: s.priority,
        default_trust: s.default_trust,
        enabled: s.enabled,
        cache_ttl_seconds: s.cache_ttl_seconds,
        last_error: None,
    }
}

fn request_to_source_config(
    r: AddCatalogSourceRequest,
) -> agent_core::Result<agent_config::CatalogSourceConfig> {
    let kind = match r.kind.as_str() {
        "mcp_registry" => agent_config::CatalogSourceKind::McpRegistry,
        other => {
            return Err(agent_core::CoreError::InvalidState(format!(
                "unsupported catalog source kind: {other}"
            )));
        }
    };
    if !r.url.starts_with("http://") && !r.url.starts_with("https://") {
        return Err(agent_core::CoreError::InvalidState(
            "url must start with http:// or https://".into(),
        ));
    }
    Ok(agent_config::CatalogSourceConfig {
        id: r.id,
        display_name: r.display_name,
        kind,
        url: r.url,
        api_key_env: r.api_key_env,
        priority: r.priority.unwrap_or(100),
        default_trust: r.default_trust.unwrap_or_else(|| "community".into()),
        enabled: r.enabled.unwrap_or(true),
        cache_ttl_seconds: r.cache_ttl_seconds,
    })
}

// ---------------------------------------------------------------------------
// Marketplace mapping helpers (agent-core mirror DTOs <-> agent-mcp canonical)
// ---------------------------------------------------------------------------

fn map_query(q: CoreCatalogQuery) -> CatalogQuery {
    CatalogQuery {
        keyword: q.keyword,
        category: q.category,
        trust_min: q.trust_min.as_deref().and_then(parse_trust),
        source: q.source,
        limit: q.limit,
    }
}

fn parse_trust(s: &str) -> Option<TrustLevel> {
    match s {
        "unverified" => Some(TrustLevel::Unverified),
        "community" => Some(TrustLevel::Community),
        "verified" => Some(TrustLevel::Verified),
        _ => None,
    }
}

fn trust_to_str(t: TrustLevel) -> &'static str {
    match t {
        TrustLevel::Unverified => "unverified",
        TrustLevel::Community => "community",
        TrustLevel::Verified => "verified",
    }
}

fn map_entry_to_core(e: ServerEntry) -> CoreServerEntry {
    let install_spec_json = serde_json::to_string(&e.install).unwrap_or_else(|_| "{}".into());
    let requirements_json = serde_json::to_string(&e.requirements).unwrap_or_else(|_| "[]".into());
    let default_env_json = serde_json::to_string(&e.default_env).unwrap_or_else(|_| "[]".into());
    CoreServerEntry {
        id: e.id,
        source: e.source,
        display_name: e.display_name,
        summary: e.summary,
        description: e.description,
        categories: e.categories,
        tags: e.tags,
        author: e.author,
        homepage: e.homepage,
        version: e.version,
        trust: trust_to_str(e.trust).into(),
        icon: e.icon,
        install_spec_json,
        requirements_json,
        default_env_json,
    }
}

fn map_install_request(r: CoreInstallRequest) -> McpInstallRequest {
    McpInstallRequest {
        catalog_id: r.catalog_id,
        source: r.source,
        server_id_override: r.server_id_override,
        env_overrides: r.env_overrides,
        trust_grant: r.trust_grant,
        auto_start: r.auto_start,
    }
}

fn map_outcome_to_core(outcome: InstallOutcomeView) -> CoreInstallOutcomeView {
    match outcome {
        InstallOutcomeView::Installed { server_id, started } => CoreInstallOutcomeView {
            kind: "installed".into(),
            server_id: Some(server_id),
            started: Some(started),
            missing_runtimes: Vec::new(),
            missing_env_keys: Vec::new(),
        },
        InstallOutcomeView::RuntimeMissing { missing } => CoreInstallOutcomeView {
            kind: "runtime_missing".into(),
            server_id: None,
            started: None,
            missing_runtimes: missing
                .into_iter()
                .map(|r| r.kind.as_str().into())
                .collect(),
            missing_env_keys: Vec::new(),
        },
        InstallOutcomeView::AlreadyInstalled { server_id } => CoreInstallOutcomeView {
            kind: "already_installed".into(),
            server_id: Some(server_id),
            started: None,
            missing_runtimes: Vec::new(),
            missing_env_keys: Vec::new(),
        },
        InstallOutcomeView::InvalidEnv { missing_keys } => CoreInstallOutcomeView {
            kind: "invalid_env".into(),
            server_id: None,
            started: None,
            missing_runtimes: Vec::new(),
            missing_env_keys: missing_keys,
        },
    }
}

fn build_server_def(entry: &ServerEntry, req: &McpInstallRequest) -> McpServerDef {
    let server_id = req
        .server_id_override
        .clone()
        .unwrap_or_else(|| entry.id.clone());

    // Resolve env: defaults overridden by request.
    let mut env: std::collections::HashMap<String, String> = entry
        .default_env
        .iter()
        .filter_map(|spec| spec.default.clone().map(|v| (spec.key.clone(), v)))
        .collect();
    for (k, v) in &req.env_overrides {
        env.insert(k.clone(), v.clone());
    }

    let (transport, args) = match &entry.install {
        InstallSpec::Stdio {
            command,
            args,
            env: spec_env,
            cwd,
        } => {
            for (k, v) in spec_env {
                env.entry(k.clone()).or_insert_with(|| v.clone());
            }
            (
                McpTransportDef::Stdio {
                    command: command.clone(),
                    cwd: cwd.clone(),
                },
                args.clone(),
            )
        }
        InstallSpec::Sse { url, headers } => (
            McpTransportDef::Sse {
                url: url.clone(),
                api_key_env: None,
                headers: headers
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            },
            Vec::new(),
        ),
    };

    McpServerDef {
        name: server_id,
        transport,
        args,
        env,
        keep_alive: false,
        idle_timeout_secs: 300,
        auto_restart: true,
        max_restart_attempts: 3,
    }
}

fn emit_marketplace_event(tx: &tokio::sync::broadcast::Sender<DomainEvent>, payload: EventPayload) {
    let event = DomainEvent::new(
        WorkspaceId::new(),
        SessionId::new(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        payload,
    );
    let _ = tx.send(event);
}

// ---------------------------------------------------------------------------
// Phase 2: catalog provider construction
// ---------------------------------------------------------------------------

fn parse_trust_str(s: &str) -> TrustLevel {
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
    async fn send_message_records_user_and_assistant_events() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hello".into()]);
        let runtime = LocalRuntime::new(store, model);

        let workspace = runtime
            .open_workspace("/tmp/workspace".into())
            .await
            .unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fake".into(),
            })
            .await
            .unwrap();

        runtime
            .send_message(SendMessageRequest {
                workspace_id: workspace.workspace_id,
                session_id: session_id.clone(),
                content: "hi".into(),
            })
            .await
            .unwrap();

        let projection = runtime.get_session_projection(session_id).await.unwrap();
        assert_eq!(projection.messages.len(), 2);
        assert_eq!(projection.messages[0].content, "hi");
        assert_eq!(projection.messages[1].content, "hello");
    }

    #[tokio::test]
    async fn open_workspace_persists_metadata() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hi".into()]);
        let runtime = LocalRuntime::new(store, model);

        let workspace = runtime.open_workspace("/tmp/project".into()).await.unwrap();

        let workspaces = runtime.list_workspaces().await.unwrap();
        assert_eq!(workspaces.len(), 1);
        assert_eq!(workspaces[0].workspace_id, workspace.workspace_id);
        assert_eq!(workspaces[0].path, "/tmp/project");
    }

    #[tokio::test]
    async fn start_session_persists_metadata() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hi".into()]);
        let runtime = LocalRuntime::new(store, model);

        let workspace = runtime.open_workspace("/tmp/project".into()).await.unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fake".into(),
            })
            .await
            .unwrap();

        let sessions = runtime
            .list_sessions(&workspace.workspace_id)
            .await
            .unwrap();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, session_id);
        assert_eq!(sessions[0].title, "Session using fake");
    }

    #[tokio::test]
    async fn rename_session_updates_metadata() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hi".into()]);
        let runtime = LocalRuntime::new(store, model);

        let workspace = runtime.open_workspace("/tmp/project".into()).await.unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fake".into(),
            })
            .await
            .unwrap();

        runtime
            .rename_session(&session_id, "My Custom Title".into())
            .await
            .unwrap();

        let sessions = runtime
            .list_sessions(&workspace.workspace_id)
            .await
            .unwrap();
        assert_eq!(sessions[0].title, "My Custom Title");
    }

    #[tokio::test]
    async fn soft_delete_hides_session() {
        let store = SqliteEventStore::in_memory().await.unwrap();
        let model = FakeModelClient::new(vec!["hi".into()]);
        let runtime = LocalRuntime::new(store, model);

        let workspace = runtime.open_workspace("/tmp/project".into()).await.unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fake".into(),
            })
            .await
            .unwrap();

        runtime.soft_delete_session(&session_id).await.unwrap();

        let sessions = runtime
            .list_sessions(&workspace.workspace_id)
            .await
            .unwrap();
        assert!(sessions.is_empty());
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

        let workspace = runtime
            .open_workspace("/tmp/workspace".into())
            .await
            .unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fake".into(),
            })
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

        let result = runtime
            .send_message(SendMessageRequest {
                workspace_id: workspace.workspace_id,
                session_id: session_id.clone(),
                content: "hello".into(),
            })
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

        let workspace = runtime.open_workspace("/tmp/ws".into()).await.unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fast".into(),
            })
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

        let workspace = runtime.open_workspace("/tmp/ws".into()).await.unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fast".into(),
            })
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

        let workspace = runtime.open_workspace("/tmp/ws".into()).await.unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fast".into(),
            })
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

        let workspace = runtime.open_workspace("/tmp/ws".into()).await.unwrap();
        let session_id = runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fast".into(),
            })
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
        };
        assert_eq!(runtime.execution_mode(&request), ExecutionMode::SingleStep);
    }
}
