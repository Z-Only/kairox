pub mod error;
pub mod events;
pub mod facade;
pub mod ids;
pub mod projection;

pub const CORE_CRATE_NAME: &str = "agent-core";

pub use error::CoreError;
pub use events::{DomainEvent, EventPayload, PrivacyClassification};
pub use facade::{
    AppFacade, PermissionDecision, SendMessageRequest, StartSessionRequest, TraceEntry,
    WorkspaceInfo,
};
pub use ids::{AgentId, SessionId, TaskId, WorkspaceId};

pub type Result<T> = std::result::Result<T, CoreError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_core_crate_name() {
        assert_eq!(CORE_CRATE_NAME, "agent-core");
    }
}
