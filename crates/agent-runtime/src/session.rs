//! Session lifecycle operations extracted from the facade.
//!
//! Each function is a free function that takes its dependencies as parameters,
//! making the session logic independently testable without requiring a full
//! `LocalRuntime`.

use crate::context_budget::UsageCorrector;
use crate::event_emitter::append_and_broadcast;
use crate::task_graph::TaskGraph;
use agent_core::{
    AgentId, CoreError, DomainEvent, EventPayload, PrivacyClassification, SessionId, SessionMeta,
    TaskGraphSnapshot, TaskSnapshot, TraceEntry, WorkspaceId, WorkspaceInfo,
};
use agent_models::ModelLimits;

/// Per-session in-memory state held by `LocalRuntime`. NOT persisted —
/// reconstructed lazily from event history if the process restarts mid-session.
///
/// Stored as `Arc<Mutex<HashMap<String, SessionState>>>` on `LocalRuntime`
/// (the key is `session_id.to_string()`).
#[derive(Debug, Clone, Default)]
pub struct SessionState {
    /// Resolved model limits. `None` until the first call to `set_session_limits`
    /// (typically right after `SessionInitialized` is emitted).
    pub model_limits: Option<ModelLimits>,
    /// EMA-corrector that turns our cl100k_base estimate into something
    /// closer to the provider's reported `input_tokens`.
    pub usage_corrector: UsageCorrector,
    /// Most recent `ContextAssembled.usage.total_tokens` for this session.
    /// Used as the denominator when `update_corrector(real_input_tokens, last_estimate)`
    /// runs on `ModelEvent::Completed`.
    pub last_estimated_tokens: u64,
    /// `true` while a `compact_session` call is in flight. `send_message`
    /// must reject with `CoreError::SessionBusy` when this is set.
    pub compacting: bool,
}
use agent_store::{EventStore, SessionRow};
use futures::stream::BoxStream;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

pub fn temporary_title_from_first_message(content: &str) -> String {
    const MAX_CHARS: usize = 48;
    let trimmed = content.trim();
    let mut title: String = trimmed.chars().take(MAX_CHARS).collect();
    if trimmed.chars().count() > MAX_CHARS {
        title.push('…');
    }
    if title.is_empty() {
        "New conversation".into()
    } else {
        title
    }
}

/// Open a workspace at the given filesystem path.
pub async fn open_workspace<S: EventStore>(
    store: &S,
    event_tx: &tokio::sync::broadcast::Sender<DomainEvent>,
    path: String,
) -> agent_core::Result<WorkspaceInfo> {
    let workspace_id = WorkspaceId::new();
    let session_id = SessionId::new();
    let event = DomainEvent::new(
        workspace_id.clone(),
        session_id,
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::WorkspaceOpened { path: path.clone() },
    );
    append_and_broadcast(store, event_tx, &event).await?;

    // Persist workspace metadata for session recovery
    if let Err(e) = store
        .upsert_workspace(&workspace_id.to_string(), &path)
        .await
    {
        eprintln!("[runtime] Failed to persist workspace metadata: {e}");
    }

    Ok(WorkspaceInfo { workspace_id, path })
}

/// Start a new agent session within a workspace.
///
/// `approval_policy` and `sandbox_policy` carry the persisted double-axis
/// values. When omitted, the runtime stores the default policy pair.
pub async fn start_session<S: EventStore>(
    store: &S,
    event_tx: &tokio::sync::broadcast::Sender<DomainEvent>,
    workspace_id: WorkspaceId,
    model_profile: String,
    approval_policy: Option<String>,
    sandbox_policy: Option<String>,
) -> agent_core::Result<SessionId> {
    let session_id = SessionId::new();
    let event = DomainEvent::new(
        workspace_id.clone(),
        session_id.clone(),
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::SessionInitialized {
            model_profile: model_profile.clone(),
        },
    );
    append_and_broadcast(store, event_tx, &event).await?;

    // Persist session metadata for session recovery.
    let now = chrono::Utc::now().to_rfc3339();
    let (approval_str, sandbox_str) = derive_policy_strings(approval_policy, sandbox_policy);
    let session_row = SessionRow {
        session_id: session_id.to_string(),
        workspace_id: workspace_id.to_string(),
        title: format!("Session using {}", model_profile),
        model_profile,
        model_id: None,
        provider: None,
        approval_policy: Some(approval_str),
        sandbox_policy: Some(sandbox_str),
        deleted_at: None,
        created_at: now.clone(),
        updated_at: now,
    };
    if let Err(e) = store.upsert_session(&session_row).await {
        eprintln!("[runtime] Failed to persist session metadata: {e}");
    }

    Ok(session_id)
}

/// Derive `(approval_policy_str, sandbox_policy_json)` for storage.
fn derive_policy_strings(
    approval_override: Option<String>,
    sandbox_override: Option<String>,
) -> (String, String) {
    use agent_tools::{ApprovalPolicy, SandboxPolicy};
    let approval: ApprovalPolicy = approval_override
        .as_deref()
        .and_then(|s| s.parse().ok())
        .unwrap_or_default();

    let sandbox: SandboxPolicy = sandbox_override
        .as_deref()
        .and_then(|s| serde_json::from_str(s).ok())
        .unwrap_or_default();

    let approval_str = approval.to_string();
    let sandbox_str = serde_json::to_string(&sandbox).unwrap_or_default();
    (approval_str, sandbox_str)
}

/// Cancel a running session.
pub(crate) async fn cancel_session<S: EventStore>(
    store: &S,
    event_tx: &tokio::sync::broadcast::Sender<DomainEvent>,
    workspace_id: WorkspaceId,
    session_id: SessionId,
) -> agent_core::Result<()> {
    let event = DomainEvent::new(
        workspace_id,
        session_id,
        AgentId::system(),
        PrivacyClassification::MinimalTrace,
        EventPayload::SessionCancelled {
            reason: "user requested cancellation".into(),
        },
    );
    append_and_broadcast(store, event_tx, &event).await
}

/// Get the projected state of a session from its event history.
pub async fn get_session_projection<S: EventStore>(
    store: &S,
    session_id: SessionId,
) -> agent_core::Result<agent_core::projection::SessionProjection> {
    let events = store
        .load_session(&session_id)
        .await
        .map_err(|e| CoreError::InvalidState(e.to_string()))?;
    Ok(agent_core::projection::SessionProjection::from_events(
        &events,
    ))
}

/// Get the trace (event history) for a session.
pub async fn get_trace<S: EventStore>(
    store: &S,
    session_id: SessionId,
) -> agent_core::Result<Vec<TraceEntry>> {
    let events = store
        .load_session(&session_id)
        .await
        .map_err(|e| CoreError::InvalidState(e.to_string()))?;
    Ok(events
        .into_iter()
        .map(|event| TraceEntry { event })
        .collect())
}

/// Subscribe to events for a specific session.
pub fn subscribe_session(
    event_tx: &tokio::sync::broadcast::Sender<DomainEvent>,
    session_id: SessionId,
) -> BoxStream<'static, DomainEvent> {
    let mut rx = event_tx.subscribe();
    Box::pin(async_stream::stream! {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    if event.session_id == session_id {
                        yield event;
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    eprintln!("[subscribe_session] Broadcast lagged, skipped {n} events");
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    })
}

/// Subscribe to all domain events.
pub fn subscribe_all(
    event_tx: &tokio::sync::broadcast::Sender<DomainEvent>,
) -> BoxStream<'static, DomainEvent> {
    let mut rx = event_tx.subscribe();
    Box::pin(async_stream::stream! {
        loop {
            match rx.recv().await {
                Ok(event) => yield event,
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    eprintln!("[subscribe_all] Broadcast lagged, skipped {n} events");
                    continue;
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
            }
        }
    })
}

/// List all workspaces.
pub async fn list_workspaces<S: EventStore>(store: &S) -> agent_core::Result<Vec<WorkspaceInfo>> {
    let rows = store
        .list_workspaces()
        .await
        .map_err(|e| CoreError::InvalidState(e.to_string()))?;
    Ok(rows
        .into_iter()
        .map(|r| WorkspaceInfo {
            workspace_id: WorkspaceId::from_string(r.workspace_id),
            path: r.path,
        })
        .collect())
}

/// List all active sessions for a workspace.
pub async fn list_sessions<S: EventStore>(
    store: &S,
    workspace_id: &WorkspaceId,
) -> agent_core::Result<Vec<SessionMeta>> {
    let rows = store
        .list_active_sessions(&workspace_id.to_string())
        .await
        .map_err(|e| CoreError::InvalidState(e.to_string()))?;
    let project_repository = store
        .sqlite_pool()
        .map(agent_store::ProjectMetaRepository::new);
    let mut session_metas = Vec::new();
    for row in rows {
        if let Some(repository) = &project_repository {
            let binding = repository
                .get_session_binding(&row.session_id)
                .await
                .map_err(|error| CoreError::InvalidState(error.to_string()))?;
            if binding.is_some() {
                continue;
            }
        }

        session_metas.push(SessionMeta {
            project_id: None,
            worktree_path: None,
            branch: None,
            visibility: None,
            approval_policy: row.approval_policy.clone(),
            sandbox_policy: row.sandbox_policy.clone(),
            session_id: SessionId::from_string(row.session_id),
            workspace_id: WorkspaceId::from_string(row.workspace_id),
            title: row.title,
            model_profile: row.model_profile,
            model_id: row.model_id,
            provider: row.provider,
            deleted_at: row.deleted_at,
            created_at: row.created_at,
            updated_at: row.updated_at,
        });
    }
    Ok(session_metas)
}

/// Rename a session.
pub async fn rename_session<S: EventStore>(
    store: &S,
    session_id: &SessionId,
    title: String,
) -> agent_core::Result<()> {
    store
        .rename_session(&session_id.to_string(), &title)
        .await
        .map_err(|e| CoreError::InvalidState(e.to_string()))
}

/// Soft-delete a session (marks it as deleted without removing data).
pub async fn soft_delete_session<S: EventStore>(
    store: &S,
    session_id: &SessionId,
) -> agent_core::Result<()> {
    store
        .soft_delete_session(&session_id.to_string())
        .await
        .map_err(|e| CoreError::InvalidState(e.to_string()))
}

/// Permanently hard-delete a session and all associated data.
pub async fn permanently_delete_session<S: EventStore>(
    store: &S,
    session_id: &str,
) -> agent_core::Result<()> {
    store
        .permanently_delete_session(session_id)
        .await
        .map_err(|e| CoreError::InvalidState(e.to_string()))
}

/// Restore an archived session back to visible status.
pub async fn restore_archived_session<S: EventStore>(
    store: &S,
    session_id: &str,
) -> agent_core::Result<()> {
    store
        .restore_archived_session(session_id)
        .await
        .map_err(|e| CoreError::InvalidState(e.to_string()))
}

/// Clean up sessions that have been soft-deleted longer than the given duration.
pub async fn cleanup_expired_sessions<S: EventStore>(
    store: &S,
    older_than: std::time::Duration,
) -> agent_core::Result<usize> {
    store
        .cleanup_expired_sessions(older_than)
        .await
        .map_err(|e| CoreError::InvalidState(e.to_string()))
}

/// Get a snapshot of the task graph for a session.
pub async fn get_task_graph(
    task_graphs: &Arc<Mutex<HashMap<String, TaskGraph>>>,
    session_id: SessionId,
) -> agent_core::Result<TaskGraphSnapshot> {
    let graphs = task_graphs.lock().await;
    match graphs.get(&session_id.to_string()) {
        Some(graph) => {
            let tasks = graph
                .snapshot()
                .into_iter()
                .map(|t| TaskSnapshot {
                    id: t.id,
                    title: t.title,
                    role: t.role,
                    state: t.state,
                    dependencies: t.dependencies,
                    error: t.error,
                    retry_count: t.retry_count,
                    max_retries: t.max_retries,
                    assigned_agent_id: t.assigned_agent_id.as_ref().map(|id| id.to_string()),
                    failure_reason: t.failure_reason.clone(),
                })
                .collect();
            Ok(TaskGraphSnapshot { tasks })
        }
        None => Ok(TaskGraphSnapshot::default()),
    }
}
