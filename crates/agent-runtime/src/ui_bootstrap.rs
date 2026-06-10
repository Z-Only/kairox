use std::collections::HashMap;
use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent_config::{CatalogSourceConfig, Config, KnowledgeBaseKind};
use agent_core::{AppFacade, DomainEvent, SessionId, StartSessionRequest, WorkspaceInfo};
use agent_memory::{
    HashedEmbeddingBackend, MemoryStore, SqliteFtsKnowledgeBase, SqliteFtsKnowledgeBaseConfig,
    SqliteMemoryStore, WorkspaceRagIndex, WorkspaceRetriever,
};
use agent_models::ModelRouter;
use agent_store::{SqliteAutonomousTaskStore, SqliteEventStore};
use agent_tools::{ApprovalPolicy, SandboxPolicy};
use futures::StreamExt;
use sqlx::sqlite::SqlitePoolOptions;
use tokio::task::JoinHandle;

use crate::{LocalRuntime, RuntimeError};

pub type UiRuntime = LocalRuntime<SqliteEventStore, ModelRouter>;

#[derive(Debug, Clone)]
pub struct UiConfigLoad {
    pub config: Config,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct UiCatalogSourceLoad {
    pub sources: Vec<CatalogSourceConfig>,
    pub warnings: Vec<String>,
}

#[derive(Clone)]
pub struct UiRuntimeOptions {
    pub home_dir: PathBuf,
    pub data_dir: PathBuf,
    pub database_filename: String,
    pub workspace_root: PathBuf,
    pub approval_policy: ApprovalPolicy,
    pub sandbox_policy: SandboxPolicy,
    pub config: Config,
    pub catalog_sources: Vec<CatalogSourceConfig>,
    pub enable_marketplace: bool,
    pub enable_mcp_servers: bool,
    pub enable_lsp_servers: bool,
    pub enable_plugin_skill_roots: bool,
}

pub struct UiRuntimeBootstrap {
    pub runtime: UiRuntime,
    pub config: Config,
    pub memory_store: Arc<dyn MemoryStore>,
    pub data_dir: PathBuf,
    pub profiles_config_path: PathBuf,
    pub catalog_sources: Vec<CatalogSourceConfig>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSessionBootstrap {
    pub workspace: WorkspaceInfo,
    pub session_id: SessionId,
    pub created_workspace: bool,
    pub created_session: bool,
}

impl UiRuntimeOptions {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        home_dir: PathBuf,
        data_dir: PathBuf,
        database_filename: impl Into<String>,
        workspace_root: PathBuf,
        approval_policy: ApprovalPolicy,
        sandbox_policy: SandboxPolicy,
        config: Config,
        catalog_sources: Vec<CatalogSourceConfig>,
    ) -> Self {
        Self {
            home_dir,
            data_dir,
            database_filename: database_filename.into(),
            workspace_root,
            approval_policy,
            sandbox_policy,
            config,
            catalog_sources,
            enable_marketplace: true,
            enable_mcp_servers: true,
            enable_lsp_servers: true,
            enable_plugin_skill_roots: true,
        }
    }
}

pub fn default_home_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
}

pub fn default_data_dir(home_dir: &Path) -> PathBuf {
    home_dir.join(".kairox")
}

pub fn sqlite_database_url(data_dir: &Path, database_filename: &str) -> String {
    let db_path = data_dir.join(database_filename);
    format!(
        "sqlite:///{}",
        db_path.display().to_string().trim_start_matches('/')
    )
}

fn sqlite_file_url(path: &Path) -> String {
    format!(
        "sqlite:///{}",
        path.display().to_string().trim_start_matches('/')
    )
}

fn resolve_knowledge_base_path(workspace_root: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_absolute() {
        path
    } else {
        workspace_root.join(path)
    }
}

pub fn load_ui_config(data_dir: &Path) -> UiConfigLoad {
    let mut warnings = Vec::new();
    let config = match Config::load() {
        Ok(config) => config,
        Err(error) => {
            warnings.push(format!("Config warning: {error}, using defaults"));
            Config::defaults()
        }
    };
    let mut loaded =
        load_config_with_profiles_overlay(config, data_dir).unwrap_or_else(|error| UiConfigLoad {
            config: Config::defaults(),
            warnings: vec![format!("Config warning: {error}, using defaults")],
        });
    warnings.append(&mut loaded.warnings);
    UiConfigLoad {
        config: loaded.config,
        warnings,
    }
}

pub fn load_user_ui_config(data_dir: &Path) -> UiConfigLoad {
    let mut warnings = Vec::new();
    let config = match Config::load_with_project_root(None) {
        Ok(config) => config,
        Err(error) => {
            warnings.push(format!("Config warning: {error}, using defaults"));
            Config::defaults()
        }
    };
    let mut loaded =
        load_config_with_profiles_overlay(config, data_dir).unwrap_or_else(|error| UiConfigLoad {
            config: Config::defaults(),
            warnings: vec![format!("Config warning: {error}, using defaults")],
        });
    warnings.append(&mut loaded.warnings);
    UiConfigLoad {
        config: loaded.config,
        warnings,
    }
}

pub fn load_config_with_profiles_overlay(
    mut config: Config,
    data_dir: &Path,
) -> crate::Result<UiConfigLoad> {
    let mut warnings = Vec::new();
    let profiles_toml_path = data_dir.join("profiles.toml");
    if profiles_toml_path.exists() {
        match std::fs::read_to_string(&profiles_toml_path) {
            Ok(raw) => {
                match agent_config::load_from_str(&raw, &profiles_toml_path.display().to_string()) {
                    Ok(overlay) => {
                        for (alias, def) in overlay.profiles {
                            if config
                                .profiles
                                .iter()
                                .all(|(existing_alias, _)| existing_alias != &alias)
                            {
                                config.profiles.push((alias, def));
                            }
                        }
                    }
                    Err(error) => {
                        warnings.push(format!("Profiles overlay warning: {error}"));
                    }
                }
            }
            Err(error) => {
                warnings.push(format!("Profiles overlay warning: {error}"));
            }
        }
    }
    Ok(UiConfigLoad { config, warnings })
}

pub fn load_catalog_sources(data_dir: &Path) -> UiCatalogSourceLoad {
    let mut warnings = Vec::new();
    let toml_path = data_dir.join("config.toml");
    let user_sources = match std::fs::read_to_string(&toml_path) {
        Ok(raw) => agent_config::parse_catalog_sources(&raw).unwrap_or_else(|error| {
            warnings.push(format!("Catalog sources warning: {error}, using defaults"));
            Vec::new()
        }),
        Err(_) => Vec::new(),
    };
    UiCatalogSourceLoad {
        sources: agent_config::merge_with_defaults(user_sources),
        warnings,
    }
}

pub async fn connect_ui_event_store(
    data_dir: &Path,
    database_filename: &str,
) -> crate::Result<SqliteEventStore> {
    tokio::fs::create_dir_all(data_dir)
        .await
        .map_err(|error| RuntimeError::Other(format!("create data dir: {error}")))?;
    let database_url = sqlite_database_url(data_dir, database_filename);
    SqliteEventStore::connect(&database_url)
        .await
        .map_err(|error| RuntimeError::Other(format!("event store: {error}")))
}

pub async fn build_ui_runtime(options: UiRuntimeOptions) -> crate::Result<UiRuntimeBootstrap> {
    let store = connect_ui_event_store(&options.data_dir, &options.database_filename).await?;
    build_ui_runtime_from_store(store, options).await
}

pub async fn build_ui_runtime_from_store(
    store: SqliteEventStore,
    options: UiRuntimeOptions,
) -> crate::Result<UiRuntimeBootstrap> {
    let memory_store = Arc::new(
        SqliteMemoryStore::new(store.pool().clone())
            .await
            .map_err(|error| RuntimeError::Other(format!("memory store: {error}")))?,
    ) as Arc<dyn MemoryStore>;
    let workspace_rag_index = Arc::new(
        WorkspaceRagIndex::new(
            store.pool().clone(),
            Arc::new(HashedEmbeddingBackend::default()),
        )
        .await
        .map_err(|error| RuntimeError::Other(format!("workspace RAG index: {error}")))?,
    );
    let knowledge_base_retrievers =
        build_knowledge_base_retrievers(&options.config, &options.workspace_root).await?;

    let builtin_skills_root = crate::skills::ensure_builtin_skills_root(&options.data_dir).await?;
    let mut skill_roots =
        crate::skills::build_default_skill_roots(&options.home_dir, &options.workspace_root);
    for root in &mut skill_roots {
        if root.kind == agent_skills::SkillSourceKind::Builtin {
            root.path = builtin_skills_root.clone();
        }
    }
    let mut skill_settings_roots = crate::skills::build_default_skill_settings_roots(
        &options.home_dir,
        &options.workspace_root,
    );
    skill_settings_roots.builtin_root = Some(builtin_skills_root);
    let agent_settings_roots = crate::agent_settings::build_default_agent_settings_roots(
        &options.home_dir,
        &options.workspace_root,
    );
    let plugin_settings_roots = crate::plugin_settings::build_default_plugin_settings_roots(
        &options.home_dir,
        &options.workspace_root,
    );
    if options.enable_plugin_skill_roots {
        let plugin_skill_roots =
            crate::skills::build_plugin_skill_roots(&plugin_settings_roots).await;
        skill_roots.extend(plugin_skill_roots);
    }
    let skill_registry = agent_skills::FileSkillRegistry::discover(skill_roots)
        .await
        .map_err(|error| RuntimeError::Other(format!("skill discovery: {error}")))?;

    let router = options.config.build_router();
    let ollama_clients = agent_config::build_ollama_clients(&options.config);
    let mcp_server_defs = options.config.mcp_server_defs();
    let config_arc = Arc::new(options.config.clone());

    // Snapshot pool before `store` is consumed by `LocalRuntime::new`.
    let pool = store.pool().clone();
    let autonomous_store = {
        let s = Arc::new(SqliteAutonomousTaskStore::new(pool));
        if let Err(e) = s.migrate().await {
            tracing::warn!("autonomous store migration failed: {e}");
        }
        s
    };

    let mut runtime = LocalRuntime::new(store, router)
        .with_approval_and_sandbox(options.approval_policy, options.sandbox_policy)
        .with_context_limit(100_000)
        .with_memory_store(memory_store.clone())
        .with_workspace_rag_index(workspace_rag_index)
        .with_knowledge_base_retrievers(knowledge_base_retrievers)
        .with_config(config_arc)
        .with_ollama_clients(ollama_clients)
        .with_skill_registry(Arc::new(skill_registry))
        .with_skill_settings_roots(skill_settings_roots)
        .with_agent_settings_roots(agent_settings_roots)
        .with_plugin_settings_roots(plugin_settings_roots)
        .with_skill_catalog(Some(options.data_dir.clone()))
        .with_builtin_tools(options.workspace_root.clone())
        .await
        .with_trajectory_store_from_pool()
        .await
        .with_autonomous_store(autonomous_store);

    if options.enable_marketplace {
        runtime =
            runtime.with_marketplace_loaded(options.data_dir.clone(), &options.catalog_sources)?;
    }
    if options.enable_mcp_servers {
        runtime = runtime.with_mcp_servers(mcp_server_defs).await;
    }
    if options.enable_lsp_servers {
        let lsp_defs = options.config.lsp_server_defs();
        let dap_defs = options.config.dap_server_defs();
        runtime = runtime.with_lsp_servers(lsp_defs, dap_defs).await;
    }

    Ok(UiRuntimeBootstrap {
        runtime,
        config: options.config,
        memory_store,
        profiles_config_path: options.data_dir.join("profiles.toml"),
        data_dir: options.data_dir,
        catalog_sources: options.catalog_sources,
    })
}

async fn build_knowledge_base_retrievers(
    config: &Config,
    workspace_root: &Path,
) -> crate::Result<HashMap<String, Arc<dyn WorkspaceRetriever>>> {
    let mut retrievers: HashMap<String, Arc<dyn WorkspaceRetriever>> = HashMap::new();
    for (id, kb) in &config.knowledge_bases {
        if !kb.enabled {
            continue;
        }
        match kb.kind {
            KnowledgeBaseKind::SqliteFts => {
                let path = kb.path.as_deref().ok_or_else(|| {
                    RuntimeError::Other(format!("knowledge base '{id}' missing SQLite path"))
                })?;
                let db_path = resolve_knowledge_base_path(workspace_root, path);
                if let Some(parent) = db_path.parent() {
                    tokio::fs::create_dir_all(parent).await.map_err(|error| {
                        RuntimeError::Other(format!(
                            "create knowledge base dir for '{id}': {error}"
                        ))
                    })?;
                }
                tokio::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&db_path)
                    .await
                    .map_err(|error| {
                        RuntimeError::Other(format!("create knowledge base db for '{id}': {error}"))
                    })?;
                let pool = SqlitePoolOptions::new()
                    .max_connections(5)
                    .connect(&sqlite_file_url(&db_path))
                    .await
                    .map_err(|error| {
                        RuntimeError::Other(format!("connect knowledge base '{id}': {error}"))
                    })?;
                let defaults = SqliteFtsKnowledgeBaseConfig::default();
                let retriever = SqliteFtsKnowledgeBase::new(
                    id.clone(),
                    pool,
                    SqliteFtsKnowledgeBaseConfig {
                        table: kb.table.clone().unwrap_or(defaults.table),
                        id_column: kb.id_column.clone().unwrap_or(defaults.id_column),
                        title_column: kb.title_column.clone().or(defaults.title_column),
                        content_column: kb
                            .content_column
                            .clone()
                            .unwrap_or(defaults.content_column),
                        workspace_id_column: kb
                            .workspace_id_column
                            .clone()
                            .or(defaults.workspace_id_column),
                    },
                )
                .await
                .map_err(|error| {
                    RuntimeError::Other(format!("initialize knowledge base '{id}': {error}"))
                })?;
                retrievers.insert(id.clone(), Arc::new(retriever));
            }
            KnowledgeBaseKind::Tantivy
            | KnowledgeBaseKind::BedrockKnowledgeBase
            | KnowledgeBaseKind::Pinecone
            | KnowledgeBaseKind::Weaviate => {
                tracing::warn!(
                    knowledge_base_id = %id,
                    kind = ?kb.kind,
                    "knowledge base connector kind is parsed but not wired in this build"
                );
            }
        }
    }
    Ok(retrievers)
}

#[cfg(test)]
#[path = "ui_bootstrap_tests.rs"]
mod tests;

pub async fn ensure_workspace_session<F>(
    facade: &F,
    workspace_path: String,
    model_profile: String,
) -> agent_core::Result<WorkspaceSessionBootstrap>
where
    F: AppFacade + Sync + ?Sized,
{
    let workspaces = AppFacade::list_workspaces(facade).await?;
    let (workspace, created_workspace) =
        if let Some(existing) = workspaces.into_iter().find(|w| w.path == workspace_path) {
            (existing, false)
        } else {
            (
                AppFacade::open_workspace(facade, workspace_path).await?,
                true,
            )
        };

    let sessions = AppFacade::list_sessions(facade, &workspace.workspace_id).await?;
    let (session_id, created_session) = if let Some(session) = sessions.first() {
        (session.session_id.clone(), false)
    } else {
        (
            AppFacade::start_session(
                facade,
                StartSessionRequest {
                    workspace_id: workspace.workspace_id.clone(),
                    model_profile,
                    approval_policy: None,
                    sandbox_policy: None,
                },
            )
            .await?,
            true,
        )
    };

    Ok(WorkspaceSessionBootstrap {
        workspace,
        session_id,
        created_workspace,
        created_session,
    })
}

pub fn spawn_runtime_event_forwarder<F, Fut>(runtime: &UiRuntime, mut forward: F) -> JoinHandle<()>
where
    F: FnMut(DomainEvent) -> Fut + Send + 'static,
    Fut: Future<Output = bool> + Send + 'static,
{
    let mut stream = runtime.subscribe_all();
    tokio::spawn(async move {
        while let Some(event) = stream.next().await {
            if !forward(event).await {
                break;
            }
        }
    })
}
