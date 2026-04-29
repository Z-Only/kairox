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
    pub content: String,
    pub accepted: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MemoryDecision {
    Accept,
    Reject(String),
}

pub fn durable_memory_requires_confirmation(scope: &MemoryScope) -> bool {
    matches!(scope, MemoryScope::User | MemoryScope::Workspace)
}
