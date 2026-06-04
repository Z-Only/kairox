use std::path::PathBuf;

use agent_core::facade::{SkillInstallTarget, SkillSettingsScope};
use agent_core::CoreError;
use agent_skills::{SkillRoot, SkillSourceKind};

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct SkillSettingsRoots {
    pub workspace_root: Option<PathBuf>,
    pub user_root: Option<PathBuf>,
    pub builtin_root: Option<PathBuf>,
    pub plugin_roots: Vec<(String, PathBuf)>,
}

pub(crate) fn skill_roots(roots: &SkillSettingsRoots) -> Vec<SkillRoot> {
    let mut skill_roots = Vec::new();
    if let Some(root) = &roots.builtin_root {
        skill_roots.push(SkillRoot::new(SkillSourceKind::Builtin, root));
    }
    if let Some(root) = &roots.user_root {
        skill_roots.push(SkillRoot::new(SkillSourceKind::User, root));
    }
    if let Some(root) = &roots.workspace_root {
        skill_roots.push(SkillRoot::new(SkillSourceKind::Workspace, root));
    }
    for (name, path) in &roots.plugin_roots {
        skill_roots.push(SkillRoot::with_namespace(
            SkillSourceKind::Plugin,
            path,
            name.clone(),
        ));
    }
    skill_roots
}

pub(super) fn root_for_scope(
    roots: &SkillSettingsRoots,
    scope: SkillSettingsScope,
) -> Option<PathBuf> {
    match scope {
        SkillSettingsScope::Project => roots.workspace_root.clone(),
        SkillSettingsScope::User => roots.user_root.clone(),
        SkillSettingsScope::Builtin => roots.builtin_root.clone(),
        SkillSettingsScope::Plugin => None,
    }
}

pub(super) fn install_root(
    roots: &SkillSettingsRoots,
    target: SkillInstallTarget,
) -> agent_core::Result<PathBuf> {
    match target {
        SkillInstallTarget::Project => roots.workspace_root.clone(),
        SkillInstallTarget::User => roots.user_root.clone(),
    }
    .ok_or_else(|| {
        CoreError::InvalidState(format!("skill install root not configured for {target:?}"))
    })
}
