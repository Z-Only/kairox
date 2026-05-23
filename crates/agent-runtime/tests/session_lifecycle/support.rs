//! Shared helpers and fixtures for session_lifecycle integration tests.

use agent_core::{DomainEvent, SessionId};
use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::{
    event_store::ProjectSessionMetaRow, EventStore, SessionRow, SqliteEventStore, WorkspaceRow,
};
use async_trait::async_trait;
use std::time::Duration;

/// Serializes tests that mutate process-global env vars (e.g. `HOME`, `PATH`).
pub static ENV_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// `EventStore` implementation backed by no SQLite metadata, used to verify
/// that project-session APIs surface the correct error when the store cannot
/// supply project metadata.
pub struct NonSqliteEventStore;

#[async_trait]
impl EventStore for NonSqliteEventStore {
    async fn append(&self, _event: &DomainEvent) -> agent_store::Result<()> {
        Ok(())
    }

    async fn load_session(&self, _session_id: &SessionId) -> agent_store::Result<Vec<DomainEvent>> {
        Ok(Vec::new())
    }

    async fn upsert_workspace(&self, _workspace_id: &str, _path: &str) -> agent_store::Result<()> {
        Ok(())
    }

    async fn upsert_session(&self, _meta: &SessionRow) -> agent_store::Result<()> {
        Ok(())
    }

    async fn list_workspaces(&self) -> agent_store::Result<Vec<WorkspaceRow>> {
        Ok(Vec::new())
    }

    async fn list_active_sessions(
        &self,
        _workspace_id: &str,
    ) -> agent_store::Result<Vec<SessionRow>> {
        Ok(Vec::new())
    }

    async fn rename_session(&self, _session_id: &str, _title: &str) -> agent_store::Result<()> {
        Ok(())
    }

    async fn soft_delete_session(&self, _session_id: &str) -> agent_store::Result<()> {
        Ok(())
    }

    async fn update_permission_mode(
        &self,
        _session_id: &str,
        _mode: &str,
    ) -> agent_store::Result<()> {
        Ok(())
    }

    async fn permanently_delete_session(&self, _session_id: &str) -> agent_store::Result<()> {
        Ok(())
    }

    async fn restore_archived_session(&self, _session_id: &str) -> agent_store::Result<()> {
        Ok(())
    }

    async fn cleanup_expired_sessions(&self, _older_than: Duration) -> agent_store::Result<usize> {
        Ok(0)
    }

    async fn list_visible_project_sessions(
        &self,
        _project_id: &str,
    ) -> agent_store::Result<Vec<ProjectSessionMetaRow>> {
        Ok(Vec::new())
    }

    async fn list_archived_project_session_metas(
        &self,
        _workspace_id: &str,
    ) -> agent_store::Result<Vec<ProjectSessionMetaRow>> {
        Ok(Vec::new())
    }

    async fn save_draft(&self, _session_id: &str, _draft_text: &str) -> agent_store::Result<()> {
        Ok(())
    }

    async fn get_draft(&self, _session_id: &str) -> agent_store::Result<String> {
        Ok(String::new())
    }
}

/// Helper: create an in-memory runtime for quick tests.
pub fn make_runtime(store: SqliteEventStore) -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    LocalRuntime::new(store, FakeModelClient::new(vec!["response".into()]))
}

/// Helper: create a file-backed SQLite store for persistence tests.
pub async fn make_file_backed_store() -> (SqliteEventStore, std::path::PathBuf) {
    let db_path = std::env::temp_dir().join(format!(
        "kairox-session-lifecycle-{}.sqlite",
        uuid::Uuid::new_v4()
    ));
    let database_url = format!("sqlite://{}", db_path.display());
    let store = SqliteEventStore::connect(&database_url)
        .await
        .expect("failed to connect to file-backed SQLite");
    (store, db_path)
}
