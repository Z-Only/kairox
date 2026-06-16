use crate::event_emitter::append_and_broadcast;
use agent_core::{
    AgentId, DomainEvent, EventPayload, PrivacyClassification, SessionId, TaskConfirmationDecision,
    TaskConfirmationOption, WorkspaceId,
};
use agent_store::EventStore;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::Mutex;

pub const TASK_CONFIRMATION_TOOL: &str = "task_confirmation.request";

pub type PendingTaskConfirmationsMap = Arc<Mutex<HashMap<String, PendingTaskConfirmation>>>;

pub struct PendingTaskConfirmation {
    session_id: SessionId,
    options: Vec<TaskConfirmationOption>,
    allow_multiple: bool,
    allow_custom: bool,
    reply: tokio::sync::oneshot::Sender<TaskConfirmationDecision>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskConfirmationRequest {
    pub request_id: String,
    pub prompt: String,
    pub options: Vec<TaskConfirmationOption>,
    pub allow_multiple: bool,
    pub allow_custom: bool,
}

#[derive(Debug, Deserialize)]
struct TaskConfirmationToolArgs {
    prompt: String,
    #[serde(default)]
    options: Vec<TaskConfirmationOption>,
    #[serde(default)]
    allow_multiple: bool,
    #[serde(default = "default_true")]
    allow_custom: bool,
}

fn default_true() -> bool {
    true
}

pub fn tool_definition() -> agent_models::ToolDefinition {
    agent_models::ToolDefinition {
        name: TASK_CONFIRMATION_TOOL.into(),
        description: "Ask the user to clarify or confirm an ambiguous task using selectable options and optional free-form text before continuing.".into(),
        parameters: serde_json::json!({
            "type": "object",
            "required": ["prompt"],
            "properties": {
                "prompt": {
                    "type": "string",
                    "description": "The concise question or confirmation prompt to show to the user."
                },
                "options": {
                    "type": "array",
                    "description": "Selectable choices to show as checkbox/radio options.",
                    "items": {
                        "type": "object",
                        "required": ["id", "label"],
                        "properties": {
                            "id": {
                                "type": "string",
                                "description": "Stable id returned when this option is selected."
                            },
                            "label": {
                                "type": "string",
                                "description": "Short label shown to the user."
                            },
                            "description": {
                                "type": "string",
                                "description": "Optional supporting detail for the option."
                            }
                        }
                    }
                },
                "allow_multiple": {
                    "type": "boolean",
                    "description": "When true, users may select more than one option."
                },
                "allow_custom": {
                    "type": "boolean",
                    "description": "When true, users may type a custom response."
                }
            }
        }),
    }
}

pub fn parse_tool_request(
    request_id: impl Into<String>,
    arguments: &serde_json::Value,
) -> agent_core::Result<TaskConfirmationRequest> {
    let args: TaskConfirmationToolArgs =
        serde_json::from_value(arguments.clone()).map_err(|e| {
            agent_core::CoreError::InvalidState(format!("invalid task confirmation arguments: {e}"))
        })?;
    if args.prompt.trim().is_empty() {
        return Err(agent_core::CoreError::InvalidState(
            "task confirmation prompt cannot be empty".into(),
        ));
    }
    validate_request_options(&args.options, args.allow_custom)?;
    Ok(TaskConfirmationRequest {
        request_id: request_id.into(),
        prompt: args.prompt,
        options: args.options,
        allow_multiple: args.allow_multiple,
        allow_custom: args.allow_custom,
    })
}

fn invalid_state(message: impl Into<String>) -> agent_core::CoreError {
    agent_core::CoreError::InvalidState(message.into())
}

fn validate_request_options(
    options: &[TaskConfirmationOption],
    allow_custom: bool,
) -> agent_core::Result<()> {
    if options.is_empty() && !allow_custom {
        return Err(invalid_state(
            "task confirmation requires at least one response path",
        ));
    }

    let mut option_ids = HashSet::new();
    for option in options {
        let option_id = option.id.trim();
        if option_id.is_empty() {
            return Err(invalid_state("task confirmation option id cannot be empty"));
        }
        if option.label.trim().is_empty() {
            return Err(invalid_state(
                "task confirmation option label cannot be empty",
            ));
        }
        if !option_ids.insert(option_id) {
            return Err(invalid_state(format!(
                "duplicate task confirmation option id: {option_id}"
            )));
        }
    }

    Ok(())
}

pub async fn request_task_confirmation<S: EventStore>(
    store: &S,
    event_tx: &tokio::sync::broadcast::Sender<DomainEvent>,
    pending_confirmations: &PendingTaskConfirmationsMap,
    workspace_id: &WorkspaceId,
    session_id: &SessionId,
    request: TaskConfirmationRequest,
) -> agent_core::Result<String> {
    let (tx, rx) = tokio::sync::oneshot::channel();
    {
        let mut pending_confirmations = pending_confirmations.lock().await;
        if pending_confirmations.contains_key(&request.request_id) {
            return Err(invalid_state(format!(
                "task confirmation request already pending: {}",
                request.request_id
            )));
        }
        pending_confirmations.insert(
            request.request_id.clone(),
            PendingTaskConfirmation {
                session_id: session_id.clone(),
                options: request.options.clone(),
                allow_multiple: request.allow_multiple,
                allow_custom: request.allow_custom,
                reply: tx,
            },
        );
    }

    let request_event = DomainEvent::new(
        workspace_id.clone(),
        session_id.clone(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        EventPayload::TaskConfirmationRequested {
            request_id: request.request_id.clone(),
            prompt: request.prompt.clone(),
            options: request.options.clone(),
            allow_multiple: request.allow_multiple,
            allow_custom: request.allow_custom,
        },
    );
    if let Err(error) = append_and_broadcast(store, event_tx, &request_event).await {
        pending_confirmations
            .lock()
            .await
            .remove(&request.request_id);
        return Err(error);
    }

    let decision = match rx.await {
        Ok(decision) => decision,
        Err(_) => TaskConfirmationDecision {
            request_id: request.request_id.clone(),
            selected_option_ids: vec![],
            custom_response: Some("task confirmation request abandoned".into()),
        },
    };

    let resolved_event = DomainEvent::new(
        workspace_id.clone(),
        session_id.clone(),
        AgentId::system(),
        PrivacyClassification::FullTrace,
        EventPayload::TaskConfirmationResolved {
            request_id: decision.request_id.clone(),
            selected_option_ids: decision.selected_option_ids.clone(),
            custom_response: decision.custom_response.clone(),
        },
    );
    append_and_broadcast(store, event_tx, &resolved_event).await?;

    Ok(format_decision_for_tool_result(&decision))
}

pub async fn resolve_task_confirmation(
    pending_confirmations: &PendingTaskConfirmationsMap,
    decision: TaskConfirmationDecision,
) -> agent_core::Result<()> {
    let pending = {
        let mut pending_confirmations = pending_confirmations.lock().await;
        let Some(pending) = pending_confirmations.get(&decision.request_id) else {
            return Ok(());
        };
        validate_decision(pending, &decision)?;
        pending_confirmations.remove(&decision.request_id)
    };
    if let Some(pending) = pending {
        let _ = pending.reply.send(decision);
    }
    Ok(())
}

fn validate_decision(
    pending: &PendingTaskConfirmation,
    decision: &TaskConfirmationDecision,
) -> agent_core::Result<()> {
    let has_custom_response = decision
        .custom_response
        .as_deref()
        .is_some_and(|response| !response.trim().is_empty());
    if has_custom_response && !pending.allow_custom {
        return Err(invalid_state(
            "task confirmation request does not allow custom response",
        ));
    }
    if decision.selected_option_ids.len() > 1 && !pending.allow_multiple {
        return Err(invalid_state(
            "task confirmation request does not allow multiple selected options",
        ));
    }
    if decision.selected_option_ids.is_empty() && !has_custom_response {
        return Err(invalid_state(
            "task confirmation decision must select an option or provide custom response",
        ));
    }

    let valid_option_ids = pending
        .options
        .iter()
        .map(|option| option.id.as_str())
        .collect::<HashSet<_>>();
    let mut selected_option_ids = HashSet::new();
    for selected_id in &decision.selected_option_ids {
        if selected_id.trim().is_empty() {
            return Err(invalid_state(
                "task confirmation selected option id cannot be empty",
            ));
        }
        if !selected_option_ids.insert(selected_id.as_str()) {
            return Err(invalid_state(format!(
                "duplicate task confirmation selected option id: {selected_id}"
            )));
        }
        if !valid_option_ids.contains(selected_id.as_str()) {
            return Err(invalid_state(format!(
                "unknown task confirmation option id: {selected_id}"
            )));
        }
    }

    Ok(())
}

pub async fn deny_pending_confirmations_for_session(
    pending_confirmations: &PendingTaskConfirmationsMap,
    session_id: &SessionId,
    reason: &str,
) -> agent_core::Result<Vec<String>> {
    let pending = {
        let mut map = pending_confirmations.lock().await;
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
        let _ = pending.reply.send(TaskConfirmationDecision {
            request_id: request_id.clone(),
            selected_option_ids: vec![],
            custom_response: Some(reason.to_string()),
        });
        denied_request_ids.push(request_id);
    }

    Ok(denied_request_ids)
}

fn format_decision_for_tool_result(decision: &TaskConfirmationDecision) -> String {
    let custom = decision.custom_response.as_deref().unwrap_or("");
    format!(
        "task_confirmation_response\nrequest_id={}\nselected_option_ids={:?}\ncustom_response={}",
        decision.request_id, decision.selected_option_ids, custom
    )
}

#[cfg(test)]
#[path = "task_confirmation_tests.rs"]
mod tests;
