use super::*;
use agent_core::facade::{InstructionsUpdateInput, InstructionsView};

#[tauri::command]
#[specta::specta]
pub async fn get_instructions(
    _state: State<'_, GuiState>,
    _scope: agent_core::ConfigScope,
    project_root: Option<String>,
) -> Result<InstructionsView, String> {
    let user_config_path = user_config_dir().join("config.toml");
    let user_instructions =
        agent_runtime::instructions_settings::read_instructions(&user_config_path)
            .map_err(|e| e.to_string())?;

    let project_instructions = if let Some(ref root) = project_root {
        let project_config_path = std::path::PathBuf::from(root)
            .join(".kairox")
            .join("config.toml");
        agent_runtime::instructions_settings::read_instructions(&project_config_path)
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

#[tauri::command]
#[specta::specta]
pub async fn upsert_instructions(
    state: State<'_, GuiState>,
    input: InstructionsUpdateInput,
    project_root: Option<String>,
) -> Result<(), String> {
    let user_config_path = user_config_dir().join("config.toml");

    let project_config_path = project_root.as_ref().map(|r| {
        std::path::PathBuf::from(r)
            .join(".kairox")
            .join("config.toml")
    });

    agent_runtime::instructions_settings::upsert_instructions(
        &input,
        &user_config_path,
        project_config_path.as_deref(),
    )
    .map_err(|e| e.to_string())?;

    // Refresh config so the runtime picks up the new instructions.
    state.refresh_config()?;

    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn get_system_prompt() -> Result<String, String> {
    Ok(agent_runtime::instructions_settings::get_system_prompt())
}

fn user_config_dir() -> std::path::PathBuf {
    std::env::var("HOME")
        .ok()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."))
        .join(".kairox")
}
