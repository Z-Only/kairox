use agent_config::Config;
use agent_core::SessionId;
use agent_core::WorkspaceId;
use agent_memory::MemoryStore;
use agent_models::ModelRouter;
use agent_runtime::ui_bootstrap::{load_config_with_profiles_overlay, load_ui_config};
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use std::sync::Arc;
use std::sync::RwLock;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

pub struct GuiState {
    pub runtime: Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    pub config: Arc<RwLock<Config>>,
    pub memory_store: Arc<dyn MemoryStore>,
    pub workspace_id: Mutex<Option<WorkspaceId>>,
    pub current_session_id: Mutex<Option<SessionId>>,
    pub forwarder_handle: Mutex<Option<JoinHandle<()>>>,
    pub profiles_config_path: Option<std::path::PathBuf>,
    pub home_dir: std::path::PathBuf,
}

impl GuiState {
    pub fn new(
        runtime: LocalRuntime<SqliteEventStore, ModelRouter>,
        config: Config,
        memory_store: Arc<dyn MemoryStore>,
    ) -> Self {
        Self {
            runtime: Arc::new(runtime),
            config: Arc::new(RwLock::new(config)),
            memory_store,
            workspace_id: Mutex::new(None),
            current_session_id: Mutex::new(None),
            forwarder_handle: Mutex::new(None),
            profiles_config_path: None,
            home_dir: std::path::PathBuf::from("."),
        }
    }

    /// Reload the project-level portion of the config from `project_root`.
    /// Merges defaults + user-level + project-level `.kairox/config.toml`.
    pub fn refresh_config_for_project(&self, project_root: &std::path::Path) -> Result<(), String> {
        let base_config =
            Config::load_with_project_root(Some(project_root)).map_err(|e| e.to_string())?;
        let new_config = load_config_with_profiles_overlay(base_config, &self.home_dir)
            .map_err(|e| e.to_string())?
            .config;
        let mut cfg = self.config.write().map_err(|e| e.to_string())?;
        *cfg = new_config;
        Ok(())
    }

    /// Reload the full config, including profiles.toml overlay.
    pub fn refresh_config(&self) -> Result<(), String> {
        let new_config = load_ui_config(&self.home_dir).config;
        let mut cfg = self.config.write().map_err(|e| e.to_string())?;
        *cfg = new_config;
        Ok(())
    }
}
