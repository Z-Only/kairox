//! Overlay/palette routing: when an overlay or focused component is active, the raw
//! crossterm event is forwarded to that component before falling back to the global
//! keymap. Each branch builds the shared [`EventContext`] used by [`Component`]s and
//! returns early once a component has consumed the event.

use crossterm::event::Event;

use crate::app_state::{InputMode, InputState};
use crate::components::{Command, Component, EventContext, FocusTarget};
use crate::keybindings::{resolve_key, resolve_paste, KeyAction};

use crate::app::App;

impl App {
    pub(super) fn handle_crossterm_event_unconfirmed(&mut self, event: &Event) -> Vec<Command> {
        match event {
            Event::Key(key_event) => {
                // Ctrl+M toggles the MCP overlay even when the overlay is
                // already visible; route through the resolver in that case.
                let is_ctrl_m = key_event
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL)
                    && matches!(key_event.code, crossterm::event::KeyCode::Char('m'));
                // Ctrl+P toggles the command palette even when already
                // visible; let the resolver fire instead of consuming the
                // event in the palette.
                let is_ctrl_p = key_event
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL)
                    && matches!(key_event.code, crossterm::event::KeyCode::Char('p'));
                let is_ctrl_s = key_event
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL)
                    && matches!(key_event.code, crossterm::event::KeyCode::Char('s'));
                let is_ctrl_g = key_event
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL)
                    && matches!(key_event.code, crossterm::event::KeyCode::Char('g'));
                // Ctrl+L toggles the model overlay even when the overlay is
                // already visible; route through the resolver in that case.
                let is_ctrl_l = key_event
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL)
                    && matches!(key_event.code, crossterm::event::KeyCode::Char('l'));
                let is_alt_i = key_event
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::ALT)
                    && matches!(key_event.code, crossterm::event::KeyCode::Char('i'));
                let is_alt_h = key_event
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::ALT)
                    && matches!(key_event.code, crossterm::event::KeyCode::Char('h'));
                let is_alt_c = key_event
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::ALT)
                    && matches!(key_event.code, crossterm::event::KeyCode::Char('c'));
                let is_f1 = matches!(key_event.code, crossterm::event::KeyCode::F(1))
                    && key_event.modifiers.is_empty();
                if self.help_overlay.is_visible() && !is_f1 {
                    let projects = self.state.projects.clone();
                    let sessions = self.state.sessions.clone();
                    let model_profile = self.state.model_profile.clone();
                    let sidebar_left = self.state.sidebar_left_visible;
                    let sidebar_right = self.state.sidebar_right_visible;
                    let focus = self.state.focus_manager.current();
                    let ctx = EventContext {
                        focus,
                        current_session: &self.state.current_session,
                        projects: &projects,
                        sessions: &sessions,
                        model_profile: &model_profile,
                        sidebar_left_visible: sidebar_left,
                        sidebar_right_visible: sidebar_right,
                        workspace_id: &self.workspace_id,
                        current_session_id: &self.current_session_id,
                    };
                    let (effects, cmds) = self.help_overlay.handle_event(&ctx, event);
                    self.dispatch_effects(effects);
                    self.state.render_scheduler.mark_dirty();
                    return cmds;
                }
                if self.permission_modal.is_visible() && !is_f1 {
                    let projects = self.state.projects.clone();
                    let sessions = self.state.sessions.clone();
                    let model_profile = self.state.model_profile.clone();
                    let sidebar_left = self.state.sidebar_left_visible;
                    let sidebar_right = self.state.sidebar_right_visible;
                    let focus = self.state.focus_manager.current();
                    let ctx = EventContext {
                        focus,
                        current_session: &self.state.current_session,
                        projects: &projects,
                        sessions: &sessions,
                        model_profile: &model_profile,
                        sidebar_left_visible: sidebar_left,
                        sidebar_right_visible: sidebar_right,
                        workspace_id: &self.workspace_id,
                        current_session_id: &self.current_session_id,
                    };
                    let (effects, cmds) = self.permission_modal.handle_event(&ctx, event);
                    if !effects.is_empty() || !cmds.is_empty() {
                        self.dispatch_effects(effects);
                        if !self.permission_modal.is_visible()
                            && self.state.focus_manager.current() == FocusTarget::PermissionModal
                        {
                            self.state.focus_manager.pop();
                            self.sync_component_focus();
                        }
                        self.state.render_scheduler.mark_dirty();
                        return cmds;
                    }
                }
                if self.command_palette.is_visible() && !is_ctrl_p && !is_f1 {
                    let projects = self.state.projects.clone();
                    let sessions = self.state.sessions.clone();
                    let model_profile = self.state.model_profile.clone();
                    let sidebar_left = self.state.sidebar_left_visible;
                    let sidebar_right = self.state.sidebar_right_visible;
                    let focus = self.state.focus_manager.current();
                    let ctx = EventContext {
                        focus,
                        current_session: &self.state.current_session,
                        projects: &projects,
                        sessions: &sessions,
                        model_profile: &model_profile,
                        sidebar_left_visible: sidebar_left,
                        sidebar_right_visible: sidebar_right,
                        workspace_id: &self.workspace_id,
                        current_session_id: &self.current_session_id,
                    };
                    let (effects, cmds) = self.command_palette.handle_event(&ctx, event);
                    self.dispatch_effects(effects);
                    self.state.render_scheduler.mark_dirty();
                    return cmds;
                }
                if self.mcp_overlay.is_visible() && !is_ctrl_m && !is_f1 {
                    let projects = self.state.projects.clone();
                    let sessions = self.state.sessions.clone();
                    let model_profile = self.state.model_profile.clone();
                    let sidebar_left = self.state.sidebar_left_visible;
                    let sidebar_right = self.state.sidebar_right_visible;
                    let focus = self.state.focus_manager.current();
                    let ctx = EventContext {
                        focus,
                        current_session: &self.state.current_session,
                        projects: &projects,
                        sessions: &sessions,
                        model_profile: &model_profile,
                        sidebar_left_visible: sidebar_left,
                        sidebar_right_visible: sidebar_right,
                        workspace_id: &self.workspace_id,
                        current_session_id: &self.current_session_id,
                    };
                    let (effects, cmds) = self.mcp_overlay.handle_event(&ctx, event);
                    self.dispatch_effects(effects);
                    self.state.render_scheduler.mark_dirty();
                    return cmds;
                }
                if self.skills_overlay.is_visible() && !is_ctrl_s && !is_f1 {
                    let projects = self.state.projects.clone();
                    let sessions = self.state.sessions.clone();
                    let model_profile = self.state.model_profile.clone();
                    let sidebar_left = self.state.sidebar_left_visible;
                    let sidebar_right = self.state.sidebar_right_visible;
                    let focus = self.state.focus_manager.current();
                    let ctx = EventContext {
                        focus,
                        current_session: &self.state.current_session,
                        projects: &projects,
                        sessions: &sessions,
                        model_profile: &model_profile,
                        sidebar_left_visible: sidebar_left,
                        sidebar_right_visible: sidebar_right,
                        workspace_id: &self.workspace_id,
                        current_session_id: &self.current_session_id,
                    };
                    let (effects, cmds) = self.skills_overlay.handle_event(&ctx, event);
                    self.dispatch_effects(effects);
                    self.state.render_scheduler.mark_dirty();
                    return cmds;
                }
                if self.plugin_overlay.is_visible() && !is_ctrl_g && !is_f1 {
                    let projects = self.state.projects.clone();
                    let sessions = self.state.sessions.clone();
                    let model_profile = self.state.model_profile.clone();
                    let sidebar_left = self.state.sidebar_left_visible;
                    let sidebar_right = self.state.sidebar_right_visible;
                    let focus = self.state.focus_manager.current();
                    let ctx = EventContext {
                        focus,
                        current_session: &self.state.current_session,
                        projects: &projects,
                        sessions: &sessions,
                        model_profile: &model_profile,
                        sidebar_left_visible: sidebar_left,
                        sidebar_right_visible: sidebar_right,
                        workspace_id: &self.workspace_id,
                        current_session_id: &self.current_session_id,
                    };
                    let (effects, cmds) = self.plugin_overlay.handle_event(&ctx, event);
                    self.dispatch_effects(effects);
                    self.state.render_scheduler.mark_dirty();
                    return cmds;
                }
                if self.model_overlay.is_visible() && !is_ctrl_l && !is_f1 {
                    let projects = self.state.projects.clone();
                    let sessions = self.state.sessions.clone();
                    let model_profile = self.state.model_profile.clone();
                    let sidebar_left = self.state.sidebar_left_visible;
                    let sidebar_right = self.state.sidebar_right_visible;
                    let focus = self.state.focus_manager.current();
                    let ctx = EventContext {
                        focus,
                        current_session: &self.state.current_session,
                        projects: &projects,
                        sessions: &sessions,
                        model_profile: &model_profile,
                        sidebar_left_visible: sidebar_left,
                        sidebar_right_visible: sidebar_right,
                        workspace_id: &self.workspace_id,
                        current_session_id: &self.current_session_id,
                    };
                    let (effects, cmds) = self.model_overlay.handle_event(&ctx, event);
                    self.dispatch_effects(effects);
                    self.state.render_scheduler.mark_dirty();
                    return cmds;
                }
                if self.agent_overlay.is_visible() && !is_f1 {
                    let projects = self.state.projects.clone();
                    let sessions = self.state.sessions.clone();
                    let model_profile = self.state.model_profile.clone();
                    let sidebar_left = self.state.sidebar_left_visible;
                    let sidebar_right = self.state.sidebar_right_visible;
                    let focus = self.state.focus_manager.current();
                    let ctx = EventContext {
                        focus,
                        current_session: &self.state.current_session,
                        projects: &projects,
                        sessions: &sessions,
                        model_profile: &model_profile,
                        sidebar_left_visible: sidebar_left,
                        sidebar_right_visible: sidebar_right,
                        workspace_id: &self.workspace_id,
                        current_session_id: &self.current_session_id,
                    };
                    let (effects, cmds) = self.agent_overlay.handle_event(&ctx, event);
                    self.dispatch_effects(effects);
                    self.state.render_scheduler.mark_dirty();
                    return cmds;
                }
                if self.instructions_overlay.is_visible() && !is_alt_i && !is_f1 {
                    let projects = self.state.projects.clone();
                    let sessions = self.state.sessions.clone();
                    let model_profile = self.state.model_profile.clone();
                    let sidebar_left = self.state.sidebar_left_visible;
                    let sidebar_right = self.state.sidebar_right_visible;
                    let focus = self.state.focus_manager.current();
                    let ctx = EventContext {
                        focus,
                        current_session: &self.state.current_session,
                        projects: &projects,
                        sessions: &sessions,
                        model_profile: &model_profile,
                        sidebar_left_visible: sidebar_left,
                        sidebar_right_visible: sidebar_right,
                        workspace_id: &self.workspace_id,
                        current_session_id: &self.current_session_id,
                    };
                    let (effects, cmds) = self.instructions_overlay.handle_event(&ctx, event);
                    self.dispatch_effects(effects);
                    self.state.render_scheduler.mark_dirty();
                    return cmds;
                }
                if self.hooks_overlay.is_visible() && !is_alt_h && !is_f1 {
                    let projects = self.state.projects.clone();
                    let sessions = self.state.sessions.clone();
                    let model_profile = self.state.model_profile.clone();
                    let sidebar_left = self.state.sidebar_left_visible;
                    let sidebar_right = self.state.sidebar_right_visible;
                    let focus = self.state.focus_manager.current();
                    let ctx = EventContext {
                        focus,
                        current_session: &self.state.current_session,
                        projects: &projects,
                        sessions: &sessions,
                        model_profile: &model_profile,
                        sidebar_left_visible: sidebar_left,
                        sidebar_right_visible: sidebar_right,
                        workspace_id: &self.workspace_id,
                        current_session_id: &self.current_session_id,
                    };
                    let (effects, cmds) = self.hooks_overlay.handle_event(&ctx, event);
                    self.dispatch_effects(effects);
                    self.state.render_scheduler.mark_dirty();
                    return cmds;
                }
                if self.sessions.is_overlay_open() && !is_f1 {
                    let projects = self.state.projects.clone();
                    let sessions = self.state.sessions.clone();
                    let model_profile = self.state.model_profile.clone();
                    let sidebar_left = self.state.sidebar_left_visible;
                    let sidebar_right = self.state.sidebar_right_visible;
                    let focus = self.state.focus_manager.current();
                    let ctx = EventContext {
                        focus,
                        current_session: &self.state.current_session,
                        projects: &projects,
                        sessions: &sessions,
                        model_profile: &model_profile,
                        sidebar_left_visible: sidebar_left,
                        sidebar_right_visible: sidebar_right,
                        workspace_id: &self.workspace_id,
                        current_session_id: &self.current_session_id,
                    };
                    let (effects, cmds) = self.sessions.handle_event(&ctx, event);
                    self.dispatch_effects(effects);
                    self.state.render_scheduler.mark_dirty();
                    return cmds;
                }
                if self.status_bar.context_details_visible() && !is_alt_c && !is_f1 {
                    let projects = self.state.projects.clone();
                    let sessions = self.state.sessions.clone();
                    let model_profile = self.state.model_profile.clone();
                    let sidebar_left = self.state.sidebar_left_visible;
                    let sidebar_right = self.state.sidebar_right_visible;
                    let focus = self.state.focus_manager.current();
                    let ctx = EventContext {
                        focus,
                        current_session: &self.state.current_session,
                        projects: &projects,
                        sessions: &sessions,
                        model_profile: &model_profile,
                        sidebar_left_visible: sidebar_left,
                        sidebar_right_visible: sidebar_right,
                        workspace_id: &self.workspace_id,
                        current_session_id: &self.current_session_id,
                    };
                    let (effects, cmds) = self.status_bar.handle_event(&ctx, event);
                    self.dispatch_effects(effects);
                    self.state.render_scheduler.mark_dirty();
                    return cmds;
                }
                let permission_pending =
                    matches!(self.chat.input_state, InputState::PermissionWait { .. })
                        || self.permission_modal.is_visible();
                if !permission_pending
                    && self.state.focus_manager.current() == FocusTarget::Trace
                    && self.trace.active_tab == crate::components::trace::RightPanelTab::Tasks
                    && matches!(key_event.code, crossterm::event::KeyCode::Enter)
                    && key_event.modifiers.is_empty()
                    && self
                        .trace
                        .toggle_selected_task_expansion(&self.state.current_session.task_graph)
                {
                    self.state.render_scheduler.mark_dirty();
                    return Vec::new();
                }
                if !permission_pending
                    && self.state.focus_manager.current() == FocusTarget::Trace
                    && self.trace.active_tab == crate::components::trace::RightPanelTab::Memory
                    && self.trace.memory_search_active
                {
                    let search_action = match key_event.code {
                        crossterm::event::KeyCode::Enter => Some(KeyAction::FocusCycleNext),
                        crossterm::event::KeyCode::Esc => Some(KeyAction::Escape),
                        crossterm::event::KeyCode::Backspace => Some(KeyAction::InputBackspace),
                        crossterm::event::KeyCode::Delete => Some(KeyAction::InputDelete),
                        crossterm::event::KeyCode::Char(ch)
                            if !key_event
                                .modifiers
                                .intersects(crossterm::event::KeyModifiers::CONTROL)
                                && !key_event
                                    .modifiers
                                    .intersects(crossterm::event::KeyModifiers::ALT) =>
                        {
                            Some(KeyAction::InputCharacter(ch))
                        }
                        _ => None,
                    };
                    if let Some(action) = search_action {
                        return self.apply_action(action);
                    }
                }
                let action = resolve_key(
                    *key_event,
                    self.state.focus_manager.current(),
                    permission_pending,
                    self.chat.input_mode,
                );
                self.apply_action(action)
            }
            Event::Resize(_, _) => {
                self.state.render_scheduler.mark_dirty_immediate();
                Vec::new()
            }
            Event::Paste(text) => {
                if text.contains('\n') && self.chat.input_mode == InputMode::SingleLine {
                    self.state.input_mode = InputMode::MultiLine;
                    self.chat.input_mode = InputMode::MultiLine;
                }
                let action = resolve_paste(text.clone());
                self.apply_action(action)
            }
            _ => Vec::new(),
        }
    }
}
