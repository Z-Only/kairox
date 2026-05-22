//! Hooks settings overlay for managing user/project command hooks from the TUI.

use agent_core::facade::{
    HookSettingsInput, HookSettingsView, HookTemplateView, HooksSettingsView,
};
use agent_core::ConfigScope;
use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap};
use ratatui::Frame;

use crate::components::{Command, Component, CrossPanelEffect, EventContext};

const HOOK_EVENTS: [&str; 6] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PermissionRequest",
    "PostToolUse",
    "Stop",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HooksTab {
    User,
    Project,
    Templates,
}

impl HooksTab {
    fn next(self) -> Self {
        match self {
            Self::User => Self::Project,
            Self::Project => Self::Templates,
            Self::Templates => Self::User,
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::User => Self::Templates,
            Self::Project => Self::User,
            Self::Templates => Self::Project,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::User => "User",
            Self::Project => "Project",
            Self::Templates => "Templates",
        }
    }

    fn scope(self) -> Option<ConfigScope> {
        match self {
            Self::User => Some(ConfigScope::User),
            Self::Project => Some(ConfigScope::Project),
            Self::Templates => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HooksMode {
    List,
    Editor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HookEditorField {
    Scope,
    Id,
    Event,
    Matcher,
    Command,
    StatusMessage,
    TimeoutSecs,
    Enabled,
}

const EDITOR_FIELDS: [HookEditorField; 8] = [
    HookEditorField::Scope,
    HookEditorField::Id,
    HookEditorField::Event,
    HookEditorField::Matcher,
    HookEditorField::Command,
    HookEditorField::StatusMessage,
    HookEditorField::TimeoutSecs,
    HookEditorField::Enabled,
];

#[derive(Debug, Clone, PartialEq, Eq)]
struct HookDraft {
    scope: ConfigScope,
    id: String,
    event: String,
    matcher: String,
    command: String,
    status_message: String,
    timeout_secs: String,
    enabled: bool,
}

impl HookDraft {
    fn new(scope: ConfigScope) -> Self {
        Self {
            scope,
            id: String::new(),
            event: "Stop".into(),
            matcher: "*".into(),
            command: String::new(),
            status_message: String::new(),
            timeout_secs: "600".into(),
            enabled: true,
        }
    }

    fn from_hook(hook: &HookSettingsView, fallback_scope: ConfigScope) -> Self {
        Self {
            scope: hook.source,
            id: hook.id.clone(),
            event: hook.event.clone(),
            matcher: hook.matcher.clone().unwrap_or_default(),
            command: hook.command.clone(),
            status_message: hook.status_message.clone().unwrap_or_default(),
            timeout_secs: hook
                .timeout_secs
                .map(|value| value.to_string())
                .unwrap_or_default(),
            enabled: hook.enabled,
        }
        .with_scope_if_read_only(fallback_scope)
    }

    fn from_template(template: &HookTemplateView, scope: ConfigScope) -> Self {
        Self {
            scope,
            id: template.id.clone(),
            event: template.event.clone(),
            matcher: template.matcher.clone().unwrap_or_default(),
            command: template.command.clone(),
            status_message: template.status_message.clone().unwrap_or_default(),
            timeout_secs: template
                .timeout_secs
                .map(|value| value.to_string())
                .unwrap_or_default(),
            enabled: true,
        }
    }

    #[cfg(test)]
    fn from_input(input: HookSettingsInput) -> Self {
        Self {
            scope: input.scope,
            id: input.id,
            event: input.event,
            matcher: input.matcher.unwrap_or_default(),
            command: input.command,
            status_message: input.status_message.unwrap_or_default(),
            timeout_secs: input
                .timeout_secs
                .map(|value| value.to_string())
                .unwrap_or_default(),
            enabled: input.enabled,
        }
    }

    fn with_scope_if_read_only(mut self, fallback_scope: ConfigScope) -> Self {
        if !matches!(self.scope, ConfigScope::User | ConfigScope::Project) {
            self.scope = fallback_scope;
        }
        self
    }

    fn to_input(&self) -> Option<HookSettingsInput> {
        let id = self.id.trim();
        let event = self.event.trim();
        let command = self.command.trim();
        if id.is_empty() || event.is_empty() || command.is_empty() {
            return None;
        }

        Some(HookSettingsInput {
            scope: self.scope,
            id: id.to_string(),
            event: event.to_string(),
            matcher: trim_option(&self.matcher),
            command: command.to_string(),
            status_message: trim_option(&self.status_message),
            timeout_secs: self
                .timeout_secs
                .trim()
                .parse::<u32>()
                .ok()
                .filter(|value| *value > 0),
            enabled: self.enabled,
        })
    }

    fn push_char(&mut self, field: HookEditorField, ch: char) {
        match field {
            HookEditorField::Scope => match ch {
                'u' | 'U' => self.scope = ConfigScope::User,
                'p' | 'P' => self.scope = ConfigScope::Project,
                _ => {}
            },
            HookEditorField::Id => self.id.push(ch),
            HookEditorField::Event => self.event.push(ch),
            HookEditorField::Matcher => self.matcher.push(ch),
            HookEditorField::Command => self.command.push(ch),
            HookEditorField::StatusMessage => self.status_message.push(ch),
            HookEditorField::TimeoutSecs if ch.is_ascii_digit() => self.timeout_secs.push(ch),
            HookEditorField::TimeoutSecs => {}
            HookEditorField::Enabled => match ch {
                ' ' | 't' | 'T' => self.enabled = !self.enabled,
                'y' | 'Y' | '1' => self.enabled = true,
                'n' | 'N' | '0' => self.enabled = false,
                _ => {}
            },
        }
    }

    fn backspace(&mut self, field: HookEditorField) {
        match field {
            HookEditorField::Id => {
                self.id.pop();
            }
            HookEditorField::Event => {
                self.event.pop();
            }
            HookEditorField::Matcher => {
                self.matcher.pop();
            }
            HookEditorField::Command => {
                self.command.pop();
            }
            HookEditorField::StatusMessage => {
                self.status_message.pop();
            }
            HookEditorField::TimeoutSecs => {
                self.timeout_secs.pop();
            }
            HookEditorField::Scope | HookEditorField::Enabled => {}
        }
    }

    fn clear_field(&mut self, field: HookEditorField) {
        match field {
            HookEditorField::Id => self.id.clear(),
            HookEditorField::Event => self.event.clear(),
            HookEditorField::Matcher => self.matcher.clear(),
            HookEditorField::Command => self.command.clear(),
            HookEditorField::StatusMessage => self.status_message.clear(),
            HookEditorField::TimeoutSecs => self.timeout_secs.clear(),
            HookEditorField::Scope | HookEditorField::Enabled => {}
        }
    }

    fn cycle_event(&mut self, direction: i32) {
        let current = HOOK_EVENTS
            .iter()
            .position(|event| *event == self.event)
            .unwrap_or(0);
        let next = if direction < 0 {
            current.checked_sub(1).unwrap_or(HOOK_EVENTS.len() - 1)
        } else {
            (current + 1) % HOOK_EVENTS.len()
        };
        self.event = HOOK_EVENTS[next].to_string();
    }
}

pub struct HooksOverlay {
    focused: bool,
    visible: bool,
    tab: HooksTab,
    mode: HooksMode,
    user: Vec<HookSettingsView>,
    project: Vec<HookSettingsView>,
    templates: Vec<HookTemplateView>,
    user_config_path: String,
    project_config_path: Option<String>,
    user_state: ListState,
    project_state: ListState,
    template_state: ListState,
    draft: HookDraft,
    editor_field_index: usize,
}

impl Default for HooksOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl HooksOverlay {
    pub fn new() -> Self {
        Self {
            focused: false,
            visible: false,
            tab: HooksTab::User,
            mode: HooksMode::List,
            user: Vec::new(),
            project: Vec::new(),
            templates: Vec::new(),
            user_config_path: String::new(),
            project_config_path: None,
            user_state: ListState::default(),
            project_state: ListState::default(),
            template_state: ListState::default(),
            draft: HookDraft::new(ConfigScope::User),
            editor_field_index: 0,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self, view: HooksSettingsView) {
        self.user = view.user;
        self.project = view.project;
        self.templates = view.templates;
        self.user_config_path = view.user_config_path;
        self.project_config_path = view.project_config_path;
        self.mode = HooksMode::List;
        self.visible = true;
        self.ensure_selection();
    }

    pub fn set_active_scope(&mut self, scope: ConfigScope) {
        self.tab = match scope {
            ConfigScope::Project => HooksTab::Project,
            _ => HooksTab::User,
        };
        self.ensure_selection();
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.mode = HooksMode::List;
        self.tab = HooksTab::User;
        self.user.clear();
        self.project.clear();
        self.templates.clear();
        self.user_state.select(None);
        self.project_state.select(None);
        self.template_state.select(None);
        self.draft = HookDraft::new(ConfigScope::User);
        self.editor_field_index = 0;
    }

    #[allow(dead_code)]
    pub fn user_hooks(&self) -> &[HookSettingsView] {
        &self.user
    }

    #[allow(dead_code)]
    pub fn project_hooks(&self) -> &[HookSettingsView] {
        &self.project
    }

    fn current_len(&self) -> usize {
        match self.tab {
            HooksTab::User => self.user.len(),
            HooksTab::Project => self.project.len(),
            HooksTab::Templates => self.templates.len(),
        }
    }

    fn current_selected(&self) -> Option<usize> {
        match self.tab {
            HooksTab::User => self.user_state.selected(),
            HooksTab::Project => self.project_state.selected(),
            HooksTab::Templates => self.template_state.selected(),
        }
    }

    fn current_state_mut(&mut self) -> &mut ListState {
        match self.tab {
            HooksTab::User => &mut self.user_state,
            HooksTab::Project => &mut self.project_state,
            HooksTab::Templates => &mut self.template_state,
        }
    }

    fn selected_hook(&self) -> Option<&HookSettingsView> {
        let index = self.current_selected()?;
        match self.tab {
            HooksTab::User => self.user.get(index),
            HooksTab::Project => self.project.get(index),
            HooksTab::Templates => None,
        }
    }

    fn selected_template(&self) -> Option<&HookTemplateView> {
        self.template_state
            .selected()
            .and_then(|index| self.templates.get(index))
    }

    fn ensure_selection(&mut self) {
        for (len, state) in [
            (self.user.len(), &mut self.user_state),
            (self.project.len(), &mut self.project_state),
            (self.templates.len(), &mut self.template_state),
        ] {
            if len == 0 {
                state.select(None);
            } else {
                let selected = state.selected().unwrap_or(0).min(len.saturating_sub(1));
                state.select(Some(selected));
            }
        }
    }

    fn move_down(&mut self) {
        match self.mode {
            HooksMode::List => {
                let len = self.current_len();
                if len == 0 {
                    return;
                }
                let next = match self.current_selected() {
                    Some(index) if index + 1 < len => index + 1,
                    Some(_) => len - 1,
                    None => 0,
                };
                self.current_state_mut().select(Some(next));
            }
            HooksMode::Editor => {
                self.editor_field_index = (self.editor_field_index + 1) % EDITOR_FIELDS.len();
            }
        }
    }

    fn move_up(&mut self) {
        match self.mode {
            HooksMode::List => {
                if self.current_len() == 0 {
                    return;
                }
                let next = match self.current_selected() {
                    Some(index) if index > 0 => index - 1,
                    _ => 0,
                };
                self.current_state_mut().select(Some(next));
            }
            HooksMode::Editor => {
                self.editor_field_index = if self.editor_field_index == 0 {
                    EDITOR_FIELDS.len() - 1
                } else {
                    self.editor_field_index - 1
                };
            }
        }
    }

    fn current_editor_field(&self) -> HookEditorField {
        EDITOR_FIELDS[self.editor_field_index]
    }

    fn switch_tab_next(&mut self) {
        self.tab = self.tab.next();
        self.ensure_selection();
    }

    fn switch_tab_previous(&mut self) {
        self.tab = self.tab.previous();
        self.ensure_selection();
    }

    fn start_create(&mut self, scope: ConfigScope) {
        self.mode = HooksMode::Editor;
        self.draft = HookDraft::new(scope);
        self.editor_field_index = 1;
    }

    fn start_edit_selected(&mut self) {
        if self.tab == HooksTab::Templates {
            self.start_template(ConfigScope::User);
            return;
        }
        let Some(scope) = self.tab.scope() else {
            return;
        };
        let Some(hook) = self.selected_hook().cloned() else {
            return;
        };
        self.mode = HooksMode::Editor;
        self.draft = HookDraft::from_hook(&hook, scope);
        self.editor_field_index = 1;
    }

    fn start_template(&mut self, scope: ConfigScope) {
        let Some(template) = self.selected_template().cloned() else {
            return;
        };
        self.mode = HooksMode::Editor;
        self.draft = HookDraft::from_template(&template, scope);
        self.editor_field_index = 1;
    }

    fn selected_delete_command(&self) -> Option<Command> {
        let scope = self.tab.scope()?;
        let hook = self.selected_hook()?;
        Some(Command::DeleteHookSettings {
            scope,
            event: hook.event.clone(),
            id: hook.id.clone(),
        })
    }

    fn selected_toggle_command(&self) -> Option<Command> {
        let scope = self.tab.scope()?;
        let hook = self.selected_hook()?;
        Some(Command::SaveHookSettings {
            input: HookSettingsInput {
                scope,
                id: hook.id.clone(),
                event: hook.event.clone(),
                matcher: hook.matcher.clone(),
                command: hook.command.clone(),
                status_message: hook.status_message.clone(),
                timeout_secs: hook.timeout_secs,
                enabled: !hook.enabled,
            },
        })
    }

    fn handle_list_key(&mut self, key: KeyCode) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let mut effects = Vec::new();
        let mut commands = Vec::new();

        match key {
            KeyCode::Char('j') | KeyCode::Down => self.move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.move_up(),
            KeyCode::Tab | KeyCode::Right => self.switch_tab_next(),
            KeyCode::BackTab | KeyCode::Left => self.switch_tab_previous(),
            KeyCode::Char('n') => {
                let scope = self.tab.scope().unwrap_or(ConfigScope::User);
                self.start_create(scope);
            }
            KeyCode::Char('N') => {
                let scope = match self.tab.scope().unwrap_or(ConfigScope::User) {
                    ConfigScope::User => ConfigScope::Project,
                    _ => ConfigScope::User,
                };
                self.start_create(scope);
            }
            KeyCode::Char('e') | KeyCode::Char('E') | KeyCode::Enter => {
                self.start_edit_selected();
            }
            KeyCode::Char('u') | KeyCode::Char('U') if self.tab == HooksTab::Templates => {
                self.start_template(ConfigScope::User);
            }
            KeyCode::Char('p') | KeyCode::Char('P') if self.tab == HooksTab::Templates => {
                self.start_template(ConfigScope::Project);
            }
            KeyCode::Char(' ') => {
                if let Some(command) = self.selected_toggle_command() {
                    commands.push(command);
                }
            }
            KeyCode::Char('x') | KeyCode::Char('X') | KeyCode::Delete => {
                if let Some(command) = self.selected_delete_command() {
                    commands.push(command);
                }
            }
            KeyCode::Char('r') | KeyCode::Char('R') => commands.push(Command::OpenHooksOverlay),
            KeyCode::Esc => {
                self.hide();
                effects.push(CrossPanelEffect::DismissHooksOverlay);
            }
            _ => {}
        }

        (effects, commands)
    }

    fn handle_editor_key(
        &mut self,
        key: KeyCode,
        modifiers: KeyModifiers,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let mut commands = Vec::new();
        let field = self.current_editor_field();

        match key {
            KeyCode::Down | KeyCode::Tab => self.move_down(),
            KeyCode::Up | KeyCode::BackTab => self.move_up(),
            KeyCode::Esc => self.mode = HooksMode::List,
            KeyCode::Backspace => self.draft.backspace(field),
            KeyCode::Delete => self.draft.clear_field(field),
            KeyCode::Left if field == HookEditorField::Event => self.draft.cycle_event(-1),
            KeyCode::Right if field == HookEditorField::Event => self.draft.cycle_event(1),
            KeyCode::Enter => {
                if let Some(input) = self.draft.to_input() {
                    commands.push(Command::SaveHookSettings { input });
                    self.mode = HooksMode::List;
                }
            }
            KeyCode::Char(ch) if !modifiers.contains(KeyModifiers::CONTROL) => {
                self.draft.push_char(field, ch);
            }
            _ => {}
        }

        (Vec::new(), commands)
    }

    fn handle_key_event(&mut self, event: &Event) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let Event::Key(key) = event else {
            return (Vec::new(), Vec::new());
        };
        if !self.visible {
            return (Vec::new(), Vec::new());
        }

        match self.mode {
            HooksMode::List => self.handle_list_key(key.code),
            HooksMode::Editor => self.handle_editor_key(key.code, key.modifiers),
        }
    }

    #[cfg(test)]
    fn handle_event_for_test(&mut self, event: &Event) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        self.handle_key_event(event)
    }

    #[cfg(test)]
    fn draft_for_test(&self) -> &HookDraft {
        &self.draft
    }

    #[cfg(test)]
    fn replace_draft_for_test(&mut self, input: HookSettingsInput) {
        self.draft = HookDraft::from_input(input);
        self.mode = HooksMode::Editor;
        self.visible = true;
    }
}

fn trim_option(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn scope_label(scope: ConfigScope) -> &'static str {
    match scope {
        ConfigScope::User => "user",
        ConfigScope::Project => "project",
        ConfigScope::Builtin => "builtin",
        ConfigScope::Local => "local",
    }
}

fn editor_field_label(field: HookEditorField) -> &'static str {
    match field {
        HookEditorField::Scope => "Scope",
        HookEditorField::Id => "Id",
        HookEditorField::Event => "Event",
        HookEditorField::Matcher => "Matcher",
        HookEditorField::Command => "Command",
        HookEditorField::StatusMessage => "Status",
        HookEditorField::TimeoutSecs => "Timeout",
        HookEditorField::Enabled => "Enabled",
    }
}

fn editor_field_value(draft: &HookDraft, field: HookEditorField) -> String {
    match field {
        HookEditorField::Scope => scope_label(draft.scope).to_string(),
        HookEditorField::Id => draft.id.clone(),
        HookEditorField::Event => draft.event.clone(),
        HookEditorField::Matcher => draft.matcher.clone(),
        HookEditorField::Command => draft.command.clone(),
        HookEditorField::StatusMessage => draft.status_message.clone(),
        HookEditorField::TimeoutSecs => draft.timeout_secs.clone(),
        HookEditorField::Enabled => draft.enabled.to_string(),
    }
}

pub fn render_hooks_overlay(area: Rect, frame: &mut Frame, overlay: &HooksOverlay) {
    let modal_width = 112.min(area.width.saturating_sub(4));
    let modal_height = 26.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);

    let title = match overlay.mode {
        HooksMode::List => " Hooks Settings ",
        HooksMode::Editor => " Hooks Settings Editor ",
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            title,
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Cyan));
    let inner = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    match overlay.mode {
        HooksMode::List => render_hooks_list(inner, frame, overlay),
        HooksMode::Editor => render_hooks_editor(inner, frame, overlay),
    }
}

fn render_hooks_list(area: Rect, frame: &mut Frame, overlay: &HooksOverlay) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(2),
        ])
        .split(area);

    frame.render_widget(Paragraph::new(render_tabs(overlay)), chunks[0]);

    match overlay.tab {
        HooksTab::User => render_hook_rows(chunks[1], frame, &overlay.user, overlay.user_state),
        HooksTab::Project => {
            render_hook_rows(chunks[1], frame, &overlay.project, overlay.project_state)
        }
        HooksTab::Templates => {
            render_template_rows(chunks[1], frame, &overlay.templates, overlay.template_state)
        }
    }

    let hints = if overlay.tab == HooksTab::Templates {
        Line::from(vec![
            Span::styled("[j/k] nav  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Enter/u] use user  ", Style::default().fg(Color::Cyan)),
            Span::styled("[p] use project  ", Style::default().fg(Color::Magenta)),
            Span::styled("[Tab] tab  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Esc] close", Style::default().fg(Color::DarkGray)),
        ])
    } else {
        Line::from(vec![
            Span::styled("[j/k] nav  ", Style::default().fg(Color::DarkGray)),
            Span::styled(
                "[n/N] new current/other  ",
                Style::default().fg(Color::Cyan),
            ),
            Span::styled("[Enter/e] edit  ", Style::default().fg(Color::Yellow)),
            Span::styled("[Space] enable  ", Style::default().fg(Color::Green)),
            Span::styled("[x/Delete] delete  ", Style::default().fg(Color::Red)),
            Span::styled("[r] refresh  ", Style::default().fg(Color::DarkGray)),
            Span::styled("[Esc] close", Style::default().fg(Color::DarkGray)),
        ])
    };
    let path_line = match overlay.tab {
        HooksTab::User => format!("config: {}", overlay.user_config_path),
        HooksTab::Project => overlay
            .project_config_path
            .as_deref()
            .map(|path| format!("config: {path}"))
            .unwrap_or_else(|| "config: project .kairox/config.toml".to_string()),
        HooksTab::Templates => "templates: builtin hook starters".to_string(),
    };
    frame.render_widget(
        Paragraph::new(vec![
            hints,
            Line::from(Span::styled(
                path_line,
                Style::default().fg(Color::DarkGray),
            )),
        ]),
        chunks[2],
    );
}

fn render_tabs(overlay: &HooksOverlay) -> Line<'static> {
    Line::from(vec![
        tab_span(
            HooksTab::User,
            overlay.tab == HooksTab::User,
            overlay.user.len(),
        ),
        Span::raw("  "),
        tab_span(
            HooksTab::Project,
            overlay.tab == HooksTab::Project,
            overlay.project.len(),
        ),
        Span::raw("  "),
        tab_span(
            HooksTab::Templates,
            overlay.tab == HooksTab::Templates,
            overlay.templates.len(),
        ),
    ])
}

fn tab_span(tab: HooksTab, active: bool, count: usize) -> Span<'static> {
    let label = format!("{} ({count})", tab.label());
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

fn render_hook_rows(
    area: Rect,
    frame: &mut Frame,
    hooks: &[HookSettingsView],
    mut state: ListState,
) {
    if hooks.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "No hooks configured",
                Style::default().fg(Color::DarkGray),
            ))),
            area,
        );
        return;
    }

    let items = hooks
        .iter()
        .map(|hook| {
            let state_label = if hook.enabled { "enabled" } else { "disabled" };
            let matcher = hook
                .matcher
                .as_deref()
                .filter(|matcher| !matcher.is_empty())
                .unwrap_or("-");
            let timeout = hook
                .timeout_secs
                .map(|value| format!("{value}s"))
                .unwrap_or_else(|| "-".to_string());
            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(
                        format!("{:<8}", state_label),
                        Style::default().fg(if hook.enabled {
                            Color::Green
                        } else {
                            Color::DarkGray
                        }),
                    ),
                    Span::styled(
                        format!(" {:<17}", hook.event),
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::styled(
                        format!(" {}", hook.id),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                ]),
                Line::from(vec![
                    Span::styled(
                        format!("  matcher: {matcher:<16} timeout: {timeout:<6} "),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::raw(&hook.command),
                ]),
            ])
        })
        .collect::<Vec<_>>();
    let list = List::new(items).highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_stateful_widget(list, area, &mut state);
}

fn render_template_rows(
    area: Rect,
    frame: &mut Frame,
    templates: &[HookTemplateView],
    mut state: ListState,
) {
    if templates.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "No hook templates available",
                Style::default().fg(Color::DarkGray),
            ))),
            area,
        );
        return;
    }

    let items = templates
        .iter()
        .map(|template| {
            let matcher = template
                .matcher
                .as_deref()
                .filter(|matcher| !matcher.is_empty())
                .unwrap_or("-");
            ListItem::new(vec![
                Line::from(vec![
                    Span::styled(
                        format!("{:<17}", template.event),
                        Style::default().fg(Color::Yellow),
                    ),
                    Span::styled(
                        format!(" {}", template.name),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(" ({})", template.id),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]),
                Line::from(vec![
                    Span::styled(
                        format!("  matcher: {matcher:<16} "),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::raw(&template.command),
                ]),
            ])
        })
        .collect::<Vec<_>>();
    let list = List::new(items).highlight_style(
        Style::default()
            .bg(Color::DarkGray)
            .add_modifier(Modifier::BOLD),
    );
    frame.render_stateful_widget(list, area, &mut state);
}

fn render_hooks_editor(area: Rect, frame: &mut Frame, overlay: &HooksOverlay) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(1), Constraint::Length(1)])
        .split(area);

    let rows = EDITOR_FIELDS
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let selected = index == overlay.editor_field_index;
            let marker = if selected { "> " } else { "  " };
            let mut value = editor_field_value(&overlay.draft, *field);
            if *field == HookEditorField::Event {
                value.push_str("  (Left/Right cycles)");
            }
            let style = if selected {
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Cyan)),
                Span::styled(format!("{:<10}", editor_field_label(*field)), style),
                Span::raw(" "),
                Span::styled(value, Style::default().fg(Color::Gray)),
            ])
        })
        .collect::<Vec<_>>();

    let body = Paragraph::new(rows)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_style(Style::default().fg(Color::Yellow)),
        )
        .wrap(Wrap { trim: false });
    frame.render_widget(body, chunks[0]);

    let hints = Line::from(vec![
        Span::styled("[Tab] field  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[Enter] save  ", Style::default().fg(Color::Yellow)),
        Span::styled("[Del] clear  ", Style::default().fg(Color::Red)),
        Span::styled("[Esc] list", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(hints), chunks[1]);
}

impl Component for HooksOverlay {
    fn handle_event(
        &mut self,
        _ctx: &EventContext,
        event: &Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        self.handle_key_event(event)
    }

    fn handle_effect(&mut self, effect: &CrossPanelEffect) {
        match effect {
            CrossPanelEffect::ShowHooksOverlay(view) => self.show(view.clone()),
            CrossPanelEffect::DismissHooksOverlay => self.hide(),
            _ => {}
        }
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        if self.visible {
            render_hooks_overlay(area, frame, self);
        }
    }

    fn focused(&self) -> bool {
        self.focused
    }

    fn set_focused(&mut self, focused: bool) {
        self.focused = focused;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::facade::{
        HookSettingsInput, HookSettingsView, HookTemplateView, HooksSettingsView,
    };
    use agent_core::ConfigScope;
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    fn hook(id: &str, scope: ConfigScope, enabled: bool) -> HookSettingsView {
        HookSettingsView {
            id: id.into(),
            event: "Stop".into(),
            matcher: Some("*".into()),
            command: "cargo test".into(),
            status_message: Some("Testing".into()),
            timeout_secs: Some(120),
            enabled,
            source: scope,
            config_path: Some(format!("/tmp/{id}.toml")),
        }
    }

    fn template() -> HookTemplateView {
        HookTemplateView {
            id: "stop-validation".into(),
            name: "Stop validation".into(),
            description: "Run validation".into(),
            event: "Stop".into(),
            matcher: Some("*".into()),
            command: "cargo test --workspace --all-targets".into(),
            status_message: Some("Running validation".into()),
            timeout_secs: Some(600),
        }
    }

    fn snapshot() -> HooksSettingsView {
        HooksSettingsView {
            user: vec![hook("user-verify", ConfigScope::User, true)],
            project: vec![hook("project-policy", ConfigScope::Project, false)],
            templates: vec![template()],
            user_config_path: "/home/me/.kairox/config.toml".into(),
            project_config_path: Some("/repo/.kairox/config.toml".into()),
        }
    }

    #[test]
    fn reads_user_and_project_hooks_from_snapshot() {
        let mut overlay = HooksOverlay::new();
        overlay.show(snapshot());

        assert!(overlay.is_visible());
        assert_eq!(overlay.user_hooks()[0].id, "user-verify");
        overlay.handle_event_for_test(&key(KeyCode::Tab));
        assert_eq!(overlay.project_hooks()[0].id, "project-policy");
    }

    #[test]
    fn template_fills_editor_form() {
        let mut overlay = HooksOverlay::new();
        overlay.show(snapshot());

        overlay.handle_event_for_test(&key(KeyCode::Tab));
        overlay.handle_event_for_test(&key(KeyCode::Tab));
        overlay.handle_event_for_test(&key(KeyCode::Enter));

        let draft = overlay.draft_for_test();
        assert_eq!(draft.id, "stop-validation");
        assert_eq!(draft.event, "Stop");
        assert_eq!(draft.command, "cargo test --workspace --all-targets");
        assert_eq!(draft.scope, ConfigScope::User);
    }

    #[test]
    fn save_and_delete_emit_hook_commands() {
        let mut overlay = HooksOverlay::new();
        overlay.show(snapshot());
        overlay.replace_draft_for_test(HookSettingsInput {
            scope: ConfigScope::Project,
            id: "project-policy".into(),
            event: "PreToolUse".into(),
            matcher: Some("shell".into()),
            command: "python3 .kairox/hooks/pre_tool_policy.py".into(),
            status_message: Some("Checking policy".into()),
            timeout_secs: Some(30),
            enabled: true,
        });

        let (_, commands) = overlay.handle_event_for_test(&key(KeyCode::Enter));
        assert!(matches!(
            &commands[..],
            [Command::SaveHookSettings { input }]
                if input.scope == ConfigScope::Project
                    && input.id == "project-policy"
                    && input.enabled
        ));

        overlay.show(snapshot());
        overlay.handle_event_for_test(&key(KeyCode::Char('x')));
        let (_, commands) = overlay.handle_event_for_test(&key(KeyCode::Delete));
        assert!(matches!(
            &commands[..],
            [Command::DeleteHookSettings { scope, event, id }]
                if *scope == ConfigScope::User && event == "Stop" && id == "user-verify"
        ));
    }

    #[test]
    fn renders_enabled_and_disabled_state() {
        let mut overlay = HooksOverlay::new();
        overlay.show(snapshot());
        let backend = ratatui::backend::TestBackend::new(120, 30);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| overlay.render(f.area(), f))
            .expect("render");

        overlay.handle_event_for_test(&key(KeyCode::Tab));
        terminal
            .draw(|f| overlay.render(f.area(), f))
            .expect("render");
        let buf = terminal.backend().buffer().clone();
        let mut rendered = String::new();
        for y in 0..buf.area.height {
            for x in 0..buf.area.width {
                rendered.push_str(buf[(x, y)].symbol());
            }
            rendered.push('\n');
        }

        assert!(rendered.contains("project-policy"), "{rendered}");
        assert!(rendered.contains("disabled"), "{rendered}");
    }
}
