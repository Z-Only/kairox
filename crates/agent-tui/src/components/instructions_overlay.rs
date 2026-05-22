//! Instructions settings overlay for viewing and editing user/project
//! instructions from the TUI.

use agent_core::facade::InstructionsView;
use agent_core::ConfigScope;
use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span, Text};
use ratatui::widgets::{Block, Borders, Clear, Paragraph, Wrap};
use ratatui::Frame;

use crate::components::{Command, Component, CrossPanelEffect, EventContext};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InstructionsTab {
    User,
    Project,
    Effective,
}

impl InstructionsTab {
    fn next(self) -> Self {
        match self {
            Self::User => Self::Project,
            Self::Project => Self::Effective,
            Self::Effective => Self::User,
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::User => Self::Effective,
            Self::Project => Self::User,
            Self::Effective => Self::Project,
        }
    }
}

pub struct InstructionsOverlay {
    focused: bool,
    visible: bool,
    tab: InstructionsTab,
    system_text: String,
    user_text: String,
    project_text: String,
    user_cursor: usize,
    project_cursor: usize,
}

impl Default for InstructionsOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl InstructionsOverlay {
    pub fn new() -> Self {
        Self {
            focused: false,
            visible: false,
            tab: InstructionsTab::User,
            system_text: String::new(),
            user_text: String::new(),
            project_text: String::new(),
            user_cursor: 0,
            project_cursor: 0,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self, view: InstructionsView) {
        self.system_text = view.system;
        self.user_text = view.user.unwrap_or_default();
        self.project_text = view.project.unwrap_or_default();
        self.user_cursor = self.user_text.len();
        self.project_cursor = self.project_text.len();
        self.tab = InstructionsTab::User;
        self.visible = true;
    }

    pub fn set_active_scope(&mut self, scope: ConfigScope) {
        self.tab = match scope {
            ConfigScope::Project => InstructionsTab::Project,
            _ => InstructionsTab::User,
        };
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.tab = InstructionsTab::User;
    }

    pub fn active_scope(&self) -> ConfigScope {
        match self.tab {
            InstructionsTab::User | InstructionsTab::Effective => ConfigScope::User,
            InstructionsTab::Project => ConfigScope::Project,
        }
    }

    pub fn user_text(&self) -> &str {
        &self.user_text
    }

    pub fn project_text(&self) -> &str {
        &self.project_text
    }

    pub fn effective_text(&self) -> String {
        let parts = [
            self.system_text.as_str(),
            self.user_text.as_str(),
            self.project_text.as_str(),
        ];
        parts
            .into_iter()
            .map(str::trim)
            .filter(|part| !part.is_empty())
            .collect::<Vec<_>>()
            .join("\n\n")
    }

    fn selected_buffer_mut(&mut self) -> Option<(&mut String, &mut usize)> {
        match self.tab {
            InstructionsTab::User => Some((&mut self.user_text, &mut self.user_cursor)),
            InstructionsTab::Project => Some((&mut self.project_text, &mut self.project_cursor)),
            InstructionsTab::Effective => None,
        }
    }

    fn insert_char(&mut self, ch: char) {
        let Some((text, cursor)) = self.selected_buffer_mut() else {
            return;
        };
        text.insert(*cursor, ch);
        *cursor += ch.len_utf8();
    }

    fn insert_newline(&mut self) {
        self.insert_char('\n');
    }

    fn backspace(&mut self) {
        let Some((text, cursor)) = self.selected_buffer_mut() else {
            return;
        };
        if *cursor == 0 {
            return;
        }
        let previous = text[..*cursor]
            .char_indices()
            .last()
            .map(|(idx, _)| idx)
            .unwrap_or(0);
        text.drain(previous..*cursor);
        *cursor = previous;
    }

    fn save_command(&self) -> Option<Command> {
        let scope = self.active_scope();
        match self.tab {
            InstructionsTab::User => Some(Command::SaveInstructions {
                scope,
                text: self.user_text.trim().to_string(),
            }),
            InstructionsTab::Project => Some(Command::SaveInstructions {
                scope,
                text: self.project_text.trim().to_string(),
            }),
            InstructionsTab::Effective => None,
        }
    }
}

pub fn render_instructions_overlay(area: Rect, frame: &mut Frame, overlay: &InstructionsOverlay) {
    let modal_width = 84.min(area.width.saturating_sub(4));
    let modal_height = 24.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " Instructions ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    if inner.height < 5 {
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(1),
        ])
        .split(inner);

    let tabs = Line::from(vec![
        tab_span("User", overlay.tab == InstructionsTab::User),
        Span::raw("  "),
        tab_span("Project", overlay.tab == InstructionsTab::Project),
        Span::raw("  "),
        tab_span("Effective", overlay.tab == InstructionsTab::Effective),
    ]);
    frame.render_widget(Paragraph::new(tabs), chunks[0]);

    let (title, content, editable) = match overlay.tab {
        InstructionsTab::User => ("User instructions", overlay.user_text(), true),
        InstructionsTab::Project => ("Project instructions", overlay.project_text(), true),
        InstructionsTab::Effective => ("Effective preview", "", false),
    };
    let content = if overlay.tab == InstructionsTab::Effective {
        overlay.effective_text()
    } else {
        content.to_string()
    };
    let empty_hint = if editable {
        "<empty - save an empty value to clear this scope>"
    } else {
        "<empty>"
    };
    let shown = if content.trim().is_empty() {
        empty_hint.to_string()
    } else {
        content
    };
    let mut body_lines = vec![
        Line::from(Span::styled(
            title,
            Style::default()
                .fg(if editable {
                    Color::Yellow
                } else {
                    Color::Magenta
                })
                .add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];
    body_lines.extend(shown.lines().map(Line::from));
    let body = Text::from(body_lines);
    let body_block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if editable {
            Color::Yellow
        } else {
            Color::DarkGray
        }));
    frame.render_widget(
        Paragraph::new(body)
            .block(body_block)
            .wrap(Wrap { trim: false }),
        chunks[1],
    );

    let hints = Line::from(vec![
        Span::styled("[Tab] scope  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[F2] save  ", Style::default().fg(Color::Yellow)),
        Span::styled("[Enter] newline  ", Style::default().fg(Color::Cyan)),
        Span::styled("[Esc] close", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(hints), chunks[2]);
}

fn tab_span(label: &'static str, active: bool) -> Span<'static> {
    if active {
        Span::styled(
            format!("[{label}]"),
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        )
    } else {
        Span::styled(format!(" {label} "), Style::default().fg(Color::DarkGray))
    }
}

impl Component for InstructionsOverlay {
    fn handle_event(
        &mut self,
        _ctx: &EventContext,
        event: &Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let Event::Key(key) = event else {
            return (Vec::new(), Vec::new());
        };
        if !self.visible {
            return (Vec::new(), Vec::new());
        }

        match key.code {
            KeyCode::Esc => {
                self.hide();
                (
                    vec![CrossPanelEffect::DismissInstructionsOverlay],
                    Vec::new(),
                )
            }
            KeyCode::Tab => {
                self.tab = self.tab.next();
                (Vec::new(), Vec::new())
            }
            KeyCode::BackTab => {
                self.tab = self.tab.previous();
                (Vec::new(), Vec::new())
            }
            KeyCode::F(2) => {
                let commands = self.save_command().into_iter().collect();
                (Vec::new(), commands)
            }
            KeyCode::Char('s') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                let commands = self.save_command().into_iter().collect();
                (Vec::new(), commands)
            }
            KeyCode::Enter => {
                self.insert_newline();
                (Vec::new(), Vec::new())
            }
            KeyCode::Backspace => {
                self.backspace();
                (Vec::new(), Vec::new())
            }
            KeyCode::Char(ch) => {
                self.insert_char(ch);
                (Vec::new(), Vec::new())
            }
            _ => (Vec::new(), Vec::new()),
        }
    }

    fn handle_effect(&mut self, effect: &CrossPanelEffect) {
        match effect {
            CrossPanelEffect::ShowInstructionsOverlay(view) => self.show(view.clone()),
            CrossPanelEffect::DismissInstructionsOverlay => self.hide(),
            _ => {}
        }
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        if self.visible {
            render_instructions_overlay(area, frame, self);
        }
    }

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}
