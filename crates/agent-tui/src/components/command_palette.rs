//! Command palette — discoverable overlay for TUI actions and slash commands.
//!
//! Search-only view over a static registry. Each entry maps to either a
//! direct [`Command`] or a chat-input prefill (e.g. `:model `) so the user
//! can finish the argument inline. The palette never reparses the existing
//! `:`-prefixed slash form; selection routes the same [`Command`] the slash
//! parser would produce, or hands the prefill back to [`ChatPanel`].

use crossterm::event::{Event, KeyCode};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::components::{Command, Component, CrossPanelEffect, EventContext, QueueAction};

/// What happens when an entry is activated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteAction {
    /// Zero-arg slash command — dispatch immediately.
    Compact,
    CancelSession,
    NewSession,
    ProjectDraftSession,
    ConfigDir,
    McpManager,
    McpConfig,
    Hooks,
    Instructions,
    Plugins,
    Agents,
    AgentsDir,
    Skills,
    SkillsDir,
    SkillsManager,
    SystemPrompt,
    ModelSelector,
    ProfilesConfig,
    SettingsSourceUser,
    SettingsSourceProject,
    SettingsProjectNext,
    SettingsProjectPrevious,
    RefreshSkillCatalog,
    QueueAction(QueueAction),
    /// Argument-taking slash command — prefill chat input with the slash
    /// prefix (trailing space) and hand focus back to chat so the user can
    /// type the argument.
    PrefillModel,
    PrefillAttach,
    PrefillDetachAll,
    PrefillDetach,
    PrefillProjectCreate,
    PrefillProjectImport,
    PrefillProjectWorktree,
    PrefillSkillShow,
    PrefillSkillActivate,
    PrefillSkillDeactivate,
    PrefillSkillCatalog,
    PrefillSkillInstall,
    PrefillSkillInstallGithub,
    PrefillSkillUpdate,
    PrefillSkillDelete,
}

/// A static entry in the palette registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaletteEntry {
    pub id: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub action: PaletteAction,
}

/// The fixed list of palette entries. Mirrors the slash forms parsed in
/// `chat::input::apply_key_action` — keep both in sync.
pub fn builtin_entries() -> &'static [PaletteEntry] {
    const ENTRIES: &[PaletteEntry] = &[
        PaletteEntry {
            id: "compact",
            label: ":compact",
            description: "Summarise older history into a compaction summary",
            action: PaletteAction::Compact,
        },
        PaletteEntry {
            id: "model",
            label: ":model <alias>",
            description: "Switch the active model profile mid-session",
            action: PaletteAction::PrefillModel,
        },
        PaletteEntry {
            id: "model-selector",
            label: "Models: open selector",
            description: "Open the model profile selector",
            action: PaletteAction::ModelSelector,
        },
        PaletteEntry {
            id: "config-dir",
            label: "Settings: open config directory",
            description: "Open the writable Kairox config directory",
            action: PaletteAction::ConfigDir,
        },
        PaletteEntry {
            id: "profiles-config",
            label: "Models: open profiles config",
            description: "Open the writable model profiles config file",
            action: PaletteAction::ProfilesConfig,
        },
        PaletteEntry {
            id: "settings-source-user",
            label: "Settings: use user config",
            description: "Read and save settings against user config",
            action: PaletteAction::SettingsSourceUser,
        },
        PaletteEntry {
            id: "settings-source-project",
            label: "Settings: use project config",
            description: "Read and save settings against the selected project config",
            action: PaletteAction::SettingsSourceProject,
        },
        PaletteEntry {
            id: "settings-project-next",
            label: "Settings: next project",
            description: "Select the next project for project-scoped settings",
            action: PaletteAction::SettingsProjectNext,
        },
        PaletteEntry {
            id: "settings-project-previous",
            label: "Settings: previous project",
            description: "Select the previous project for project-scoped settings",
            action: PaletteAction::SettingsProjectPrevious,
        },
        PaletteEntry {
            id: "mcp-manager",
            label: "MCP: open manager",
            description: "Open MCP servers, catalog, and sources",
            action: PaletteAction::McpManager,
        },
        PaletteEntry {
            id: "mcp-config",
            label: "MCP: open config",
            description: "Open the writable MCP config file",
            action: PaletteAction::McpConfig,
        },
        PaletteEntry {
            id: "skills",
            label: ":skills",
            description: "List discovered native skills",
            action: PaletteAction::Skills,
        },
        PaletteEntry {
            id: "skills-manager",
            label: "Skills: open manager",
            description: "Open installed skills and catalog controls",
            action: PaletteAction::SkillsManager,
        },
        PaletteEntry {
            id: "skills-dir",
            label: "Skills: open directory",
            description: "Open the writable user skills directory",
            action: PaletteAction::SkillsDir,
        },
        PaletteEntry {
            id: "skill-catalog-refresh",
            label: "Skills: refresh catalog",
            description: "Refresh the configured skill catalog cache",
            action: PaletteAction::RefreshSkillCatalog,
        },
        PaletteEntry {
            id: "instructions",
            label: ":instructions",
            description: "Open user/project instructions settings",
            action: PaletteAction::Instructions,
        },
        PaletteEntry {
            id: "system-prompt",
            label: "Instructions: view system prompt",
            description: "Open the read-only system prompt view",
            action: PaletteAction::SystemPrompt,
        },
        PaletteEntry {
            id: "hooks",
            label: ":hooks",
            description: "Open user/project hooks settings",
            action: PaletteAction::Hooks,
        },
        PaletteEntry {
            id: "plugins",
            label: "Plugins: open manager",
            description: "Open the plugin manager",
            action: PaletteAction::Plugins,
        },
        PaletteEntry {
            id: "agents",
            label: ":agents",
            description: "Open planner, worker, and reviewer agent settings",
            action: PaletteAction::Agents,
        },
        PaletteEntry {
            id: "agents-dir",
            label: "Agents: open directory",
            description: "Open the writable user agents directory",
            action: PaletteAction::AgentsDir,
        },
        PaletteEntry {
            id: "session-new",
            label: "Session: new",
            description: "Start a new session using the active model",
            action: PaletteAction::NewSession,
        },
        PaletteEntry {
            id: "project-draft",
            label: ":project draft",
            description: "Start a draft session in the active project",
            action: PaletteAction::ProjectDraftSession,
        },
        PaletteEntry {
            id: "project-create",
            label: ":project create <name>",
            description: "Create a new local project",
            action: PaletteAction::PrefillProjectCreate,
        },
        PaletteEntry {
            id: "project-import",
            label: ":project import <path>",
            description: "Import an existing project path",
            action: PaletteAction::PrefillProjectImport,
        },
        PaletteEntry {
            id: "project-worktree",
            label: ":project worktree <branch>",
            description: "Start a worktree session in the active project",
            action: PaletteAction::PrefillProjectWorktree,
        },
        PaletteEntry {
            id: "session-cancel",
            label: "Session: cancel",
            description: "Cancel the current running session",
            action: PaletteAction::CancelSession,
        },
        PaletteEntry {
            id: "attach",
            label: ":attach <path>",
            description: "Attach a local file path to the next message",
            action: PaletteAction::PrefillAttach,
        },
        PaletteEntry {
            id: "detach-all",
            label: ":detach",
            description: "Detach all pending attachments from the composer",
            action: PaletteAction::PrefillDetachAll,
        },
        PaletteEntry {
            id: "detach",
            label: ":detach <name-or-path>",
            description: "Detach one pending attachment by name or path",
            action: PaletteAction::PrefillDetach,
        },
        PaletteEntry {
            id: "queue-send-now",
            label: "Queue: send selected now",
            description: "Send the selected queued message immediately",
            action: PaletteAction::QueueAction(QueueAction::SendSelectedNow),
        },
        PaletteEntry {
            id: "queue-edit",
            label: "Queue: restore selected for edit",
            description: "Move the selected queued message back into the composer",
            action: PaletteAction::QueueAction(QueueAction::RestoreSelectedForEdit),
        },
        PaletteEntry {
            id: "queue-delete",
            label: "Queue: delete selected",
            description: "Remove the selected queued message",
            action: PaletteAction::QueueAction(QueueAction::DeleteSelected),
        },
        PaletteEntry {
            id: "queue-move-up",
            label: "Queue: move selected up",
            description: "Move the selected queued message earlier",
            action: PaletteAction::QueueAction(QueueAction::MoveSelectedUp),
        },
        PaletteEntry {
            id: "queue-move-down",
            label: "Queue: move selected down",
            description: "Move the selected queued message later",
            action: PaletteAction::QueueAction(QueueAction::MoveSelectedDown),
        },
        PaletteEntry {
            id: "queue-previous",
            label: "Queue: select previous",
            description: "Select the previous queued message",
            action: PaletteAction::QueueAction(QueueAction::SelectPrevious),
        },
        PaletteEntry {
            id: "queue-next",
            label: "Queue: select next",
            description: "Select the next queued message",
            action: PaletteAction::QueueAction(QueueAction::SelectNext),
        },
        PaletteEntry {
            id: "skill-show",
            label: ":skill show <id>",
            description: "Show one native skill's body",
            action: PaletteAction::PrefillSkillShow,
        },
        PaletteEntry {
            id: "skill-activate",
            label: ":skill activate <id>",
            description: "Activate one skill for the current session",
            action: PaletteAction::PrefillSkillActivate,
        },
        PaletteEntry {
            id: "skill-deactivate",
            label: ":skill deactivate <id>",
            description: "Deactivate one skill for the current session",
            action: PaletteAction::PrefillSkillDeactivate,
        },
        PaletteEntry {
            id: "skill-catalog",
            label: ":skill catalog <keyword>",
            description: "Search the configured skill catalog",
            action: PaletteAction::PrefillSkillCatalog,
        },
        PaletteEntry {
            id: "skill-install",
            label: ":skill install <package>",
            description: "Install one skill package into user settings",
            action: PaletteAction::PrefillSkillInstall,
        },
        PaletteEntry {
            id: "skill-install-github",
            label: ":skill install github <repo>",
            description: "Install one GitHub skill into user settings",
            action: PaletteAction::PrefillSkillInstallGithub,
        },
        PaletteEntry {
            id: "skill-update",
            label: ":skill update <id>",
            description: "Update one installed skill",
            action: PaletteAction::PrefillSkillUpdate,
        },
        PaletteEntry {
            id: "skill-delete",
            label: ":skill delete <id>",
            description: "Delete one installed skill setting",
            action: PaletteAction::PrefillSkillDelete,
        },
    ];
    ENTRIES
}

/// Filter entries by case-insensitive substring match against label and
/// description. An empty filter returns all entries in registry order.
pub fn filter_entries<'a>(filter: &str, entries: &'a [PaletteEntry]) -> Vec<&'a PaletteEntry> {
    let trimmed = filter.trim();
    if trimmed.is_empty() {
        return entries.iter().collect();
    }
    let needle = trimmed.to_lowercase();
    let tokens: Vec<&str> = needle.split_whitespace().collect();
    entries
        .iter()
        .filter(|e| {
            let haystack = format!(
                "{} {} {}",
                e.label.to_lowercase(),
                e.description.to_lowercase(),
                e.id.to_lowercase()
            );
            haystack.contains(&needle) || tokens.iter().all(|token| haystack.contains(token))
        })
        .collect()
}

/// Slash prefix used to prefill chat input for argument-taking actions.
pub fn prefill_text(action: &PaletteAction) -> Option<&'static str> {
    match action {
        PaletteAction::PrefillModel => Some(":model "),
        PaletteAction::PrefillAttach => Some(":attach "),
        PaletteAction::PrefillDetachAll => Some(":detach"),
        PaletteAction::PrefillDetach => Some(":detach "),
        PaletteAction::PrefillProjectCreate => Some(":project create "),
        PaletteAction::PrefillProjectImport => Some(":project import "),
        PaletteAction::PrefillProjectWorktree => Some(":project worktree "),
        PaletteAction::PrefillSkillShow => Some(":skill show "),
        PaletteAction::PrefillSkillActivate => Some(":skill activate "),
        PaletteAction::PrefillSkillDeactivate => Some(":skill deactivate "),
        PaletteAction::PrefillSkillCatalog => Some(":skill catalog "),
        PaletteAction::PrefillSkillInstall => Some(":skill install "),
        PaletteAction::PrefillSkillInstallGithub => Some(":skill install github "),
        PaletteAction::PrefillSkillUpdate => Some(":skill update "),
        PaletteAction::PrefillSkillDelete => Some(":skill delete "),
        PaletteAction::Compact
        | PaletteAction::CancelSession
        | PaletteAction::NewSession
        | PaletteAction::ProjectDraftSession
        | PaletteAction::ConfigDir
        | PaletteAction::McpManager
        | PaletteAction::McpConfig
        | PaletteAction::Hooks
        | PaletteAction::Instructions
        | PaletteAction::Plugins
        | PaletteAction::Agents
        | PaletteAction::AgentsDir
        | PaletteAction::Skills
        | PaletteAction::SkillsDir
        | PaletteAction::SkillsManager
        | PaletteAction::SystemPrompt
        | PaletteAction::ModelSelector
        | PaletteAction::ProfilesConfig
        | PaletteAction::SettingsSourceUser
        | PaletteAction::SettingsSourceProject
        | PaletteAction::SettingsProjectNext
        | PaletteAction::SettingsProjectPrevious
        | PaletteAction::RefreshSkillCatalog
        | PaletteAction::QueueAction(_) => None,
    }
}

pub struct CommandPalette {
    focused: bool,
    visible: bool,
    filter: String,
    selected: usize,
    list_state: ListState,
}

impl Default for CommandPalette {
    fn default() -> Self {
        Self::new()
    }
}

impl CommandPalette {
    pub fn new() -> Self {
        Self {
            focused: false,
            visible: false,
            filter: String::new(),
            selected: 0,
            list_state: ListState::default(),
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn show(&mut self) {
        self.visible = true;
        self.filter.clear();
        self.selected = 0;
        self.list_state.select(Some(0));
    }

    pub fn hide(&mut self) {
        self.visible = false;
        self.filter.clear();
        self.selected = 0;
        self.list_state.select(None);
    }

    pub fn filter(&self) -> &str {
        &self.filter
    }

    #[cfg(test)]
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    pub fn visible_entries(&self) -> Vec<&'static PaletteEntry> {
        filter_entries(&self.filter, builtin_entries())
    }

    fn clamp_selection(&mut self) {
        let len = self.visible_entries().len();
        if len == 0 {
            self.selected = 0;
            self.list_state.select(None);
        } else {
            if self.selected >= len {
                self.selected = len - 1;
            }
            self.list_state.select(Some(self.selected));
        }
    }

    fn move_down(&mut self) {
        let len = self.visible_entries().len();
        if len == 0 {
            return;
        }
        if self.selected + 1 < len {
            self.selected += 1;
        }
        self.list_state.select(Some(self.selected));
    }

    fn move_up(&mut self) {
        if self.selected > 0 {
            self.selected -= 1;
        }
        self.list_state.select(Some(self.selected));
    }

    fn current_action(&self) -> Option<PaletteAction> {
        self.visible_entries()
            .get(self.selected)
            .map(|e| e.action.clone())
    }

    fn activate(&mut self, ctx: &EventContext) -> (Vec<CrossPanelEffect>, Vec<Command>) {
        let Some(action) = self.current_action() else {
            return (Vec::new(), Vec::new());
        };
        let mut effects = Vec::new();
        let mut commands = Vec::new();

        match action {
            PaletteAction::Compact => {
                if let Some(session_id) = ctx.current_session_id {
                    commands.push(Command::CompactSession {
                        workspace_id: ctx.workspace_id.clone(),
                        session_id: session_id.clone(),
                    });
                }
            }
            PaletteAction::CancelSession => {
                if let Some(session_id) = ctx.current_session_id {
                    commands.push(Command::CancelSession {
                        workspace_id: ctx.workspace_id.clone(),
                        session_id: session_id.clone(),
                    });
                }
            }
            PaletteAction::NewSession => {
                commands.push(Command::StartSession {
                    workspace_id: ctx.workspace_id.clone(),
                    model_profile: ctx.model_profile.to_string(),
                });
            }
            PaletteAction::ProjectDraftSession => {
                if let Some(project_id) = active_project_id(ctx) {
                    commands.push(Command::CreateProjectDraftSession { project_id });
                }
            }
            PaletteAction::ConfigDir => {
                commands.push(Command::OpenConfigDir);
            }
            PaletteAction::McpManager => {
                commands.push(Command::OpenMcpOverlay);
            }
            PaletteAction::McpConfig => {
                commands.push(Command::OpenMcpConfig);
            }
            PaletteAction::Skills => {
                commands.push(Command::ListSkills);
            }
            PaletteAction::SkillsManager => {
                commands.push(Command::OpenSkillsOverlay);
            }
            PaletteAction::SkillsDir => {
                commands.push(Command::OpenSkillsDir);
            }
            PaletteAction::RefreshSkillCatalog => {
                commands.push(Command::RefreshSkillCatalog {
                    keyword: None,
                    sources: None,
                });
            }
            PaletteAction::Instructions => {
                commands.push(Command::OpenInstructionsOverlay);
            }
            PaletteAction::SystemPrompt => {
                commands.push(Command::OpenSystemPromptOverlay);
            }
            PaletteAction::Hooks => {
                commands.push(Command::OpenHooksOverlay);
            }
            PaletteAction::Plugins => {
                commands.push(Command::OpenPluginsOverlay);
            }
            PaletteAction::Agents => {
                commands.push(Command::OpenAgentSettingsOverlay);
            }
            PaletteAction::AgentsDir => {
                commands.push(Command::OpenAgentsDir);
            }
            PaletteAction::ModelSelector => {
                commands.push(Command::OpenModelOverlay);
            }
            PaletteAction::ProfilesConfig => {
                commands.push(Command::OpenProfilesConfig);
            }
            PaletteAction::SettingsSourceUser => {
                commands.push(Command::SetSettingsConfigSource {
                    source: crate::app_state::SettingsConfigSource::User,
                });
            }
            PaletteAction::SettingsSourceProject => {
                commands.push(Command::SetSettingsConfigSource {
                    source: crate::app_state::SettingsConfigSource::Project,
                });
            }
            PaletteAction::SettingsProjectNext => {
                commands.push(Command::CycleSettingsProject { direction: 1 });
            }
            PaletteAction::SettingsProjectPrevious => {
                commands.push(Command::CycleSettingsProject { direction: -1 });
            }
            PaletteAction::QueueAction(action) => {
                commands.push(Command::ApplyQueueAction(action));
            }
            ref prefill => {
                if let Some(text) = prefill_text(prefill) {
                    effects.push(CrossPanelEffect::PrefillChatInput(text.to_string()));
                }
            }
        }

        self.hide();
        effects.push(CrossPanelEffect::DismissCommandPalette);
        (effects, commands)
    }
}

fn active_project_id(ctx: &EventContext) -> Option<agent_core::ProjectId> {
    let session_id = ctx.current_session_id.as_ref()?;
    ctx.sessions
        .iter()
        .find(|session| &session.id == session_id)
        .and_then(|session| session.project_id.clone())
}

pub fn render_command_palette(
    area: Rect,
    frame: &mut Frame,
    palette: &CommandPalette,
    entries: &[&PaletteEntry],
    list_state: &mut ListState,
) {
    let modal_width = 72.min(area.width.saturating_sub(4));
    let modal_height = 18.min(area.height.saturating_sub(4));
    let x = (area.width.saturating_sub(modal_width)) / 2;
    let y = (area.height.saturating_sub(modal_height)) / 2;
    let modal_area = Rect::new(x, y, modal_width, modal_height);

    frame.render_widget(Clear, modal_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .title(Span::styled(
            " ⌘ Command Palette ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ))
        .border_style(Style::default().fg(Color::Cyan));

    let inner = block.inner(modal_area);
    frame.render_widget(block, modal_area);

    if inner.height < 4 {
        return;
    }

    let filter_area = Rect::new(inner.x, inner.y, inner.width, 1);
    let list_area = Rect::new(
        inner.x,
        inner.y + 1,
        inner.width,
        inner.height.saturating_sub(2),
    );
    let hint_area = Rect::new(
        inner.x,
        inner.y + inner.height.saturating_sub(1),
        inner.width,
        1,
    );

    let filter_line = Line::from(vec![
        Span::styled(
            "> ",
            Style::default()
                .fg(Color::Cyan)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw(palette.filter().to_string()),
        Span::styled("▌", Style::default().fg(Color::Cyan)),
    ]);
    frame.render_widget(Paragraph::new(filter_line), filter_area);

    if entries.is_empty() {
        let empty = Paragraph::new(Line::from(Span::styled(
            "No matching commands",
            Style::default().fg(Color::DarkGray),
        )));
        frame.render_widget(empty, list_area);
    } else {
        let items: Vec<ListItem> = entries
            .iter()
            .map(|e| {
                let line = Line::from(vec![
                    Span::styled(
                        e.label,
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::styled(e.description, Style::default().fg(Color::Gray)),
                ]);
                ListItem::new(line)
            })
            .collect();

        let list = List::new(items).highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .add_modifier(Modifier::BOLD),
        );
        frame.render_stateful_widget(list, list_area, list_state);
    }

    let hints = Line::from(vec![
        Span::styled("[↑/↓] nav  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[Enter] run  ", Style::default().fg(Color::Yellow)),
        Span::styled("[Esc] close  ", Style::default().fg(Color::DarkGray)),
        Span::styled("[type] filter", Style::default().fg(Color::Cyan)),
    ]);
    frame.render_widget(Paragraph::new(hints), hint_area);
}

impl Component for CommandPalette {
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

        match key.code {
            KeyCode::Esc => {
                self.hide();
                (vec![CrossPanelEffect::DismissCommandPalette], Vec::new())
            }
            KeyCode::Down => {
                self.move_down();
                (Vec::new(), Vec::new())
            }
            KeyCode::Up => {
                self.move_up();
                (Vec::new(), Vec::new())
            }
            KeyCode::Enter => self.activate(ctx),
            KeyCode::Backspace => {
                self.filter.pop();
                self.clamp_selection();
                (Vec::new(), Vec::new())
            }
            KeyCode::Char(c) => {
                self.filter.push(c);
                self.selected = 0;
                self.clamp_selection();
                (Vec::new(), Vec::new())
            }
            _ => (Vec::new(), Vec::new()),
        }
    }

    fn handle_effect(&mut self, effect: &CrossPanelEffect) {
        match effect {
            CrossPanelEffect::ShowCommandPalette => self.show(),
            CrossPanelEffect::DismissCommandPalette => self.hide(),
            _ => {}
        }
    }

    fn render(&self, area: Rect, frame: &mut Frame) {
        if !self.visible {
            return;
        }
        let entries = self.visible_entries();
        let mut state = self.list_state;
        render_command_palette(area, frame, self, &entries, &mut state);
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
    use crate::components::FocusTarget;

    fn test_ctx() -> EventContext<'static> {
        use agent_core::projection::SessionProjection;
        static PROJECTION: std::sync::OnceLock<SessionProjection> = std::sync::OnceLock::new();
        let projection = PROJECTION.get_or_init(SessionProjection::default);
        static SESSIONS: std::sync::OnceLock<Vec<crate::components::SessionInfo>> =
            std::sync::OnceLock::new();
        let sessions = SESSIONS.get_or_init(Vec::new);
        static WORKSPACE: std::sync::OnceLock<agent_core::WorkspaceId> = std::sync::OnceLock::new();
        let workspace = WORKSPACE.get_or_init(agent_core::WorkspaceId::new);
        static SESSION: std::sync::OnceLock<Option<agent_core::SessionId>> =
            std::sync::OnceLock::new();
        let session = SESSION.get_or_init(|| Some(agent_core::SessionId::new()));
        EventContext {
            focus: FocusTarget::CommandPalette,
            current_session: projection,
            projects: &[],
            sessions,
            model_profile: "fake",
            permission_mode: agent_tools::PermissionMode::Suggest,
            sidebar_left_visible: true,
            sidebar_right_visible: true,
            workspace_id: workspace,
            current_session_id: session,
        }
    }

    fn key(code: KeyCode) -> Event {
        Event::Key(crossterm::event::KeyEvent::new(
            code,
            crossterm::event::KeyModifiers::NONE,
        ))
    }

    #[test]
    fn filters_commands_by_prefix() {
        let entries = builtin_entries();
        let filtered = filter_entries("skill", entries);
        assert!(!filtered.is_empty());
        assert!(filtered.iter().all(|e| e.label.contains("skill")
            || e.description.to_lowercase().contains("skill")
            || e.id.contains("skill")));
        // ":compact" should NOT be in skill results.
        assert!(!filtered.iter().any(|e| e.id == "compact"));
    }

    #[test]
    fn empty_filter_returns_all_entries() {
        let entries = builtin_entries();
        let filtered = filter_entries("", entries);
        assert_eq!(filtered.len(), entries.len());
    }

    #[test]
    fn case_insensitive_match() {
        let entries = builtin_entries();
        let filtered = filter_entries("MODEL", entries);
        assert!(filtered.iter().any(|e| e.id == "model"));
    }

    #[test]
    fn invisible_by_default() {
        let p = CommandPalette::new();
        assert!(!p.is_visible());
    }

    #[test]
    fn show_makes_visible_and_resets_state() {
        let mut p = CommandPalette::new();
        p.filter.push('x');
        p.selected = 5;
        p.show();
        assert!(p.is_visible());
        assert_eq!(p.filter(), "");
        assert_eq!(p.selected_index(), 0);
    }

    #[test]
    fn typing_filters_and_navigation_clamps() {
        let mut p = CommandPalette::new();
        p.show();
        let _ = p.handle_event(&test_ctx(), &key(KeyCode::Char('s')));
        let _ = p.handle_event(&test_ctx(), &key(KeyCode::Char('k')));
        let visible: Vec<_> = p.visible_entries().iter().map(|e| e.id).collect();
        assert!(visible
            .iter()
            .all(|id| id.contains("skill") || id == &"skills"));
        // Navigate past end and back.
        for _ in 0..10 {
            let _ = p.handle_event(&test_ctx(), &key(KeyCode::Down));
        }
        assert!(p.selected_index() < p.visible_entries().len());
        let _ = p.handle_event(&test_ctx(), &key(KeyCode::Up));
        assert!(p.selected_index() < p.visible_entries().len());
    }

    #[test]
    fn enter_dispatches_compact_command() {
        let mut p = CommandPalette::new();
        p.show();
        // First entry is :compact.
        let (effects, commands) = p.handle_event(&test_ctx(), &key(KeyCode::Enter));
        assert!(matches!(&commands[..], [Command::CompactSession { .. }]));
        assert!(effects
            .iter()
            .any(|e| matches!(e, CrossPanelEffect::DismissCommandPalette)));
        assert!(!p.is_visible());
    }

    #[test]
    fn enter_dispatches_list_skills() {
        let mut p = CommandPalette::new();
        p.show();
        // Filter to :skills exactly (id "skills"). Type "skills".
        for c in "skills".chars() {
            let _ = p.handle_event(&test_ctx(), &key(KeyCode::Char(c)));
        }
        // The first matching entry should be ":skills" itself.
        let first = p.visible_entries()[0].id;
        assert_eq!(first, "skills");
        let (_, commands) = p.handle_event(&test_ctx(), &key(KeyCode::Enter));
        assert!(matches!(&commands[..], [Command::ListSkills]));
    }

    #[test]
    fn enter_dispatches_open_plugins_overlay() {
        let mut p = CommandPalette::new();
        p.show();
        for c in "plugins".chars() {
            let _ = p.handle_event(&test_ctx(), &key(KeyCode::Char(c)));
        }
        let (_, commands) = p.handle_event(&test_ctx(), &key(KeyCode::Enter));
        assert!(matches!(&commands[..], [Command::OpenPluginsOverlay]));
    }

    #[test]
    fn enter_dispatches_open_agent_settings_overlay() {
        let mut p = CommandPalette::new();
        p.show();
        for c in "agents".chars() {
            let _ = p.handle_event(&test_ctx(), &key(KeyCode::Char(c)));
        }
        let (_, commands) = p.handle_event(&test_ctx(), &key(KeyCode::Enter));
        assert!(matches!(&commands[..], [Command::OpenAgentSettingsOverlay]));
    }

    #[test]
    fn enter_dispatches_overlay_and_session_actions() {
        let expected = [
            ("mcp", "mcp-manager"),
            ("skills manager", "skills-manager"),
            ("hooks", "hooks"),
            ("model selector", "model-selector"),
            ("new session", "session-new"),
            ("cancel session", "session-cancel"),
        ];

        for (filter, expected_id) in expected {
            let mut p = CommandPalette::new();
            p.show();
            for c in filter.chars() {
                let _ = p.handle_event(&test_ctx(), &key(KeyCode::Char(c)));
            }
            assert_eq!(p.visible_entries()[0].id, expected_id);
            let (_, commands) = p.handle_event(&test_ctx(), &key(KeyCode::Enter));
            match expected_id {
                "mcp-manager" => assert!(matches!(&commands[..], [Command::OpenMcpOverlay])),
                "skills-manager" => {
                    assert!(matches!(&commands[..], [Command::OpenSkillsOverlay]))
                }
                "hooks" => assert!(matches!(&commands[..], [Command::OpenHooksOverlay])),
                "model-selector" => assert!(matches!(&commands[..], [Command::OpenModelOverlay])),
                "session-new" => assert!(matches!(&commands[..], [Command::StartSession { .. }])),
                "session-cancel" => {
                    assert!(matches!(&commands[..], [Command::CancelSession { .. }]))
                }
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn palette_exposes_project_create_and_import_prefills() {
        let entries = builtin_entries();

        let create = entries
            .iter()
            .find(|entry| entry.id == "project-create")
            .expect("project create entry should exist");
        assert_eq!(
            prefill_text(&create.action),
            Some(":project create "),
            "project create should prefill a slash command"
        );

        let import = entries
            .iter()
            .find(|entry| entry.id == "project-import")
            .expect("project import entry should exist");
        assert_eq!(
            prefill_text(&import.action),
            Some(":project import "),
            "project import should prefill a slash command"
        );
    }

    #[test]
    fn palette_exposes_attachment_prefills() {
        let entries = builtin_entries();

        let attach = entries
            .iter()
            .find(|entry| entry.id == "attach")
            .expect("attach entry should exist");
        assert_eq!(prefill_text(&attach.action), Some(":attach "));

        let detach_all = entries
            .iter()
            .find(|entry| entry.id == "detach-all")
            .expect("detach all entry should exist");
        assert_eq!(prefill_text(&detach_all.action), Some(":detach"));

        let detach = entries
            .iter()
            .find(|entry| entry.id == "detach")
            .expect("detach entry should exist");
        assert_eq!(prefill_text(&detach.action), Some(":detach "));
    }

    #[test]
    fn enter_dispatches_queue_actions() {
        let expected = [
            (
                "queue send",
                "queue-send-now",
                crate::components::QueueAction::SendSelectedNow,
            ),
            (
                "queue edit",
                "queue-edit",
                crate::components::QueueAction::RestoreSelectedForEdit,
            ),
            (
                "queue delete",
                "queue-delete",
                crate::components::QueueAction::DeleteSelected,
            ),
            (
                "queue up",
                "queue-move-up",
                crate::components::QueueAction::MoveSelectedUp,
            ),
            (
                "queue down",
                "queue-move-down",
                crate::components::QueueAction::MoveSelectedDown,
            ),
        ];

        for (filter, expected_id, expected_action) in expected {
            let mut p = CommandPalette::new();
            p.show();
            for c in filter.chars() {
                let _ = p.handle_event(&test_ctx(), &key(KeyCode::Char(c)));
            }
            assert_eq!(p.visible_entries()[0].id, expected_id);
            let (_, commands) = p.handle_event(&test_ctx(), &key(KeyCode::Enter));
            assert!(matches!(
                commands.as_slice(),
                [Command::ApplyQueueAction(action)] if action == &expected_action
            ));
        }
    }

    #[test]
    fn enter_dispatches_overlay_utility_actions() {
        let expected = [
            ("config dir", "config-dir"),
            ("mcp config", "mcp-config"),
            ("profiles config", "profiles-config"),
            ("agents dir", "agents-dir"),
            ("skills dir", "skills-dir"),
            ("system prompt", "system-prompt"),
            ("refresh catalog", "skill-catalog-refresh"),
        ];

        for (filter, expected_id) in expected {
            let mut p = CommandPalette::new();
            p.show();
            for c in filter.chars() {
                let _ = p.handle_event(&test_ctx(), &key(KeyCode::Char(c)));
            }
            assert_eq!(p.visible_entries()[0].id, expected_id);
            let (_, commands) = p.handle_event(&test_ctx(), &key(KeyCode::Enter));
            match expected_id {
                "config-dir" => assert!(matches!(&commands[..], [Command::OpenConfigDir])),
                "mcp-config" => assert!(matches!(&commands[..], [Command::OpenMcpConfig])),
                "profiles-config" => {
                    assert!(matches!(&commands[..], [Command::OpenProfilesConfig]))
                }
                "agents-dir" => assert!(matches!(&commands[..], [Command::OpenAgentsDir])),
                "skills-dir" => assert!(matches!(&commands[..], [Command::OpenSkillsDir])),
                "system-prompt" => {
                    assert!(matches!(&commands[..], [Command::OpenSystemPromptOverlay]))
                }
                "skill-catalog-refresh" => {
                    assert!(matches!(
                        &commands[..],
                        [Command::RefreshSkillCatalog {
                            keyword: None,
                            sources: None
                        }]
                    ))
                }
                _ => unreachable!(),
            }
        }
    }

    #[test]
    fn enter_emits_prefill_for_model() {
        let mut p = CommandPalette::new();
        p.show();
        for c in "model".chars() {
            let _ = p.handle_event(&test_ctx(), &key(KeyCode::Char(c)));
        }
        let (effects, commands) = p.handle_event(&test_ctx(), &key(KeyCode::Enter));
        assert!(commands.is_empty());
        assert!(effects.iter().any(|e| matches!(
            e,
            CrossPanelEffect::PrefillChatInput(text) if text == ":model "
        )));
        assert!(effects
            .iter()
            .any(|e| matches!(e, CrossPanelEffect::DismissCommandPalette)));
    }

    #[test]
    fn esc_dismisses_palette() {
        let mut p = CommandPalette::new();
        p.show();
        let (effects, commands) = p.handle_event(&test_ctx(), &key(KeyCode::Esc));
        assert!(commands.is_empty());
        assert!(matches!(
            effects.as_slice(),
            [CrossPanelEffect::DismissCommandPalette]
        ));
        assert!(!p.is_visible());
    }

    #[test]
    fn backspace_removes_last_filter_char() {
        let mut p = CommandPalette::new();
        p.show();
        let _ = p.handle_event(&test_ctx(), &key(KeyCode::Char('s')));
        let _ = p.handle_event(&test_ctx(), &key(KeyCode::Char('k')));
        assert_eq!(p.filter(), "sk");
        let _ = p.handle_event(&test_ctx(), &key(KeyCode::Backspace));
        assert_eq!(p.filter(), "s");
    }

    #[test]
    fn renders_without_panic() {
        let mut p = CommandPalette::new();
        p.show();
        let backend = ratatui::backend::TestBackend::new(80, 24);
        let mut terminal = ratatui::Terminal::new(backend).unwrap();
        terminal.draw(|f| p.render(f.area(), f)).expect("render");
    }
}
