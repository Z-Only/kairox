use std::future::Future;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use agent_config::{CatalogSourceConfig, Config};
use agent_core::{AppFacade, DomainEvent, SessionId, StartSessionRequest, WorkspaceInfo};
use agent_memory::{MemoryStore, SqliteMemoryStore};
use agent_models::ModelRouter;
use agent_store::SqliteEventStore;
use agent_tools::{ApprovalPolicy, SandboxPolicy};
use futures::StreamExt;
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

    let mut skill_roots =
        crate::skills::build_default_skill_roots(&options.home_dir, &options.workspace_root);
    let skill_settings_roots = crate::skills::build_default_skill_settings_roots(
        &options.home_dir,
        &options.workspace_root,
    );
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
    let mut runtime = LocalRuntime::new(store, router)
        .with_approval_and_sandbox(options.approval_policy, options.sandbox_policy)
        .with_context_limit(100_000)
        .with_memory_store(memory_store.clone())
        .with_config(config_arc)
        .with_ollama_clients(ollama_clients)
        .with_skill_registry(Arc::new(skill_registry))
        .with_skill_settings_roots(skill_settings_roots)
        .with_agent_settings_roots(agent_settings_roots)
        .with_plugin_settings_roots(plugin_settings_roots)
        .with_skill_catalog(Some(options.data_dir.clone()))
        .with_builtin_tools(options.workspace_root.clone())
        .await;

    if options.enable_marketplace {
        runtime =
            runtime.with_marketplace_loaded(options.data_dir.clone(), &options.catalog_sources)?;
    }
    if options.enable_mcp_servers {
        runtime = runtime.with_mcp_servers(mcp_server_defs).await;
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

pub async fn ensure_workspace_session<F>(
    facade: &F,
    workspace_path: String,
    model_profile: String,
    permission_mode: Option<String>,
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
                    permission_mode,
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
