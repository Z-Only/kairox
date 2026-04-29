use serde::{Deserialize, Serialize};
use std::fmt;
use uuid::Uuid;

macro_rules! prefixed_id {
    ($name:ident, $prefix:literal) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new() -> Self {
                Self(format!("{}_{}", $prefix, Uuid::new_v4().simple()))
            }

            pub fn as_str(&self) -> &str {
                &self.0
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

#[cfg(test)]
mod tests {
    use super::AgentId;

    #[test]
    fn agent_id_exposes_string_value_consistently() {
        let agent_id = AgentId::planner();

        assert_eq!(agent_id.as_str(), "agent_planner");
        assert_eq!(agent_id.to_string(), "agent_planner");
    }
}
