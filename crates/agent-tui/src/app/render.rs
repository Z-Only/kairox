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

        let has_queue = !self.chat.message_queue.is_empty();
        let has_file_mentions = self.chat.file_mentions_visible();
        let mut chat_constraints = vec![Constraint::Min(1)];
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
        crate::components::chat::render_messages(
            chat_chunks[0],
            frame,
            &self.state.current_session,
        );
        let mut chat_chunk_idx = 1;
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
                        crate::components::trace::render_task_graph(
                            trace_area,
                            frame,
                            &tasks,
                            self.trace.focused(),
                            self.trace.selected_task_index,
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
        frame.render_widget(paragraph, area);

        let border_block = ratatui::widgets::Block::default()
            .borders(ratatui::widgets::Borders::TOP)
            .border_style(border_style);
        frame.render_widget(
            ratatui::widgets::Paragraph::new("").block(border_block),
            area,
        );
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
