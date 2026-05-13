use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

macro_rules! prefixed_id {
    ($name:ident, $prefix:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl $name {
            pub fn new() -> Self {
                Self(format!("{}_{}", $prefix, Uuid::new_v4().simple()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }

            /// Reconstruct an ID from a previously serialized string.
            /// This should only be used when receiving IDs from external sources
            /// (e.g., Tauri frontend, API). Prefer `new()` for creating fresh IDs.
            pub fn from_string(s: String) -> Self {
                Self(s)
            }
        }

        impl From<String> for $name {
            fn from(s: String) -> Self {
                Self(s)
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

prefixed_id!(WorkspaceId, "wrk");
prefixed_id!(SessionId, "ses");
prefixed_id!(TaskId, "tsk");
prefixed_id!(ProjectId, "prj");

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentId(String);

impl AgentId {
    pub fn system() -> Self {
        Self("agent_system".into())
    }

    pub fn planner() -> Self {
        Self("agent_planner".into())
    }

    pub fn worker(worker_name: impl Into<String>) -> Self {
        Self(format!("agent_worker_{}", worker_name.into()))
    }

    pub fn reviewer() -> Self {
        Self("agent_reviewer".into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AgentId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// --- specta Type implementations for ID newtypes ---
// These map IDs to TypeScript `string` since they serialize transparently.
#[cfg(feature = "specta")]
use specta::datatype::DataType;

#[cfg(feature = "specta")]
impl specta::Type for WorkspaceId {
    fn definition(types: &mut specta::Types) -> DataType {
        <String as specta::Type>::definition(types)
    }
}

#[cfg(feature = "specta")]
impl specta::Type for SessionId {
    fn definition(types: &mut specta::Types) -> DataType {
        <String as specta::Type>::definition(types)
    }
}

#[cfg(feature = "specta")]
impl specta::Type for TaskId {
    fn definition(types: &mut specta::Types) -> DataType {
        <String as specta::Type>::definition(types)
    }
}

#[cfg(feature = "specta")]
impl specta::Type for ProjectId {
    fn definition(types: &mut specta::Types) -> DataType {
        <String as specta::Type>::definition(types)
    }
}

#[cfg(feature = "specta")]
impl specta::Type for AgentId {
    fn definition(types: &mut specta::Types) -> DataType {
        <String as specta::Type>::definition(types)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_id_exposes_string_value_consistently() {
        let agent_id = AgentId::planner();

        assert_eq!(agent_id.as_str(), "agent_planner");
        assert_eq!(agent_id.to_string(), "agent_planner");
    }

    #[test]
    fn session_id_creation_and_display() {
        let id = SessionId::new();
        let displayed = id.to_string();

        // Display produces a non-empty string with the expected prefix.
        assert!(!displayed.is_empty(), "SessionId Display must be non-empty");
        assert!(
            displayed.starts_with("ses_"),
            "SessionId must start with 'ses_', got: {displayed}"
        );

        // Roundtrip: Display → from_string → Display again.
        let roundtripped = SessionId::from_string(displayed.clone());
        assert_eq!(
            roundtripped.to_string(),
            displayed,
            "SessionId Display → from_string roundtrip mismatch"
        );

        // as_str matches Display.
        assert_eq!(id.as_str(), displayed);
    }

    #[test]
    fn workspace_id_creation_and_display() {
        let id = WorkspaceId::new();
        let displayed = id.to_string();

        // Display produces a non-empty string with the expected prefix.
        assert!(
            !displayed.is_empty(),
            "WorkspaceId Display must be non-empty"
        );
        assert!(
            displayed.starts_with("wrk_"),
            "WorkspaceId must start with 'wrk_', got: {displayed}"
        );

        // Roundtrip: Display → from_string → Display again.
        let roundtripped = WorkspaceId::from_string(displayed.clone());
        assert_eq!(
            roundtripped.to_string(),
            displayed,
            "WorkspaceId Display → from_string roundtrip mismatch"
        );

        // as_str matches Display.
        assert_eq!(id.as_str(), displayed);
    }

    #[test]
    fn session_id_default_creates_fresh_id() {
        let id = SessionId::default();
        assert!(!id.to_string().is_empty());
        assert!(id.to_string().starts_with("ses_"));
    }

    #[test]
    fn workspace_id_default_creates_fresh_id() {
        let id = WorkspaceId::default();
        assert!(!id.to_string().is_empty());
        assert!(id.to_string().starts_with("wrk_"));
    }

    #[test]
    fn session_id_from_string_preserves_exact_value() {
        let original = "ses_custom_abc123".to_string();
        let id = SessionId::from_string(original.clone());
        assert_eq!(id.to_string(), original);
        assert_eq!(id.as_str(), "ses_custom_abc123");
    }

    #[test]
    fn workspace_id_from_string_preserves_exact_value() {
        let original = "wrk_custom_xyz789".to_string();
        let id = WorkspaceId::from_string(original.clone());
        assert_eq!(id.to_string(), original);
        assert_eq!(id.as_str(), "wrk_custom_xyz789");
    }

    #[test]
    fn session_id_from_impl_preserves_value() {
        let s = "ses_from_impl".to_string();
        let id: SessionId = s.clone().into();
        assert_eq!(id.to_string(), s);
    }

    #[test]
    fn workspace_id_from_impl_preserves_value() {
        let s = "wrk_from_impl".to_string();
        let id: WorkspaceId = s.clone().into();
        assert_eq!(id.to_string(), s);
    }
}
