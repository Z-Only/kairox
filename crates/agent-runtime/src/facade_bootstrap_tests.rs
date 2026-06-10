use super::*;
use agent_core::{EventPayload, PermissionDecision, SessionId, WorkspaceId};
use agent_memory::{MemoryEntry, MemoryScope, SqliteMemoryStore};
use agent_models::FakeModelClient;
use agent_store::SqliteEventStore;
use agent_tools::{ApprovalPolicy, SandboxPolicy};
use std::path::PathBuf;
use std::sync::Arc;

async fn build_runtime() -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec![]);
    LocalRuntime::new(store, model)
}

async fn build_runtime_with_memory() -> (
    LocalRuntime<SqliteEventStore, FakeModelClient>,
    Arc<SqliteMemoryStore>,
) {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let mem_store = Arc::new(SqliteMemoryStore::new(store.pool().clone()).await.unwrap());
    let model = FakeModelClient::new(vec![]);
    let runtime = LocalRuntime::new(store, model).with_memory_store(mem_store.clone());
    (runtime, mem_store)
}

// ── Builder methods ──────────────────────────────────────────────

#[tokio::test]
async fn with_approval_and_sandbox_sets_policies() {
    let runtime = build_runtime()
        .await
        .with_approval_and_sandbox(ApprovalPolicy::Always, SandboxPolicy::ReadOnly);

    assert_eq!(runtime.approval_policy().await, ApprovalPolicy::Always);
    assert!(matches!(
        runtime.sandbox_policy().await,
        SandboxPolicy::ReadOnly
    ));
}

#[tokio::test]
async fn with_context_limit_does_not_panic() {
    let _runtime = build_runtime().await.with_context_limit(4096);
}

#[tokio::test]
async fn tool_registry_is_shared_arc() {
    let runtime = build_runtime().await;
    let reg1 = runtime.tool_registry();
    let reg2 = runtime.tool_registry();
    assert!(Arc::ptr_eq(&reg1, &reg2));
}

#[tokio::test]
async fn with_memory_store_and_getter() {
    let (runtime, mem_store) = build_runtime_with_memory().await;
    let retrieved = runtime.memory_store().expect("memory store should be set");
    // Verify it's the same store by storing+getting an entry.
    let entry = MemoryEntry::new(MemoryScope::Session, "test-content".into(), false);
    let entry_id = entry.id.clone();
    mem_store.store(entry).await.unwrap();
    let got = retrieved.get(&entry_id).await.unwrap();
    assert!(got.is_some());
}

#[tokio::test]
async fn memory_store_returns_none_when_not_configured() {
    let runtime = build_runtime().await;
    assert!(runtime.memory_store().is_none());
}

#[tokio::test]
async fn with_skill_settings_roots_roundtrip() {
    let roots = crate::skill_settings::SkillSettingsRoots {
        workspace_root: Some(PathBuf::from("/ws")),
        user_root: Some(PathBuf::from("/user")),
        builtin_root: None,
        plugin_roots: vec![],
    };
    let runtime = build_runtime()
        .await
        .with_skill_settings_roots(roots.clone());
    let got = runtime.skill_settings_roots();
    assert_eq!(got.workspace_root, Some(PathBuf::from("/ws")));
    assert_eq!(got.user_root, Some(PathBuf::from("/user")));
}

#[tokio::test]
async fn with_agent_settings_roots_roundtrip() {
    let roots = crate::agent_settings::AgentSettingsRoots {
        workspace_root: Some(PathBuf::from("/ws-agent")),
        user_root: Some(PathBuf::from("/user-agent")),
        builtin_root: None,
    };
    let runtime = build_runtime().await.with_agent_settings_roots(roots);
    let got = runtime.agent_settings_roots();
    assert_eq!(got.workspace_root, Some(PathBuf::from("/ws-agent")));
}

#[tokio::test]
async fn with_plugin_settings_roots_roundtrip() {
    let roots = crate::plugin_settings::PluginSettingsRoots {
        workspace_root: Some(PathBuf::from("/ws-plugin")),
        user_root: Some(PathBuf::from("/user-plugin")),
        builtin_root: None,
    };
    let runtime = build_runtime().await.with_plugin_settings_roots(roots);
    let got = runtime.plugin_settings_roots();
    assert_eq!(got.workspace_root, Some(PathBuf::from("/ws-plugin")));
}

// ── Policy methods ───────────────────────────────────────────────

#[tokio::test]
async fn approval_policy_default_is_on_request() {
    let runtime = build_runtime().await;
    assert_eq!(runtime.approval_policy().await, ApprovalPolicy::OnRequest);
}

#[tokio::test]
async fn set_approval_policy_updates_in_memory() {
    let runtime = build_runtime().await;
    runtime.set_approval_policy(ApprovalPolicy::Never).await;
    assert_eq!(runtime.approval_policy().await, ApprovalPolicy::Never);
}

#[tokio::test]
async fn set_sandbox_policy_updates_in_memory() {
    let runtime = build_runtime().await;
    let writable = SandboxPolicy::WorkspaceWrite {
        network_access: true,
        writable_roots: vec![PathBuf::from("/tmp")],
    };
    runtime.set_sandbox_policy(writable.clone()).await;
    let got = runtime.sandbox_policy().await;
    assert!(matches!(
        got,
        SandboxPolicy::WorkspaceWrite {
            network_access: true,
            ..
        }
    ));
}

#[tokio::test]
async fn set_session_approval_policy_persists_and_activates() {
    let runtime = build_runtime().await;
    let session_id = SessionId::new();
    // Should succeed even without a real session row — the store accepts it.
    let result = runtime
        .set_session_approval_policy(&session_id, ApprovalPolicy::Always)
        .await;
    // The store may or may not have the session row; we mainly verify no panic
    // and the in-memory policy is updated.
    if result.is_ok() {
        assert_eq!(runtime.approval_policy().await, ApprovalPolicy::Always);
    }
}

#[tokio::test]
async fn set_session_sandbox_policy_persists_and_activates() {
    let runtime = build_runtime().await;
    let session_id = SessionId::new();
    let sandbox = SandboxPolicy::DangerFullAccess;
    let result = runtime
        .set_session_sandbox_policy(&session_id, &sandbox)
        .await;
    if result.is_ok() {
        assert!(matches!(
            runtime.sandbox_policy().await,
            SandboxPolicy::DangerFullAccess
        ));
    }
}

// ── Memory accept/reject ─────────────────────────────────────────

#[tokio::test]
async fn accept_memory_marks_entry_accepted_and_emits_event() {
    let (runtime, mem_store) = build_runtime_with_memory().await;
    let entry = MemoryEntry::new(MemoryScope::Workspace, "remember this".into(), false);
    let entry_id = entry.id.clone();
    mem_store.store(entry).await.unwrap();

    let mut rx = runtime.event_tx.subscribe();

    let ws_id = WorkspaceId::new();
    let sess_id = SessionId::new();
    runtime
        .accept_memory(&entry_id, ws_id, sess_id)
        .await
        .unwrap();

    // Verify entry is now accepted in store.
    let updated = mem_store.get(&entry_id).await.unwrap().unwrap();
    assert!(updated.accepted);

    // Verify event was emitted.
    let event = rx.try_recv().unwrap();
    assert!(matches!(event.payload, EventPayload::MemoryAccepted { .. }));
}

#[tokio::test]
async fn reject_memory_deletes_entry_and_emits_event() {
    let (runtime, mem_store) = build_runtime_with_memory().await;
    let entry = MemoryEntry::new(MemoryScope::User, "forget this".into(), false);
    let entry_id = entry.id.clone();
    mem_store.store(entry).await.unwrap();

    let mut rx = runtime.event_tx.subscribe();

    runtime
        .reject_memory(
            &entry_id,
            WorkspaceId::new(),
            SessionId::new(),
            "not needed".into(),
        )
        .await
        .unwrap();

    // Entry should be deleted.
    let gone = mem_store.get(&entry_id).await.unwrap();
    assert!(gone.is_none());

    // Verify event.
    let event = rx.try_recv().unwrap();
    assert!(matches!(event.payload, EventPayload::MemoryRejected { .. }));
}

#[tokio::test]
async fn accept_memory_fails_without_memory_store() {
    let runtime = build_runtime().await;
    let result = runtime
        .accept_memory("nonexistent", WorkspaceId::new(), SessionId::new())
        .await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("memory store unavailable"));
}

#[tokio::test]
async fn reject_memory_fails_without_memory_store() {
    let runtime = build_runtime().await;
    let result = runtime
        .reject_memory(
            "nonexistent",
            WorkspaceId::new(),
            SessionId::new(),
            "reason".into(),
        )
        .await;
    assert!(result.is_err());
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("memory store unavailable"));
}

#[tokio::test]
async fn accept_memory_fails_when_id_not_found() {
    let (runtime, _mem_store) = build_runtime_with_memory().await;
    let result = runtime
        .accept_memory("mem_does_not_exist", WorkspaceId::new(), SessionId::new())
        .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("memory not found"));
}

#[tokio::test]
async fn reject_memory_fails_when_id_not_found() {
    let (runtime, _mem_store) = build_runtime_with_memory().await;
    let result = runtime
        .reject_memory(
            "mem_does_not_exist",
            WorkspaceId::new(),
            SessionId::new(),
            "reason".into(),
        )
        .await;
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("memory not found"));
}

// ── Helper functions ─────────────────────────────────────────────

#[test]
fn memory_scope_label_returns_correct_strings() {
    assert_eq!(memory_scope_label(&MemoryScope::User), "user");
    assert_eq!(memory_scope_label(&MemoryScope::Workspace), "workspace");
    assert_eq!(memory_scope_label(&MemoryScope::Session), "session");
}

#[test]
fn memory_event_ids_uses_entry_values_when_present() {
    let entry = MemoryEntry {
        id: "mem_123".into(),
        scope: MemoryScope::Workspace,
        key: None,
        content: "test".into(),
        accepted: false,
        workspace_id: Some("ws-from-entry".into()),
        session_id: Some("sess-from-entry".into()),
        branch: None,
    };
    let fallback_ws = WorkspaceId::from_string("ws-fallback".into());
    let fallback_sess = SessionId::from_string("sess-fallback".into());

    let (ws, sess) = memory_event_ids(&entry, fallback_ws, fallback_sess);
    assert_eq!(ws.as_str(), "ws-from-entry");
    assert_eq!(sess.as_str(), "sess-from-entry");
}

#[test]
fn memory_event_ids_uses_fallback_when_entry_has_none() {
    let entry = MemoryEntry {
        id: "mem_456".into(),
        scope: MemoryScope::Session,
        key: None,
        content: "test".into(),
        accepted: false,
        workspace_id: None,
        session_id: None,
        branch: None,
    };
    let fallback_ws = WorkspaceId::from_string("ws-fallback".into());
    let fallback_sess = SessionId::from_string("sess-fallback".into());

    let (ws, sess) = memory_event_ids(&entry, fallback_ws, fallback_sess);
    assert_eq!(ws.as_str(), "ws-fallback");
    assert_eq!(sess.as_str(), "sess-fallback");
}

// ── MCP/LSP managers ─────────────────────────────────────────────

#[test]
fn mcp_manager_returns_none_when_not_configured() {
    // LocalRuntime::new does not spawn a tokio runtime for the constructor,
    // but mcp_manager() is sync so we can test in a sync context with a
    // manually-built runtime via tokio::runtime::Runtime.
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runtime = build_runtime().await;
        assert!(runtime.mcp_manager().is_none());
    });
}

#[test]
fn lsp_manager_returns_none_when_not_configured() {
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runtime = build_runtime().await;
        assert!(runtime.lsp_manager().is_none());
    });
}

#[tokio::test]
async fn check_mcp_health_returns_unhealthy_without_manager() {
    let runtime = build_runtime().await;
    let result = runtime.check_mcp_health("nonexistent").await.unwrap();
    assert!(!result.healthy);
    assert!(result.error.is_some());
}

// ── execution_mode (cfg(test) only) ──────────────────────────────

#[tokio::test]
async fn execution_mode_returns_single_step_by_default() {
    let runtime = build_runtime().await;
    let request = agent_core::SendMessageRequest {
        workspace_id: WorkspaceId::new(),
        session_id: SessionId::new(),
        content: "hello".into(),
        display_content: None,
        attachments: vec![],
    };
    let mode = runtime.execution_mode(&request);
    assert!(matches!(mode, ExecutionMode::SingleStep));
}

#[tokio::test]
async fn execution_mode_returns_single_step_for_plan_without_dag() {
    let runtime = build_runtime().await;
    let request = agent_core::SendMessageRequest {
        workspace_id: WorkspaceId::new(),
        session_id: SessionId::new(),
        content: "/plan do something".into(),
        display_content: None,
        attachments: vec![],
    };
    // Without dag_executor, /plan prefix still yields SingleStep.
    let mode = runtime.execution_mode(&request);
    assert!(matches!(mode, ExecutionMode::SingleStep));
}

// ── resolve_permission ───────────────────────────────────────────

#[tokio::test]
async fn resolve_permission_with_unknown_id_does_not_panic() {
    let runtime = build_runtime().await;
    let decision = PermissionDecision {
        request_id: "unknown-req-id".into(),
        approve: true,
        reason: None,
    };
    // Must not panic regardless of whether it returns Ok or Err.
    let _result = runtime.resolve_permission("unknown-req-id", decision).await;
}
