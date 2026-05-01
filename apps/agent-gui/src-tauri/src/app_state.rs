use agent_config::Config;
use agent_core::{SessionId, WorkspaceId};
use agent_models::ModelRouter;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;

#[allow(dead_code)]
pub struct WorkspaceSession {
    pub workspace_id: WorkspaceId,
    pub session_id: SessionId,
    pub profile: String,
}

pub struct GuiState {
    pub runtime: Arc<LocalRuntime<SqliteEventStore, ModelRouter>>,
    pub config: Arc<Config>,
    pub workspace_id: Mutex<Option<WorkspaceId>>,
    pub sessions: Mutex<HashMap<String, WorkspaceSession>>,
    pub current_session_id: Mutex<Option<SessionId>>,
    pub forwarder_handle: Mutex<Option<JoinHandle<()>>>,
}

impl GuiState {
    #[allow(dead_code)]
    pub fn new(runtime: LocalRuntime<SqliteEventStore, ModelRouter>, config: Config) -> Self {
        Self {
            runtime: Arc::new(runtime),
            config: Arc::new(config),
            workspace_id: Mutex::new(None),
            sessions: Mutex::new(HashMap::new()),
            current_session_id: Mutex::new(None),
            forwarder_handle: Mutex::new(None),
        }
    }
}
