pub mod event_store;

pub use event_store::{EventStore, SessionRow, SqliteEventStore, WorkspaceRow};

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
