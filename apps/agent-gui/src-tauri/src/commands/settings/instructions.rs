use super::*;
use agent_core::facade::{InstructionsUpdateInput, InstructionsView};
use std::path::{Path, PathBuf};

#[tauri::command]
#[specta::specta]
pub async fn get_instructions(
    _state: State<'_, GuiState>,
    _scope: agent_core::ConfigScope,
    project_root: Option<String>,
) -> Result<InstructionsView, String> {
    let (user_config_path, project_config_path) = instruction_config_paths(project_root.as_deref());
    read_instructions_view(&user_config_path, project_config_path.as_deref())
}

#[tauri::command]
#[specta::specta]
pub async fn upsert_instructions(
    state: State<'_, GuiState>,
    input: InstructionsUpdateInput,
    project_root: Option<String>,
) -> Result<(), String> {
    let (user_config_path, project_config_path) = instruction_config_paths(project_root.as_deref());
    upsert_instructions_at_paths(&input, &user_config_path, project_config_path.as_deref())?;

    // Refresh config so the runtime picks up the new instructions.
    state.refresh_config().await?;

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn get_system_prompt() -> Result<String, String> {
    Ok(agent_runtime::instructions_settings::get_system_prompt())
}

fn instruction_config_paths(project_root: Option<&str>) -> (PathBuf, Option<PathBuf>) {
    (
        user_config_path(),
        project_root.map(|root| project_config_path(Path::new(root))),
    )
}

fn user_config_path() -> PathBuf {
    let home = std::env::var("HOME").ok().map(PathBuf::from);
    user_config_path_from_home(home.as_deref())
}

fn user_config_path_from_home(home: Option<&Path>) -> PathBuf {
    home.unwrap_or_else(|| Path::new("."))
        .join(".kairox")
        .join("config.toml")
}

fn project_config_path(project_root: impl AsRef<Path>) -> PathBuf {
    project_root.as_ref().join(".kairox").join("config.toml")
}

fn read_instructions_view(
    user_config_path: &Path,
    project_config_path: Option<&Path>,
) -> Result<InstructionsView, String> {
    let user_instructions =
        agent_runtime::instructions_settings::read_instructions(user_config_path)
            .map_err(|e| e.to_string())?;

    let project_instructions = if let Some(project_config_path) = project_config_path {
        agent_runtime::instructions_settings::read_instructions(project_config_path)
            .map_err(|e| e.to_string())?
    } else {
        None
    };

    Ok(
        agent_runtime::instructions_settings::build_instructions_view(
            user_instructions,
            project_instructions,
        ),
    )
}

fn upsert_instructions_at_paths(
    input: &InstructionsUpdateInput,
    user_config_path: &Path,
    project_config_path: Option<&Path>,
) -> Result<(), String> {
    agent_runtime::instructions_settings::upsert_instructions(
        input,
        user_config_path,
        project_config_path,
    )
    .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use agent_core::ConfigScope;

    #[test]
    fn instruction_config_paths_keep_user_and_project_boundaries() {
        let user_home = std::path::Path::new("/tmp/kairox-user");

        assert_eq!(
            user_config_path_from_home(Some(user_home)),
            std::path::PathBuf::from("/tmp/kairox-user/.kairox/config.toml")
        );
        assert_eq!(
            user_config_path_from_home(None),
            std::path::PathBuf::from("./.kairox/config.toml")
        );
        assert_eq!(
            project_config_path("/tmp/kairox-project"),
            std::path::PathBuf::from("/tmp/kairox-project/.kairox/config.toml")
        );
    }

    #[test]
    fn read_instructions_view_reads_user_and_project_text_from_distinct_paths() {
        let user_root = tempfile::tempdir().expect("user root");
        let project_root = tempfile::tempdir().expect("project root");
        let user_config_path = user_root.path().join(".kairox/config.toml");
        let project_config_path = project_config_path(project_root.path());

        std::fs::create_dir_all(user_config_path.parent().unwrap()).expect("user config dir");
        std::fs::create_dir_all(project_config_path.parent().unwrap()).expect("project config dir");
        std::fs::write(&user_config_path, "instructions = \"  User text  \"\n")
            .expect("user config");
        std::fs::write(&project_config_path, "instructions = \"Project text\"\n")
            .expect("project config");

        let view = read_instructions_view(&user_config_path, Some(&project_config_path))
            .expect("view should load");

        assert!(view.system.contains("Kairox"));
        assert_eq!(view.user.as_deref(), Some("User text"));
        assert_eq!(view.project.as_deref(), Some("Project text"));
    }

    #[test]
    fn upsert_instructions_at_paths_writes_only_selected_scope() {
        let temp = tempfile::tempdir().expect("temp root");
        let user_config_path = temp.path().join("user/config.toml");
        let project_config_path = temp.path().join("project/.kairox/config.toml");

        upsert_instructions_at_paths(
            &InstructionsUpdateInput {
                scope: ConfigScope::User,
                text: "User only".into(),
            },
            &user_config_path,
            Some(&project_config_path),
        )
        .expect("user update should succeed");

        assert!(std::fs::read_to_string(&user_config_path)
            .expect("user config should exist")
            .contains("instructions = \"User only\""));
        assert!(!project_config_path.exists());

        upsert_instructions_at_paths(
            &InstructionsUpdateInput {
                scope: ConfigScope::Project,
                text: "Project only".into(),
            },
            &user_config_path,
            Some(&project_config_path),
        )
        .expect("project update should succeed");

        assert!(std::fs::read_to_string(&project_config_path)
            .expect("project config should exist")
            .contains("instructions = \"Project only\""));
        assert!(std::fs::read_to_string(&user_config_path)
            .expect("user config should still exist")
            .contains("instructions = \"User only\""));
    }

    #[test]
    fn project_scope_without_project_path_returns_error_context() {
        let temp = tempfile::tempdir().expect("temp root");
        let result = upsert_instructions_at_paths(
            &InstructionsUpdateInput {
                scope: ConfigScope::Project,
                text: "Project text".into(),
            },
            &temp.path().join("user/config.toml"),
            None,
        );

        let error = result.expect_err("project scope should require project path");
        assert!(error.contains("no project config path for project-scoped instructions"));
    }
}
