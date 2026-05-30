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
    let entry = MemoryEntry::from_marker(marker, Some("ses_1".into()), Some("wrk_1".into()), true);
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
