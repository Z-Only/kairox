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
                &self.state.sessions,
                self.sessions.focused(),
                &mut self.sessions.state,
            );
        }

        let has_queue = !self.chat.message_queue.is_empty();
        let chat_constraints: Vec<Constraint> = if has_queue {
            vec![
                Constraint::Min(1),
                Constraint::Length(1),
                Constraint::Length(3),
            ]
        } else {
            vec![Constraint::Min(1), Constraint::Length(3)]
        };
        let chat_chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(chat_constraints)
            .split(chat_area);
        crate::components::chat::render_messages(
            chat_chunks[0],
            frame,
            &self.state.current_session,
        );
        if has_queue {
            crate::components::chat::render_queue_strip(
                chat_chunks[1],
                frame,
                &self.chat.message_queue,
            );
            self.render_input(chat_chunks[2], frame);
        } else {
            self.render_input(chat_chunks[1], frame);
        }

        if let Some(trace_area) = trace_area {
            match self.trace.density {
                crate::keybindings::TraceDensity::TaskGraph => {
                    let tasks = crate::components::trace::extract_task_traces(&self.domain_events);
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
                        );
                    }
                }
                _ => {
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

        self.state.render_scheduler.did_render();
    }

    fn render_input(&self, area: Rect, frame: &mut Frame) {
        let is_focused = self.state.focus_manager.current() == FocusTarget::Chat;
        let border_style = if is_focused {
            Style::default().fg(Color::Cyan)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        let mode_label = match self.state.input_mode {
            InputMode::SingleLine => "│ ",
            InputMode::MultiLine => "│M ",
        };
        let display_content =
            if let InputState::PermissionWait { pending_prompt, .. } = &self.state.input_state {
                format!("[permission] {}", pending_prompt)
            } else {
                let mut content = format!("{}{}", mode_label, self.chat.input_content);
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
