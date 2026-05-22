//! App — Component composition and event routing for the interactive TUI.

mod commands;
mod events;
mod input;
mod render;

pub use commands::{
    clear_session_projection, dispatch_commands, refresh_command_palette, refresh_mcp_overlay,
};

use agent_core::{DomainEvent, SessionId, WorkspaceId};
use agent_tools::PermissionMode;

use crate::app_state::AppState;
use crate::components::agent_overlay::AgentOverlay;
use crate::components::chat::ChatPanel;
use crate::components::command_palette::CommandPalette;
use crate::components::help_overlay::HelpOverlay;
use crate::components::hooks_overlay::HooksOverlay;
use crate::components::instructions_overlay::InstructionsOverlay;
use crate::components::mcp_overlay::McpOverlay;
use crate::components::model_overlay::ModelOverlay;
use crate::components::permission_modal::PermissionModal;
use crate::components::plugin_overlay::PluginOverlay;
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
    pub help_overlay: HelpOverlay,
    pub skills_overlay: SkillsOverlay,
    pub model_overlay: ModelOverlay,
    pub agent_overlay: AgentOverlay,
    pub plugin_overlay: PluginOverlay,
    pub hooks_overlay: HooksOverlay,
    pub instructions_overlay: InstructionsOverlay,
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
            help_overlay: HelpOverlay::new(),
            skills_overlay: SkillsOverlay::new(),
            model_overlay: ModelOverlay::new(),
            agent_overlay: AgentOverlay::new(),
            plugin_overlay: PluginOverlay::new(),
            hooks_overlay: HooksOverlay::new(),
            instructions_overlay: InstructionsOverlay::new(),
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

    fn current_session(&self) -> Option<&SessionInfo> {
        let sid = self.current_session_id.as_ref()?;
        self.state
            .sessions
            .iter()
            .find(|session| &session.id == sid)
    }

    fn current_project(&self) -> Option<&crate::components::ProjectInfo> {
        let project_id = self.current_session()?.project_id.as_ref()?;
        self.state
            .projects
            .iter()
            .find(|project| &project.id == project_id)
    }

    pub(crate) fn current_session_git_metadata(&self) -> Vec<String> {
        let Some(session) = self.current_session() else {
            return Vec::new();
        };

        let branch = session
            .branch
            .as_deref()
            .filter(|branch| !branch.is_empty());
        let worktree_path = session
            .worktree_path
            .as_deref()
            .filter(|path| !path.is_empty());
        let project_root = self
            .current_project()
            .map(|project| project.root_path.as_str());
        let is_worktree_session = match (worktree_path, project_root) {
            (Some(path), Some(root)) => path != root,
            (Some(path), None) => {
                path.contains("/.worktrees/")
                    || path.contains("/.kairox/worktrees/")
                    || branch.is_some()
            }
            (None, _) => false,
        };

        let mut parts = Vec::new();
        if is_worktree_session {
            parts.push("worktree".to_string());
        }
        if let Some(branch) = branch {
            parts.push(branch.to_string());
        }
        if let Some(path) = worktree_path {
            let compact = crate::components::compact_worktree_path(path);
            if !parts.contains(&compact) {
                parts.push(compact);
            }
        }
        if parts.is_empty() {
            parts.extend(session.project_id.iter().map(ToString::to_string));
        }

        parts
    }

    pub(crate) fn current_project_instruction_summary(&self) -> Option<String> {
        let summary = self.current_project()?.instruction_summary.as_ref()?;
        crate::components::project_instruction_source_label(summary)
            .map(|label| format!("Loaded {label}"))
    }

    pub(crate) fn current_session_metadata(&self) -> Vec<String> {
        let mut parts = self.current_session_git_metadata();
        if let Some(summary) = self.current_project_instruction_summary() {
            parts.push(summary);
        }
        parts
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
                CrossPanelEffect::ShowModelOverlay(_)
                    if self.state.focus_manager.current() != FocusTarget::ModelOverlay =>
                {
                    self.state.focus_manager.push(FocusTarget::ModelOverlay);
                }
                CrossPanelEffect::DismissModelOverlay
                    if self.state.focus_manager.current() == FocusTarget::ModelOverlay =>
                {
                    self.state.focus_manager.pop();
                }
                CrossPanelEffect::ShowAgentSettingsOverlay(_)
                    if self.state.focus_manager.current() != FocusTarget::AgentOverlay =>
                {
                    self.state.focus_manager.push(FocusTarget::AgentOverlay);
                }
                CrossPanelEffect::DismissAgentSettingsOverlay
                    if self.state.focus_manager.current() == FocusTarget::AgentOverlay =>
                {
                    self.state.focus_manager.pop();
                }
                CrossPanelEffect::ShowPluginsOverlay(_)
                    if self.state.focus_manager.current() != FocusTarget::PluginOverlay =>
                {
                    self.state.focus_manager.push(FocusTarget::PluginOverlay);
                }
                CrossPanelEffect::DismissPluginsOverlay
                    if self.state.focus_manager.current() == FocusTarget::PluginOverlay =>
                {
                    self.state.focus_manager.pop();
                }
                CrossPanelEffect::ShowHooksOverlay(_)
                    if self.state.focus_manager.current() != FocusTarget::HooksOverlay =>
                {
                    self.state.focus_manager.push(FocusTarget::HooksOverlay);
                }
                CrossPanelEffect::DismissHooksOverlay
                    if self.state.focus_manager.current() == FocusTarget::HooksOverlay =>
                {
                    self.state.focus_manager.pop();
                }
                CrossPanelEffect::ShowInstructionsOverlay(_)
                | CrossPanelEffect::ShowSystemPromptOverlay(_)
                    if self.state.focus_manager.current() != FocusTarget::InstructionsOverlay =>
                {
                    self.state
                        .focus_manager
                        .push(FocusTarget::InstructionsOverlay);
                }
                CrossPanelEffect::DismissInstructionsOverlay
                    if self.state.focus_manager.current() == FocusTarget::InstructionsOverlay =>
                {
                    self.state.focus_manager.pop();
                }
                CrossPanelEffect::ShowHelpOverlay(_) | CrossPanelEffect::DismissHelpOverlay => {}
                _ => {}
            }
            self.chat.handle_effect(&effect);
            self.sessions.handle_effect(&effect);
            self.trace.handle_effect(&effect);
            self.status_bar.handle_effect(&effect);
            self.permission_modal.handle_effect(&effect);
            self.mcp_overlay.handle_effect(&effect);
            self.command_palette.handle_effect(&effect);
            self.help_overlay.handle_effect(&effect);
            self.skills_overlay.handle_effect(&effect);
            self.model_overlay.handle_effect(&effect);
            self.agent_overlay.handle_effect(&effect);
            self.plugin_overlay.handle_effect(&effect);
            self.hooks_overlay.handle_effect(&effect);
            self.instructions_overlay.handle_effect(&effect);
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
        self.help_overlay.set_focused(false);
        self.skills_overlay
            .set_focused(current == FocusTarget::SkillsOverlay);
        self.model_overlay
            .set_focused(current == FocusTarget::ModelOverlay);
        self.agent_overlay
            .set_focused(current == FocusTarget::AgentOverlay);
        self.plugin_overlay
            .set_focused(current == FocusTarget::PluginOverlay);
        self.hooks_overlay
            .set_focused(current == FocusTarget::HooksOverlay);
        self.instructions_overlay
            .set_focused(current == FocusTarget::InstructionsOverlay);
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
            session_count: self
                .state
                .sessions
                .iter()
                .filter(|session| !session.archived)
                .count(),
            mcp_server_count: 0,
            session_metadata: self.current_session_metadata(),
            hint,
            error: None,
            context_usage: self.last_context_usage.clone(),
            compacting: self.compacting,
        };
        self.status_bar
            .handle_effect(&CrossPanelEffect::SetStatus(info));
    }
}
