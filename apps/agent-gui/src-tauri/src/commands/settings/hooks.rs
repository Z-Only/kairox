use super::*;
use agent_core::facade::{HookSettingsInput, HooksSettingsView};

#[tauri::command]
#[specta::specta]
pub async fn get_hooks_settings(project_root: Option<String>) -> Result<HooksSettingsView, String> {
    let user_config_path = user_config_dir().join("config.toml");
    let project_config_path = project_root.as_ref().map(|root| {
        std::path::PathBuf::from(root)
            .join(".kairox")
            .join("config.toml")
    });

    let user = agent_runtime::hooks_settings::read_hooks_from_config(
        &user_config_path,
        agent_core::ConfigScope::User,
    )
    .map_err(|e| e.to_string())?;
    let project = if let Some(path) = project_config_path.as_deref() {
        agent_runtime::hooks_settings::read_hooks_from_config(
            path,
            agent_core::ConfigScope::Project,
        )
        .map_err(|e| e.to_string())?
    } else {
        Vec::new()
    };

    Ok(HooksSettingsView {
        user,
        project,
        templates: agent_runtime::hooks_settings::builtin_hook_templates(),
        user_config_path: user_config_path.display().to_string(),
        project_config_path: project_config_path.map(|path| path.display().to_string()),
    })
}

#[tauri::command]
#[specta::specta]
pub async fn upsert_hook_settings(
    state: State<'_, GuiState>,
    input: HookSettingsInput,
    project_root: Option<String>,
) -> Result<(), String> {
    let config_path = config_path_for_scope(input.scope, project_root.as_deref())?;
    agent_runtime::hooks_settings::upsert_hook(&input, &config_path).map_err(|e| e.to_string())?;
    state.refresh_config()?;
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn delete_hook_settings(
    state: State<'_, GuiState>,
    scope: agent_core::ConfigScope,
    event: String,
    id: String,
    project_root: Option<String>,
) -> Result<(), String> {
    let config_path = config_path_for_scope(scope, project_root.as_deref())?;
    agent_runtime::hooks_settings::delete_hook(&config_path, &event, &id)
        .map_err(|e| e.to_string())?;
    state.refresh_config()?;
    Ok(())
}

fn config_path_for_scope(
    scope: agent_core::ConfigScope,
    project_root: Option<&str>,
) -> Result<std::path::PathBuf, String> {
    match scope {
        agent_core::ConfigScope::User => Ok(user_config_dir().join("config.toml")),
        agent_core::ConfigScope::Project => {
            let root = project_root
                .ok_or_else(|| "project root is required for project hooks".to_string())?;
            Ok(std::path::PathBuf::from(root)
                .join(".kairox")
                .join("config.toml"))
        }
        other => Err(format!(
            "hooks can only be managed at User or Project scope, got {other}"
        )),
    }
}

fn user_config_dir() -> std::path::PathBuf {
    std::env::var("HOME")
        .ok()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".kairox")
}

#[cfg(test)]
mod hooks_path_tests {
    use super::*;

    #[test]
    fn config_path_for_scope_uses_user_config_dir_for_user_scope() {
        let path = config_path_for_scope(agent_core::ConfigScope::User, None)
            .expect("user scope should not require a project root");
        let path_str = path.to_string_lossy();
        // user_config_dir always ends with ".kairox" and we append config.toml.
        assert!(path_str.ends_with(".kairox/config.toml"), "got {path_str}");
    }

    #[test]
    fn config_path_for_scope_appends_kairox_config_for_project_scope() {
        let path =
            config_path_for_scope(agent_core::ConfigScope::Project, Some("/tmp/some/project"))
                .expect("project scope with root should resolve");
        assert_eq!(
            path,
            std::path::PathBuf::from("/tmp/some/project/.kairox/config.toml")
        );
    }

    #[test]
    fn config_path_for_scope_requires_project_root_when_project_scope() {
        let error = config_path_for_scope(agent_core::ConfigScope::Project, None)
            .expect_err("project scope without root should be rejected");
        assert!(error.contains("project root"), "got: {error}");
    }

    #[test]
    fn config_path_for_scope_rejects_builtin_scope() {
        let error = config_path_for_scope(agent_core::ConfigScope::Builtin, None)
            .expect_err("builtin scope cannot host hooks");
        assert!(
            error.contains("User or Project") && error.to_lowercase().contains("builtin"),
            "got: {error}"
        );
    }
}
