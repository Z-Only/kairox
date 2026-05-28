//! Command palette state — the [`CommandPalette`] component data model and
//! runtime behavior (open/close, selection, scroll, dynamic entries).
//!
//! Static registry types and data live in [`super::registry`].

use ratatui::widgets::ListState;

use super::registry::{builtin_entries, filter_entries, prefill_text, PaletteAction, PaletteEntry};
use crate::components::{Command, CrossPanelEffect, EventContext, ModelProfileEntry, SkillEntry};

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
