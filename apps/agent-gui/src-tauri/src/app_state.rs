use agent_core::{SessionId, WorkspaceId};
use agent_models::FakeModelClient;
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

#[allow(dead_code)]
pub struct GuiState {
    pub runtime: Arc<LocalRuntime<SqliteEventStore, FakeModelClient>>,
    pub workspace_id: Mutex<Option<WorkspaceId>>,
    pub sessions: Mutex<HashMap<String, WorkspaceSession>>,
    pub current_session_id: Mutex<Option<SessionId>>,
    pub forwarder_handle: Mutex<Option<JoinHandle<()>>>,
}

impl GuiState {
    #[allow(dead_code)]
    pub fn new(runtime: LocalRuntime<SqliteEventStore, FakeModelClient>) -> Self {
        Self {
            runtime: Arc::new(runtime),
            workspace_id: Mutex::new(None),
            sessions: Mutex::new(HashMap::new()),
            current_session_id: Mutex::new(None),
            forwarder_handle: Mutex::new(None),
        }
    }
}
