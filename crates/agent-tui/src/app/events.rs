use agent_core::projection::{ProjectedMessage, ProjectedRole};
use agent_core::{DomainEvent, EventPayload};

use crate::components::{
    CrossPanelEffect, FocusTarget, PermissionRequest, RiskLevel, SessionState,
};

use super::App;

impl App {
    /// Process a domain event from the runtime, updating projection and state.
    pub fn handle_domain_event(&mut self, event: &DomainEvent) {
        let mut effects = Vec::new();
        let mut sync_permission_focus_after_dispatch = false;

        match &event.payload {
            EventPayload::UserMessageAdded { content, .. } => {
                self.state.current_session.messages.push(ProjectedMessage {
                    role: ProjectedRole::User,
                    content: content.clone(),
                });
                self.state.render_scheduler.mark_dirty();
            }
            EventPayload::ModelTokenDelta { delta } => {
                self.state.current_session.token_stream.push_str(delta);
                self.state.render_scheduler.mark_dirty();
                self.state.render_scheduler.set_streaming(true);
                self.state.render_scheduler.add_tokens(delta.len());
            }
            EventPayload::AssistantMessageCompleted { content, .. } => {
                self.state.current_session.messages.push(ProjectedMessage {
                    role: ProjectedRole::Assistant,
                    content: content.clone(),
                });
                self.state.current_session.token_stream.clear();
                self.state.render_scheduler.set_streaming(false);
                self.state.render_scheduler.mark_dirty();
                effects.push(CrossPanelEffect::StopStreaming);
            }
            EventPayload::SessionCancelled { .. } => {
                self.state.current_session.cancelled = true;
                self.state.render_scheduler.mark_dirty();
            }
            EventPayload::ToolInvocationStarted { .. } => {
                self.state.render_scheduler.mark_dirty();
                if let Some(session) = self.current_session_mut() {
                    session.state = SessionState::Active;
                }
            }
            EventPayload::ToolInvocationCompleted { .. } => {
                self.state.render_scheduler.mark_dirty();
                if let Some(session) = self.current_session_mut() {
                    session.state = SessionState::Idle;
                }
            }
            EventPayload::ToolInvocationFailed { .. } => {
                self.state.render_scheduler.mark_dirty();
                if let Some(session) = self.current_session_mut() {
                    session.state = SessionState::Idle;
                }
            }
            EventPayload::PermissionRequested {
                request_id,
                tool_id,
                preview,
            } => {
                let risk_level = if tool_id.starts_with("mcp.") {
                    let parts: Vec<&str> = tool_id.splitn(3, '.').collect();
                    let server_id = parts.get(1).map(|s| (*s).to_string()).unwrap_or_default();
                    RiskLevel::McpTool { server_id }
                } else {
                    RiskLevel::Write
                };
                let req = PermissionRequest {
                    request_id: request_id.clone(),
                    tool_id: tool_id.clone(),
                    tool_preview: preview.clone(),
                    risk_level: risk_level.clone(),
                };
                effects.push(CrossPanelEffect::ShowPermissionPrompt(req));

                if self.state.focus_manager.current() != FocusTarget::PermissionModal {
                    self.state.focus_manager.push(FocusTarget::PermissionModal);
                    self.sync_component_focus();
                }

                if let Some(session) = self.current_session_mut() {
                    session.state = SessionState::AwaitingPermission;
                }
                self.state.render_scheduler.mark_dirty();
            }
            EventPayload::PermissionGranted { request_id } => {
                effects.push(CrossPanelEffect::ResolvePermissionPrompt {
                    request_id: request_id.clone(),
                    approved: true,
                });
                effects.push(CrossPanelEffect::DismissPermissionPrompt);
                sync_permission_focus_after_dispatch = true;
                self.state.render_scheduler.mark_dirty();
            }
            EventPayload::PermissionDenied { request_id, .. } => {
                effects.push(CrossPanelEffect::ResolvePermissionPrompt {
                    request_id: request_id.clone(),
                    approved: false,
                });
                effects.push(CrossPanelEffect::DismissPermissionPrompt);
                sync_permission_focus_after_dispatch = true;
                self.state.render_scheduler.mark_dirty();
            }
            EventPayload::SessionInitialized { model_profile } => {
                if let Some(session) = self.current_session_mut() {
                    if session.title.starts_with("Session using ") {
                        session.title = format!("Session using {}", model_profile);
                    }
                }
                self.state.render_scheduler.mark_dirty();
            }
            EventPayload::AgentTaskCreated { title, .. } => {
                self.state.current_session.task_titles.push(title.clone());
                self.state.render_scheduler.mark_dirty();
            }
            EventPayload::MemoryProposed {
                memory_id: _,
                scope,
                key,
                content,
            } => {
                let label = match key {
                    Some(k) => format!("[{scope}] {k}: {content}"),
                    None => format!("[{scope}] {content}"),
                };
                self.state.current_session.messages.push(ProjectedMessage {
                    role: ProjectedRole::Assistant,
                    content: format!("🧠 Memory proposed: {label}"),
                });
                self.state.render_scheduler.mark_dirty();
            }
            EventPayload::MemoryAccepted { memory_id: _, .. } => {
                self.state.current_session.messages.push(ProjectedMessage {
                    role: ProjectedRole::Assistant,
                    content: "✅ Memory saved".to_string(),
                });
                self.state.render_scheduler.mark_dirty();
            }
            EventPayload::MemoryRejected {
                memory_id: _,
                reason,
                ..
            } => {
                self.state.current_session.messages.push(ProjectedMessage {
                    role: ProjectedRole::Assistant,
                    content: format!("❌ Memory rejected: {reason}"),
                });
                self.state.render_scheduler.mark_dirty();
            }
            EventPayload::ContextAssembled { usage } => {
                self.last_context_usage = Some(usage.clone());
                self.compacting = false;
                self.state.render_scheduler.mark_dirty();
            }
            EventPayload::ContextCompactionStarted { .. } => {
                self.compacting = true;
                self.state.render_scheduler.mark_dirty();
            }
            EventPayload::ContextCompactionCompleted { .. } => {
                self.compacting = false;
                self.state.render_scheduler.mark_dirty();
            }
            EventPayload::ContextCompactionFailed { .. } => {
                self.compacting = false;
                self.state.render_scheduler.mark_dirty();
            }
            EventPayload::ContextCompactionSkipped { .. } => {
                // The skipped event is one-shot — there is no in-flight
                // compaction to track. Clearing the flag also recovers
                // gracefully if a stale spinner was somehow left running.
                self.compacting = false;
                self.state.render_scheduler.mark_dirty();
            }
            EventPayload::ModelProfileSwitched {
                to_profile,
                reasoning_effort,
                ..
            } => {
                self.state.model_profile = to_profile.clone();
                self.state.reasoning_effort = reasoning_effort.clone();
                if let Some(session) = self.current_session_mut() {
                    session.model_profile = to_profile.clone();
                }
                self.sync_status_bar();
                self.state.render_scheduler.mark_dirty();
            }
            _ => {
                self.state.render_scheduler.mark_dirty();
            }
        }

        self.domain_events.push(event.clone());
        self.dispatch_effects(effects);
        if sync_permission_focus_after_dispatch {
            if self.permission_modal.is_visible() {
                if let Some(session) = self.current_session_mut() {
                    session.state = SessionState::AwaitingPermission;
                }
            } else {
                if self.state.focus_manager.current() == FocusTarget::PermissionModal {
                    self.state.focus_manager.pop();
                    self.sync_component_focus();
                }
                if let Some(session) = self.current_session_mut() {
                    session.state = SessionState::Active;
                }
            }
        }
        self.sync_status_bar();
    }
}

#[cfg(test)]
#[path = "events_tests.rs"]
mod tests;
