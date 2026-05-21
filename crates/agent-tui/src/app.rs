//! App — Component composition and event routing for the interactive TUI.

mod commands;
mod events;
mod input;
mod render;

pub use commands::dispatch_commands;

use agent_core::{DomainEvent, SessionId, WorkspaceId};
use agent_tools::PermissionMode;

use crate::app_state::AppState;
use crate::components::chat::ChatPanel;
use crate::components::command_palette::CommandPalette;
use crate::components::mcp_overlay::McpOverlay;
use crate::components::permission_modal::PermissionModal;
use crate::components::sessions::SessionsPanel;
use crate::components::skills_overlay::SkillsOverlay;
use crate::components::status_bar::{PermissionModeExt, StatusBar};
use crate::components::trace::TracePanel;
use crate::components::{Component, CrossPanelEffect, FocusTarget, SessionInfo, SessionState};

pub struct App {
    pub state: AppState,
    pub chat: ChatPanel,
    pub sessions: SessionsPanel,
    pub trace: TracePanel,
    pub status_bar: StatusBar,
    pub permission_modal: PermissionModal,
    pub mcp_overlay: McpOverlay,
    pub command_palette: CommandPalette,
    pub skills_overlay: SkillsOverlay,
    pub workspace_id: WorkspaceId,
    pub current_session_id: Option<SessionId>,
    pub domain_events: Vec<DomainEvent>,
    pub quit_confirmed: bool,
    pub quitting: bool,
    /// P3: latest `ContextAssembled.usage`, propagated into the status bar.
    pub last_context_usage: Option<agent_core::context_types::ContextUsage>,
    /// P3: `true` between `ContextCompactionStarted` and `Completed`/`Failed`.
    pub compacting: bool,
}

impl App {
    pub fn new(
        model_profile: &str,
        permission_mode: PermissionMode,
        workspace_id: WorkspaceId,
    ) -> Self {
        Self {
            state: AppState::new(model_profile, permission_mode),
            chat: ChatPanel::new(),
            sessions: SessionsPanel::new(),
            trace: TracePanel::new(),
            status_bar: StatusBar::new(),
            permission_modal: PermissionModal::new(),
            mcp_overlay: McpOverlay::new(),
            command_palette: CommandPalette::new(),
            skills_overlay: SkillsOverlay::new(),
            workspace_id,
            current_session_id: None,
            domain_events: Vec::new(),
            quit_confirmed: false,
            quitting: false,
            last_context_usage: None,
            compacting: false,
        }
    }

    /// Find the currently active session by `current_session_id`.
    fn current_session_mut(&mut self) -> Option<&mut SessionInfo> {
        let sid = self.current_session_id.clone()?;
        self.state.sessions.iter_mut().find(|s| s.id == sid)
    }

    /// Fan-out cross-panel effects to all components.
    pub fn dispatch_effects(&mut self, effects: Vec<CrossPanelEffect>) {
        for effect in effects {
            if let CrossPanelEffect::NavigateToSession(session_id) = &effect {
                self.current_session_id = Some(session_id.clone());
                for session in &mut self.state.sessions {
                    if session.id == *session_id {
                        session.state = SessionState::Active;
                    } else if session.state == SessionState::Active {
                        session.state = SessionState::Idle;
                    }
                }
            }
            // Drive focus stack from overlay effects so Tab cycling is
            // suppressed while the overlay is open.
            match &effect {
                CrossPanelEffect::ShowMcpOverlay(_)
                    if self.state.focus_manager.current() != FocusTarget::McpOverlay =>
                {
                    self.state.focus_manager.push(FocusTarget::McpOverlay);
                }
                CrossPanelEffect::DismissMcpOverlay
                    if self.state.focus_manager.current() == FocusTarget::McpOverlay =>
                {
                    self.state.focus_manager.pop();
                }
                CrossPanelEffect::ShowCommandPalette
                    if self.state.focus_manager.current() != FocusTarget::CommandPalette =>
                {
                    self.state.focus_manager.push(FocusTarget::CommandPalette);
                }
                CrossPanelEffect::DismissCommandPalette
                    if self.state.focus_manager.current() == FocusTarget::CommandPalette =>
                {
                    self.state.focus_manager.pop();
                }
                CrossPanelEffect::ShowSkillsOverlay(_)
                    if self.state.focus_manager.current() != FocusTarget::SkillsOverlay =>
                {
                    self.state.focus_manager.push(FocusTarget::SkillsOverlay);
                }
                CrossPanelEffect::DismissSkillsOverlay
                    if self.state.focus_manager.current() == FocusTarget::SkillsOverlay =>
                {
                    self.state.focus_manager.pop();
                }
                _ => {}
            }
            self.chat.handle_effect(&effect);
            self.sessions.handle_effect(&effect);
            self.trace.handle_effect(&effect);
            self.status_bar.handle_effect(&effect);
            self.permission_modal.handle_effect(&effect);
            self.mcp_overlay.handle_effect(&effect);
            self.command_palette.handle_effect(&effect);
            self.skills_overlay.handle_effect(&effect);
        }
        self.sync_component_focus();
    }

    /// Load projection and trace for a session from historical data.
    /// Called when switching to a different session.
    #[allow(dead_code)]
    pub fn load_session_projection(
        &mut self,
        projection: agent_core::projection::SessionProjection,
        trace_events: Vec<DomainEvent>,
    ) {
        self.state.current_session = projection;
        self.domain_events = trace_events;
        self.state.render_scheduler.mark_dirty_immediate();
    }

    /// Sync all components' focused states based on the current focus target.
    pub fn sync_component_focus(&mut self) {
        let current = self.state.focus_manager.current();
        self.chat.set_focused(current == FocusTarget::Chat);
        self.sessions.set_focused(current == FocusTarget::Sessions);
        self.trace.set_focused(current == FocusTarget::Trace);
        self.permission_modal
            .set_focused(current == FocusTarget::PermissionModal);
        self.mcp_overlay
            .set_focused(current == FocusTarget::McpOverlay);
        self.command_palette
            .set_focused(current == FocusTarget::CommandPalette);
        self.skills_overlay
            .set_focused(current == FocusTarget::SkillsOverlay);
        self.status_bar.set_focused(false);
        self.state.render_scheduler.mark_dirty();
    }

    pub fn sync_status_bar(&mut self) {
        let hint = if self.quit_confirmed {
            "Press Ctrl+C again to quit, or Esc to cancel".to_string()
        } else {
            "Alt+Q quit | Tab cycle | Alt+S sessions | Alt+T trace".to_string()
        };

        let info = crate::components::StatusInfo {
            profile: self.state.model_profile.clone(),
            permission_mode: self.state.permission_mode.as_str().to_string(),
            session_count: self.state.sessions.len(),
            mcp_server_count: 0,
            hint,
            error: None,
            context_usage: self.last_context_usage.clone(),
            compacting: self.compacting,
        };
        self.status_bar
            .handle_effect(&CrossPanelEffect::SetStatus(info));
    }
}
