#[derive(Debug, Clone, PartialEq, Eq)]
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
mod tests {
    use super::*;

    #[test]
    fn new_entry_has_mem_prefix_id() {
        let entry = MemoryEntry::new(MemoryScope::Session, "test".into(), true);
        assert!(entry.id.starts_with("mem_"));
        assert!(entry.key.is_none());
        assert!(entry.accepted);
    }

    #[test]
    fn from_marker_preserves_scope_and_key() {
        use crate::marker::MemoryMarker;
        let marker = MemoryMarker {
            scope: MemoryScope::Workspace,
            key: Some("build-cmd".into()),
            content: "Use cargo nextest".into(),
        };
        let entry =
            MemoryEntry::from_marker(marker, Some("ses_1".into()), Some("wrk_1".into()), true);
        assert_eq!(entry.scope, MemoryScope::Workspace);
        assert_eq!(entry.key, Some("build-cmd".into()));
        assert_eq!(entry.content, "Use cargo nextest");
        assert_eq!(entry.session_id, Some("ses_1".into()));
        assert!(entry.accepted);
    }

    #[test]
    fn durable_memory_requires_confirmation_for_user_and_workspace() {
        assert!(durable_memory_requires_confirmation(&MemoryScope::User));
        assert!(durable_memory_requires_confirmation(
            &MemoryScope::Workspace
        ));
        assert!(!durable_memory_requires_confirmation(&MemoryScope::Session));
    }
}
