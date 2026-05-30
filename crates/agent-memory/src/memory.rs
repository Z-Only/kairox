use serde::{Deserialize, Serialize};
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "specta", derive(specta::Type))]
#[serde(rename_all = "PascalCase")]
pub enum MemoryScope {
    User,
    Workspace,
    Session,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MemoryEntry {
    pub id: String,
    pub scope: MemoryScope,
    pub key: Option<String>,
    pub content: String,
    pub accepted: bool,
    pub session_id: Option<String>,
    pub workspace_id: Option<String>,
}

impl MemoryEntry {
    pub fn new(scope: MemoryScope, content: String, accepted: bool) -> Self {
        Self {
            id: format!("mem_{}", uuid::Uuid::new_v4().simple()),
            scope,
            key: None,
            content,
            accepted,
            session_id: None,
            workspace_id: None,
        }
    }

    pub fn from_marker(
        marker: crate::marker::MemoryMarker,
        session_id: Option<String>,
        workspace_id: Option<String>,
        accepted: bool,
    ) -> Self {
        Self {
            id: format!("mem_{}", uuid::Uuid::new_v4().simple()),
            scope: marker.scope,
            key: marker.key,
            content: marker.content,
            accepted,
            session_id,
            workspace_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryDecision {
    Accept,
    Reject(String),
}

pub fn durable_memory_requires_confirmation(scope: &MemoryScope) -> bool {
    matches!(scope, MemoryScope::User | MemoryScope::Workspace)
}

#[cfg(test)]
#[path = "memory_tests.rs"]
mod tests;
