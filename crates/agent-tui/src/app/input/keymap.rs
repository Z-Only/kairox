//! Global keymap dispatch: convert a resolved [`KeyAction`] into a list of
//! [`Command`]s and apply local UI state changes (focus cycling, overlay
//! toggles, scrolling, trace tab cycling, ...). Chat-input-specific actions
//! delegate to [`App::apply_chat_action`] in `session.rs`.

use crate::app_state::CtrlCAction;
use crate::components::{Command, CrossPanelEffect, FocusTarget};
use crate::keybindings::KeyAction;

use crate::app::App;

impl App {
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
                if self.trace.active_tab == crate::components::trace::RightPanelTab::Memory
                    && (self.trace.memory_search_active
                        || self.trace.pending_delete_memory_id().is_some())
                {
                    self.trace.clear_memory_transient_state();
                    self.state.render_scheduler.mark_dirty();
                    return commands;
                }
                if self.quit_confirmed {
                    self.quit_confirmed = false;
                    self.state.reset_ctrl_c();
                    self.state.render_scheduler.mark_dirty();
                }
                let (effects, cmds) = self.apply_chat_action(KeyAction::Escape);
                commands.extend(cmds);
                self.dispatch_effects(effects);
            }
            KeyAction::FocusCycleNext
                if self.trace.active_tab == crate::components::trace::RightPanelTab::Memory
                    && self.trace.memory_search_active =>
            {
                self.trace.apply_memory_search();
                commands.push(self.trace.memory_load_command());
                self.state.render_scheduler.mark_dirty();
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
            KeyAction::TogglePluginsOverlay => {
                if self.plugin_overlay.is_visible() {
                    self.dispatch_effects(vec![CrossPanelEffect::DismissPluginsOverlay]);
                    self.state.render_scheduler.mark_dirty_immediate();
                } else {
                    commands.push(Command::OpenPluginsOverlay);
                }
            }
            KeyAction::ToggleInstructionsOverlay => {
                if self.instructions_overlay.is_visible() {
                    self.dispatch_effects(vec![CrossPanelEffect::DismissInstructionsOverlay]);
                    self.state.render_scheduler.mark_dirty_immediate();
                } else {
                    commands.push(Command::OpenInstructionsOverlay);
                }
            }
            KeyAction::ToggleHooksOverlay => {
                if self.hooks_overlay.is_visible() {
                    self.dispatch_effects(vec![CrossPanelEffect::DismissHooksOverlay]);
                    self.state.render_scheduler.mark_dirty_immediate();
                } else {
                    commands.push(Command::OpenHooksOverlay);
                }
            }
            KeyAction::ToggleTraceDensity => {
                self.trace.cycle_density();
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::CycleTraceTabNext => {
                self.trace.cycle_tab_next();
                if self.trace.active_tab == crate::components::trace::RightPanelTab::Memory {
                    commands.push(self.trace.memory_load_command());
                }
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::CycleTraceTabPrevious => {
                self.trace.cycle_tab_previous();
                if self.trace.active_tab == crate::components::trace::RightPanelTab::Memory {
                    commands.push(self.trace.memory_load_command());
                }
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::CycleMemoryScope => {
                if self.trace.active_tab == crate::components::trace::RightPanelTab::Memory {
                    self.trace.cycle_memory_scope_filter();
                    commands.push(self.trace.memory_load_command());
                }
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::StartMemorySearch => {
                if self.trace.active_tab == crate::components::trace::RightPanelTab::Memory {
                    self.trace.start_memory_search();
                    self.state.render_scheduler.mark_dirty();
                }
            }
            KeyAction::RetrySelectedTask => {
                if self.trace.active_tab == crate::components::trace::RightPanelTab::Memory {
                    commands.push(self.trace.memory_load_command());
                } else if let Some(session_id) = &self.current_session_id {
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
                if let Some(memory_id) = self.trace.begin_memory_delete_confirmation() {
                    commands.push(Command::DeleteMemory { memory_id });
                }
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::ConfirmMemoryDelete => {
                if let Some(memory_id) = self.trace.confirm_memory_delete() {
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
            KeyAction::CycleApprovalPolicy => {
                let new_approval = self.state.cycle_approval_policy();
                if let Some(session_id) = self.current_session_id.clone() {
                    commands.push(Command::SetSessionApprovalPolicy {
                        workspace_id: self.workspace_id.clone(),
                        session_id,
                        approval: new_approval,
                    });
                }
                self.sync_status_bar();
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::CycleSandboxPolicy => {
                let new_sandbox = self.state.cycle_sandbox_policy();
                if let Some(session_id) = self.current_session_id.clone() {
                    commands.push(Command::SetSessionSandboxPolicy {
                        workspace_id: self.workspace_id.clone(),
                        session_id,
                        sandbox: new_sandbox,
                    });
                }
                self.sync_status_bar();
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::ToggleContextDetails => {
                self.status_bar.toggle_context_details();
                self.state.render_scheduler.mark_dirty_immediate();
            }
            KeyAction::Help => {
                if self.help_overlay.is_visible() {
                    self.dispatch_effects(vec![CrossPanelEffect::DismissHelpOverlay]);
                } else {
                    self.dispatch_effects(vec![CrossPanelEffect::ShowHelpOverlay(
                        self.help_overlay_snapshot(),
                    )]);
                }
                self.state.render_scheduler.mark_dirty_immediate();
            }
            KeyAction::NewSession => {
                if let Some(command) = self.current_draft_save_command() {
                    commands.push(command);
                }
                commands.push(Command::StartSession {
                    workspace_id: self.workspace_id.clone(),
                    model_profile: self.state.model_profile.clone(),
                });
            }
            KeyAction::ContextMenu
                if self.state.focus_manager.current() == FocusTarget::Sessions =>
            {
                self.sessions
                    .open_action_menu(&self.state.projects, &self.state.sessions);
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::OpenArchiveManager
                if self.state.focus_manager.current() == FocusTarget::Sessions =>
            {
                self.sessions.open_archive_manager(&self.state.sessions);
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::InputCharacter(ch)
                if self.trace.active_tab == crate::components::trace::RightPanelTab::Memory
                    && self.trace.memory_search_active =>
            {
                self.trace.push_memory_search_char(ch);
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::InputBackspace
                if self.trace.active_tab == crate::components::trace::RightPanelTab::Memory
                    && self.trace.memory_search_active =>
            {
                self.trace.pop_memory_search_char();
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::InputDelete
                if self.trace.active_tab == crate::components::trace::RightPanelTab::Memory
                    && self.trace.memory_search_active =>
            {
                self.trace.memory_search_query.clear();
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
            | KeyAction::ApplyQueueAction(_)
            | KeyAction::AllowPermission
            | KeyAction::DenyPermission
            | KeyAction::DenyAllPermission
            | KeyAction::OpenArchiveManager
            | KeyAction::ContextMenu => {
                let (effects, cmds) = self.apply_chat_action(action);
                commands.extend(cmds);
                self.dispatch_effects(effects);
            }
            KeyAction::SelectSession => {
                if let Some(session) = self
                    .sessions
                    .selected_session_in(&self.state.projects, &self.state.sessions)
                {
                    if !session.archived {
                        if self.current_session_id.as_ref() == Some(&session.id) {
                            return commands;
                        }
                        if let Some(command) = self.current_draft_save_command() {
                            commands.push(command);
                        }
                        commands.push(Command::SwitchSession {
                            session_id: session.id.clone(),
                        });
                    }
                }
            }
            KeyAction::ScrollUp => {
                if self.state.focus_manager.current() == FocusTarget::Sessions {
                    self.sessions.scroll_up(
                        crate::components::sessions::session_list_rows(
                            &self.state.projects,
                            &self.state.sessions,
                        )
                        .len(),
                    );
                } else if self.state.focus_manager.current() == FocusTarget::Trace {
                    self.trace.select_previous();
                }
                self.state.render_scheduler.mark_dirty();
            }
            KeyAction::ScrollDown => {
                if self.state.focus_manager.current() == FocusTarget::Sessions {
                    self.sessions.scroll_down(
                        crate::components::sessions::session_list_rows(
                            &self.state.projects,
                            &self.state.sessions,
                        )
                        .len(),
                    );
                } else if self.state.focus_manager.current() == FocusTarget::Trace {
                    let row_count = match self.trace.active_tab {
                        crate::components::trace::RightPanelTab::Tasks => self
                            .trace
                            .visible_task_row_count(&self.state.current_session.task_graph),
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
                        .start_rename_for_selected(&self.state.projects, &self.state.sessions);
                    self.state.render_scheduler.mark_dirty();
                }
            }
            KeyAction::Unhandled => {}
        }

        commands
    }
}
