use crate::dag_executor::{DagConfig, DagExecutor};
use crate::execution_runtime::SessionExecutionRuntime;
use crate::skill_package::{DirectDownloadPackageManager, SkillPackageManager};
use crate::{LspServerManager, McpServerManager};
use agent_core::DomainEvent;
use agent_mcp::catalog::skills::aggregate::AggregateSkillCatalogProvider;
use agent_mcp::catalog::{AggregateCatalogProvider, CatalogProvider};
use agent_mcp::installer::Installer;
use agent_mcp::{HttpResponseCache, SharedHttpClient};
use agent_memory::{ContextAssembler, MemoryStore};
use agent_store::EventStore;
use agent_tools::{MonitorRegistry, PermissionEngine, ToolRegistry, WorkspaceScopedBuiltinTools};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tokio::sync::Mutex;

pub(crate) const EVENT_CHANNEL_CAPACITY: usize = 1024;

#[derive(Clone)]
pub(crate) struct RuntimeConfig {
    inner: Arc<RwLock<Arc<agent_config::Config>>>,
}

impl RuntimeConfig {
    pub(crate) fn from_arc(config: Arc<agent_config::Config>) -> Self {
        Self {
            inner: Arc::new(RwLock::new(config)),
        }
    }

    pub(crate) fn snapshot(&self) -> Arc<agent_config::Config> {
        self.inner
            .read()
            .expect("runtime config lock should not be poisoned")
            .clone()
    }

    pub(crate) fn replace(&self, config: Arc<agent_config::Config>) {
        *self
            .inner
            .write()
            .expect("runtime config lock should not be poisoned") = config;
    }
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
    pub(crate) lsp_manager: Option<Arc<Mutex<LspServerManager>>>,
    pub(crate) tool_registry: Arc<Mutex<ToolRegistry>>,
    pub(crate) context_assembler: ContextAssembler,
    pub(crate) memory_store: Option<Arc<dyn MemoryStore>>,
    pub(crate) pending_permissions: crate::permission::PendingPermissionsMap,
    pub(crate) event_tx: tokio::sync::broadcast::Sender<DomainEvent>,
    pub(crate) task_graphs: Arc<Mutex<HashMap<String, crate::task_graph::TaskGraph>>>,
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
    pub(crate) config: RuntimeConfig,
    /// Profile-alias → typed Ollama client. Populated by `with_ollama_clients`
    /// at wiring time so Task 10 can fire `probe_context_window`. Empty when
    /// no Ollama profiles are configured.
    pub(crate) ollama_clients: HashMap<String, Arc<agent_models::OllamaClient>>,
    pub(crate) monitor_registry: Option<Arc<MonitorRegistry>>,
    pub(crate) workspace_scoped_builtin_tools: Option<Arc<WorkspaceScopedBuiltinTools>>,
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
            lsp_manager: None,
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
            config: RuntimeConfig::from_arc(Arc::new(agent_config::Config {
                profiles: vec![],
                mcp_servers: vec![],
                source: agent_config::ConfigSource::Defaults,
                context: agent_config::ContextPolicy::default(),
                disabled_mcp_servers: vec![],
                instructions: None,
                features: agent_config::FeatureFlags::default(),
                hooks: vec![],
                lsp_servers: vec![],
                dap_servers: vec![],
            })),
            ollama_clients: HashMap::new(),
            monitor_registry: None,
            workspace_scoped_builtin_tools: None,
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
    pub fn config(&self) -> Arc<agent_config::Config> {
        self.config.snapshot()
    }

    pub fn update_config(&self, config: Arc<agent_config::Config>) {
        self.config.replace(config);
    }

    pub fn monitor_registry(&self) -> Option<&Arc<MonitorRegistry>> {
        self.monitor_registry.as_ref()
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
