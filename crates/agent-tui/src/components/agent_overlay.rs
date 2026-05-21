//! Agent settings manager overlay — TUI access to the same custom agent
//! profiles managed by the GUI settings pane.

use agent_core::facade::{AgentSettingsInput, AgentSettingsScope, AgentSettingsView};
use crossterm::event::{Event, KeyCode, KeyModifiers};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::components::{AgentOverlaySnapshot, Command, Component, CrossPanelEffect, EventContext};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentOverlayMode {
    List,
    Editor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AgentEditorField {
    Scope,
    Name,
    Description,
    Tools,
    ModelProfile,
    PermissionMode,
    Skills,
    Nicknames,
    Enabled,
    Instructions,
}

const EDITOR_FIELDS: [AgentEditorField; 10] = [
    AgentEditorField::Scope,
    AgentEditorField::Name,
    AgentEditorField::Description,
    AgentEditorField::Tools,
    AgentEditorField::ModelProfile,
    AgentEditorField::PermissionMode,
    AgentEditorField::Skills,
    AgentEditorField::Nicknames,
    AgentEditorField::Enabled,
    AgentEditorField::Instructions,
];

#[derive(Debug, Clone, PartialEq, Eq)]
struct AgentDraft {
    scope: AgentSettingsScope,
    name: String,
    description: String,
    tools_text: String,
    model_profile: String,
    permission_mode: String,
    skills_text: String,
    nicknames_text: String,
    enabled: bool,
    instructions: String,
}

impl AgentDraft {
    fn new(scope: AgentSettingsScope) -> Self {
        Self {
            scope,
            name: String::new(),
            description: String::new(),
            tools_text: String::new(),
            model_profile: String::new(),
            permission_mode: String::new(),
            skills_text: String::new(),
            nicknames_text: String::new(),
            enabled: true,
            instructions: String::new(),
        }
    }

    fn from_view(view: &AgentSettingsView) -> Self {
        Self {
            scope: if view.scope == AgentSettingsScope::Builtin {
                AgentSettingsScope::User
            } else {
                view.scope
            },
            name: view.name.clone(),
            description: view.description.clone(),
            tools_text: view.tools.join(", "),
            model_profile: view.model_profile.clone().unwrap_or_default(),
            permission_mode: view.permission_mode.clone().unwrap_or_default(),
            skills_text: view.skills.join(", "),
            nicknames_text: view.nickname_candidates.join(", "),
            enabled: view.enabled,
            instructions: view.instructions.clone(),
        }
    }

    #[cfg(test)]
    fn from_input(input: AgentSettingsInput) -> Self {
        Self {
            scope: input.scope,
            name: input.name,
            description: input.description,
            tools_text: input.tools.join(", "),
            model_profile: input.model_profile.unwrap_or_default(),
            permission_mode: input.permission_mode.unwrap_or_default(),
            skills_text: input.skills.join(", "),
            nicknames_text: input.nickname_candidates.join(", "),
            enabled: input.enabled,
            instructions: input.instructions,
        }
    }

    fn to_input(&self) -> Option<AgentSettingsInput> {
        let name = self.name.trim();
        let description = self.description.trim();
        if name.is_empty() || description.is_empty() {
            return None;
        }

        Some(AgentSettingsInput {
            scope: self.scope,
            name: name.to_string(),
            description: description.to_string(),
            tools: split_csv(&self.tools_text),
            model_profile: trim_option(&self.model_profile),
            permission_mode: trim_option(&self.permission_mode),
            skills: split_csv(&self.skills_text),
            nickname_candidates: split_csv(&self.nicknames_text),
            enabled: self.enabled,
            instructions: self.instructions.trim_end().to_string(),
        })
    }

    fn push_char(&mut self, field: AgentEditorField, ch: char) {
        match field {
            AgentEditorField::Scope => match ch {
                'u' | 'U' => self.scope = AgentSettingsScope::User,
                'p' | 'P' => self.scope = AgentSettingsScope::Project,
                _ => {}
            },
            AgentEditorField::Name => self.name.push(ch),
            AgentEditorField::Description => self.description.push(ch),
            AgentEditorField::Tools => self.tools_text.push(ch),
            AgentEditorField::ModelProfile => self.model_profile.push(ch),
            AgentEditorField::PermissionMode => self.permission_mode.push(ch),
            AgentEditorField::Skills => self.skills_text.push(ch),
            AgentEditorField::Nicknames => self.nicknames_text.push(ch),
            AgentEditorField::Enabled => match ch {
                'y' | 'Y' | '1' | 't' | 'T' => self.enabled = true,
                'n' | 'N' | '0' | 'f' | 'F' => self.enabled = false,
                ' ' => self.enabled = !self.enabled,
                _ => {}
            },
            AgentEditorField::Instructions => self.instructions.push(ch),
        }
    }

    fn backspace(&mut self, field: AgentEditorField) {
        match field {
            AgentEditorField::Name => {
                self.name.pop();
            }
            AgentEditorField::Description => {
                self.description.pop();
            }
            AgentEditorField::Tools => {
                self.tools_text.pop();
            }
            AgentEditorField::ModelProfile => {
                self.model_profile.pop();
            }
            AgentEditorField::PermissionMode => {
                self.permission_mode.pop();
            }
            AgentEditorField::Skills => {
                self.skills_text.pop();
            }
            AgentEditorField::Nicknames => {
                self.nicknames_text.pop();
            }
            AgentEditorField::Instructions => {
                self.instructions.pop();
            }
            AgentEditorField::Scope | AgentEditorField::Enabled => {}
        }
    }

    fn clear_field(&mut self, field: AgentEditorField) {
        match field {
            AgentEditorField::Name => self.name.clear(),
            AgentEditorField::Description => self.description.clear(),
            AgentEditorField::Tools => self.tools_text.clear(),
            AgentEditorField::ModelProfile => self.model_profile.clear(),
            AgentEditorField::PermissionMode => self.permission_mode.clear(),
            AgentEditorField::Skills => self.skills_text.clear(),
            AgentEditorField::Nicknames => self.nicknames_text.clear(),
            AgentEditorField::Instructions => self.instructions.clear(),
            AgentEditorField::Scope | AgentEditorField::Enabled => {}
        }
    }
}

pub struct AgentOverlay {
    focused: bool,
    visible: bool,
    agents: Vec<AgentSettingsView>,
    list_state: ListState,
    mode: AgentOverlayMode,
    draft: AgentDraft,
    editor_field_index: usize,
}

impl Default for AgentOverlay {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentOverlay {
    pub fn new() -> Self {
        Self {
            focused: false,
            visible: false,
            agents: Vec::new(),
            list_state: ListState::default(),
            mode: AgentOverlayMode::List,
            draft: AgentDraft::new(AgentSettingsScope::User),
            editor_field_index: 0,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self, snapshot: AgentOverlaySnapshot) {
        let selected = if snapshot.agents.is_empty() {
            None
        } else {
            Some(
                self.list_state
                    .selected()
                    .unwrap_or(0)
                    .min(snapshot.agents.len().saturating_sub(1)),
            )
        };
        self.agents = snapshot.agents;
        self.list_state.select(selected);
        self.mode = AgentOverlayMode::List;
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.agents.clear();
        self.list_state.select(None);
        self.mode = AgentOverlayMode::List;
        self.draft = AgentDraft::new(AgentSettingsScope::User);
        self.editor_field_index = 0;
    }

    #[allow(dead_code)]
    pub fn agents(&self) -> &[AgentSettingsView] {
        &self.agents
    }

    #[allow(dead_code)]
    pub fn selected_index(&self) -> Option<usize> {
        self.list_state.selected()
    }

    fn selected_agent(&self) -> Option<&AgentSettingsView> {
        self.list_state
            .selected()
            .and_then(|index| self.agents.get(index))
    }

    fn start_create(&mut self, scope: AgentSettingsScope) {
        self.mode = AgentOverlayMode::Editor;
        self.draft = AgentDraft::new(scope);
        self.editor_field_index = 1;
        self.visible = true;
    }

    fn start_edit_selected(&mut self) {
        let Some(agent) = self
            .selected_agent()
            .filter(|agent| agent.editable)
            .cloned()
        else {
            return;
        };
        self.mode = AgentOverlayMode::Editor;
        self.draft = AgentDraft::from_view(&agent);
        self.editor_field_index = 1;
    }

    fn move_down(&mut self) {
        match self.mode {
            AgentOverlayMode::List => {
                if self.agents.is_empty() {
                    return;
                }
                let next = match self.list_state.selected() {
                    Some(index) if index + 1 < self.agents.len() => index + 1,
                    Some(_) => self.agents.len() - 1,
                    None => 0,
                };
                self.list_state.select(Some(next));
            }
            AgentOverlayMode::Editor => {
                self.editor_field_index = (self.editor_field_index + 1) % EDITOR_FIELDS.len();
            }
        }
    }

    fn move_up(&mut self) {
        match self.mode {
            AgentOverlayMode::List => {
                if self.agents.is_empty() {
                    return;
                }
                let next = match self.list_state.selected() {
                    Some(index) if index > 0 => index - 1,
                    _ => 0,
                };
                self.list_state.select(Some(next));
            }
            AgentOverlayMode::Editor => {
                self.editor_field_index = if self.editor_field_index == 0 {
                    EDITOR_FIELDS.len() - 1
                } else {
                    self.editor_field_index - 1
                };
            }
        }
    }

    fn current_editor_field(&self) -> AgentEditorField {
        EDITOR_FIELDS[self.editor_field_index]
    }

    fn handle_list_key(&mut self, key: KeyCode) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let mut effects = Vec::new();
        let mut commands = Vec::new();

        match key {
            KeyCode::Char('j') | KeyCode::Down => self.move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.move_up(),
            KeyCode::Char('n') => self.start_create(AgentSettingsScope::User),
            KeyCode::Char('N') => self.start_create(AgentSettingsScope::Project),
            KeyCode::Char('e') | KeyCode::Char('E') | KeyCode::Enter => {
                self.start_edit_selected();
            }
            KeyCode::Char('c') | KeyCode::Char('C') => {
                if let Some(agent) = self.selected_agent() {
                    commands.push(Command::CopyAgentSettings {
                        settings_id: agent.settings_id.clone(),
                        scope: AgentSettingsScope::User,
                    });
                }
            }
            KeyCode::Char('p') | KeyCode::Char('P') => {
                if let Some(agent) = self.selected_agent() {
                    commands.push(Command::CopyAgentSettings {
                        settings_id: agent.settings_id.clone(),
                        scope: AgentSettingsScope::Project,
                    });
                }
            }
            KeyCode::Char('x') | KeyCode::Char('X') | KeyCode::Delete => {
                if let Some(agent) = self.selected_agent().filter(|agent| agent.deletable) {
                    commands.push(Command::DeleteAgentSettings {
                        settings_id: agent.settings_id.clone(),
                    });
                }
            }
            KeyCode::Char('o') | KeyCode::Char('O') => commands.push(Command::OpenAgentsDir),
            KeyCode::Char('r') | KeyCode::Char('R') => {
                commands.push(Command::OpenAgentSettingsOverlay);
            }
            KeyCode::Esc => {
                self.hide();
                effects.push(CrossPanelEffect::DismissAgentSettingsOverlay);
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
            KeyCode::Esc => self.mode = AgentOverlayMode::List,
            KeyCode::Backspace => self.draft.backspace(self.current_editor_field()),
            KeyCode::Delete => self.draft.clear_field(self.current_editor_field()),
            KeyCode::Enter => {
                if let Some(input) = self.draft.to_input() {
                    commands.push(Command::SaveAgentSettings { input });
                    self.mode = AgentOverlayMode::List;
                }
            }
            KeyCode::Char(ch) if !modifiers.contains(KeyModifiers::CONTROL) => {
                self.draft.push_char(self.current_editor_field(), ch);
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
            AgentOverlayMode::List => self.handle_list_key(key.code),
            AgentOverlayMode::Editor => self.handle_editor_key(key.code, key.modifiers),
        }
    }

    #[cfg(test)]
    fn handle_event_for_test(&mut self, event: &Event) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        self.handle_key_event(event)
    }

    #[cfg(test)]
    fn start_create_for_test(&mut self, scope: AgentSettingsScope) {
        self.start_create(scope);
    }

    #[cfg(test)]
    fn replace_draft_for_test(&mut self, input: AgentSettingsInput) {
        self.draft = AgentDraft::from_input(input);
        self.mode = AgentOverlayMode::Editor;
        self.visible = true;
    }
}

fn split_csv(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_string)
        .collect()
}

fn trim_option(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}

fn scope_label(scope: AgentSettingsScope) -> &'static str {
    match scope {
        AgentSettingsScope::Builtin => "builtin",
        AgentSettingsScope::User => "user",
        AgentSettingsScope::Project => "project",
    }
}

fn scope_color(scope: AgentSettingsScope) -> Color {
    match scope {
        AgentSettingsScope::Builtin => Color::DarkGray,
        AgentSettingsScope::User => Color::Cyan,
        AgentSettingsScope::Project => Color::Magenta,
    }
}

fn editor_field_label(field: AgentEditorField) -> &'static str {
    match field {
        AgentEditorField::Scope => "Scope",
        AgentEditorField::Name => "Name",
        AgentEditorField::Description => "Description",
        AgentEditorField::Tools => "Tools",
        AgentEditorField::ModelProfile => "Model",
        AgentEditorField::PermissionMode => "Permission",
        AgentEditorField::Skills => "Skills",
        AgentEditorField::Nicknames => "Nicknames",
        AgentEditorField::Enabled => "Enabled",
        AgentEditorField::Instructions => "Instructions",
    }
}

fn editor_field_value(draft: &AgentDraft, field: AgentEditorField) -> String {
    match field {
        AgentEditorField::Scope => scope_label(draft.scope).to_string(),
        AgentEditorField::Name => draft.name.clone(),
        AgentEditorField::Description => draft.description.clone(),
        AgentEditorField::Tools => draft.tools_text.clone(),
        AgentEditorField::ModelProfile => draft.model_profile.clone(),
        AgentEditorField::PermissionMode => draft.permission_mode.clone(),
        AgentEditorField::Skills => draft.skills_text.clone(),
        AgentEditorField::Nicknames => draft.nicknames_text.clone(),
        AgentEditorField::Enabled => draft.enabled.to_string(),
        AgentEditorField::Instructions => draft.instructions.clone(),
    }
}

pub fn render_agent_overlay(area: Rect, frame: &mut Frame, overlay: &AgentOverlay) {
    let modal_width = 108.min(area.width.saturating_sub(4));
    let modal_height = 24.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);

    let title = match overlay.mode {
        AgentOverlayMode::List => " Agent Settings ",
        AgentOverlayMode::Editor => " Agent Settings Editor ",
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
        AgentOverlayMode::List => render_agent_list(inner, frame, overlay),
        AgentOverlayMode::Editor => render_agent_editor(inner, frame, overlay),
    }
}

fn render_agent_list(area: Rect, frame: &mut Frame, overlay: &AgentOverlay) {
    let list_height = area.height.saturating_sub(2);
    let list_area = Rect::new(area.x, area.y, area.width, list_height);
    let hint_area = Rect::new(
        area.x,
        area.y + list_height,
        area.width,
        area.height.saturating_sub(list_height),
    );

    if overlay.agents.is_empty() {
        frame.render_widget(
            Paragraph::new(Line::from(Span::styled(
                "No agent profiles configured",
                Style::default().fg(Color::DarkGray),
            ))),
            list_area,
        );
    } else {
        let items = overlay
            .agents
            .iter()
            .map(|agent| {
                let effective_marker = if agent.effective { "* " } else { "  " };
                let state = if agent.enabled { "enabled" } else { "disabled" };
                let validity = if agent.valid { "" } else { " invalid" };
                let edit = if agent.editable {
                    " editable"
                } else {
                    " read-only"
                };
                ListItem::new(Line::from(vec![
                    Span::styled(effective_marker, Style::default().fg(Color::Green)),
                    Span::styled(
                        format!("{:<7}", scope_label(agent.scope)),
                        Style::default().fg(scope_color(agent.scope)),
                    ),
                    Span::styled(
                        format!(" {:<8}", state),
                        Style::default().fg(if agent.enabled {
                            Color::Green
                        } else {
                            Color::DarkGray
                        }),
                    ),
                    Span::styled(
                        format!(" {}", agent.name),
                        Style::default().add_modifier(Modifier::BOLD),
                    ),
                    Span::styled(
                        format!(" - {}", agent.description),
                        Style::default().fg(Color::Gray),
                    ),
                    Span::styled(
                        format!("{edit}{validity}"),
                        Style::default().fg(Color::DarkGray),
                    ),
                ]))
            })
            .collect::<Vec<_>>();
        let list = List::new(items).highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );
        let mut state = overlay.list_state;
        frame.render_stateful_widget(list, list_area, &mut state);
    }

    let hints = Line::from(vec![
        Span::styled("[j/k] nav  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[n/N] new user/project  ", Style::default().fg(Color::Cyan)),
        Span::styled("[Enter/e] edit  ", Style::default().fg(Color::Yellow)),
        Span::styled(
            "[c/p] copy user/project  ",
            Style::default().fg(Color::Green),
        ),
        Span::styled("[x] delete  ", Style::default().fg(Color::Red)),
        Span::styled("[o] folder  ", Style::default().fg(Color::Blue)),
        Span::styled("[Esc] close", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(hints), hint_area);
}

fn render_agent_editor(area: Rect, frame: &mut Frame, overlay: &AgentOverlay) {
    let list_height = area.height.saturating_sub(2);
    let list_area = Rect::new(area.x, area.y, area.width, list_height);
    let hint_area = Rect::new(
        area.x,
        area.y + list_height,
        area.width,
        area.height.saturating_sub(list_height),
    );

    let items = EDITOR_FIELDS
        .iter()
        .enumerate()
        .map(|(index, field)| {
            let marker = if index == overlay.editor_field_index {
                "> "
            } else {
                "  "
            };
            let value = editor_field_value(&overlay.draft, *field);
            ListItem::new(Line::from(vec![
                Span::styled(marker, Style::default().fg(Color::Cyan)),
                Span::styled(
                    format!("{:<12}", editor_field_label(*field)),
                    Style::default().add_modifier(Modifier::BOLD),
                ),
                Span::raw(" "),
                Span::styled(value, Style::default().fg(Color::Gray)),
            ]))
        })
        .collect::<Vec<_>>();
    let list = List::new(items);
    frame.render_widget(list, list_area);

    let hints = Line::from(vec![
        Span::styled("[Tab/j/k] field  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[u/p] scope  ", Style::default().fg(Color::Cyan)),
        Span::styled("[space/y/n] enabled  ", Style::default().fg(Color::Green)),
        Span::styled("[Enter] save  ", Style::default().fg(Color::Yellow)),
        Span::styled("[Esc] cancel", Style::default().fg(Color::DarkGray)),
    ]);
    frame.render_widget(Paragraph::new(hints), hint_area);
}

impl Component for AgentOverlay {
    fn handle_event(
        &mut self,
        _ctx: &EventContext,
        event: &Event,
    ) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        self.handle_key_event(event)
    }

    fn handle_effect(&mut self, effect: &CrossPanelEffect) {
        match effect {
            CrossPanelEffect::ShowAgentSettingsOverlay(snapshot) => self.show(snapshot.clone()),
            CrossPanelEffect::DismissAgentSettingsOverlay => self.hide(),
            _ => {}
        }
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        if self.visible {
            render_agent_overlay(area, frame, self);
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
    use agent_core::facade::{AgentSettingsInput, AgentSettingsScope, AgentSettingsView};
    use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};

    use super::AgentOverlay;
    use crate::components::{AgentOverlaySnapshot, Command};

    fn agent(name: &str, scope: AgentSettingsScope) -> AgentSettingsView {
        let scope_label = match scope {
            AgentSettingsScope::Builtin => "Builtin",
            AgentSettingsScope::User => "User",
            AgentSettingsScope::Project => "Project",
        };
        AgentSettingsView {
            settings_id: format!("{scope_label}:{name}"),
            name: name.to_string(),
            description: format!("{name} description"),
            scope,
            path: format!("{name}.md"),
            tools: vec!["fs.read".to_string()],
            model_profile: Some("fast".to_string()),
            permission_mode: Some("read_only".to_string()),
            skills: vec!["kairox-dev-workflow".to_string()],
            nickname_candidates: vec![name.to_string()],
            enabled: true,
            instructions: format!("{name} instructions"),
            effective: scope != AgentSettingsScope::Builtin,
            shadowed_by: None,
            valid: true,
            validation_error: None,
            editable: scope != AgentSettingsScope::Builtin,
            deletable: scope != AgentSettingsScope::Builtin,
        }
    }

    fn key(code: KeyCode) -> Event {
        Event::Key(KeyEvent::new(code, KeyModifiers::NONE))
    }

    #[test]
    fn show_lists_builtin_user_and_project_profiles() {
        let mut overlay = AgentOverlay::new();
        overlay.show(AgentOverlaySnapshot {
            agents: vec![
                agent("worker", AgentSettingsScope::Builtin),
                agent("worker", AgentSettingsScope::User),
                agent("reviewer", AgentSettingsScope::Project),
            ],
        });

        assert!(overlay.is_visible());
        assert_eq!(overlay.agents().len(), 3);
        assert_eq!(overlay.agents()[0].scope, AgentSettingsScope::Builtin);
        assert_eq!(overlay.agents()[1].scope, AgentSettingsScope::User);
        assert_eq!(overlay.agents()[2].scope, AgentSettingsScope::Project);
        assert_eq!(overlay.selected_index(), Some(0));
    }

    #[test]
    fn copy_builtin_to_user_dispatches_command() {
        let mut overlay = AgentOverlay::new();
        overlay.show(AgentOverlaySnapshot {
            agents: vec![agent("worker", AgentSettingsScope::Builtin)],
        });

        let (_, commands) = overlay.handle_event_for_test(&key(KeyCode::Char('c')));

        assert!(matches!(
            &commands[..],
            [Command::CopyAgentSettings {
                settings_id,
                scope: AgentSettingsScope::User,
            }] if settings_id == "Builtin:worker"
        ));
    }

    #[test]
    fn save_editor_dispatches_agent_settings_input() {
        let mut overlay = AgentOverlay::new();
        overlay.start_create_for_test(AgentSettingsScope::Project);
        overlay.replace_draft_for_test(AgentSettingsInput {
            scope: AgentSettingsScope::Project,
            name: "planner".to_string(),
            description: "Plans work".to_string(),
            tools: vec!["search".to_string()],
            model_profile: Some("reasoning".to_string()),
            permission_mode: Some("workspace_write".to_string()),
            skills: vec!["kairox-dev-workflow".to_string()],
            nickname_candidates: vec!["Planner".to_string()],
            enabled: false,
            instructions: "Break work into reviewable steps.".to_string(),
        });

        let (_, commands) = overlay.handle_event_for_test(&key(KeyCode::Enter));

        assert!(matches!(
            &commands[..],
            [Command::SaveAgentSettings { input }] if input.name == "planner"
                && input.scope == AgentSettingsScope::Project
                && input.tools == ["search"]
                && !input.enabled
        ));
    }

    #[test]
    fn delete_editable_profile_dispatches_command() {
        let mut overlay = AgentOverlay::new();
        overlay.show(AgentOverlaySnapshot {
            agents: vec![agent("reviewer", AgentSettingsScope::User)],
        });

        let (_, commands) = overlay.handle_event_for_test(&key(KeyCode::Char('x')));

        assert!(matches!(
            &commands[..],
            [Command::DeleteAgentSettings { settings_id }] if settings_id == "User:reviewer"
        ));
    }

    #[test]
    fn open_agents_dir_dispatches_command() {
        let mut overlay = AgentOverlay::new();
        overlay.show(AgentOverlaySnapshot { agents: Vec::new() });

        let (_, commands) = overlay.handle_event_for_test(&key(KeyCode::Char('o')));

        assert!(matches!(&commands[..], [Command::OpenAgentsDir]));
    }
}
