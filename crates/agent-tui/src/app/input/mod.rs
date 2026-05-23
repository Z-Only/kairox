//! Input dispatch for the TUI [`App`]. The top-level
//! [`App::handle_crossterm_event`] entry point lives here. It first runs the
//! event through [`App::handle_crossterm_event_unconfirmed`] (overlay/palette
//! routing in [`palette`]), then funnels any returned [`Command`]s through
//! [`App::confirm_destructive_commands`] for two-step destructive
//! confirmation. The keymap → [`Command`] mapping lives in [`keymap`] and
//! the chat composer / queue handling lives in [`session`].

use crossterm::event::Event;

use crate::components::Command;

use super::App;

mod keymap;
mod palette;
mod session;

impl App {
    /// Handle a raw crossterm event, returning any commands to dispatch.
    pub fn handle_crossterm_event(&mut self, event: &Event) -> Vec<Command> {
        let commands = self.handle_crossterm_event_unconfirmed(event);
        self.confirm_destructive_commands(commands)
    }

    fn confirm_destructive_commands(&mut self, commands: Vec<Command>) -> Vec<Command> {
        let mut saw_destructive_command = false;
        let mut confirmed = Vec::with_capacity(commands.len());

        for command in commands {
            let Some(target) = command.destructive_confirmation_target() else {
                confirmed.push(command);
                continue;
            };

            saw_destructive_command = true;
            if self.destructive_confirmation.arm_or_confirm(target) {
                self.finalize_confirmed_destructive_command(&command);
                confirmed.push(command);
            } else if let Some(hint) = self.destructive_confirmation.pending_hint() {
                self.state.push_status_message(hint.clone());
                self.status_bar.push_notification(hint);
                self.state.render_scheduler.mark_dirty();
            }
        }

        if !saw_destructive_command {
            self.destructive_confirmation.clear();
        }

        confirmed
    }

    fn finalize_confirmed_destructive_command(&mut self, command: &Command) {
        match command {
            Command::ArchiveSession { .. } | Command::RemoveProject { .. } => {
                self.sessions.close_action_menu();
            }
            Command::DeleteSession { .. } => {
                self.sessions.close_action_menu();
                self.sessions.close_archive_manager();
            }
            _ => {}
        }
    }

    pub(super) fn current_draft_save_command(&self) -> Option<Command> {
        Some(Command::SaveDraft {
            session_id: self.current_session_id.clone()?,
            draft_text: self.chat.input_content.clone(),
        })
    }

    pub(super) fn help_overlay_snapshot(&self) -> crate::components::HelpOverlaySnapshot {
        crate::components::HelpOverlaySnapshot {
            focus: self.state.focus_manager.current(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::app::App;
    use crate::components::trace::{MemoryRow, MemoryScopeFilter, RightPanelTab};
    use crate::components::{
        Command, CrossPanelEffect, FocusTarget, PermissionRequest, RiskLevel, SessionInfo,
        SessionState,
    };
    use crate::keybindings::KeyAction;
    use agent_core::facade::{TaskGraphSnapshot, TaskSnapshot};
    use agent_core::{
        AgentRole, ProjectSessionVisibility, SessionId, TaskId, TaskState, WorkspaceId,
    };
    use agent_memory::MemoryScope;
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

    fn session_info(id: SessionId, title: &str) -> SessionInfo {
        SessionInfo {
            id,
            title: title.to_string(),
            model_profile: "fast".to_string(),
            state: SessionState::Idle,
            pinned: false,
            archived: false,
            project_id: None,
            worktree_path: None,
            branch: None,
            visibility: Some(ProjectSessionVisibility::Visible),
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
    fn trace_tasks_tab_enter_toggles_selected_task_fold_without_cycling_focus() {
        let workspace_id = WorkspaceId::from_string("wrk_test".into());
        let root_id = TaskId::from_string("task_root".into());
        let child_id = TaskId::from_string("task_child".into());
        let grandchild_id = TaskId::from_string("task_grandchild".into());
        let mut app = App::new("test", PermissionMode::Suggest, workspace_id);
        app.state.focus_manager.set(FocusTarget::Trace);
        app.sync_component_focus();
        app.trace.active_tab = RightPanelTab::Tasks;
        app.trace.selected_task_index = 0;
        app.state.current_session.task_graph = TaskGraphSnapshot {
            tasks: vec![
                task_snapshot(
                    root_id.clone(),
                    "Plan",
                    AgentRole::Planner,
                    TaskState::Completed,
                    vec![],
                    0,
                    3,
                ),
                task_snapshot(
                    child_id.clone(),
                    "Build",
                    AgentRole::Worker,
                    TaskState::Running,
                    vec![root_id.clone()],
                    0,
                    3,
                ),
                task_snapshot(
                    grandchild_id,
                    "Review",
                    AgentRole::Reviewer,
                    TaskState::Pending,
                    vec![child_id],
                    0,
                    3,
                ),
            ],
        };

        let commands = app.handle_crossterm_event(&crossterm::event::Event::Key(
            crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Enter,
                crossterm::event::KeyModifiers::NONE,
            ),
        ));

        assert!(commands.is_empty());
        assert_eq!(app.state.focus_manager.current(), FocusTarget::Trace);
        assert_eq!(
            app.trace
                .visible_task_row_count(&app.state.current_session.task_graph),
            1
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
    fn memory_scope_cycle_updates_filter_and_loads_memories() {
        let workspace_id = WorkspaceId::from_string("wrk_test".into());
        let mut app = App::new("test", PermissionMode::Suggest, workspace_id);
        app.trace.active_tab = RightPanelTab::Memory;

        let commands = app.apply_action(KeyAction::CycleMemoryScope);

        assert_eq!(app.trace.memory_scope_filter, MemoryScopeFilter::Session);
        assert_eq!(
            commands,
            vec![Command::LoadMemories {
                scope: Some(MemoryScope::Session),
                keywords: Vec::new(),
                limit: 100,
            }]
        );
    }

    #[test]
    fn memory_search_enter_loads_keyword_filter() {
        let workspace_id = WorkspaceId::from_string("wrk_test".into());
        let mut app = App::new("test", PermissionMode::Suggest, workspace_id);
        app.trace.active_tab = RightPanelTab::Memory;

        assert!(app.apply_action(KeyAction::StartMemorySearch).is_empty());
        for ch in "cargo test".chars() {
            assert!(app.apply_action(KeyAction::InputCharacter(ch)).is_empty());
        }
        let commands = app.apply_action(KeyAction::FocusCycleNext);

        assert_eq!(app.trace.memory_search_query, "cargo test");
        assert_eq!(
            commands,
            vec![Command::LoadMemories {
                scope: None,
                keywords: vec!["cargo".into(), "test".into()],
                limit: 100,
            }]
        );
    }

    #[test]
    fn memory_search_mode_captures_filter_shortcut_characters() {
        let workspace_id = WorkspaceId::from_string("wrk_test".into());
        let mut app = App::new("test", PermissionMode::Suggest, workspace_id);
        app.state.focus_manager.set(FocusTarget::Trace);
        app.sync_component_focus();
        app.trace.active_tab = RightPanelTab::Memory;
        app.trace.start_memory_search();

        for ch in ['s', 'r'] {
            let commands = app.handle_crossterm_event(&crossterm::event::Event::Key(
                crossterm::event::KeyEvent::new(
                    crossterm::event::KeyCode::Char(ch),
                    crossterm::event::KeyModifiers::NONE,
                ),
            ));
            assert!(commands.is_empty());
        }

        assert_eq!(app.trace.memory_search_query, "sr");
        assert_eq!(app.trace.memory_scope_filter, MemoryScopeFilter::All);
    }

    #[test]
    fn memory_refresh_uses_current_filters() {
        let workspace_id = WorkspaceId::from_string("wrk_test".into());
        let mut app = App::new("test", PermissionMode::Suggest, workspace_id);
        app.trace.active_tab = RightPanelTab::Memory;
        app.trace.memory_scope_filter = MemoryScopeFilter::Workspace;
        app.trace.memory_search_query = "release notes".into();

        let commands = app.apply_action(KeyAction::RetrySelectedTask);

        assert_eq!(
            commands,
            vec![Command::LoadMemories {
                scope: Some(MemoryScope::Workspace),
                keywords: vec!["release".into(), "notes".into()],
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

        assert!(commands.is_empty());
        assert_eq!(
            app.trace.pending_delete_memory_id(),
            Some("mem_user".to_string())
        );
        let commands = app.apply_action(KeyAction::ConfirmMemoryDelete);

        assert_eq!(
            commands,
            vec![Command::DeleteMemory {
                memory_id: "mem_user".into(),
            }]
        );
        assert!(app.trace.pending_delete_memory_id().is_none());
    }

    #[test]
    fn context_details_shortcut_routes_compact_command() {
        let workspace_id = WorkspaceId::from_string("wrk_test".into());
        let session_id = SessionId::from_string("ses_current".into());
        let mut app = App::new("test", PermissionMode::Suggest, workspace_id.clone());
        app.current_session_id = Some(session_id.clone());
        app.last_context_usage = Some(agent_core::context_types::ContextUsage {
            total_tokens: 110_000,
            budget_tokens: 180_000,
            context_window: 200_000,
            output_reservation: 20_000,
            by_source: vec![(agent_core::context_types::ContextSource::History, 90_000)],
            estimator: "cl100k_base".to_string(),
            corrected_by_real_usage: false,
        });
        app.sync_status_bar();

        let commands = app.handle_crossterm_event(&crossterm::event::Event::Key(
            crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char('c'),
                crossterm::event::KeyModifiers::ALT,
            ),
        ));
        assert!(commands.is_empty());
        assert!(app.status_bar.context_details_visible());

        let commands = app.handle_crossterm_event(&crossterm::event::Event::Key(
            crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char('c'),
                crossterm::event::KeyModifiers::NONE,
            ),
        ));

        assert_eq!(
            commands,
            vec![Command::CompactSession {
                workspace_id,
                session_id,
            }]
        );
        assert!(!app.status_bar.context_details_visible());
    }

    #[test]
    fn f1_toggles_help_overlay_without_commands() {
        let workspace_id = WorkspaceId::from_string("wrk_test".into());
        let mut app = App::new("test", PermissionMode::Suggest, workspace_id);

        let commands = app.handle_crossterm_event(&crossterm::event::Event::Key(
            crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::F(1),
                crossterm::event::KeyModifiers::NONE,
            ),
        ));

        assert!(commands.is_empty());
        assert!(app.help_overlay.is_visible());
        assert_eq!(app.state.focus_manager.current(), FocusTarget::Chat);

        let commands = app.handle_crossterm_event(&crossterm::event::Event::Key(
            crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::F(1),
                crossterm::event::KeyModifiers::NONE,
            ),
        ));

        assert!(commands.is_empty());
        assert!(!app.help_overlay.is_visible());
        assert_eq!(app.state.focus_manager.current(), FocusTarget::Chat);
    }

    #[test]
    fn f1_opens_help_overlay_above_existing_overlay() {
        let workspace_id = WorkspaceId::from_string("wrk_test".into());
        let mut app = App::new("test", PermissionMode::Suggest, workspace_id);
        app.dispatch_effects(vec![CrossPanelEffect::ShowCommandPalette]);

        let commands = app.handle_crossterm_event(&crossterm::event::Event::Key(
            crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::F(1),
                crossterm::event::KeyModifiers::NONE,
            ),
        ));

        assert!(commands.is_empty());
        assert!(app.command_palette.is_visible());
        assert!(app.help_overlay.is_visible());
        assert_eq!(
            app.state.focus_manager.current(),
            FocusTarget::CommandPalette
        );
    }

    #[test]
    fn selecting_session_saves_current_draft_before_switching() {
        let workspace_id = WorkspaceId::from_string("wrk_test".into());
        let current = SessionId::from_string("ses_current".into());
        let next = SessionId::from_string("ses_next".into());
        let mut app = App::new("test", PermissionMode::Suggest, workspace_id);
        app.current_session_id = Some(current.clone());
        app.state.sessions = vec![
            session_info(current.clone(), "current"),
            session_info(next.clone(), "next"),
        ];
        app.sessions.state.select(Some(1));
        app.chat.input_content = "unfinished draft".to_string();
        app.chat.input_cursor = app.chat.input_content.len();

        let commands = app.apply_action(KeyAction::SelectSession);

        assert_eq!(
            commands,
            vec![
                Command::SaveDraft {
                    session_id: current,
                    draft_text: "unfinished draft".to_string(),
                },
                Command::SwitchSession { session_id: next },
            ]
        );
    }

    #[test]
    fn archive_manager_restores_selected_archived_session_from_app_event_route() {
        let workspace_id = WorkspaceId::from_string("wrk_test".into());
        let archived_id = SessionId::from_string("ses_archived".into());
        let mut archived = session_info(archived_id.clone(), "archived");
        archived.archived = true;
        archived.visibility = Some(ProjectSessionVisibility::Archived);
        let mut app = App::new("test", PermissionMode::Suggest, workspace_id);
        app.state.sessions = vec![archived];
        app.state.focus_manager.set(FocusTarget::Sessions);

        let commands = app.apply_action(KeyAction::OpenArchiveManager);
        assert!(commands.is_empty());
        assert!(app.sessions.archive_manager_open);

        let commands = app.handle_crossterm_event(&crossterm::event::Event::Key(
            crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Enter,
                crossterm::event::KeyModifiers::NONE,
            ),
        ));

        assert_eq!(
            commands,
            vec![Command::RestoreSession {
                session_id: archived_id,
            }]
        );
        assert!(!app.sessions.archive_manager_open);
    }

    #[test]
    fn typing_updates_current_session_draft() {
        let workspace_id = WorkspaceId::from_string("wrk_test".into());
        let session_id = SessionId::from_string("ses_current".into());
        let mut app = App::new("test", PermissionMode::Suggest, workspace_id);
        app.current_session_id = Some(session_id.clone());

        let commands = app.apply_action(KeyAction::InputCharacter('a'));

        assert_eq!(
            commands,
            vec![Command::SaveDraft {
                session_id,
                draft_text: "a".to_string(),
            }]
        );
    }

    #[test]
    fn sending_message_clears_current_session_draft_after_send_command() {
        let workspace_id = WorkspaceId::from_string("wrk_test".into());
        let session_id = SessionId::from_string("ses_current".into());
        let mut app = App::new("test", PermissionMode::Suggest, workspace_id.clone());
        app.current_session_id = Some(session_id.clone());
        app.state.sessions = vec![session_info(session_id.clone(), "current")];
        app.chat.input_content = "ready to send".to_string();
        app.chat.input_cursor = app.chat.input_content.len();

        let commands = app.apply_action(KeyAction::SendInput);

        assert_eq!(commands.len(), 2);
        assert!(matches!(
            &commands[0],
            Command::SendMessage {
                workspace_id: command_workspace_id,
                session_id: command_session_id,
                content,
                attachments,
            } if command_workspace_id == &workspace_id
                && command_session_id == &session_id
                && content == "ready to send"
                && attachments.is_empty()
        ));
        assert_eq!(
            commands[1],
            Command::SaveDraft {
                session_id,
                draft_text: String::new(),
            }
        );
    }

    #[test]
    fn mcp_trust_key_routes_to_permission_modal() {
        let workspace_id = WorkspaceId::from_string("wrk_test".into());
        let mut app = App::new("test", PermissionMode::Suggest, workspace_id);
        app.dispatch_effects(vec![CrossPanelEffect::ShowPermissionPrompt(
            PermissionRequest {
                request_id: "req_mcp".into(),
                tool_id: "mcp.beta.echo".into(),
                tool_preview: "MCP tool call".into(),
                risk_level: RiskLevel::McpTool {
                    server_id: "beta".into(),
                },
            },
        )]);

        let commands = app.handle_crossterm_event(&crossterm::event::Event::Key(
            crossterm::event::KeyEvent::new(
                crossterm::event::KeyCode::Char('t'),
                crossterm::event::KeyModifiers::NONE,
            ),
        ));

        assert_eq!(
            commands,
            vec![
                Command::TrustMcpServer {
                    server_id: "beta".into(),
                },
                Command::DecidePermission {
                    request_id: "req_mcp".into(),
                    approved: true,
                },
            ]
        );
    }
}
