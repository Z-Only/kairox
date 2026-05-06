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
