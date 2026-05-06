//! Memory retrieval and persistence logic extracted from the runtime facade.
//!
//! This module is responsible for two operations that occur during the
//! `send_message` flow:
//!
//! 1. **Retrieval** – before the system prompt is sent to the model, relevant
//!    memories are fetched from the `MemoryStore` and formatted as a section
//!    that gets appended to the prompt.
//!
//! 2. **Storage** – after the model responds, `<memory>` markers are extracted
//!    from the assistant text and persisted according to the current
//!    `PermissionMode` and the memory scope.

use crate::event_emitter::append_and_broadcast;
use agent_core::{
    AgentId, DomainEvent, EventPayload, PermissionDecision, PrivacyClassification, SessionId,
    WorkspaceId,
};
use agent_memory::{
    durable_memory_requires_confirmation, extract_memory_markers, MemoryEntry, MemoryQuery,
    MemoryStore,
};
use agent_store::EventStore;
use agent_tools::{PermissionEngine, PermissionMode};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Mutex;

/// Retrieve relevant memories and format them as a system prompt section.
///
/// Returns `None` if no relevant memories are found (or if `memory_store` is
/// `None`).  The caller should append the returned string to the system prompt
/// verbatim – it already includes the Markdown header.
pub async fn retrieve_memory_section(
    memory_store: &Option<Arc<dyn MemoryStore>>,
    user_content: &str,
) -> Option<String> {
    let mem_store = memory_store.as_ref()?;

    let keywords = agent_memory::extract_keywords(user_content);

    // First try keyword-based retrieval; if no matches found,
    // fall back to returning all accepted user/workspace memories.
    // This ensures cross-session context is always available even
    // when the query keywords don't directly match memory content
    // (common with Chinese text where extract_keywords is limited).
    let mut memories = mem_store
        .query(MemoryQuery {
            scope: None,
            keywords: keywords.clone(),
            limit: 20,
            session_id: None,
            workspace_id: None,
        })
        .await
        .unwrap_or_default();

    if memories.is_empty() {
        memories = mem_store
            .query(MemoryQuery {
                scope: None,
                keywords: Vec::new(),
                limit: 20,
                session_id: None,
                workspace_id: None,
            })
            .await
            .unwrap_or_default();
    }

    if memories.is_empty() {
        return None;
    }

    let memory_section = memories
        .iter()
        .filter(|m| m.accepted)
        .map(|m| {
            let scope_label = match m.scope {
                agent_memory::MemoryScope::User => "user",
                agent_memory::MemoryScope::Workspace => "workspace",
                agent_memory::MemoryScope::Session => "session",
            };
            match &m.key {
                Some(k) => format!("- [{scope_label}] {k}: {}", m.content),
                None => format!("- [{scope_label}] {}", m.content),
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    if memory_section.is_empty() {
        return None;
    }

    Some(format!(
        "\n\n## Relevant Memories\nThe following memories were previously saved and may be relevant to the user's request. Use this context naturally in your response.\n\n{memory_section}"
    ))
}

/// Process memory markers extracted from the assistant response.
///
/// Strips `<memory>` tags from display text and persists each marker according
/// to the active `PermissionMode`:
///
/// - **Session scope** – auto-accepted and stored.
/// - **User / Workspace scope** – behaviour depends on `PermissionMode`:
///   - `Interactive` – emits `MemoryProposed`, waits for a oneshot decision,
///     then emits `MemoryAccepted` or `MemoryRejected`.
///   - `Suggest` / `ReadOnly` – auto-denied (`MemoryRejected`).
///   - `Agent` / `Autonomous` – auto-accepted (`MemoryAccepted`).
#[allow(clippy::too_many_arguments)]
pub async fn store_memory_markers<S: EventStore>(
    store: &S,
    event_tx: &tokio::sync::broadcast::Sender<DomainEvent>,
    permission_engine: &Arc<Mutex<PermissionEngine>>,
    pending_permissions: &Arc<
        Mutex<HashMap<String, tokio::sync::oneshot::Sender<PermissionDecision>>>,
    >,
    memory_store: &Option<Arc<dyn MemoryStore>>,
    workspace_id: &WorkspaceId,
    session_id: &SessionId,
    assistant_text: &str,
) {
    if assistant_text.is_empty() {
        return;
    }
    let Some(ref mem_store) = memory_store else {
        return;
    };

    let markers = extract_memory_markers(assistant_text);
    for marker in markers {
        let entry = MemoryEntry::from_marker(marker, None, None, false);
        let mem_id = entry.id.clone();
        let mem_scope = entry.scope.clone();
        let mem_key = entry.key.clone();
        let mem_content = entry.content.clone();
        if durable_memory_requires_confirmation(&entry.scope) {
            match *permission_engine.lock().await.mode() {
                PermissionMode::Interactive => {
                    let (tx, rx) = tokio::sync::oneshot::channel();
                    pending_permissions.lock().await.insert(mem_id.clone(), tx);
                    let perm_event = DomainEvent::new(
                        workspace_id.clone(),
                        session_id.clone(),
                        AgentId::system(),
                        PrivacyClassification::FullTrace,
                        EventPayload::MemoryProposed {
                            memory_id: mem_id.clone(),
                            scope: format!("{:?}", entry.scope).to_lowercase(),
                            key: mem_key.clone(),
                            content: mem_content.clone(),
                        },
                    );
                    let _ = append_and_broadcast(store, event_tx, &perm_event).await;
                    match rx.await {
                        Ok(PermissionDecision { approve: true, .. }) => {
                            let mut accepted = entry.clone();
                            accepted.accepted = true;
                            let _ = mem_store.store(accepted).await;
                            let accept_event = DomainEvent::new(
                                workspace_id.clone(),
                                session_id.clone(),
                                AgentId::system(),
                                PrivacyClassification::FullTrace,
                                EventPayload::MemoryAccepted {
                                    memory_id: mem_id,
                                    scope: format!("{:?}", mem_scope).to_lowercase(),
                                    key: mem_key,
                                    content: mem_content,
                                },
                            );
                            let _ = append_and_broadcast(store, event_tx, &accept_event).await;
                        }
                        Ok(PermissionDecision {
                            approve: false,
                            reason,
                            ..
                        }) => {
                            let reject_event = DomainEvent::new(
                                workspace_id.clone(),
                                session_id.clone(),
                                AgentId::system(),
                                PrivacyClassification::FullTrace,
                                EventPayload::MemoryRejected {
                                    memory_id: mem_id,
                                    reason: reason.unwrap_or_else(|| "denied".into()),
                                },
                            );
                            let _ = append_and_broadcast(store, event_tx, &reject_event).await;
                        }
                        Err(_) => {
                            let reject_event = DomainEvent::new(
                                workspace_id.clone(),
                                session_id.clone(),
                                AgentId::system(),
                                PrivacyClassification::FullTrace,
                                EventPayload::MemoryRejected {
                                    memory_id: mem_id,
                                    reason: "cancelled".into(),
                                },
                            );
                            let _ = append_and_broadcast(store, event_tx, &reject_event).await;
                        }
                    }
                }
                PermissionMode::Suggest | PermissionMode::ReadOnly => {
                    let reject_event = DomainEvent::new(
                        workspace_id.clone(),
                        session_id.clone(),
                        AgentId::system(),
                        PrivacyClassification::FullTrace,
                        EventPayload::MemoryRejected {
                            memory_id: mem_id,
                            reason: "Auto-denied in Suggest mode".into(),
                        },
                    );
                    let _ = append_and_broadcast(store, event_tx, &reject_event).await;
                }
                PermissionMode::Agent | PermissionMode::Autonomous => {
                    let mut accepted = entry.clone();
                    accepted.accepted = true;
                    let _ = mem_store.store(accepted).await;
                    let accept_event = DomainEvent::new(
                        workspace_id.clone(),
                        session_id.clone(),
                        AgentId::system(),
                        PrivacyClassification::FullTrace,
                        EventPayload::MemoryAccepted {
                            memory_id: mem_id,
                            scope: format!("{:?}", mem_scope).to_lowercase(),
                            key: mem_key,
                            content: mem_content,
                        },
                    );
                    let _ = append_and_broadcast(store, event_tx, &accept_event).await;
                }
            }
        } else {
            // Session scope: auto-accept
            let mut accepted = entry.clone();
            accepted.accepted = true;
            let _ = mem_store.store(accepted).await;
            let accept_event = DomainEvent::new(
                workspace_id.clone(),
                session_id.clone(),
                AgentId::system(),
                PrivacyClassification::FullTrace,
                EventPayload::MemoryAccepted {
                    memory_id: mem_id,
                    scope: format!("{:?}", mem_scope).to_lowercase(),
                    key: mem_key,
                    content: mem_content,
                },
            );
            let _ = append_and_broadcast(store, event_tx, &accept_event).await;
        }
    }
}
