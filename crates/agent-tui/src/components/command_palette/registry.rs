//! Command palette registry — static entry definitions, filter logic, and
//! prefill text for argument-taking slash commands.
//!
//! The static registry mirrors the slash forms parsed in
//! `chat::input::apply_key_action` — keep both in sync.

use std::borrow::Cow;

use crate::components::QueueAction;

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
    MonitorList,
    PrefillMonitorStop,
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

    pub(super) fn dynamic(
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
            "monitors",
            ":monitors",
            "List active background monitors",
            PaletteAction::MonitorList,
        ),
        PaletteEntry::static_entry(
            "monitor-stop",
            ":monitor stop <id>",
            "Stop a running monitor by ID",
            PaletteAction::PrefillMonitorStop,
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
        PaletteAction::PrefillMonitorStop => Some(":monitor stop "),
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
        | PaletteAction::MonitorList
        | PaletteAction::QueueAction(_)
        | PaletteAction::SwitchModel { .. }
        | PaletteAction::ActivateSkill { .. } => None,
    }
}
