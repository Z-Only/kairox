use std::path::{Path, PathBuf};

use agent_core::facade::{
    PluginComponentInventoryView, PluginDetailView, PluginInstallTarget, PluginSettingsView,
};
use agent_core::{ConfigScope, CoreError};
use agent_plugins::{PluginRoot, PluginScope};

#[derive(Debug, Clone, Default, Eq, PartialEq)]
pub struct PluginSettingsRoots {
    pub workspace_root: Option<PathBuf>,
    pub user_root: Option<PathBuf>,
    pub builtin_root: Option<PathBuf>,
}

pub fn build_default_plugin_settings_roots(home: &Path, workspace: &Path) -> PluginSettingsRoots {
    PluginSettingsRoots {
        workspace_root: Some(workspace.join(".kairox/plugins")),
        user_root: Some(home.join(".config/kairox/plugins")),
        builtin_root: None,
    }
}

pub async fn list_plugin_settings(
    roots: PluginSettingsRoots,
) -> agent_core::Result<Vec<PluginSettingsView>> {
    let projection = agent_plugins::discover_plugin_settings(plugin_roots(&roots))
        .await
        .map_err(plugin_error)?;
    Ok(projection
        .plugins
        .into_iter()
        .map(local_view_to_core_view)
        .collect())
}

pub async fn get_plugin_detail(
    roots: PluginSettingsRoots,
    settings_id: &str,
) -> agent_core::Result<Option<PluginDetailView>> {
    let Some(view) = list_plugin_settings(roots)
        .await?
        .into_iter()
        .find(|view| view.settings_id == settings_id || view.id == settings_id)
    else {
        return Ok(None);
    };

    Ok(Some(PluginDetailView {
        manifest_path: view.path.clone(),
        homepage: None,
        repository: None,
        license: None,
        keywords: vec![],
        view,
    }))
}

pub async fn set_plugin_enabled(
    roots: PluginSettingsRoots,
    settings_id: &str,
    enabled: bool,
) -> agent_core::Result<()> {
    let view = find_plugin_settings_view(roots.clone(), settings_id).await?;
    reject_builtin_mutation(&view, "enable or disable")?;
    let root = root_for_scope(&roots, view.scope).ok_or_else(|| {
        CoreError::InvalidState(format!("plugin root not configured for {}", view.scope))
    })?;
    agent_plugins::write_plugin_state(
        &root,
        &view.id,
        enabled,
        view.install_source.as_deref(),
        view.marketplace.as_deref(),
    )
    .await
    .map_err(plugin_error)
}

pub async fn delete_plugin(
    roots: PluginSettingsRoots,
    settings_id: &str,
) -> agent_core::Result<()> {
    let view = find_plugin_settings_view(roots.clone(), settings_id).await?;
    reject_builtin_mutation(&view, "delete")?;
    let root = root_for_scope(&roots, view.scope).ok_or_else(|| {
        CoreError::InvalidState(format!("plugin root not configured for {}", view.scope))
    })?;
    let plugin_directory = PathBuf::from(&view.path);
    validate_directory_under_root(&plugin_directory, &root)?;
    tokio::fs::remove_dir_all(&plugin_directory)
        .await
        .map_err(|error| CoreError::InvalidState(format!("failed to delete plugin: {error}")))
}

pub fn install_root(
    roots: &PluginSettingsRoots,
    target: PluginInstallTarget,
) -> agent_core::Result<PathBuf> {
    match target {
        PluginInstallTarget::Project => roots.workspace_root.clone(),
        PluginInstallTarget::User => roots.user_root.clone(),
    }
    .ok_or_else(|| {
        CoreError::InvalidState(format!("plugin install root not configured for {target:?}"))
    })
}

pub fn user_config_dir(roots: &PluginSettingsRoots) -> agent_core::Result<PathBuf> {
    roots
        .user_root
        .as_ref()
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .ok_or_else(|| CoreError::InvalidState("user plugin config root not configured".into()))
}

pub fn local_view_to_core_view(view: agent_plugins::PluginSettingsView) -> PluginSettingsView {
    PluginSettingsView {
        settings_id: view.settings_id,
        id: view.id,
        name: view.name,
        description: view.description,
        version: view.version,
        scope: scope_to_config_scope(view.scope),
        path: view.path,
        enabled: view.enabled,
        install_source: view.install_source,
        marketplace: view.marketplace,
        effective: view.effective,
        shadowed_by: view.shadowed_by,
        valid: view.valid,
        validation_error: view.validation_error,
        inventory: PluginComponentInventoryView {
            skill_count: view.manifest.inventory.skill_count as u32,
            skill_names: view.manifest.inventory.skill_names,
            mcp_server_count: view.manifest.inventory.mcp_server_count as u32,
            app_count: view.manifest.inventory.app_count as u32,
            agent_count: view.manifest.inventory.agent_count as u32,
            hook_count: view.manifest.inventory.hook_count as u32,
        },
        manifest_kind: match view.manifest.manifest_kind {
            agent_plugins::PluginManifestKind::Kairox => "kairox",
            agent_plugins::PluginManifestKind::Codex => "codex",
            agent_plugins::PluginManifestKind::Claude => "claude",
        }
        .to_string(),
    }
}

fn plugin_roots(roots: &PluginSettingsRoots) -> Vec<PluginRoot> {
    let mut plugin_roots = Vec::new();
    if let Some(root) = &roots.builtin_root {
        plugin_roots.push(PluginRoot::new(PluginScope::Builtin, root));
    }
    if let Some(root) = &roots.user_root {
        plugin_roots.push(PluginRoot::new(PluginScope::User, root));
    }
    if let Some(root) = &roots.workspace_root {
        plugin_roots.push(PluginRoot::new(PluginScope::Project, root));
    }
    plugin_roots
}

fn scope_to_config_scope(scope: PluginScope) -> ConfigScope {
    match scope {
        PluginScope::Builtin => ConfigScope::Builtin,
        PluginScope::User => ConfigScope::User,
        PluginScope::Project => ConfigScope::Project,
    }
}

fn root_for_scope(roots: &PluginSettingsRoots, scope: ConfigScope) -> Option<PathBuf> {
    match scope {
        ConfigScope::Project => roots.workspace_root.clone(),
        ConfigScope::User => roots.user_root.clone(),
        ConfigScope::Builtin => roots.builtin_root.clone(),
        ConfigScope::Local => None,
    }
}

async fn find_plugin_settings_view(
    roots: PluginSettingsRoots,
    plugin_identifier: &str,
) -> agent_core::Result<PluginSettingsView> {
    let matching_views = list_plugin_settings(roots)
        .await?
        .into_iter()
        .filter(|view| view.settings_id == plugin_identifier || view.id == plugin_identifier)
        .collect::<Vec<_>>();
    match matching_views.as_slice() {
        [view] => Ok(view.clone()),
        [] => Err(CoreError::InvalidState(format!(
            "plugin not found: {plugin_identifier}"
        ))),
        _ => Err(CoreError::InvalidState(format!(
            "ambiguous plugin id: {plugin_identifier}"
        ))),
    }
}

fn reject_builtin_mutation(view: &PluginSettingsView, operation: &str) -> agent_core::Result<()> {
    if view.scope == ConfigScope::Builtin {
        return Err(CoreError::InvalidState(format!(
            "cannot {operation} built-in plugin: {}",
            view.id
        )));
    }
    Ok(())
}

fn validate_directory_under_root(directory: &Path, root: &Path) -> agent_core::Result<()> {
    if !directory.starts_with(root) {
        return Err(CoreError::InvalidState(format!(
            "plugin directory {} is outside root {}",
            directory.display(),
            root.display()
        )));
    }
    Ok(())
}

fn plugin_error(error: agent_plugins::PluginError) -> CoreError {
    CoreError::InvalidState(error.to_string())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;

    fn write_plugin(root: &Path, name: &str) {
        let manifest_dir = root.join(name).join(".kairox-plugin");
        fs::create_dir_all(&manifest_dir).expect("manifest dir");
        fs::write(
            manifest_dir.join("plugin.json"),
            format!(r#"{{"name":"{name}","description":"Plugin {name}"}}"#),
        )
        .expect("manifest");
    }

    #[tokio::test]
    async fn list_plugin_settings_maps_scope_and_inventory() {
        let user = tempfile::tempdir().expect("user");
        write_plugin(user.path(), "github");

        let views = list_plugin_settings(PluginSettingsRoots {
            user_root: Some(user.path().to_path_buf()),
            ..PluginSettingsRoots::default()
        })
        .await
        .expect("views");

        assert_eq!(views.len(), 1);
        assert_eq!(views[0].settings_id, "user:github");
        assert_eq!(views[0].scope, ConfigScope::User);
        assert_eq!(views[0].manifest_kind, "kairox");
    }
}
