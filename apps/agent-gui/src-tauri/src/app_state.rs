use agent_config::Config;
use agent_core::SessionId;
use agent_core::WorkspaceId;
use agent_memory::MemoryStore;
use agent_models::ModelRouter;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

pub struct GuiState {
    pub runtime: Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    pub config: Arc<Config>,
    pub memory_store: Arc<dyn MemoryStore>,
    pub workspace_id: Mutex<Option<WorkspaceId>>,
    pub current_session_id: Mutex<Option<SessionId>>,
    pub forwarder_handle: Mutex<Option<JoinHandle<()>>>,
}

impl GuiState {
    #[allow(dead_code)]
    pub fn new(
        runtime: LocalRuntime<SqliteEventStore, ModelRouter>,
        config: Config,
        memory_store: Arc<dyn MemoryStore>,
    ) -> Self {
        Self {
            runtime: Arc::new(runtime),
            config: Arc::new(config),
            memory_store,
            workspace_id: Mutex::new(None),
            current_session_id: Mutex::new(None),
            forwarder_handle: Mutex::new(None),
        }
    }
}
