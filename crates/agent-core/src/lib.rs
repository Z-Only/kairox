pub mod error;
pub mod events;
pub mod ids;
pub mod projection;

pub const CORE_CRATE_NAME: &str = "agent-core";

pub use error::CoreError;
pub use events::{DomainEvent, EventPayload, PrivacyClassification};
pub use ids::{AgentId, SessionId, TaskId, WorkspaceId};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_core_crate_name() {
        assert_eq!(CORE_CRATE_NAME, "agent-core");
    }
}
