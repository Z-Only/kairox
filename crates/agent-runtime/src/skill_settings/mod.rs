mod actions;
mod install;
mod roots;
mod view;

#[cfg(test)]
mod tests;

pub use actions::{delete_skill, set_skill_activation_mode, set_skill_enabled, update_skill};
pub use install::{install_github_skill, install_remote_skill};
pub(crate) use roots::skill_roots;
pub use roots::SkillSettingsRoots;
pub use view::{get_skill_settings_detail, list_skill_settings};

use agent_core::facade::SkillSettingsScope;
use agent_core::CoreError;

const SKILLS_STATE_FILE_NAME: &str = "skills-state.toml";

fn scope_label(scope: SkillSettingsScope) -> &'static str {
    match scope {
        SkillSettingsScope::Project => "project",
        SkillSettingsScope::User => "user",
        SkillSettingsScope::Builtin => "builtin",
        SkillSettingsScope::Plugin => "plugin",
    }
}

fn skill_error(error: agent_skills::SkillError) -> CoreError {
    CoreError::InvalidState(error.to_string())
}
