pub mod autonomous_store;
pub mod event_store;
pub mod project_meta;
pub mod trajectory_store;

pub use autonomous_store::{
    AutonomousCheckpointRow, AutonomousTaskRow, AutonomousTaskStore, SessionChainRow,
    SqliteAutonomousTaskStore,
};
pub use event_store::{EventStore, SessionRow, SqliteEventStore, WorkspaceRow};
pub use project_meta::{
    ProjectMetaRepository, ProjectRow, ProjectSessionBindingRow, SessionVisibilityRow,
};
pub use trajectory_store::{SqliteTrajectoryStore, TrajectoryStore};

pub const STORE_CRATE_NAME: &str = "agent-store";

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, StoreError>;
