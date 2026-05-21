use crossterm::event::Event;

use crate::app_state::{CtrlCAction, InputMode, InputState};
use crate::components::{Command, Component, CrossPanelEffect, EventContext, FocusTarget};
use crate::keybindings::{resolve_key, resolve_paste, KeyAction};

use super::App;

impl App {
    /// Handle a raw crossterm event, returning any commands to dispatch.
    pub fn handle_crossterm_event(&mut self, event: &Event) -> Vec<Command> {
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
                // Ctrl+L toggles the model overlay even when the overlay is
                // already visible; route through the resolver in that case.
                let is_ctrl_l = key_event
                    .modifiers
                    .contains(crossterm::event::KeyModifiers::CONTROL)
                    && matches!(key_event.code, crossterm::event::KeyCode::Char('l'));
                if self.command_palette.is_visible() && !is_ctrl_p {
                    let sessions = self.state.sessions.clone();
                    let model_profile = self.state.model_profile.clone();
                    let permission_mode = self.state.permission_mode;
                    let sidebar_left = self.state.sidebar_left_visible;
                    let sidebar_right = self.state.sidebar_right_visible;
                    let focus = self.state.focus_manager.current();
                    let ctx = EventContext {
                        focus,
                        current_session: &self.state.current_session,
                        sessions: &sessions,
                        model_profile: &model_profile,
                        permission_mode,
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
                if self.mcp_overlay.is_visible() && !is_ctrl_m {
                    let sessions = self.state.sessions.clone();
                    let model_profile = self.state.model_profile.clone();
                    let permission_mode = self.state.permission_mode;
                    let sidebar_left = self.state.sidebar_left_visible;
                    let sidebar_right = self.state.sidebar_right_visible;
                    let focus = self.state.focus_manager.current();
                    let ctx = EventContext {
                        focus,
                        current_session: &self.state.current_session,
                        sessions: &sessions,
                        model_profile: &model_profile,
                        permission_mode,
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
                if self.skills_overlay.is_visible() && !is_ctrl_s {
                    let sessions = self.state.sessions.clone();
                    let model_profile = self.state.model_profile.clone();
                    let permission_mode = self.state.permission_mode;
                    let sidebar_left = self.state.sidebar_left_visible;
                    let sidebar_right = self.state.sidebar_right_visible;
                    let focus = self.state.focus_manager.current();
                    let ctx = EventContext {
                        focus,
                        current_session: &self.state.current_session,
                        sessions: &sessions,
                        model_profile: &model_profile,
                        permission_mode,
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
                if self.model_overlay.is_visible() && !is_ctrl_l {
                    let sessions = self.state.sessions.clone();
                    let model_profile = self.state.model_profile.clone();
                    let permission_mode = self.state.permission_mode;
                    let sidebar_left = self.state.sidebar_left_visible;
                    let sidebar_right = self.state.sidebar_right_visible;
                    let focus = self.state.focus_manager.current();
                    let ctx = EventContext {
                        focus,
                        current_session: &self.state.current_session,
                        sessions: &sessions,
                        model_profile: &model_profile,
                        permission_mode,
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
                if self.sessions.context_menu_open {
                    let sessions = self.state.sessions.clone();
                    let model_profile = self.state.model_profile.clone();
                    let permission_mode = self.state.permission_mode;
                    let sidebar_left = self.state.sidebar_left_visible;
                    let sidebar_right = self.state.sidebar_right_visible;
                    let focus = self.state.focus_manager.current();
                    let ctx = EventContext {
                        focus,
                        current_session: &self.state.current_session,
                        sessions: &sessions,
                        model_profile: &model_profile,
                        permission_mode,
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
                let permission_pending =
                    matches!(self.state.input_state, InputState::PermissionWait { .. })
                        || self.permission_modal.is_visible();
                let action = resolve_key(
                    *key_event,
                    self.state.focus_manager.current(),
                    permission_pending,
                    self.state.input_mode,
                );
                self.apply_action(action)
            }
            Event::Resize(_, _) => {
                self.state.render_scheduler.mark_dirty_immediate();
                Vec::new()
            }
            Event::Paste(text) => {
                if text.contains('\n') && self.state.input_mode == InputMode::SingleLine {
                    self.state.input_mode = InputMode::MultiLine;
                    self.chat.input_mode = InputMode::MultiLine;
                }
                let action = resolve_paste(text.clone());
                self.apply_action(action)
            }
            _ => Vec::new(),
        }
    }

    /// Route a resolved key action, returning any commands to dispatch.
    pub fn apply_action(&mut self, action: KeyAction) -> Vec<Command> {
        let mut commands = Vec::new();

        match action {
            KeyAction::InterruptOrQuit => match self.state.record_ctrl_c() {
                CtrlCAction::Interrupt => {
                    if let Some(session_id) = &self.current_session_id {
                        commands.push(Command::CancelSession {
                            workspace_id: self.workspace_id.clone(),
                            session_id: session_id.clone(),
                        });
                    }
                    self.state.render_scheduler.mark_dirty();
                }
                CtrlCAction::ConfirmQuit => {
                    self.quit_confirmed = true;
                    self.state.render_scheduler.mark_dirty();
                }
                CtrlCAction::ForceQuit => {
                    self.quitting = true;
                }
            },
            KeyAction::Quit => {
                self.quit_confirmed = true;
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::Escape => {
                if self.quit_confirmed {
                    self.quit_confirmed = false;
                    self.state.reset_ctrl_c();
                    self.state.render_scheduler.mark_dirty();
                }
                let (effects, cmds) = self.apply_chat_action(KeyAction::Escape);
                commands.extend(cmds);
                self.dispatch_effects(effects);
            }
            KeyAction::ToggleSessionsSidebar => {
                self.state.sidebar_left_visible = !self.state.sidebar_left_visible;
                self.state.render_scheduler.mark_dirty_immediate();
            }
            KeyAction::ToggleTraceSidebar => {
                self.state.sidebar_right_visible = !self.state.sidebar_right_visible;
                self.state.render_scheduler.mark_dirty_immediate();
            }
            KeyAction::FocusCycleNext => {
                self.state.focus_manager.cycle_next();
                self.sync_component_focus();
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::FocusChat => {
                self.state.focus_manager.set(FocusTarget::Chat);
                self.sync_component_focus();
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::FocusSessions => {
                self.state.focus_manager.set(FocusTarget::Sessions);
                self.sync_component_focus();
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::FocusTrace => {
                self.state.focus_manager.set(FocusTarget::Trace);
                self.sync_component_focus();
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::ToggleMcpOverlay => {
                if self.mcp_overlay.is_visible() {
                    self.dispatch_effects(vec![CrossPanelEffect::DismissMcpOverlay]);
                    self.state.render_scheduler.mark_dirty_immediate();
                } else {
                    commands.push(Command::OpenMcpOverlay);
                }
            }
            KeyAction::ToggleCommandPalette => {
                if self.command_palette.is_visible() {
                    self.dispatch_effects(vec![CrossPanelEffect::DismissCommandPalette]);
                } else {
                    self.dispatch_effects(vec![CrossPanelEffect::ShowCommandPalette]);
                }
                self.state.render_scheduler.mark_dirty_immediate();
            }
            KeyAction::ToggleSkillsOverlay => {
                if self.skills_overlay.is_visible() {
                    self.dispatch_effects(vec![CrossPanelEffect::DismissSkillsOverlay]);
                    self.state.render_scheduler.mark_dirty_immediate();
                } else {
                    commands.push(Command::OpenSkillsOverlay);
                }
            }
            KeyAction::ToggleModelOverlay => {
                if self.model_overlay.is_visible() {
                    self.dispatch_effects(vec![CrossPanelEffect::DismissModelOverlay]);
                    self.state.render_scheduler.mark_dirty_immediate();
                } else {
                    commands.push(Command::OpenModelOverlay);
                }
            }
            KeyAction::ToggleTraceDensity => {
                self.trace.cycle_density();
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::CycleTraceTabNext => {
                self.trace.cycle_tab_next();
                if self.trace.active_tab == crate::components::trace::RightPanelTab::Memory {
                    commands.push(Command::LoadMemories {
                        scope: None,
                        keywords: Vec::new(),
                        limit: 100,
                    });
                }
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::CycleTraceTabPrevious => {
                self.trace.cycle_tab_previous();
                if self.trace.active_tab == crate::components::trace::RightPanelTab::Memory {
                    commands.push(Command::LoadMemories {
                        scope: None,
                        keywords: Vec::new(),
                        limit: 100,
                    });
                }
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::RetrySelectedTask => {
                if let Some(session_id) = &self.current_session_id {
                    if let Some(task_id) = self
                        .trace
                        .selected_retry_task_id(&self.state.current_session.task_graph)
                    {
                        commands.push(Command::RetryTask {
                            workspace_id: self.workspace_id.clone(),
                            session_id: session_id.clone(),
                            task_id,
                        });
                    }
                }
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::CancelSelectedTask => {
                if let Some(session_id) = &self.current_session_id {
                    if let Some(task_id) = self
                        .trace
                        .selected_cancel_task_id(&self.state.current_session.task_graph)
                    {
                        commands.push(Command::CancelTask {
                            workspace_id: self.workspace_id.clone(),
                            session_id: session_id.clone(),
                            task_id,
                        });
                    }
                }
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::DeleteSelectedMemory => {
                if let Some(memory_id) = self.trace.selected_memory_id() {
                    commands.push(Command::DeleteMemory { memory_id });
                }
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::CyclePermissionMode => {
                let new_mode = self.state.cycle_permission_mode();
                commands.push(Command::SetPermissionMode { mode: new_mode });
                self.sync_status_bar();
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::NewSession => {
                commands.push(Command::StartSession {
                    workspace_id: self.workspace_id.clone(),
                    model_profile: self.state.model_profile.clone(),
                });
            }
            KeyAction::ContextMenu
                if self.state.focus_manager.current() == FocusTarget::Sessions =>
            {
                self.sessions.open_action_menu(&self.state.sessions);
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::SendInput
            | KeyAction::InputCharacter(_)
            | KeyAction::InputBackspace
            | KeyAction::InputDelete
            | KeyAction::InputNewline
            | KeyAction::ToggleInputMode
            | KeyAction::InputHistoryUp
            | KeyAction::InputHistoryDown
            | KeyAction::InputPaste(_)
            | KeyAction::AllowPermission
            | KeyAction::DenyPermission
            | KeyAction::DenyAllPermission
            | KeyAction::ContextMenu => {
                let (effects, cmds) = self.apply_chat_action(action);
                commands.extend(cmds);
                self.dispatch_effects(effects);
            }
            KeyAction::SelectSession => {
                if let Some(session) = self.sessions.selected_session(&self.state.sessions) {
                    if !session.archived {
                        commands.push(Command::SwitchSession {
                            session_id: session.id.clone(),
                        });
                    }
                }
            }
            KeyAction::ScrollUp => {
                if self.state.focus_manager.current() == FocusTarget::Sessions {
                    self.sessions.scroll_up(self.state.sessions.len());
                } else if self.state.focus_manager.current() == FocusTarget::Trace {
                    self.trace.select_previous();
                }
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::ScrollDown => {
                if self.state.focus_manager.current() == FocusTarget::Sessions {
                    self.sessions.scroll_down(self.state.sessions.len());
                } else if self.state.focus_manager.current() == FocusTarget::Trace {
                    let row_count = match self.trace.active_tab {
                        crate::components::trace::RightPanelTab::Tasks => {
                            crate::components::trace::flatten_task_tree(
                                &crate::components::trace::build_task_tree_from_snapshot(
                                    &self.state.current_session.task_graph,
                                ),
                            )
                            .len()
                        }
                        crate::components::trace::RightPanelTab::Memory => {
                            self.trace.memory_rows.len()
                        }
                        _ => 0,
                    };
                    self.trace.select_next(row_count);
                }
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::OpenProfileSelector => {
                if self.model_overlay.is_visible() {
                    self.dispatch_effects(vec![CrossPanelEffect::DismissModelOverlay]);
                    self.state.render_scheduler.mark_dirty_immediate();
                } else {
                    commands.push(Command::OpenModelOverlay);
                }
            }
            KeyAction::RenameSession => {
                if self.state.focus_manager.current() == FocusTarget::Sessions {
                    self.sessions
                        .start_rename_for_selected(&self.state.sessions);
                    self.state.render_scheduler.mark_dirty();
                }
            }
            KeyAction::Help | KeyAction::Unhandled => {}
        }

        commands
    }

    fn apply_chat_action(&mut self, action: KeyAction) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let focus = self.state.focus_manager.current();
        let sessions = self.state.sessions.clone();
        let model_profile = self.state.model_profile.clone();
        let permission_mode = self.state.permission_mode;
        let sidebar_left = self.state.sidebar_left_visible;
        let sidebar_right = self.state.sidebar_right_visible;
        let ctx = EventContext {
            focus,
            current_session: &self.state.current_session,
            sessions: &sessions,
            model_profile: &model_profile,
            permission_mode,
            sidebar_left_visible: sidebar_left,
            sidebar_right_visible: sidebar_right,
            workspace_id: &self.workspace_id,
            current_session_id: &self.current_session_id,
        };
        self.chat.apply_key_action(action, &ctx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::trace::{MemoryRow, RightPanelTab};
    use agent_core::facade::{TaskGraphSnapshot, TaskSnapshot};
    use agent_core::{AgentRole, TaskId, TaskState, WorkspaceId};
    use agent_tools::PermissionMode;

    fn task_snapshot(
        id: TaskId,
        title: &str,
        role: AgentRole,
        state: TaskState,
        dependencies: Vec<TaskId>,
        retry_count: usize,
        max_retries: usize,
    ) -> TaskSnapshot {
        TaskSnapshot {
            id,
            title: title.into(),
            role,
            state,
            dependencies,
            error: None,
            retry_count,
            max_retries,
            assigned_agent_id: None,
            failure_reason: None,
        }
    }

    #[test]
    fn tasks_tab_emits_retry_command_for_selected_failed_task() {
        let workspace_id = WorkspaceId::from_string("wrk_test".into());
        let session_id = agent_core::SessionId::from_string("ses_test".into());
        let task_id = TaskId::from_string("task_failed".into());
        let mut app = App::new("test", PermissionMode::Suggest, workspace_id.clone());
        app.current_session_id = Some(session_id.clone());
        app.trace.active_tab = RightPanelTab::Tasks;
        app.trace.selected_task_index = 0;
        app.state.current_session.task_graph = TaskGraphSnapshot {
            tasks: vec![task_snapshot(
                task_id.clone(),
                "Fix failure",
                AgentRole::Worker,
                TaskState::Failed,
                vec![],
                1,
                3,
            )],
        };

        let commands = app.apply_action(KeyAction::RetrySelectedTask);

        assert_eq!(
            commands,
            vec![Command::RetryTask {
                workspace_id,
                session_id,
                task_id,
            }]
        );
    }

    #[test]
    fn tasks_tab_emits_cancel_command_for_selected_blocked_task() {
        let workspace_id = WorkspaceId::from_string("wrk_test".into());
        let session_id = agent_core::SessionId::from_string("ses_test".into());
        let task_id = TaskId::from_string("task_blocked".into());
        let mut app = App::new("test", PermissionMode::Suggest, workspace_id.clone());
        app.current_session_id = Some(session_id.clone());
        app.trace.active_tab = RightPanelTab::Tasks;
        app.trace.selected_task_index = 0;
        app.state.current_session.task_graph = TaskGraphSnapshot {
            tasks: vec![task_snapshot(
                task_id.clone(),
                "Blocked task",
                AgentRole::Reviewer,
                TaskState::Blocked,
                vec![],
                0,
                3,
            )],
        };

        let commands = app.apply_action(KeyAction::CancelSelectedTask);

        assert_eq!(
            commands,
            vec![Command::CancelTask {
                workspace_id,
                session_id,
                task_id,
            }]
        );
    }

    #[test]
    fn cycling_to_memory_tab_requests_memory_load() {
        let workspace_id = WorkspaceId::from_string("wrk_test".into());
        let mut app = App::new("test", PermissionMode::Suggest, workspace_id);
        app.trace.active_tab = RightPanelTab::Tasks;

        let commands = app.apply_action(KeyAction::CycleTraceTabNext);

        assert_eq!(app.trace.active_tab, RightPanelTab::Memory);
        assert_eq!(
            commands,
            vec![Command::LoadMemories {
                scope: None,
                keywords: Vec::new(),
                limit: 100,
            }]
        );
    }

    #[test]
    fn memory_tab_emits_delete_command_for_selected_memory() {
        let workspace_id = WorkspaceId::from_string("wrk_test".into());
        let mut app = App::new("test", PermissionMode::Suggest, workspace_id);
        app.trace.active_tab = RightPanelTab::Memory;
        app.trace.set_memory_rows(vec![MemoryRow::new(
            "mem_user".into(),
            "user".into(),
            Some("preferred-command".into()),
            "Use cargo test".into(),
        )]);
        app.trace.selected_memory_index = 0;

        let commands = app.apply_action(KeyAction::DeleteSelectedMemory);

        assert_eq!(
            commands,
            vec![Command::DeleteMemory {
                memory_id: "mem_user".into(),
            }]
        );
    }
}
