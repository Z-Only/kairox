//! Command palette — discoverable overlay for TUI actions and slash commands.
//!
//! Search-only view over a static registry. Each entry maps to either a
//! direct [`Command`] or a chat-input prefill (e.g. `:model `) so the user
//! can finish the argument inline. The palette never reparses the existing
//! `:`-prefixed slash form; selection routes the same [`Command`] the slash
//! parser would produce, or hands the prefill back to [`ChatPanel`].

use std::borrow::Cow;

use crossterm::event::{Event, KeyCode};
use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph};
use ratatui::Frame;

use crate::components::{
    Command, Component, CrossPanelEffect, EventContext, ModelProfileEntry, QueueAction, SkillEntry,
};

/// What happens when an entry is activated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteAction {
    /// Zero-arg slash command — dispatch immediately.
    Clear,
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
    SwitchModel {
        alias: String,
    },
    ActivateSkill {
        skill_id: String,
    },
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
    pub id: Cow<'static, str>,
    pub label: Cow<'static, str>,
    pub description: Cow<'static, str>,
    pub action: PaletteAction,
}

impl PaletteEntry {
    const fn static_entry(
        id: &'static str,
        label: &'static str,
        description: &'static str,
        action: PaletteAction,
    ) -> Self {
        Self {
            id: Cow::Borrowed(id),
            label: Cow::Borrowed(label),
            description: Cow::Borrowed(description),
            action,
        }
    }

    fn dynamic(
        id: impl Into<String>,
        label: impl Into<String>,
        description: impl Into<String>,
        action: PaletteAction,
    ) -> Self {
        Self {
            id: Cow::Owned(id.into()),
            label: Cow::Owned(label.into()),
            description: Cow::Owned(description.into()),
            action,
        }
    }
}

/// The fixed list of palette entries. Mirrors the slash forms parsed in
/// `chat::input::apply_key_action` — keep both in sync.
pub fn builtin_entries() -> &'static [PaletteEntry] {
    const ENTRIES: &[PaletteEntry] = &[
        PaletteEntry::static_entry(
            "clear",
            ":clear",
            "Clear the current conversation projection locally",
            PaletteAction::Clear,
        ),
        PaletteEntry::static_entry(
            "compact",
            ":compact",
            "Summarise older history into a compaction summary",
            PaletteAction::Compact,
        ),
        PaletteEntry::static_entry(
            "model",
            ":model <alias>",
            "Switch the active model profile mid-session",
            PaletteAction::PrefillModel,
        ),
        PaletteEntry::static_entry(
            "model-selector",
            "Models: open selector",
            "Open the model profile selector",
            PaletteAction::ModelSelector,
        ),
        PaletteEntry::static_entry(
            "config-dir",
            "Settings: open config directory",
            "Open the writable Kairox config directory",
            PaletteAction::ConfigDir,
        ),
        PaletteEntry::static_entry(
            "profiles-config",
            "Models: open profiles config",
            "Open the writable model profiles config file",
            PaletteAction::ProfilesConfig,
        ),
        PaletteEntry::static_entry(
            "settings-source-user",
            "Settings: use user config",
            "Read and save settings against user config",
            PaletteAction::SettingsSourceUser,
        ),
        PaletteEntry::static_entry(
            "settings-source-project",
            "Settings: use project config",
            "Read and save settings against the selected project config",
            PaletteAction::SettingsSourceProject,
        ),
        PaletteEntry::static_entry(
            "settings-project-next",
            "Settings: next project",
            "Select the next project for project-scoped settings",
            PaletteAction::SettingsProjectNext,
        ),
        PaletteEntry::static_entry(
            "settings-project-previous",
            "Settings: previous project",
            "Select the previous project for project-scoped settings",
            PaletteAction::SettingsProjectPrevious,
        ),
        PaletteEntry::static_entry(
            "mcp-manager",
            "MCP: open manager",
            "Open MCP servers, catalog, and sources",
            PaletteAction::McpManager,
        ),
        PaletteEntry::static_entry(
            "mcp-config",
            "MCP: open config",
            "Open the writable MCP config file",
            PaletteAction::McpConfig,
        ),
        PaletteEntry::static_entry(
            "skills",
            ":skills",
            "List discovered native skills",
            PaletteAction::Skills,
        ),
        PaletteEntry::static_entry(
            "skills-manager",
            "Skills: open manager",
            "Open installed skills and catalog controls",
            PaletteAction::SkillsManager,
        ),
        PaletteEntry::static_entry(
            "skills-dir",
            "Skills: open directory",
            "Open the writable user skills directory",
            PaletteAction::SkillsDir,
        ),
        PaletteEntry::static_entry(
            "skill-catalog-refresh",
            "Skills: refresh catalog",
            "Refresh the configured skill catalog cache",
            PaletteAction::RefreshSkillCatalog,
        ),
        PaletteEntry::static_entry(
            "instructions",
            ":instructions",
            "Open user/project instructions settings",
            PaletteAction::Instructions,
        ),
        PaletteEntry::static_entry(
            "system-prompt",
            "Instructions: view system prompt",
            "Open the read-only system prompt view",
            PaletteAction::SystemPrompt,
        ),
        PaletteEntry::static_entry(
            "hooks",
            ":hooks",
            "Open user/project hooks settings",
            PaletteAction::Hooks,
        ),
        PaletteEntry::static_entry(
            "plugins",
            "Plugins: open manager",
            "Open the plugin manager",
            PaletteAction::Plugins,
        ),
        PaletteEntry::static_entry(
            "agents",
            ":agents",
            "Open planner, worker, and reviewer agent settings",
            PaletteAction::Agents,
        ),
        PaletteEntry::static_entry(
            "agents-dir",
            "Agents: open directory",
            "Open the writable user agents directory",
            PaletteAction::AgentsDir,
        ),
        PaletteEntry::static_entry(
            "session-new",
            "Session: new",
            "Start a new session using the active model",
            PaletteAction::NewSession,
        ),
        PaletteEntry::static_entry(
            "project-draft",
            ":project draft",
            "Start a draft session in the active project",
            PaletteAction::ProjectDraftSession,
        ),
        PaletteEntry::static_entry(
            "project-create",
            ":project create <name>",
            "Create a new local project",
            PaletteAction::PrefillProjectCreate,
        ),
        PaletteEntry::static_entry(
            "project-import",
            ":project import <path>",
            "Import an existing project path",
            PaletteAction::PrefillProjectImport,
        ),
        PaletteEntry::static_entry(
            "project-worktree",
            ":project worktree <branch>",
            "Start a worktree session in the active project",
            PaletteAction::PrefillProjectWorktree,
        ),
        PaletteEntry::static_entry(
            "session-cancel",
            "Session: cancel",
            "Cancel the current running session",
            PaletteAction::CancelSession,
        ),
        PaletteEntry::static_entry(
            "attach",
            ":attach <path>",
            "Attach a local file path to the next message",
            PaletteAction::PrefillAttach,
        ),
        PaletteEntry::static_entry(
            "detach-all",
            ":detach",
            "Detach all pending attachments from the composer",
            PaletteAction::PrefillDetachAll,
        ),
        PaletteEntry::static_entry(
            "detach",
            ":detach <name-or-path>",
            "Detach one pending attachment by name or path",
            PaletteAction::PrefillDetach,
        ),
        PaletteEntry::static_entry(
            "queue-send-now",
            "Queue: send selected now",
            "Send the selected queued message immediately",
            PaletteAction::QueueAction(QueueAction::SendSelectedNow),
        ),
        PaletteEntry::static_entry(
            "queue-edit",
            "Queue: restore selected for edit",
            "Move the selected queued message back into the composer",
            PaletteAction::QueueAction(QueueAction::RestoreSelectedForEdit),
        ),
        PaletteEntry::static_entry(
            "queue-delete",
            "Queue: delete selected",
            "Remove the selected queued message",
            PaletteAction::QueueAction(QueueAction::DeleteSelected),
        ),
        PaletteEntry::static_entry(
            "queue-move-up",
            "Queue: move selected up",
            "Move the selected queued message earlier",
            PaletteAction::QueueAction(QueueAction::MoveSelectedUp),
        ),
        PaletteEntry::static_entry(
            "queue-move-down",
            "Queue: move selected down",
            "Move the selected queued message later",
            PaletteAction::QueueAction(QueueAction::MoveSelectedDown),
        ),
        PaletteEntry::static_entry(
            "queue-previous",
            "Queue: select previous",
            "Select the previous queued message",
            PaletteAction::QueueAction(QueueAction::SelectPrevious),
        ),
        PaletteEntry::static_entry(
            "queue-next",
            "Queue: select next",
            "Select the next queued message",
            PaletteAction::QueueAction(QueueAction::SelectNext),
        ),
        PaletteEntry::static_entry(
            "skill-show",
            ":skill show <id>",
            "Show one native skill's body",
            PaletteAction::PrefillSkillShow,
        ),
        PaletteEntry::static_entry(
            "skill-activate",
            ":skill activate <id>",
            "Activate one skill for the current session",
            PaletteAction::PrefillSkillActivate,
        ),
        PaletteEntry::static_entry(
            "skill-deactivate",
            ":skill deactivate <id>",
            "Deactivate one skill for the current session",
            PaletteAction::PrefillSkillDeactivate,
        ),
        PaletteEntry::static_entry(
            "skill-catalog",
            ":skill catalog <keyword>",
            "Search the configured skill catalog",
            PaletteAction::PrefillSkillCatalog,
        ),
        PaletteEntry::static_entry(
            "skill-install",
            ":skill install <package>",
            "Install one skill package into user settings",
            PaletteAction::PrefillSkillInstall,
        ),
        PaletteEntry::static_entry(
            "skill-install-github",
            ":skill install github <repo>",
            "Install one GitHub skill into user settings",
            PaletteAction::PrefillSkillInstallGithub,
        ),
        PaletteEntry::static_entry(
            "skill-update",
            ":skill update <id>",
            "Update one installed skill",
            PaletteAction::PrefillSkillUpdate,
        ),
        PaletteEntry::static_entry(
            "skill-delete",
            ":skill delete <id>",
            "Delete one installed skill setting",
            PaletteAction::PrefillSkillDelete,
        ),
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
        PaletteAction::Clear
        | PaletteAction::Compact
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
        | PaletteAction::QueueAction(_)
        | PaletteAction::SwitchModel { .. }
        | PaletteAction::ActivateSkill { .. } => None,
    }
}

pub struct CommandPalette {
    focused: bool,
    visible: bool,
    filter: String,
    selected: usize,
    list_state: ListState,
    model_profiles: Vec<ModelProfileEntry>,
    skills: Vec<SkillEntry>,
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
            model_profiles: Vec::new(),
            skills: Vec::new(),
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

    pub fn visible_entries(&self) -> Vec<PaletteEntry> {
        let entries = self.all_entries();
        filter_entries(&self.filter, &entries)
            .into_iter()
            .cloned()
            .collect()
    }

    fn all_entries(&self) -> Vec<PaletteEntry> {
        let mut entries = builtin_entries().to_vec();
        entries.extend(self.dynamic_entries());
        entries
    }

    fn dynamic_entries(&self) -> Vec<PaletteEntry> {
        let mut entries = Vec::new();
        for profile in &self.model_profiles {
            if !profile.enabled {
                continue;
            }
            let display = model_profile_display(profile);
            entries.push(PaletteEntry::dynamic(
                format!("model-profile-{}", profile.alias),
                format!(":model {}", profile.alias),
                format!("Switch to {display}"),
                PaletteAction::SwitchModel {
                    alias: profile.alias.clone(),
                },
            ));
        }

        for skill in &self.skills {
            let active_suffix = if skill.active { " (active)" } else { "" };
            entries.push(PaletteEntry::dynamic(
                format!("skill-{}", skill.id),
                format!(":skill activate {}", skill.id),
                format!("Activate {}{}", skill.name, active_suffix),
                PaletteAction::ActivateSkill {
                    skill_id: skill.id.clone(),
                },
            ));
        }
        entries
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
            PaletteAction::Clear => {
                commands.push(Command::ClearSessionProjection);
            }
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
            PaletteAction::SwitchModel { alias } => {
                if let Some(session_id) = ctx.current_session_id {
                    commands.push(Command::SwitchModel {
                        workspace_id: ctx.workspace_id.clone(),
                        session_id: session_id.clone(),
                        alias,
                        reasoning_effort: None,
                    });
                }
            }
            PaletteAction::ActivateSkill { skill_id } => {
                if let Some(session_id) = ctx.current_session_id {
                    commands.push(Command::ActivateSkill {
                        workspace_id: ctx.workspace_id.clone(),
                        session_id: session_id.clone(),
                        skill_id,
                    });
                }
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

fn model_profile_display(profile: &ModelProfileEntry) -> String {
    if !profile.provider_display.is_empty() && !profile.model_display.is_empty() {
        format!("{} / {}", profile.provider_display, profile.model_display)
    } else if !profile.model_display.is_empty() {
        profile.model_display.clone()
    } else {
        profile.alias.clone()
    }
}

pub fn render_command_palette(
    area: Rect,
    frame: &mut Frame,
    palette: &CommandPalette,
    entries: &[PaletteEntry],
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
                        e.label.as_ref(),
                        Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw("  "),
                    Span::styled(e.description.as_ref(), Style::default().fg(Color::Gray)),
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
            CrossPanelEffect::UpdateCommandPalette(snapshot) => {
                self.model_profiles = snapshot.model_profiles.clone();
                self.skills = snapshot.skills.clone();
                self.clamp_selection();
            }
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
    use crate::components::{CommandPaletteSnapshot, FocusTarget};

    fn model_profile(alias: &str) -> ModelProfileEntry {
        ModelProfileEntry {
            alias: alias.into(),
            provider_display: "fake".into(),
            model_display: alias.into(),
            context_window: Some(128_000),
            output_limit: Some(4096),
            temperature: None,
            top_p: None,
            top_k: None,
            max_tokens: None,
            base_url: None,
            api_key_env: None,
            supports_reasoning: false,
            enabled: true,
            writable: true,
            source: "test".into(),
            has_api_key: true,
        }
    }

    fn skill_entry(id: &str, active: bool) -> SkillEntry {
        SkillEntry {
            id: id.into(),
            name: format!("{id} skill"),
            description: format!("{id} description"),
            source: "test".into(),
            activation_mode: "manual".into(),
            active,
        }
    }

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
        let visible: Vec<_> = p
            .visible_entries()
            .iter()
            .map(|e| e.id.as_ref().to_string())
            .collect();
        assert!(visible
            .iter()
            .all(|id| id.contains("skill") || id == "skills"));
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
        for c in "compact".chars() {
            let _ = p.handle_event(&test_ctx(), &key(KeyCode::Char(c)));
        }
        let (effects, commands) = p.handle_event(&test_ctx(), &key(KeyCode::Enter));
        assert!(matches!(&commands[..], [Command::CompactSession { .. }]));
        assert!(effects
            .iter()
            .any(|e| matches!(e, CrossPanelEffect::DismissCommandPalette)));
        assert!(!p.is_visible());
    }

    #[test]
    fn enter_dispatches_clear_projection_command() {
        let mut p = CommandPalette::new();
        p.show();
        for c in "clear".chars() {
            let _ = p.handle_event(&test_ctx(), &key(KeyCode::Char(c)));
        }

        let visible_ids: Vec<_> = p
            .visible_entries()
            .into_iter()
            .map(|e| e.id.into_owned())
            .collect();
        assert_eq!(visible_ids, vec!["clear".to_string()]);
        let (effects, commands) = p.handle_event(&test_ctx(), &key(KeyCode::Enter));

        assert!(matches!(&commands[..], [Command::ClearSessionProjection]));
        assert!(effects
            .iter()
            .any(|e| matches!(e, CrossPanelEffect::DismissCommandPalette)));
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
        let first = p.visible_entries()[0].id.clone();
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
            assert_eq!(p.visible_entries()[0].id.as_ref(), expected_id);
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
            assert_eq!(p.visible_entries()[0].id.as_ref(), expected_id);
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
            assert_eq!(p.visible_entries()[0].id.as_ref(), expected_id);
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
    fn dynamic_model_profile_entries_switch_model_directly() {
        let mut p = CommandPalette::new();
        p.handle_effect(&CrossPanelEffect::UpdateCommandPalette(
            CommandPaletteSnapshot {
                model_profiles: vec![model_profile("fast")],
                skills: Vec::new(),
            },
        ));
        p.show();
        for c in "fast".chars() {
            let _ = p.handle_event(&test_ctx(), &key(KeyCode::Char(c)));
        }

        let visible_ids: Vec<_> = p
            .visible_entries()
            .into_iter()
            .map(|e| e.id.into_owned())
            .collect();
        assert_eq!(visible_ids, vec!["model-profile-fast".to_string()]);
        let (_effects, commands) = p.handle_event(&test_ctx(), &key(KeyCode::Enter));

        assert!(matches!(
            &commands[..],
            [Command::SwitchModel {
                alias,
                reasoning_effort: None,
                ..
            }] if alias == "fast"
        ));
    }

    #[test]
    fn dynamic_skill_entries_activate_discovered_skill() {
        let mut p = CommandPalette::new();
        p.handle_effect(&CrossPanelEffect::UpdateCommandPalette(
            CommandPaletteSnapshot {
                model_profiles: Vec::new(),
                skills: vec![skill_entry("review", true)],
            },
        ));
        p.show();
        for c in "skill-review".chars() {
            let _ = p.handle_event(&test_ctx(), &key(KeyCode::Char(c)));
        }

        let visible_ids: Vec<_> = p
            .visible_entries()
            .into_iter()
            .map(|e| e.id.into_owned())
            .collect();
        assert_eq!(visible_ids, vec!["skill-review".to_string()]);
        let (_effects, commands) = p.handle_event(&test_ctx(), &key(KeyCode::Enter));

        assert!(matches!(
            &commands[..],
            [Command::ActivateSkill { skill_id, .. }] if skill_id == "review"
        ));
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
