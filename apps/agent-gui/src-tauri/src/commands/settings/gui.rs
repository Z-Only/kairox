use super::*;
use std::path::{Path, PathBuf};
use tauri::Manager;

const GUI_SETTINGS_FILENAME: &str = "gui-settings.toml";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, specta::Type)]
pub struct GuiSettingsView {
    pub devtools_enabled: bool,
    pub default_devtools_enabled: bool,
    pub requires_restart: bool,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct GuiSettingsFile {
    #[serde(default)]
    devtools_enabled: Option<bool>,
}

pub fn default_devtools_enabled_for(debug_assertions: bool) -> bool {
    debug_assertions
}

pub fn default_devtools_enabled() -> bool {
    default_devtools_enabled_for(cfg!(debug_assertions))
}

pub fn gui_settings_path(data_dir: &Path) -> PathBuf {
    data_dir.join(GUI_SETTINGS_FILENAME)
}

pub fn read_gui_settings(
    data_dir: &Path,
    debug_assertions: bool,
    running_devtools_enabled: Option<bool>,
) -> Result<GuiSettingsView, String> {
    let default_devtools_enabled = default_devtools_enabled_for(debug_assertions);
    let path = gui_settings_path(data_dir);
    let settings_file = match std::fs::read_to_string(&path) {
        Ok(raw) => toml::from_str::<GuiSettingsFile>(&raw)
            .map_err(|error| format!("failed to parse {}: {error}", path.display()))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => GuiSettingsFile::default(),
        Err(error) => return Err(format!("failed to read {}: {error}", path.display())),
    };

    Ok(GuiSettingsView {
        devtools_enabled: settings_file
            .devtools_enabled
            .unwrap_or(default_devtools_enabled),
        default_devtools_enabled,
        requires_restart: running_devtools_enabled.is_some_and(|running| {
            running
                != settings_file
                    .devtools_enabled
                    .unwrap_or(default_devtools_enabled)
        }),
    })
}

pub fn write_gui_devtools_enabled(
    data_dir: &Path,
    enabled: bool,
    debug_assertions: bool,
    running_devtools_enabled: Option<bool>,
) -> Result<GuiSettingsView, String> {
    std::fs::create_dir_all(data_dir)
        .map_err(|error| format!("failed to create {}: {error}", data_dir.display()))?;
    let settings_file = GuiSettingsFile {
        devtools_enabled: Some(enabled),
    };
    let raw = toml::to_string_pretty(&settings_file)
        .map_err(|error| format!("failed to serialize GUI settings: {error}"))?;
    let path = gui_settings_path(data_dir);
    std::fs::write(&path, raw)
        .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    read_gui_settings(data_dir, debug_assertions, running_devtools_enabled)
}

#[tauri::command]
#[specta::specta]
pub async fn get_gui_settings(state: State<'_, GuiState>) -> Result<GuiSettingsView, String> {
    read_gui_settings(
        &state.home_dir,
        cfg!(debug_assertions),
        Some(state.devtools_enabled_at_startup),
    )
}

#[tauri::command]
#[specta::specta]
pub async fn set_gui_devtools_enabled(
    app: tauri::AppHandle,
    state: State<'_, GuiState>,
    enabled: bool,
) -> Result<GuiSettingsView, String> {
    let view = write_gui_devtools_enabled(
        &state.home_dir,
        enabled,
        cfg!(debug_assertions),
        Some(state.devtools_enabled_at_startup),
    )?;
    if !enabled {
        if let Some(window) = app.get_webview_window("main") {
            window.close_devtools();
        }
    }
    Ok(view)
}

#[cfg(test)]
#[path = "gui_tests.rs"]
mod tests;
