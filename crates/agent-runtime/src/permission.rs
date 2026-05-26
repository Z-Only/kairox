use crate::event_emitter::append_and_broadcast;
use agent_core::{
    AgentId, DomainEvent, EventPayload, PermissionDecision, PrivacyClassification, SessionId,
    WorkspaceId,
};
use agent_store::EventStore;
use agent_tools::{PermissionEngine, PermissionOutcome, ToolRisk};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

type PendingPermissionsMap =
    Arc<Mutex<HashMap<String, tokio::sync::oneshot::Sender<PermissionDecision>>>>;

/// Result of a permission check for a tool invocation.
pub struct ToolPermissionResult {
    pub event: DomainEvent,
    pub should_execute: bool,
}

/// Check permission for a tool call, waiting for user approval if needed.
///
/// This handles all permission modes:
/// - Allowed: returns PermissionGranted event
/// - Denied: returns PermissionDenied event
/// - Interactive: emits PermissionRequested, waits for user decision
#[allow(clippy::too_many_arguments)]
pub async fn check_tool_permission<S: EventStore>(
    store: &S,
    event_tx: &tokio::sync::broadcast::Sender<DomainEvent>,
    permission_engine: &Arc<Mutex<PermissionEngine>>,
    pending_permissions: &PendingPermissionsMap,
    workspace_id: &WorkspaceId,
    session_id: &SessionId,
    tool_call_id: &str,
    tool_id: &str,
    preview: &str,
    risk: &ToolRisk,
    config: &agent_config::Config,
    root_path: Option<&std::path::Path>,
) -> agent_core::Result<ToolPermissionResult> {
    let permission_outcome = permission_engine.lock().await.decide(risk);
    let (event, should_execute) = match &permission_outcome {
        PermissionOutcome::Allowed => (
            DomainEvent::new(
                workspace_id.clone(),
                session_id.clone(),
                AgentId::system(),
                PrivacyClassification::FullTrace,
                EventPayload::PermissionGranted {
                    request_id: tool_call_id.to_string(),
                },
            ),
            true,
        ),
        PermissionOutcome::Denied(reason) => (
            DomainEvent::new(
                workspace_id.clone(),
                session_id.clone(),
                AgentId::system(),
                PrivacyClassification::FullTrace,
                EventPayload::PermissionDenied {
                    request_id: tool_call_id.to_string(),
                    reason: reason.clone(),
                },
            ),
            false,
        ),
        PermissionOutcome::RequiresApproval
        | PermissionOutcome::Pending
        | PermissionOutcome::PromptWithTrust => {
            crate::hooks::run_hooks_logged(
                config,
                agent_config::HookEvent::PermissionRequest,
                tool_id,
                root_path,
                serde_json::json!({
                    "workspace_id": workspace_id,
                    "session_id": session_id,
                    "tool_call_id": tool_call_id,
                    "tool_id": tool_id,
                    "preview": preview,
                }),
            )
            .await;

            // Emit PermissionRequested so the UI can show a prompt,
            // then wait for the user's decision via resolve_permission.
            let request_event = DomainEvent::new(
                workspace_id.clone(),
                session_id.clone(),
                AgentId::system(),
                PrivacyClassification::FullTrace,
                EventPayload::PermissionRequested {
                    request_id: tool_call_id.to_string(),
                    tool_id: tool_id.to_string(),
                    preview: preview.to_string(),
                },
            );
            append_and_broadcast(store, event_tx, &request_event).await?;

            // Wait for the user to resolve the permission request
            let (tx, rx) = tokio::sync::oneshot::channel();
            pending_permissions
                .lock()
                .await
                .insert(tool_call_id.to_string(), tx);

            let decision = rx.await;
            let approved = matches!(decision, Ok(PermissionDecision { approve: true, .. }));

            let result_event = if approved {
                DomainEvent::new(
                    workspace_id.clone(),
                    session_id.clone(),
                    AgentId::system(),
                    PrivacyClassification::FullTrace,
                    EventPayload::PermissionGranted {
                        request_id: tool_call_id.to_string(),
                    },
                )
            } else {
                DomainEvent::new(
                    workspace_id.clone(),
                    session_id.clone(),
                    AgentId::system(),
                    PrivacyClassification::FullTrace,
                    EventPayload::PermissionDenied {
                        request_id: tool_call_id.to_string(),
                        reason: "denied by user".into(),
                    },
                )
            };
            (result_event, approved)
        }
    };
    Ok(ToolPermissionResult {
        event,
        should_execute,
    })
}

/// Resolve a pending permission request (used by GUI Interactive mode).
pub async fn resolve_permission(
    pending_permissions: &PendingPermissionsMap,
    request_id: &str,
    decision: PermissionDecision,
) -> agent_core::Result<()> {
    if let Some(tx) = pending_permissions.lock().await.remove(request_id) {
        let _ = tx.send(decision);
    }
    Ok(())
}

#[cfg(test)]
mod permission_tests {
    use super::*;

    fn pending_map() -> PendingPermissionsMap {
        Arc::new(Mutex::new(HashMap::new()))
    }

    fn approve(request_id: &str) -> PermissionDecision {
        PermissionDecision {
            request_id: request_id.to_string(),
            approve: true,
            reason: None,
        }
    }

    fn deny(request_id: &str, reason: &str) -> PermissionDecision {
        PermissionDecision {
            request_id: request_id.to_string(),
            approve: false,
            reason: Some(reason.to_string()),
        }
    }

    #[tokio::test]
    async fn resolve_permission_delivers_approval_to_waiting_receiver() {
        let pending = pending_map();
        let (tx, rx) = tokio::sync::oneshot::channel();
        pending.lock().await.insert("call-1".to_string(), tx);

        resolve_permission(&pending, "call-1", approve("call-1"))
            .await
            .expect("resolve should succeed");

        let decision = rx.await.expect("sender should be alive");
        assert!(decision.approve);
        assert_eq!(decision.request_id, "call-1");
        assert!(decision.reason.is_none());

        // The pending entry is consumed by remove() and not reinserted.
        assert!(pending.lock().await.is_empty());
    }

    #[tokio::test]
    async fn resolve_permission_delivers_denial_with_reason() {
        let pending = pending_map();
        let (tx, rx) = tokio::sync::oneshot::channel();
        pending.lock().await.insert("call-2".to_string(), tx);

        resolve_permission(&pending, "call-2", deny("call-2", "blocked by policy"))
            .await
            .expect("resolve should succeed");

        let decision = rx.await.expect("sender should be alive");
        assert!(!decision.approve);
        assert_eq!(decision.reason.as_deref(), Some("blocked by policy"));
    }

    #[tokio::test]
    async fn resolve_permission_is_a_noop_when_request_id_is_unknown() {
        let pending = pending_map();
        let (tx, _rx) = tokio::sync::oneshot::channel();
        pending.lock().await.insert("call-3".to_string(), tx);

        resolve_permission(&pending, "call-unknown", approve("call-unknown"))
            .await
            .expect("resolve should succeed even when request_id is missing");

        // The unrelated entry is left intact — only matching ids are removed.
        let map = pending.lock().await;
        assert!(map.contains_key("call-3"));
        assert_eq!(map.len(), 1);
    }

    #[tokio::test]
    async fn resolve_permission_drops_decision_silently_when_receiver_already_gone() {
        let pending = pending_map();
        let (tx, rx) = tokio::sync::oneshot::channel();
        pending.lock().await.insert("call-4".to_string(), tx);
        // Drop the receiver to simulate a UI that abandoned the request.
        drop(rx);

        // The tx.send call inside resolve_permission will return Err but we
        // explicitly discard it; the function should still return Ok and
        // remove the entry.
        resolve_permission(&pending, "call-4", approve("call-4"))
            .await
            .expect("resolve should not surface a closed-receiver error");

        assert!(pending.lock().await.is_empty());
    }

    #[tokio::test]
    async fn resolve_permission_only_removes_the_targeted_entry() {
        let pending = pending_map();
        let (tx_a, rx_a) = tokio::sync::oneshot::channel();
        let (tx_b, _rx_b) = tokio::sync::oneshot::channel();
        {
            let mut map = pending.lock().await;
            map.insert("a".to_string(), tx_a);
            map.insert("b".to_string(), tx_b);
        }

        resolve_permission(&pending, "a", approve("a"))
            .await
            .expect("resolve a");

        // Receiver `a` got the decision; entry `b` is still pending.
        let decision = rx_a.await.expect("a sender alive");
        assert!(decision.approve);

        let map = pending.lock().await;
        assert!(!map.contains_key("a"));
        assert!(map.contains_key("b"));
        assert_eq!(map.len(), 1);
    }
}
