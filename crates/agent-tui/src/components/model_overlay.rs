//! Model profile manager overlay — pop-up modal listing profile settings with
//! the current profile/effort highlighted. It keeps the fast model switch path
//! while exposing the same first-pass settings actions as the GUI model pane.

use std::collections::BTreeMap;

use agent_core::facade::ProfileSettingsInput;
use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::components::{
    Command, Component, CrossPanelEffect, EventContext, ModelOverlaySnapshot, ModelProfileEntry,
    ModelProfileTestResult,
};

/// Effort presets exposed for reasoning-capable profiles. Mirrors the GUI's
/// `DEFAULT_REASONING_EFFORTS` constant in `apps/agent-gui/src/stores/session.ts`.
pub const REASONING_EFFORTS: [&str; 4] = ["low", "middle", "high", "xhigh"];

/// Which sub-panel currently consumes navigation keys inside the overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OverlayFocus {
    ProfileList,
    EffortList,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OverlayMode {
    List,
    Editor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ProfileEditorField {
    Alias,
    Provider,
    ModelId,
    BaseUrl,
    ApiKeyEnv,
    ContextWindow,
    OutputLimit,
    Temperature,
    TopP,
    TopK,
    MaxTokens,
    Enabled,
}

const PROFILE_EDITOR_FIELDS: [ProfileEditorField; 12] = [
    ProfileEditorField::Alias,
    ProfileEditorField::Provider,
    ProfileEditorField::ModelId,
    ProfileEditorField::BaseUrl,
    ProfileEditorField::ApiKeyEnv,
    ProfileEditorField::ContextWindow,
    ProfileEditorField::OutputLimit,
    ProfileEditorField::Temperature,
    ProfileEditorField::TopP,
    ProfileEditorField::TopK,
    ProfileEditorField::MaxTokens,
    ProfileEditorField::Enabled,
];

#[derive(Debug, Clone, PartialEq)]
struct ProfileDraft {
    alias: String,
    provider: String,
    model_id: String,
    base_url: String,
    api_key_env: String,
    context_window: String,
    output_limit: String,
    temperature: String,
    top_p: String,
    top_k: String,
    max_tokens: String,
    enabled: bool,
    alias_editable: bool,
}

impl ProfileDraft {
    fn new() -> Self {
        Self {
            alias: String::new(),
            provider: String::new(),
            model_id: String::new(),
            base_url: String::new(),
            api_key_env: String::new(),
            context_window: String::new(),
            output_limit: String::new(),
            temperature: String::new(),
            top_p: String::new(),
            top_k: String::new(),
            max_tokens: String::new(),
            enabled: true,
            alias_editable: true,
        }
    }

    fn from_entry(entry: &ModelProfileEntry) -> Self {
        Self {
            alias: entry.alias.clone(),
            provider: entry.provider_display.clone(),
            model_id: entry.model_display.clone(),
            base_url: entry.base_url.clone().unwrap_or_default(),
            api_key_env: entry.api_key_env.clone().unwrap_or_default(),
            context_window: format_optional(entry.context_window),
            output_limit: format_optional(entry.output_limit),
            temperature: format_optional(entry.temperature),
            top_p: format_optional(entry.top_p),
            top_k: format_optional(entry.top_k),
            max_tokens: format_optional(entry.max_tokens),
            enabled: entry.enabled,
            alias_editable: false,
        }
    }

    #[cfg(test)]
    fn from_input(input: ProfileSettingsInput) -> Self {
        Self {
            alias: input.alias,
            provider: input.provider,
            model_id: input.model_id,
            base_url: input.base_url.unwrap_or_default(),
            api_key_env: input.api_key_env.unwrap_or_default(),
            context_window: format_optional(input.context_window),
            output_limit: format_optional(input.output_limit),
            temperature: format_optional(input.temperature),
            top_p: format_optional(input.top_p),
            top_k: format_optional(input.top_k),
            max_tokens: format_optional(input.max_tokens),
            enabled: input.enabled,
            alias_editable: true,
        }
    }

    fn to_input(&self) -> Option<ProfileSettingsInput> {
        let alias = self.alias.trim();
        let provider = self.provider.trim();
        let model_id = self.model_id.trim();
        if alias.is_empty() || provider.is_empty() || model_id.is_empty() {
            return None;
        }

        Some(ProfileSettingsInput {
            alias: alias.to_string(),
            provider: provider.to_string(),
            model_id: model_id.to_string(),
            enabled: self.enabled,
            context_window: parse_optional(&self.context_window),
            output_limit: parse_optional(&self.output_limit),
            temperature: parse_optional(&self.temperature),
            top_p: parse_optional(&self.top_p),
            top_k: parse_optional(&self.top_k),
            max_tokens: parse_optional(&self.max_tokens),
            base_url: trim_option(&self.base_url),
            api_key_env: trim_option(&self.api_key_env),
        })
    }

    fn push_char(&mut self, field: ProfileEditorField, ch: char) {
        match field {
            ProfileEditorField::Alias if self.alias_editable => self.alias.push(ch),
            ProfileEditorField::Provider => self.provider.push(ch),
            ProfileEditorField::ModelId => self.model_id.push(ch),
            ProfileEditorField::BaseUrl => self.base_url.push(ch),
            ProfileEditorField::ApiKeyEnv => self.api_key_env.push(ch),
            ProfileEditorField::ContextWindow => self.context_window.push(ch),
            ProfileEditorField::OutputLimit => self.output_limit.push(ch),
            ProfileEditorField::Temperature => self.temperature.push(ch),
            ProfileEditorField::TopP => self.top_p.push(ch),
            ProfileEditorField::TopK => self.top_k.push(ch),
            ProfileEditorField::MaxTokens => self.max_tokens.push(ch),
            ProfileEditorField::Enabled => match ch {
                'y' | 'Y' | '1' | 't' | 'T' => self.enabled = true,
                'n' | 'N' | '0' | 'f' | 'F' => self.enabled = false,
                ' ' => self.enabled = !self.enabled,
                _ => {}
            },
            ProfileEditorField::Alias => {}
        }
    }

    fn backspace(&mut self, field: ProfileEditorField) {
        match field {
            ProfileEditorField::Alias if self.alias_editable => {
                self.alias.pop();
            }
            ProfileEditorField::Provider => {
                self.provider.pop();
            }
            ProfileEditorField::ModelId => {
                self.model_id.pop();
            }
            ProfileEditorField::BaseUrl => {
                self.base_url.pop();
            }
            ProfileEditorField::ApiKeyEnv => {
                self.api_key_env.pop();
            }
            ProfileEditorField::ContextWindow => {
                self.context_window.pop();
            }
            ProfileEditorField::OutputLimit => {
                self.output_limit.pop();
            }
            ProfileEditorField::Temperature => {
                self.temperature.pop();
            }
            ProfileEditorField::TopP => {
                self.top_p.pop();
            }
            ProfileEditorField::TopK => {
                self.top_k.pop();
            }
            ProfileEditorField::MaxTokens => {
                self.max_tokens.pop();
            }
            ProfileEditorField::Alias | ProfileEditorField::Enabled => {}
        }
    }

    fn clear_field(&mut self, field: ProfileEditorField) {
        match field {
            ProfileEditorField::Alias if self.alias_editable => self.alias.clear(),
            ProfileEditorField::Provider => self.provider.clear(),
            ProfileEditorField::ModelId => self.model_id.clear(),
            ProfileEditorField::BaseUrl => self.base_url.clear(),
            ProfileEditorField::ApiKeyEnv => self.api_key_env.clear(),
            ProfileEditorField::ContextWindow => self.context_window.clear(),
            ProfileEditorField::OutputLimit => self.output_limit.clear(),
            ProfileEditorField::Temperature => self.temperature.clear(),
            ProfileEditorField::TopP => self.top_p.clear(),
            ProfileEditorField::TopK => self.top_k.clear(),
            ProfileEditorField::MaxTokens => self.max_tokens.clear(),
            ProfileEditorField::Alias | ProfileEditorField::Enabled => {}
        }
    }
}

pub struct ModelOverlay {
    focused: bool,
    visible: bool,
    profiles: Vec<ModelProfileEntry>,
    current_alias: Option<String>,
    current_effort: Option<String>,
    list_state: ListState,
    effort_state: ListState,
    overlay_focus: OverlayFocus,
    mode: OverlayMode,
    draft: ProfileDraft,
    editor_field_index: usize,
    test_results: BTreeMap<String, ModelProfileTestResult>,
}

impl Default for ModelOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl ModelOverlay {
    pub fn new() -> Self {
        Self {
            focused: false,
            visible: false,
            profiles: Vec::new(),
            current_alias: None,
            current_effort: None,
            list_state: ListState::default(),
            effort_state: ListState::default(),
            overlay_focus: OverlayFocus::ProfileList,
            mode: OverlayMode::List,
            draft: ProfileDraft::new(),
            editor_field_index: 0,
            test_results: BTreeMap::new(),
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self, snapshot: ModelOverlaySnapshot) {
        // Default selection: the current alias if it exists in the list, else 0.
        let select = if snapshot.profiles.is_empty() {
            None
        } else {
            snapshot
                .current_alias
                .as_ref()
                .and_then(|a| snapshot.profiles.iter().position(|p| &p.alias == a))
                .or(Some(0))
        };
        self.list_state.select(select);

        // Effort selection mirrors current_effort when present and the selected
        // profile supports reasoning; else default to "low" so the picker has
        // a visible cursor.
        self.current_alias = snapshot.current_alias;
        self.current_effort = snapshot.current_effort;
        self.profiles = snapshot.profiles;
        let initial_effort = self
            .current_effort
            .as_deref()
            .and_then(|e| REASONING_EFFORTS.iter().position(|x| *x == e))
            .unwrap_or(0);
        self.effort_state.select(Some(initial_effort));
        self.overlay_focus = OverlayFocus::ProfileList;
        self.mode = OverlayMode::List;
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.profiles.clear();
        self.list_state.select(None);
        self.effort_state.select(None);
        self.current_alias = None;
        self.current_effort = None;
        self.overlay_focus = OverlayFocus::ProfileList;
        self.mode = OverlayMode::List;
        self.draft = ProfileDraft::new();
        self.editor_field_index = 0;
        self.test_results.clear();
    }

    #[allow(dead_code)]
    pub fn profiles(&self) -> &[ModelProfileEntry] {
        &self.profiles
    }

    #[allow(dead_code)]
    pub fn selected_index(&self) -> Option<usize> {
        self.list_state.selected()
    }

    pub fn selected_profile(&self) -> Option<&ModelProfileEntry> {
        self.list_state
            .selected()
            .and_then(|i| self.profiles.get(i))
    }

    fn start_create(&mut self) {
        self.mode = OverlayMode::Editor;
        self.draft = ProfileDraft::new();
        self.editor_field_index = 0;
        self.visible = true;
    }

    fn start_edit_selected(&mut self) {
        let Some(entry) = self.selected_profile().cloned() else {
            return;
        };
        self.mode = OverlayMode::Editor;
        self.draft = ProfileDraft::from_entry(&entry);
        self.editor_field_index = 1;
    }

    fn current_editor_field(&self) -> ProfileEditorField {
        PROFILE_EDITOR_FIELDS[self.editor_field_index]
    }

    /// `true` when the selected profile is reasoning-capable, so the effort
    /// picker should be rendered.
    pub fn shows_effort_picker(&self) -> bool {
        self.selected_profile()
            .map(|p| p.enabled && p.supports_reasoning)
            .unwrap_or(false)
    }

    /// Currently highlighted effort string (only meaningful when the selected
    /// profile supports reasoning).
    pub fn selected_effort(&self) -> Option<&'static str> {
        if !self.shows_effort_picker() {
            return None;
        }
        self.effort_state
            .selected()
            .and_then(|i| REASONING_EFFORTS.get(i).copied())
    }

    /// Available effort options for the selected profile. Empty for
    /// non-reasoning models.
    #[allow(dead_code)]
    pub fn effort_options(&self) -> &'static [&'static str] {
        if self.shows_effort_picker() {
            &REASONING_EFFORTS
        } else {
            &[]
        }
    }

    fn move_down(&mut self) {
        if self.mode == OverlayMode::Editor {
            self.editor_field_index = (self.editor_field_index + 1) % PROFILE_EDITOR_FIELDS.len();
            return;
        }

        match self.overlay_focus {
            OverlayFocus::ProfileList => {
                if self.profiles.is_empty() {
                    return;
                }
                let next = match self.list_state.selected() {
                    Some(i) if i + 1 < self.profiles.len() => i + 1,
                    Some(_) => self.profiles.len() - 1,
                    None => 0,
                };
                self.list_state.select(Some(next));
            }
            OverlayFocus::EffortList => {
                let len = REASONING_EFFORTS.len();
                let next = match self.effort_state.selected() {
                    Some(i) if i + 1 < len => i + 1,
                    Some(_) => len - 1,
                    None => 0,
                };
                self.effort_state.select(Some(next));
            }
        }
    }

    fn move_up(&mut self) {
        if self.mode == OverlayMode::Editor {
            self.editor_field_index = if self.editor_field_index == 0 {
                PROFILE_EDITOR_FIELDS.len() - 1
            } else {
                self.editor_field_index - 1
            };
            return;
        }

        match self.overlay_focus {
            OverlayFocus::ProfileList => {
                if self.profiles.is_empty() {
                    return;
                }
                let next = match self.list_state.selected() {
                    Some(i) if i > 0 => i - 1,
                    _ => 0,
                };
                self.list_state.select(Some(next));
            }
            OverlayFocus::EffortList => {
                let next = match self.effort_state.selected() {
                    Some(i) if i > 0 => i - 1,
                    _ => 0,
                };
                self.effort_state.select(Some(next));
            }
        }
    }

    fn cycle_inner_focus(&mut self) {
        if !self.shows_effort_picker() {
            return;
        }
        self.overlay_focus = match self.overlay_focus {
            OverlayFocus::ProfileList => OverlayFocus::EffortList,
            OverlayFocus::EffortList => OverlayFocus::ProfileList,
        };
    }

    fn commit_command(&self, ctx: &EventContext) -> Option<Command> {
        let entry = self.selected_profile()?;
        if !entry.enabled {
            return None;
        }
        let session_id = ctx.current_session_id.clone()?;
        let reasoning_effort = if entry.supports_reasoning {
            self.selected_effort().map(|s| s.to_string())
        } else {
            None
        };
        Some(Command::SwitchModel {
            workspace_id: ctx.workspace_id.clone(),
            session_id,
            alias: entry.alias.clone(),
            reasoning_effort,
        })
    }

    fn settings_command(&self, key: KeyCode) -> Option<Command> {
        match key {
            KeyCode::Char('e') | KeyCode::Char('E') => {
                self.selected_profile()
                    .map(|entry| Command::SetProfileEnabled {
                        alias: entry.alias.clone(),
                        enabled: !entry.enabled,
                    })
            }
            KeyCode::Char('x') | KeyCode::Char('X') | KeyCode::Delete => self
                .selected_profile()
                .filter(|entry| entry.writable)
                .map(|entry| Command::DeleteProfileSettings {
                    alias: entry.alias.clone(),
                }),
            KeyCode::Char('J') => {
                self.selected_profile()
                    .map(|entry| Command::MoveProfileInOrder {
                        alias: entry.alias.clone(),
                        direction: 1,
                    })
            }
            KeyCode::Char('K') => {
                self.selected_profile()
                    .map(|entry| Command::MoveProfileInOrder {
                        alias: entry.alias.clone(),
                        direction: -1,
                    })
            }
            KeyCode::Char('t') | KeyCode::Char('T') => {
                self.selected_profile()
                    .map(|entry| Command::TestModelProfile {
                        alias: entry.alias.clone(),
                    })
            }
            KeyCode::Char('o') | KeyCode::Char('O') => Some(Command::OpenProfilesConfig),
            _ => None,
        }
    }

    fn draft_test_command(&self) -> Option<Command> {
        let base_url = self.draft.base_url.trim();
        if base_url.is_empty() {
            return None;
        }
        let alias = self.draft.alias.trim();
        Some(Command::TestModelProfileUrl {
            alias: if alias.is_empty() {
                base_url.to_string()
            } else {
                alias.to_string()
            },
            base_url: base_url.to_string(),
        })
    }

    fn handle_list_key(
        &mut self,
        ctx: &EventContext,
        key: KeyCode,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let mut effects = Vec::new();
        let mut commands = Vec::new();

        match key {
            KeyCode::Char('j') | KeyCode::Down => self.move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.move_up(),
            KeyCode::Tab | KeyCode::Char('l') | KeyCode::Char('h') => self.cycle_inner_focus(),
            KeyCode::Char('n') | KeyCode::Char('N') => self.start_create(),
            KeyCode::Char('u') | KeyCode::Char('U') => self.start_edit_selected(),
            KeyCode::Char('e')
            | KeyCode::Char('E')
            | KeyCode::Char('x')
            | KeyCode::Char('X')
            | KeyCode::Char('J')
            | KeyCode::Char('K')
            | KeyCode::Char('t')
            | KeyCode::Char('T')
            | KeyCode::Char('o')
            | KeyCode::Char('O')
            | KeyCode::Delete => {
                if let Some(cmd) = self.settings_command(key) {
                    commands.push(cmd);
                }
            }
            KeyCode::Esc => {
                self.hide();
                effects.push(CrossPanelEffect::DismissModelOverlay);
            }
            KeyCode::Enter => {
                if let Some(cmd) = self.commit_command(ctx) {
                    commands.push(cmd);
                    self.hide();
                    effects.push(CrossPanelEffect::DismissModelOverlay);
                }
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

        match key {
            KeyCode::Down | KeyCode::Tab => self.move_down(),
            KeyCode::Up | KeyCode::BackTab => self.move_up(),
            KeyCode::Esc => self.mode = OverlayMode::List,
            KeyCode::Backspace => self.draft.backspace(self.current_editor_field()),
            KeyCode::Delete => self.draft.clear_field(self.current_editor_field()),
            KeyCode::Char('t') | KeyCode::Char('T')
                if modifiers.contains(KeyModifiers::CONTROL) =>
            {
                if let Some(command) = self.draft_test_command() {
                    commands.push(command);
                }
            }
            KeyCode::Enter => {
                if let Some(input) = self.draft.to_input() {
                    commands.push(Command::SaveProfileSettings { input });
                    self.mode = OverlayMode::List;
                }
            }
            KeyCode::Char(ch) if !modifiers.contains(KeyModifiers::CONTROL) => {
                self.draft.push_char(self.current_editor_field(), ch);
            }
            _ => {}
        }

        (Vec::new(), commands)
    }

    fn set_test_result(&mut self, result: ModelProfileTestResult) {
        self.test_results.insert(result.alias.clone(), result);
    }

    #[cfg(test)]
    fn replace_draft_for_test(&mut self, input: ProfileSettingsInput) {
        self.draft = ProfileDraft::from_input(input);
        self.mode = OverlayMode::Editor;
        self.visible = true;
    }
}

fn format_optional<T: ToString>(value: Option<T>) -> String {
    value.map(|v| v.to_string()).unwrap_or_default()
}

fn parse_optional<T: std::str::FromStr>(value: &str) -> Option<T> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        trimmed.parse().ok()
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

pub fn render_model_overlay(
    area: Rect,
    frame: &mut Frame,
    overlay: &ModelOverlay,
    list_state: &mut ListState,
    effort_state: &mut ListState,
) {
    let modal_width = 96.min(area.width.saturating_sub(4));
    let modal_height = 22.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            match overlay.mode {
                OverlayMode::List => " 🤖 Model Profile ",
                OverlayMode::Editor => " 🤖 Model Profile Editor ",
            },
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    match overlay.mode {
        OverlayMode::List => {
            render_model_profile_list(inner, frame, overlay, list_state, effort_state);
        }
        OverlayMode::Editor => render_model_profile_editor(inner, frame, overlay),
    }
}

fn render_model_profile_list(
    inner: Rect,
    frame: &mut Frame,
    overlay: &ModelOverlay,
    list_state: &mut ListState,
    effort_state: &mut ListState,
) {
    let list_height = inner.height.saturating_sub(2);
    let list_area = Rect::new(inner.x, inner.y, inner.width, list_height);
    let hint_area = Rect::new(
        inner.x,
        inner.y + list_height,
        inner.width,
        inner.height.saturating_sub(list_height),
    );

    let show_effort = overlay.shows_effort_picker();
    let columns = if show_effort {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(65), Constraint::Percentage(35)])
            .split(list_area)
    } else {
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Percentage(100)])
            .split(list_area)
    };

    if overlay.profiles.is_empty() {
        let empty = Paragraph::new(Line::from(Span::styled(
            "No model profiles configured",
            Style::default().fg(Color::DarkGray),
        )));
        frame.render_widget(empty, columns[0]);
    } else {
        let items: Vec<ListItem> = overlay
            .profiles
            .iter()
            .map(|p| {
                let is_current = overlay.current_alias.as_deref() == Some(p.alias.as_str());
                let marker = if is_current { "● " } else { "  " };
                let enabled_label = if p.enabled { "enabled " } else { "disabled" };
                let enabled_color = if p.enabled {
                    Color::Green
                } else {
                    Color::DarkGray
                };
                let reasoning_tag = if p.supports_reasoning { " [R]" } else { "" };
                let writable_tag = if p.writable {
                    " writable"
                } else {
                    " read-only"
                };
                let key_tag = if p.has_api_key { " key" } else { " no-key" };
                let test_tag = overlay
                    .test_results
                    .get(&p.alias)
                    .map(|result| {
                        if result.ok {
                            " test:ok".to_string()
                        } else {
                            result
                                .message
                                .as_deref()
                                .map(|message| format!(" test:{message}"))
                                .unwrap_or_else(|| " test:failed".to_string())
                        }
                    })
                    .unwrap_or_default();
                let line = Line::from(vec![
                    Span::styled(marker, Style::default().fg(Color::Green)),
                    Span::styled(enabled_label, Style::default().fg(enabled_color)),
                    Span::raw("  "),
                    Span::styled(
                        p.alias.clone(),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!("  {}/{}", p.provider_display, p.model_display),
                        Style::default().fg(Color::DarkGray),
                    ),
                    Span::styled(reasoning_tag, Style::default().fg(Color::Magenta)),
                    Span::styled(
                        format!("  [{}{writable_tag}{key_tag}{test_tag}]", p.source),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]);
                ListItem::new(line)
            })
            .collect();

        let highlight = if overlay.overlay_focus == OverlayFocus::ProfileList {
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().bg(Color::Reset)
        };
        let list = List::new(items).highlight_style(highlight);
        frame.render_stateful_widget(list, columns[0], list_state);
    }

    if show_effort {
        let items: Vec<ListItem> = REASONING_EFFORTS
            .iter()
            .map(|effort| {
                let is_current = overlay.current_effort.as_deref() == Some(*effort);
                let marker = if is_current { "● " } else { "  " };
                let line = Line::from(vec![
                    Span::styled(marker, Style::default().fg(Color::Green)),
                    Span::raw(*effort),
                ]);
                ListItem::new(line)
            })
            .collect();
        let highlight = if overlay.overlay_focus == OverlayFocus::EffortList {
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().bg(Color::Reset)
        };
        let effort_block = Block::default()
            .borders(Borders::LEFT)
            .border_style(Style::default().fg(Color::DarkGray))
            .title(Span::styled(
                " effort ",
                Style::default().fg(Color::Magenta),
            ));
        let effort_inner = effort_block.inner(columns[1]);
        frame.render_widget(effort_block, columns[1]);
        let list = List::new(items).highlight_style(highlight);
        frame.render_stateful_widget(list, effort_inner, effort_state);
    }

    let hints = Line::from(vec![
        Span::styled("[j/k] nav  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[n] new  ", Style::default().fg(Color::Cyan)),
        Span::styled("[u] edit  ", Style::default().fg(Color::Yellow)),
        Span::styled("[J/K] order  ", Style::default().fg(Color::Cyan)),
        Span::styled("[e] enable  ", Style::default().fg(Color::Green)),
        Span::styled("[t] test  ", Style::default().fg(Color::Yellow)),
        Span::styled("[x] delete  ", Style::default().fg(Color::Red)),
        Span::styled("[o] config  ", Style::default().fg(Color::Blue)),
        Span::styled("[Tab] effort  ", Style::default().fg(Color::Magenta)),
        Span::styled("[Enter] switch  ", Style::default().fg(Color::Yellow)),
        Span::styled("[Esc] close", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(hints), hint_area);
}

fn render_model_profile_editor(area: Rect, frame: &mut Frame, overlay: &ModelOverlay) {
    let list_height = area.height.saturating_sub(2);
    let list_area = Rect::new(area.x, area.y, area.width, list_height);
    let hint_area = Rect::new(
        area.x,
        area.y + list_height,
        area.width,
        area.height.saturating_sub(list_height),
    );

    let items = PROFILE_EDITOR_FIELDS
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let marker = if index == overlay.editor_field_index {
                "> "
            } else {
                "  "
            };
            let label = profile_editor_field_label(*field);
            let value = profile_editor_field_value(&overlay.draft, *field);
            let lock_hint = if *field == ProfileEditorField::Alias && !overlay.draft.alias_editable
            {
                " (locked)"
            } else {
                ""
            };
            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Cyan)),
                Span::styled(
                    format!("{label:<14}"),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(value, Style::default().fg(Color::Gray)),
                Span::styled(lock_hint, Style::default().fg(Color::DarkGray)),
            ]))
        })
        .collect::<Vec<_>>();
    frame.render_widget(List::new(items), list_area);

    let hints = Line::from(vec![
        Span::styled("[Tab/j/k] field  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[space/y/n] enabled  ", Style::default().fg(Color::Green)),
        Span::styled("[Ctrl+T] test URL  ", Style::default().fg(Color::Yellow)),
        Span::styled("[Enter] save  ", Style::default().fg(Color::Yellow)),
        Span::styled("[Esc] cancel", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(hints), hint_area);
}

fn profile_editor_field_label(field: ProfileEditorField) -> &'static str {
    match field {
        ProfileEditorField::Alias => "Alias",
        ProfileEditorField::Provider => "Provider",
        ProfileEditorField::ModelId => "Model ID",
        ProfileEditorField::BaseUrl => "Base URL",
        ProfileEditorField::ApiKeyEnv => "API key env",
        ProfileEditorField::ContextWindow => "Context",
        ProfileEditorField::OutputLimit => "Output",
        ProfileEditorField::Temperature => "Temperature",
        ProfileEditorField::TopP => "Top P",
        ProfileEditorField::TopK => "Top K",
        ProfileEditorField::MaxTokens => "Max tokens",
        ProfileEditorField::Enabled => "Enabled",
    }
}

fn profile_editor_field_value(draft: &ProfileDraft, field: ProfileEditorField) -> String {
    match field {
        ProfileEditorField::Alias => draft.alias.clone(),
        ProfileEditorField::Provider => draft.provider.clone(),
        ProfileEditorField::ModelId => draft.model_id.clone(),
        ProfileEditorField::BaseUrl => draft.base_url.clone(),
        ProfileEditorField::ApiKeyEnv => draft.api_key_env.clone(),
        ProfileEditorField::ContextWindow => draft.context_window.clone(),
        ProfileEditorField::OutputLimit => draft.output_limit.clone(),
        ProfileEditorField::Temperature => draft.temperature.clone(),
        ProfileEditorField::TopP => draft.top_p.clone(),
        ProfileEditorField::TopK => draft.top_k.clone(),
        ProfileEditorField::MaxTokens => draft.max_tokens.clone(),
        ProfileEditorField::Enabled => draft.enabled.to_string(),
    }
}

impl Component for ModelOverlay {
    fn handle_event(
        &mut self,
        ctx: &EventContext,
        event: &Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let Event::Key(key) = event else {
            return (Vec::new(), Vec::new());
        };
        if !self.visible {
            return (Vec::new(), Vec::new());
        }

        match self.mode {
            OverlayMode::List => self.handle_list_key(ctx, key.code),
            OverlayMode::Editor => self.handle_editor_key(key.code, key.modifiers),
        }
    }

    fn handle_effect(&mut self, effect: &CrossPanelEffect) {
        match effect {
            CrossPanelEffect::ShowModelOverlay(snapshot) => self.show(snapshot.clone()),
            CrossPanelEffect::ModelProfileTested(result) => self.set_test_result(result.clone()),
            CrossPanelEffect::DismissModelOverlay => self.hide(),
            _ => {}
        }
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        if !self.visible {
            return;
        }
        let mut list_state = self.list_state;
        let mut effort_state = self.effort_state;
        render_model_overlay(area, frame, self, &mut list_state, &mut effort_state);
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
    use crate::components::{FocusTarget, SessionInfo};

    fn entry(alias: &str, supports_reasoning: bool) -> ModelProfileEntry {
        ModelProfileEntry {
            alias: alias.to_string(),
            provider_display: "provider".to_string(),
            model_display: format!("{alias}-model"),
            context_window: None,
            output_limit: None,
            temperature: None,
            top_p: None,
            top_k: None,
            max_tokens: None,
            base_url: None,
            api_key_env: None,
            supports_reasoning,
            enabled: true,
            writable: true,
            source: "profiles_toml".to_string(),
            has_api_key: true,
        }
    }

    fn disabled_entry(alias: &str) -> ModelProfileEntry {
        ModelProfileEntry {
            enabled: false,
            ..entry(alias, false)
        }
    }

    fn snapshot(
        profiles: Vec<ModelProfileEntry>,
        current_alias: Option<&str>,
        current_effort: Option<&str>,
    ) -> ModelOverlaySnapshot {
        ModelOverlaySnapshot {
            profiles,
            current_alias: current_alias.map(str::to_string),
            current_effort: current_effort.map(str::to_string),
        }
    }

    fn test_ctx_with_session(
        session_id: Option<agent_core::SessionId>,
    ) -> (
        agent_core::WorkspaceId,
        Option<agent_core::SessionId>,
        Vec<SessionInfo>,
        agent_core::projection::SessionProjection,
    ) {
        (
            agent_core::WorkspaceId::new(),
            session_id,
            Vec::new(),
            agent_core::projection::SessionProjection::default(),
        )
    }

    fn ctx<'a>(
        ws: &'a agent_core::WorkspaceId,
        sid: &'a Option<agent_core::SessionId>,
        sessions: &'a [SessionInfo],
        projection: &'a agent_core::projection::SessionProjection,
    ) -> EventContext<'a> {
        EventContext {
            focus: FocusTarget::ModelOverlay,
            current_session: projection,
            projects: &[],
            sessions,
            model_profile: "fake",
            permission_mode: agent_tools::PermissionMode::Suggest,
            sidebar_left_visible: true,
            sidebar_right_visible: true,
            workspace_id: ws,
            current_session_id: sid,
        }
    }

    fn key(code: KeyCode) -> Event {
        Event::Key(crossterm::event::KeyEvent::new(
            code,
            crossterm::event::KeyModifiers::NONE,
        ))
    }

    fn modified_key(code: KeyCode, modifiers: crossterm::event::KeyModifiers) -> Event {
        Event::Key(crossterm::event::KeyEvent::new(code, modifiers))
    }

    fn press(overlay: &mut ModelOverlay, code: KeyCode) -> Vec<Command> {
        let (ws, sid, sessions, proj) = test_ctx_with_session(None);
        let (_, commands) = overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(code));
        commands
    }

    fn type_text(overlay: &mut ModelOverlay, value: &str) {
        for ch in value.chars() {
            let _ = press(overlay, KeyCode::Char(ch));
        }
    }

    #[test]
    fn overlay_invisible_by_default() {
        let overlay = ModelOverlay::new();
        assert!(!overlay.is_visible());
        assert!(overlay.profiles().is_empty());
    }

    #[test]
    fn shows_reasoning_effort_for_reasoning_models() {
        // TDD start: when a reasoning-capable profile is highlighted, the
        // overlay surfaces the effort picker pre-selecting the current
        // effort. Mirrors the GUI's `ChatModelSelector` reasoning panel.
        let mut overlay = ModelOverlay::new();
        overlay.show(snapshot(
            vec![
                entry("fast", false),
                entry("opus-reasoning", true),
                entry("local", false),
            ],
            Some("opus-reasoning"),
            Some("high"),
        ));

        assert!(overlay.is_visible());
        assert_eq!(overlay.selected_index(), Some(1));
        assert!(
            overlay.shows_effort_picker(),
            "reasoning-capable selection must expose effort picker"
        );
        assert_eq!(overlay.selected_effort(), Some("high"));
        assert_eq!(overlay.effort_options(), REASONING_EFFORTS);
    }

    #[test]
    fn hides_effort_picker_for_non_reasoning_profile() {
        let mut overlay = ModelOverlay::new();
        overlay.show(snapshot(
            vec![entry("fast", false), entry("opus-reasoning", true)],
            Some("fast"),
            None,
        ));
        assert!(!overlay.shows_effort_picker());
        assert!(overlay.selected_effort().is_none());
        assert!(overlay.effort_options().is_empty());
    }

    #[test]
    fn enter_emits_switch_model_with_alias_and_no_effort_for_plain_profile() {
        let mut overlay = ModelOverlay::new();
        overlay.show(snapshot(
            vec![entry("fast", false), entry("opus-reasoning", true)],
            Some("opus-reasoning"),
            None,
        ));
        // Navigate up to the non-reasoning profile.
        let (ws, sid, sessions, proj) = test_ctx_with_session(Some(agent_core::SessionId::new()));
        let _ = overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Char('k')));
        assert_eq!(
            overlay.selected_profile().map(|e| e.alias.as_str()),
            Some("fast")
        );
        let (effects, commands) =
            overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Enter));
        assert_eq!(commands.len(), 1);
        assert!(matches!(
            &commands[0],
            Command::SwitchModel { alias, reasoning_effort, .. }
                if alias == "fast" && reasoning_effort.is_none()
        ));
        assert!(effects.contains(&CrossPanelEffect::DismissModelOverlay));
        assert!(!overlay.is_visible());
    }

    #[test]
    fn enter_does_not_switch_disabled_profile() {
        let mut overlay = ModelOverlay::new();
        overlay.show(snapshot(vec![disabled_entry("slow")], Some("fast"), None));
        let (ws, sid, sessions, proj) = test_ctx_with_session(Some(agent_core::SessionId::new()));

        let (_, commands) =
            overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Enter));

        assert!(
            commands.is_empty(),
            "disabled profiles are visible for management but cannot be switched to"
        );
        assert!(overlay.is_visible());
    }

    #[test]
    fn profile_management_keys_emit_settings_commands() {
        let mut overlay = ModelOverlay::new();
        overlay.show(snapshot(
            vec![entry("fast", false), disabled_entry("slow")],
            Some("fast"),
            None,
        ));
        let (ws, sid, sessions, proj) = test_ctx_with_session(Some(agent_core::SessionId::new()));
        let _ = overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Char('j')));

        let (_, commands) =
            overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Char('e')));
        assert!(matches!(
            &commands[..],
            [Command::SetProfileEnabled { alias, enabled }] if alias == "slow" && *enabled
        ));

        let (_, commands) =
            overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Char('x')));
        assert!(matches!(
            &commands[..],
            [Command::DeleteProfileSettings { alias }] if alias == "slow"
        ));

        let (_, commands) =
            overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Char('t')));
        assert!(matches!(
            &commands[..],
            [Command::TestModelProfile { alias }] if alias == "slow"
        ));

        let (_, commands) =
            overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Char('o')));
        assert!(matches!(&commands[..], [Command::OpenProfilesConfig]));
    }

    #[test]
    fn editor_ctrl_t_tests_draft_base_url() {
        let mut overlay = ModelOverlay::new();
        overlay.show(snapshot(vec![entry("fast", false)], Some("fast"), None));
        let (ws, sid, sessions, proj) = test_ctx_with_session(Some(agent_core::SessionId::new()));

        let _ = overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Char('n')));
        type_text(&mut overlay, "draft");
        let _ = overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Down));
        type_text(&mut overlay, "openai");
        let _ = overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Down));
        type_text(&mut overlay, "gpt-4.1");
        let _ = overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Down));
        type_text(&mut overlay, "https://api.example.test/v1");

        let (_, commands) = overlay.handle_event(
            &ctx(&ws, &sid, &sessions, &proj),
            &modified_key(KeyCode::Char('t'), crossterm::event::KeyModifiers::CONTROL),
        );

        assert!(matches!(
            &commands[..],
            [Command::TestModelProfileUrl { alias, base_url }]
                if alias == "draft" && base_url == "https://api.example.test/v1"
        ));
    }

    #[test]
    fn shift_j_and_k_emit_profile_reorder_commands() {
        let mut overlay = ModelOverlay::new();
        overlay.show(snapshot(
            vec![entry("fast", false), entry("slow", false)],
            Some("fast"),
            None,
        ));
        let (ws, sid, sessions, proj) = test_ctx_with_session(Some(agent_core::SessionId::new()));

        let (_, commands) =
            overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Char('J')));
        assert!(matches!(
            &commands[..],
            [Command::MoveProfileInOrder { alias, direction }] if alias == "fast" && *direction == 1
        ));

        let (_, commands) =
            overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Char('K')));
        assert!(matches!(
            &commands[..],
            [Command::MoveProfileInOrder { alias, direction }] if alias == "fast" && *direction == -1
        ));
    }

    #[test]
    fn enter_emits_switch_model_with_selected_effort_for_reasoning_profile() {
        let mut overlay = ModelOverlay::new();
        overlay.show(snapshot(
            vec![entry("opus-reasoning", true)],
            Some("opus-reasoning"),
            Some("low"),
        ));
        let (ws, sid, sessions, proj) = test_ctx_with_session(Some(agent_core::SessionId::new()));
        // Tab into effort picker, j to "middle", j to "high".
        let _ = overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Tab));
        let _ = overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Char('j')));
        let _ = overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Char('j')));
        assert_eq!(overlay.selected_effort(), Some("high"));
        let (_, commands) =
            overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Enter));
        assert!(matches!(
            &commands[0],
            Command::SwitchModel { alias, reasoning_effort, .. }
                if alias == "opus-reasoning" && reasoning_effort.as_deref() == Some("high")
        ));
    }

    #[test]
    fn enter_with_no_session_emits_no_command() {
        let mut overlay = ModelOverlay::new();
        overlay.show(snapshot(vec![entry("fast", false)], Some("fast"), None));
        let (ws, sid, sessions, proj) = test_ctx_with_session(None);
        let (_, commands) =
            overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Enter));
        assert!(commands.is_empty());
    }

    #[test]
    fn esc_hides_and_emits_dismiss_effect() {
        let mut overlay = ModelOverlay::new();
        overlay.show(snapshot(vec![entry("fast", false)], Some("fast"), None));
        let (ws, sid, sessions, proj) = test_ctx_with_session(None);
        let (effects, _) =
            overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Esc));
        assert!(effects.contains(&CrossPanelEffect::DismissModelOverlay));
        assert!(!overlay.is_visible());
    }

    #[test]
    fn show_effect_makes_visible() {
        let mut overlay = ModelOverlay::new();
        overlay.handle_effect(&CrossPanelEffect::ShowModelOverlay(snapshot(
            vec![entry("fast", false)],
            Some("fast"),
            None,
        )));
        assert!(overlay.is_visible());
        assert_eq!(overlay.profiles().len(), 1);
    }

    #[test]
    fn j_and_k_navigate_profile_list() {
        let mut overlay = ModelOverlay::new();
        overlay.show(snapshot(
            vec![entry("a", false), entry("b", false), entry("c", false)],
            Some("a"),
            None,
        ));
        let (ws, sid, sessions, proj) = test_ctx_with_session(Some(agent_core::SessionId::new()));
        let _ = overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Char('j')));
        assert_eq!(overlay.selected_index(), Some(1));
        let _ = overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Char('j')));
        assert_eq!(overlay.selected_index(), Some(2));
        let _ = overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Char('j')));
        assert_eq!(overlay.selected_index(), Some(2), "clamps at end");
        let _ = overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Char('k')));
        assert_eq!(overlay.selected_index(), Some(1));
    }

    #[test]
    fn renders_into_test_buffer() {
        let mut overlay = ModelOverlay::new();
        overlay.show(snapshot(
            vec![entry("fast", false), entry("opus-reasoning", true)],
            Some("opus-reasoning"),
            Some("middle"),
        ));
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal
            .draw(|f| overlay.render(f.area(), f))
            .expect("render");
    }

    #[test]
    fn ignores_keys_when_hidden() {
        let mut overlay = ModelOverlay::new();
        let (ws, sid, sessions, proj) = test_ctx_with_session(None);
        let (effects, commands) =
            overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Enter));
        assert!(effects.is_empty());
        assert!(commands.is_empty());
    }

    #[test]
    fn new_profile_editor_saves_full_profile_input() {
        let mut overlay = ModelOverlay::new();
        overlay.show(snapshot(Vec::new(), None, None));
        overlay.replace_draft_for_test(agent_core::facade::ProfileSettingsInput {
            alias: "local-qwen".to_string(),
            provider: "openai-compatible".to_string(),
            model_id: "qwen3-coder".to_string(),
            enabled: true,
            context_window: Some(128000),
            output_limit: Some(8192),
            temperature: Some(0.2),
            top_p: Some(0.9),
            top_k: Some(40),
            max_tokens: Some(4096),
            base_url: Some("http://localhost:11434/v1".to_string()),
            api_key_env: Some("LOCAL_LLM_API_KEY".to_string()),
        });

        let (effects, commands) = overlay.handle_event(
            &ctx(
                &agent_core::WorkspaceId::new(),
                &None,
                &[],
                &agent_core::projection::SessionProjection::default(),
            ),
            &key(KeyCode::Enter),
        );

        assert!(effects.is_empty());
        assert!(matches!(
            &commands[..],
            [Command::SaveProfileSettings { input }]
                if input.alias == "local-qwen"
                    && input.provider == "openai-compatible"
                    && input.model_id == "qwen3-coder"
                    && input.context_window == Some(128000)
                    && input.output_limit == Some(8192)
                    && input.temperature == Some(0.2)
                    && input.top_p == Some(0.9)
                    && input.top_k == Some(40)
                    && input.max_tokens == Some(4096)
                    && input.base_url.as_deref() == Some("http://localhost:11434/v1")
                    && input.api_key_env.as_deref() == Some("LOCAL_LLM_API_KEY")
        ));
        assert!(overlay.is_visible());
    }

    #[test]
    fn keyboard_driven_new_profile_editor_collects_required_fields() {
        let mut overlay = ModelOverlay::new();
        overlay.show(snapshot(Vec::new(), None, None));

        assert!(press(&mut overlay, KeyCode::Char('n')).is_empty());
        type_text(&mut overlay, "local");
        assert!(press(&mut overlay, KeyCode::Tab).is_empty());
        type_text(&mut overlay, "fake");
        assert!(press(&mut overlay, KeyCode::Tab).is_empty());
        type_text(&mut overlay, "fake-model");

        let commands = press(&mut overlay, KeyCode::Enter);

        assert!(matches!(
            &commands[..],
            [Command::SaveProfileSettings { input }]
                if input.alias == "local"
                    && input.provider == "fake"
                    && input.model_id == "fake-model"
                    && input.enabled
        ));
    }

    #[test]
    fn edit_profile_editor_preserves_alias_and_enabled_state() {
        let mut profile = entry("fast", false);
        profile.provider_display = "openai".to_string();
        profile.model_display = "gpt-5.4".to_string();
        profile.enabled = false;

        let mut overlay = ModelOverlay::new();
        overlay.show(snapshot(vec![profile], Some("fast"), None));
        let (ws, sid, sessions, proj) = test_ctx_with_session(None);
        let (_, commands) =
            overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Char('u')));
        assert!(commands.is_empty());

        overlay.replace_draft_for_test(agent_core::facade::ProfileSettingsInput {
            alias: "fast".to_string(),
            provider: "anthropic".to_string(),
            model_id: "claude-opus-4.1".to_string(),
            enabled: false,
            context_window: None,
            output_limit: None,
            temperature: None,
            top_p: None,
            top_k: None,
            max_tokens: None,
            base_url: None,
            api_key_env: Some("ANTHROPIC_API_KEY".to_string()),
        });

        let (_, commands) =
            overlay.handle_event(&ctx(&ws, &sid, &sessions, &proj), &key(KeyCode::Enter));

        assert!(matches!(
            &commands[..],
            [Command::SaveProfileSettings { input }]
                if input.alias == "fast"
                    && input.provider == "anthropic"
                    && input.model_id == "claude-opus-4.1"
                    && !input.enabled
                    && input.api_key_env.as_deref() == Some("ANTHROPIC_API_KEY")
        ));
    }
}
