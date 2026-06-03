use agent_config::Config;
use agent_core::SessionId;
use agent_core::WorkspaceId;
use agent_memory::MemoryStore;
use agent_models::ModelRouter;
use agent_runtime::ui_bootstrap::{
    load_config_with_profiles_overlay, load_ui_config, load_user_ui_config,
};
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
    pub devtools_enabled_at_startup: bool,
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
            devtools_enabled_at_startup: false,
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
        self.runtime.update_config(Arc::new(new_config.clone()));
        self.runtime.refresh_model_router_from_config(&new_config);
        let mut cfg = self.config.write().map_err(|e| e.to_string())?;
        *cfg = new_config;
        Ok(())
    }

    /// Reload the full config, including profiles.toml overlay.
    pub fn refresh_config(&self) -> Result<(), String> {
        let new_config = load_ui_config(&self.home_dir).config;
        self.runtime.update_config(Arc::new(new_config.clone()));
        self.runtime.refresh_model_router_from_config(&new_config);
        let mut cfg = self.config.write().map_err(|e| e.to_string())?;
        *cfg = new_config;
        Ok(())
    }

    /// Reload the user-level config, including profiles.toml overlay, without
    /// discovering project-level `.kairox/config.toml` from the GUI cwd.
    pub fn refresh_user_config(&self) -> Result<(), String> {
        let new_config = load_user_ui_config(&self.home_dir).config;
        self.runtime.update_config(Arc::new(new_config.clone()));
        self.runtime.refresh_model_router_from_config(&new_config);
        let mut cfg = self.config.write().map_err(|e| e.to_string())?;
        *cfg = new_config;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
    use agent_memory::{MemoryEntry, MemoryQuery, MemoryScope, MemoryStore, MemoryStoreError};
    use async_trait::async_trait;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    static HOME_ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

    struct NoopMemoryStore;

    #[async_trait]
    impl MemoryStore for NoopMemoryStore {
        async fn store(&self, _entry: MemoryEntry) -> Result<(), MemoryStoreError> {
            Ok(())
        }

        async fn query(&self, _query: MemoryQuery) -> Result<Vec<MemoryEntry>, MemoryStoreError> {
            Ok(Vec::new())
        }

        async fn query_including_pending(
            &self,
            _query: MemoryQuery,
        ) -> Result<Vec<MemoryEntry>, MemoryStoreError> {
            Ok(Vec::new())
        }

        async fn get(&self, _id: &str) -> Result<Option<MemoryEntry>, MemoryStoreError> {
            Ok(None)
        }

        async fn set_accepted(&self, _id: &str, _accepted: bool) -> Result<(), MemoryStoreError> {
            Ok(())
        }

        async fn delete(&self, _id: &str) -> Result<(), MemoryStoreError> {
            Ok(())
        }

        async fn list_by_scope(
            &self,
            _scope: MemoryScope,
        ) -> Result<Vec<MemoryEntry>, MemoryStoreError> {
            Ok(Vec::new())
        }

        async fn count(&self, _scope: Option<MemoryScope>) -> Result<usize, MemoryStoreError> {
            Ok(0)
        }
    }

    fn config_with_instructions(text: &str) -> Config {
        let mut config = Config::defaults();
        config.instructions = Some(text.to_string());
        config
    }

    fn unique_home() -> std::path::PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock should be after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "kairox-gui-state-refresh-config-{}-{nanos}",
            std::process::id()
        ))
    }

    struct HomeEnvGuard(Option<std::ffi::OsString>);

    impl HomeEnvGuard {
        fn set(home_dir: &std::path::Path) -> Self {
            let previous = std::env::var_os("HOME");
            std::env::set_var("HOME", home_dir);
            Self(previous)
        }
    }

    impl Drop for HomeEnvGuard {
        fn drop(&mut self) {
            if let Some(previous) = self.0.take() {
                std::env::set_var("HOME", previous);
            } else {
                std::env::remove_var("HOME");
            }
        }
    }

    #[tokio::test]
    async fn refresh_config_updates_runtime_config_used_by_turns() {
        let _env_lock = HOME_ENV_LOCK.lock().await;
        let home_dir = unique_home();
        let config_dir = home_dir.join(".kairox");
        fs::create_dir_all(&config_dir).expect("config dir should be created");
        fs::write(
            config_dir.join("config.toml"),
            "instructions = \"new runtime instructions\"\n",
        )
        .expect("config should be written");

        let initial_config = config_with_instructions("old runtime instructions");
        let router = initial_config.build_router();
        let runtime = LocalRuntime::new(SqliteEventStore::in_memory().await.unwrap(), router)
            .with_config(Arc::new(initial_config.clone()));
        let mut state = GuiState::new(
            runtime,
            initial_config,
            Arc::new(NoopMemoryStore) as Arc<dyn MemoryStore>,
        );
        state.home_dir = config_dir;
        let _home_guard = HomeEnvGuard::set(&home_dir);

        state.refresh_config().expect("refresh should succeed");

        assert_eq!(
            state.config.read().unwrap().instructions.as_deref(),
            Some("new runtime instructions")
        );
        assert_eq!(
            state.runtime.config().instructions.as_deref(),
            Some("new runtime instructions")
        );

        fs::remove_dir_all(home_dir).ok();
    }

    #[tokio::test]
    async fn refresh_config_registers_new_profiles_for_model_requests() {
        let _env_lock = HOME_ENV_LOCK.lock().await;
        let home_dir = unique_home();
        let config_dir = home_dir.join(".kairox");
        fs::create_dir_all(&config_dir).expect("config dir should be created");
        fs::write(
            config_dir.join("config.toml"),
            r#"
[profiles.fresh]
provider = "fake"
model_id = "fake"
"#,
        )
        .expect("config should be written");

        let initial_config = Config::defaults();
        let router = initial_config.build_router();
        let runtime = LocalRuntime::new(SqliteEventStore::in_memory().await.unwrap(), router)
            .with_config(Arc::new(initial_config.clone()));
        let mut state = GuiState::new(
            runtime,
            initial_config,
            Arc::new(NoopMemoryStore) as Arc<dyn MemoryStore>,
        );
        state.home_dir = config_dir;
        let _home_guard = HomeEnvGuard::set(&home_dir);

        state.refresh_config().expect("refresh should succeed");

        let workspace = state
            .runtime
            .open_workspace("/tmp/kairox-refresh-profile-router".into())
            .await
            .expect("workspace should open");
        let session_id = state
            .runtime
            .start_session(StartSessionRequest {
                workspace_id: workspace.workspace_id.clone(),
                model_profile: "fresh".into(),
                approval_policy: None,
                sandbox_policy: None,
            })
            .await
            .expect("session should start with refreshed profile");

        state
            .runtime
            .send_message(SendMessageRequest {
                workspace_id: workspace.workspace_id,
                session_id,
                content: "hello".into(),
                display_content: None,
                attachments: vec![],
            })
            .await
            .expect("refreshed profile should be routable");

        fs::remove_dir_all(home_dir).ok();
    }
}
