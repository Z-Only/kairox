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

pub type PendingPermissionsMap = Arc<Mutex<HashMap<String, PendingPermission>>>;

pub struct PendingPermission {
    session_id: SessionId,
    reply: tokio::sync::oneshot::Sender<PermissionDecision>,
}

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
            pending_permissions.lock().await.insert(
                tool_call_id.to_string(),
                PendingPermission {
                    session_id: session_id.clone(),
                    reply: tx,
                },
            );

            let (approved, denial_reason) = match rx.await {
                Ok(PermissionDecision {
                    approve, reason, ..
                }) => (
                    approve,
                    reason.unwrap_or_else(|| "denied by user".to_string()),
                ),
                Err(_) => (false, "permission request abandoned".to_string()),
            };

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
                        reason: denial_reason,
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
    if let Some(pending) = pending_permissions.lock().await.remove(request_id) {
        let _ = pending.reply.send(decision);
    }
    Ok(())
}

/// Resolve all pending permission requests for a cancelled session as denials.
///
/// Session cancellation is delivered through a cancellation token, but a turn
/// can be parked inside `check_tool_permission` waiting for a UI decision. The
/// token cannot make progress until that oneshot resolves, so cancellation must
/// also close pending permission requests for the same session.
pub async fn deny_pending_permissions_for_session(
    pending_permissions: &PendingPermissionsMap,
    session_id: &SessionId,
    reason: &str,
) -> agent_core::Result<Vec<String>> {
    let pending = {
        let mut map = pending_permissions.lock().await;
        let matching_ids: Vec<String> = map
            .iter()
            .filter_map(|(request_id, pending)| {
                if pending.session_id == *session_id {
                    Some(request_id.clone())
                } else {
                    None
                }
            })
            .collect();
        matching_ids
            .into_iter()
            .filter_map(|request_id| map.remove(&request_id).map(|pending| (request_id, pending)))
            .collect::<Vec<_>>()
    };

    let mut denied_request_ids = Vec::with_capacity(pending.len());
    for (request_id, pending) in pending {
        let _ = pending.reply.send(PermissionDecision {
            request_id: request_id.clone(),
            approve: false,
            reason: Some(reason.to_string()),
        });
        denied_request_ids.push(request_id);
    }

    Ok(denied_request_ids)
}

#[cfg(test)]
#[path = "permission_tests.rs"]
mod tests;
