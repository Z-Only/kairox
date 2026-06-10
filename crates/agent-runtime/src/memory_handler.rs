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
//!    from the assistant text and persisted according to their memory scope.

use crate::event_emitter::append_and_broadcast;
use agent_core::{
    AgentId, DomainEvent, EventPayload, PrivacyClassification, SessionId, WorkspaceId,
};
use agent_memory::{
    durable_memory_requires_confirmation, extract_memory_markers, MemoryEntry, MemoryQuery,
    MemoryStore,
};
use agent_store::EventStore;
use std::sync::Arc;

/// Retrieve relevant memories and format them as a system prompt section.
///
/// Returns `None` if no relevant memories are found (or if `memory_store` is
/// `None`).  The caller should append the returned string to the system prompt
/// verbatim – it already includes the Markdown header.
pub async fn retrieve_memory_section(
    memory_store: &Option<Arc<dyn MemoryStore>>,
    user_content: &str,
) -> Option<String> {
    let memories = retrieve_relevant_memories(memory_store, user_content).await;
    render_memory_section(&memories)
}

/// Retrieve accepted memories relevant to the current user content.
///
/// Keyword search is attempted first. If it finds no matches, fall back to all
/// accepted durable memories so cross-session context remains available even
/// for sparse queries or key-only prompts.
pub async fn retrieve_relevant_memories(
    memory_store: &Option<Arc<dyn MemoryStore>>,
    user_content: &str,
) -> Vec<MemoryEntry> {
    retrieve_relevant_memories_for_context(memory_store, user_content, None, None, None).await
}

pub async fn retrieve_relevant_memories_for_context(
    memory_store: &Option<Arc<dyn MemoryStore>>,
    user_content: &str,
    session_id: Option<String>,
    workspace_id: Option<String>,
    branch: Option<String>,
) -> Vec<MemoryEntry> {
    let Some(mem_store) = memory_store.as_ref() else {
        return Vec::new();
    };

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
            session_id: session_id.clone(),
            workspace_id: workspace_id.clone(),
            branch: branch.clone(),
        })
        .await
        .unwrap_or_default();

    if memories.is_empty() {
        memories = mem_store
            .query(MemoryQuery {
                scope: None,
                keywords: Vec::new(),
                limit: 20,
                session_id,
                workspace_id,
                branch,
            })
            .await
            .unwrap_or_default();
    }

    memories
}

/// Render the model-facing memory prompt section.
pub fn render_memory_section(memories: &[MemoryEntry]) -> Option<String> {
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
/// to the memory scope:
///
/// - **Session scope** – always auto-accepted and stored.
/// - **User / Workspace scope** – stored as a pending proposal. The user must
///   explicitly accept or reject it before it can affect future context.
pub async fn store_memory_markers<S: EventStore>(
    store: &S,
    event_tx: &tokio::sync::broadcast::Sender<DomainEvent>,
    memory_store: &Option<Arc<dyn MemoryStore>>,
    workspace_id: &WorkspaceId,
    session_id: &SessionId,
    assistant_text: &str,
) {
    store_memory_markers_with_branch(
        store,
        event_tx,
        memory_store,
        workspace_id,
        session_id,
        assistant_text,
        None,
    )
    .await;
}

pub async fn store_memory_markers_with_branch<S: EventStore>(
    store: &S,
    event_tx: &tokio::sync::broadcast::Sender<DomainEvent>,
    memory_store: &Option<Arc<dyn MemoryStore>>,
    workspace_id: &WorkspaceId,
    session_id: &SessionId,
    assistant_text: &str,
    branch: Option<&str>,
) {
    if assistant_text.is_empty() {
        return;
    }
    let Some(ref mem_store) = memory_store else {
        return;
    };

    let markers = extract_memory_markers(assistant_text);
    for marker in markers {
        let entry = MemoryEntry::from_marker_with_branch(
            marker,
            Some(session_id.to_string()),
            Some(workspace_id.to_string()),
            branch.map(ToOwned::to_owned),
            false,
        );
        let mem_id = entry.id.clone();
        let mem_scope = entry.scope.clone();
        let mem_key = entry.key.clone();
        let mem_content = entry.content.clone();
        let requires_confirmation = durable_memory_requires_confirmation(&entry.scope);

        if requires_confirmation {
            if mem_store.store(entry).await.is_err() {
                continue;
            }
            let propose_event = DomainEvent::new(
                workspace_id.clone(),
                session_id.clone(),
                AgentId::system(),
                PrivacyClassification::FullTrace,
                EventPayload::MemoryProposed {
                    memory_id: mem_id,
                    scope: format!("{:?}", mem_scope).to_lowercase(),
                    key: mem_key,
                    content: mem_content,
                },
            );
            let _ = append_and_broadcast(store, event_tx, &propose_event).await;
        } else {
            let mut accepted = entry.clone();
            accepted.accepted = true;
            if mem_store.store(accepted).await.is_err() {
                continue;
            }
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

#[cfg(test)]
#[path = "memory_handler_tests.rs"]
mod memory_handler_tests;
