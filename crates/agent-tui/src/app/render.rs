use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::app_state::{InputMode, InputState};
use crate::components::{Component, FocusTarget};

use super::App;

impl App {
    pub fn render(&mut self, frame: &mut Frame) {
        let area = frame.area();

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(area);
        let main_area = chunks[0];
        let status_area = chunks[1];

        let mut constraints = Vec::new();
        let sessions_visible = self.state.sidebar_left_visible;
        let trace_visible = self.state.sidebar_right_visible;

        if sessions_visible {
            constraints.push(Constraint::Length(24));
        }
        constraints.push(Constraint::Min(20));
        if trace_visible {
            constraints.push(Constraint::Length(32));
        }

        let main_chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(main_area);

        let mut chunk_idx = 0;
        let sessions_area = if sessions_visible {
            let area = main_chunks[chunk_idx];
            chunk_idx += 1;
            Some(area)
        } else {
            None
        };
        let chat_area = main_chunks[chunk_idx];
        chunk_idx += 1;
        let trace_area = if trace_visible {
            let area = main_chunks[chunk_idx];
            Some(area)
        } else {
            None
        };

        if let Some(sessions_area) = sessions_area {
            crate::components::sessions::render_sessions(
                sessions_area,
                frame,
                &self.state.projects,
                &self.state.sessions,
                self.sessions.focused(),
                &mut self.sessions.state,
            );
        }

        let session_metadata = self.current_session_metadata();
        let has_session_metadata = !session_metadata.is_empty();
        let has_queue = !self.chat.message_queue.is_empty();
        let has_file_mentions = self.chat.file_mentions_visible();
        let mut chat_constraints = Vec::new();
        if has_session_metadata {
            chat_constraints.push(Constraint::Length(1));
        }
        chat_constraints.push(Constraint::Min(1));
        if has_queue {
            chat_constraints.push(Constraint::Length(queue_strip_height(
                self.chat.message_queue.len(),
            )));
        }
        if has_file_mentions {
            chat_constraints.push(Constraint::Length(file_mention_palette_height(
                self.chat.file_mention_matches().len(),
            )));
        }
        chat_constraints.push(Constraint::Length(3));
        let chat_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(chat_constraints)
            .split(chat_area);
        let mut chat_chunk_idx = 0;
        if has_session_metadata {
            render_current_session_header(chat_chunks[chat_chunk_idx], frame, &session_metadata);
            chat_chunk_idx += 1;
        }
        crate::components::chat::render_messages(
            chat_chunks[chat_chunk_idx],
            frame,
            &self.state.current_session,
        );
        chat_chunk_idx += 1;
        if has_queue {
            crate::components::chat::render_queue_strip(
                chat_chunks[chat_chunk_idx],
                frame,
                &self.chat.message_queue,
                self.chat.selected_queue_index(),
            );
            chat_chunk_idx += 1;
        }
        if has_file_mentions {
            crate::components::chat::render_file_mention_palette(
                chat_chunks[chat_chunk_idx],
                frame,
                self.chat.file_mention_matches(),
                self.chat.selected_file_mention_index(),
            );
            chat_chunk_idx += 1;
        }
        self.render_input(chat_chunks[chat_chunk_idx], frame);

        if let Some(trace_area) = trace_area {
            match self.trace.active_tab {
                crate::components::trace::RightPanelTab::Tasks => {
                    let mut tasks = crate::components::trace::build_task_tree_from_snapshot(
                        &self.state.current_session.task_graph,
                    );
                    if tasks.is_empty() {
                        tasks = crate::components::trace::extract_task_traces(&self.domain_events);
                    }
                    if tasks.is_empty() {
                        crate::components::trace::render_task_graph_placeholder(
                            trace_area,
                            frame,
                            self.trace.focused(),
                        );
                    } else {
                        crate::components::trace::render_task_graph_with_collapsed(
                            trace_area,
                            frame,
                            &tasks,
                            self.trace.focused(),
                            self.trace.selected_task_index,
                            self.trace.collapsed_task_ids(),
                        );
                    }
                }
                crate::components::trace::RightPanelTab::Memory => {
                    crate::components::trace::render_memory_browser(trace_area, frame, &self.trace);
                }
                crate::components::trace::RightPanelTab::Trace => {
                    let traces = crate::components::trace::extract_tool_traces(&self.domain_events);
                    crate::components::trace::render_trace_l1(
                        trace_area,
                        frame,
                        &traces,
                        self.trace.focused(),
                    );
                }
            }
        }

        self.status_bar.render(status_area, frame);

        if self.permission_modal.is_visible() {
            self.permission_modal.render(area, frame);
        }

        if self.mcp_overlay.is_visible() {
            self.mcp_overlay.render(area, frame);
        }

        if self.command_palette.is_visible() {
            self.command_palette.render(area, frame);
        }

        if self.skills_overlay.is_visible() {
            self.skills_overlay.render(area, frame);
        }

        if self.model_overlay.is_visible() {
            self.model_overlay.render(area, frame);
        }

        if self.agent_overlay.is_visible() {
            self.agent_overlay.render(area, frame);
        }

        self.sessions.render_action_overlay(
            area,
            frame,
            &self.state.projects,
            &self.state.sessions,
        );

        if self.plugin_overlay.is_visible() {
            self.plugin_overlay.render(area, frame);
        }

        if self.hooks_overlay.is_visible() {
            self.hooks_overlay.render(area, frame);
        }

        if self.instructions_overlay.is_visible() {
            self.instructions_overlay.render(area, frame);
        }

        if self.help_overlay.is_visible() {
            self.help_overlay.render(area, frame);
        }

        self.state.render_scheduler.did_render();
    }

    fn render_input(&self, area: Rect, frame: &mut Frame) {
        let is_focused = self.state.focus_manager.current() == FocusTarget::Chat;
        let border_style = if is_focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let mode_label = match self.chat.input_mode {
            InputMode::SingleLine => "│ ",
            InputMode::MultiLine => "│M ",
        };
        let display_content = if let InputState::PermissionWait { pending_prompt, .. } =
            &self.chat.input_state
        {
            format!("[permission] {}", pending_prompt)
        } else {
            let mut content = format!("{}{}", mode_label, self.chat.input_content);
            let attachment_labels =
                crate::components::chat::format_attachment_labels(&self.chat.pending_attachments);
            if !attachment_labels.is_empty() {
                if !self.chat.input_content.is_empty() {
                    content.push(' ');
                }
                content.push_str(&attachment_labels);
            }
            if self.state.render_scheduler.is_streaming() {
                content.push('▌');
            }
            content
        };

        let input_line = Line::from(vec![
            Span::styled(
                if is_focused { ">" } else { " " },
                Style::default()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::raw(display_content),
        ]);

        let paragraph = Paragraph::new(input_line).block(
            ratatui::widgets::Block::default()
                .borders(ratatui::widgets::Borders::NONE)
                .style(Style::default()),
        );

        let border_block = ratatui::widgets::Block::default()
            .borders(ratatui::widgets::Borders::TOP)
            .border_style(border_style);
        let input_area = border_block.inner(area);
        frame.render_widget(
            ratatui::widgets::Paragraph::new("").block(border_block),
            area,
        );
        frame.render_widget(paragraph, input_area);
    }
}

fn queue_strip_height(queue_len: usize) -> u16 {
    let visible_rows = queue_len.min(4) as u16;
    visible_rows.saturating_add(1).max(2)
}

fn file_mention_palette_height(match_count: usize) -> u16 {
    let visible_rows = match_count.clamp(1, 4) as u16;
    visible_rows.saturating_add(1).max(2)
}

fn render_current_session_header(area: Rect, frame: &mut Frame, metadata: &[String]) {
    let mut spans = vec![Span::styled(
        "Chat",
        Style::default()
            .fg(Color::Cyan)
            .add_modifier(Modifier::BOLD),
    )];
    for part in metadata {
        spans.push(Span::styled(" · ", Style::default().fg(Color::DarkGray)));
        spans.push(Span::styled(
            part.clone(),
            Style::default().fg(Color::DarkGray),
        ));
    }
    frame.render_widget(Paragraph::new(Line::from(spans)), area);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{ProjectInfo, SessionInfo, SessionState};
    use agent_core::{
        ProjectId, ProjectInstructionSummary, ProjectSessionVisibility, SessionId, WorkspaceId,
    };

    fn render_text(app: &mut App, width: u16, height: u16) -> String {
        let backend = ratatui::backend::TestBackend::new(width, height);
        let mut terminal = ratatui::Terminal::new(backend).expect("terminal");
        terminal.draw(|frame| app.render(frame)).expect("draw");
        terminal
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|cell| cell.symbol())
            .collect::<Vec<_>>()
            .join("")
    }

    fn project_session_app() -> App {
        let workspace_id = WorkspaceId::from_string("wrk_test".to_string());
        let project_id = ProjectId::from_string("prj_alpha".to_string());
        let session_id = SessionId::from_string("ses_active".to_string());
        let mut app = App::new("fast", agent_tools::PermissionMode::Suggest, workspace_id);
        app.current_session_id = Some(session_id.clone());
        app.state.sidebar_left_visible = false;
        app.state.projects = vec![ProjectInfo {
            id: project_id.clone(),
            display_name: "alpha".to_string(),
            root_path: "/tmp/alpha".to_string(),
            expanded: true,
            git_status: None,
            instruction_summary: Some(ProjectInstructionSummary {
                source_paths: vec!["/tmp/alpha/AGENTS.md".to_string()],
                contents: None,
                warning: None,
            }),
        }];
        app.state.sessions = vec![SessionInfo {
            id: session_id,
            title: "Worktree session".to_string(),
            model_profile: "fast".to_string(),
            state: SessionState::Active,
            pinned: false,
            archived: false,
            project_id: Some(project_id),
            worktree_path: Some("/tmp/alpha/.kairox/worktrees/feat-tui".to_string()),
            branch: Some("feat/tui".to_string()),
            visibility: Some(ProjectSessionVisibility::Visible),
        }];
        app.sync_status_bar();
        app
    }

    #[test]
    fn session_git_meta_renders_in_current_chat_header_and_status() {
        let mut app = project_session_app();

        let rendered = render_text(&mut app, 120, 12);

        assert!(rendered.contains("worktree"), "{rendered}");
        assert!(rendered.contains("feat/tui"), "{rendered}");
        assert!(rendered.contains("worktrees/feat-tui"), "{rendered}");
    }

    #[test]
    fn session_git_meta_renders_project_instruction_summary() {
        let mut app = project_session_app();

        let rendered = render_text(&mut app, 120, 12);

        assert!(rendered.contains("Loaded AGENTS.md"), "{rendered}");
    }
}
