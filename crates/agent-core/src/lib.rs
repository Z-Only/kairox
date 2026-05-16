pub mod account;
pub mod build_info;
pub mod config_scope;
pub mod context_types;
pub mod effective;
pub mod error;
pub mod events;
pub mod facade;
pub mod ids;
pub mod manifest;
pub mod projection;
pub mod task_types;

pub const CORE_CRATE_NAME: &str = "agent-core";

pub use account::{AccountService, AccountState, LocalNoAccountService};
pub use config_scope::ConfigScope;
pub use context_types::{ContextSource, ContextUsage};
pub use effective::EffectiveItem;
pub use error::CoreError;
pub use events::{CompactionReason, DomainEvent, EventPayload, PrivacyClassification};
pub use facade::{
    ActivateSkillRequest, ActiveSkillView, AddCatalogSourceRequest, AgentStatusInfo, AppFacade,
    AttachmentInfo, CatalogQuery, CatalogSourceView, DeactivateSkillRequest, InstallOutcomeView,
    InstallRequest, InstalledEntry, InstructionsUpdateInput, InstructionsView, McpFacade,
    PermissionDecision, ProjectFacade, ProjectGitStatus, ProjectGitStatusKind,
    ProjectInstructionSummary, ProjectMeta, ProjectSessionBinding, ProjectSessionVisibility,
    SendMessageRequest, ServerEntry, SessionFacade, SessionMeta, SkillDetail, SkillView,
    SkillsFacade, StartSessionRequest, TaskGraphSnapshot, TaskSnapshot, TraceEntry, WorkspaceInfo,
};
pub use ids::{AgentId, ProjectId, SessionId, TaskId, WorkspaceId};
pub use manifest::{ExtensionManifest, ExtensionType};
pub use projection::{
    CompactionStatus, ProjectedMessage, ProjectedModelLimits, ProjectedRole, SessionProjection,
};
pub use task_types::{
    AgentRole, BackoffStrategy, FailurePolicy, RetryConfig, TaskFailureReason, TaskState,
};

pub type Result<T> = std::result::Result<T, CoreError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_core_crate_name() {
        assert_eq!(CORE_CRATE_NAME, "agent-core");
    }
}
