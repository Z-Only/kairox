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
#[path = "ids_tests.rs"]
mod tests;
