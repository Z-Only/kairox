//! Command palette state — registry, filter logic, and the [`CommandPalette`]
//! component data model.
//!
//! The static registry mirrors the slash forms parsed in
//! `chat::input::apply_key_action` — keep both in sync.

use std::borrow::Cow;

use ratatui::widgets::ListState;

use crate::components::{
    Command, CrossPanelEffect, EventContext, ModelProfileEntry, QueueAction, SkillEntry,
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
    pub(super) focused: bool,
    pub(super) visible: bool,
    pub(super) filter: String,
    pub(super) selected: usize,
    pub(super) list_state: ListState,
    pub(super) model_profiles: Vec<ModelProfileEntry>,
    pub(super) skills: Vec<SkillEntry>,
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

    pub(super) fn clamp_selection(&mut self) {
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

    pub(super) fn move_down(&mut self) {
        let len = self.visible_entries().len();
        if len == 0 {
            return;
        }
        if self.selected + 1 < len {
            self.selected += 1;
        }
        self.list_state.select(Some(self.selected));
    }

    pub(super) fn move_up(&mut self) {
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

    pub(super) fn activate(&mut self, ctx: &EventContext) -> (Vec<CrossPanelEffect>, Vec<Command>) {
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
