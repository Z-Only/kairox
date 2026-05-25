use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ConnectivityTestResult {
    pub ok: bool,
    pub error: Option<String>,
}

#[tauri::command]
#[specta::specta]
pub async fn test_url_connectivity(url: String) -> Result<ConnectivityTestResult, String> {
    let trimmed = url.trim().to_string();
    if trimmed.is_empty() {
        return Ok(ConnectivityTestResult {
            ok: false,
            error: Some("no URL provided".into()),
        });
    }

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .map_err(|e| e.to_string())?;

    // Try the URL directly and also /models for common API endpoints.
    let endpoints = [
        trimmed.clone(),
        format!("{}/models", trimmed.trim_end_matches('/')),
    ];

    let mut last_error: Option<String> = None;
    for endpoint in &endpoints {
        match client.get(endpoint).send().await {
            Ok(response) => {
                if response.status().is_success() || response.status().is_client_error() {
                    return Ok(ConnectivityTestResult {
                        ok: true,
                        error: None,
                    });
                }
                last_error = Some(format!("unexpected status: {}", response.status()));
            }
            Err(e) => {
                last_error = Some(format!("connection failed: {e}"));
            }
        }
    }

    Ok(ConnectivityTestResult {
        ok: false,
        error: last_error,
    })
}

#[tauri::command]
#[specta::specta]
pub async fn open_config_dir(state: State<'_, GuiState>) -> Result<Option<String>, String> {
    let Some(config_dir) = state
        .runtime
        .open_config_dir()
        .await
        .map_err(|error| error.to_string())?
    else {
        return Ok(None);
    };
    let config_dir = std::path::PathBuf::from(config_dir);
    open_path_in_system_file_manager(&config_dir)?;
    Ok(Some(config_dir.display().to_string()))
}

#[tauri::command]
#[specta::specta]
pub async fn open_profiles_config_file(
    state: State<'_, GuiState>,
) -> Result<Option<String>, String> {
    let Some(config_file_path) = state
        .runtime
        .open_profiles_config_file()
        .await
        .map_err(|error| error.to_string())?
    else {
        return Ok(None);
    };

    let config_file_path = std::path::PathBuf::from(config_file_path);
    open_path_in_system_file_manager(&config_file_path)?;
    Ok(Some(config_file_path.display().to_string()))
}

#[tauri::command]
#[specta::specta]
pub async fn open_skills_dir(state: State<'_, GuiState>) -> Result<Option<String>, String> {
    let Some(skills_dir) = state
        .runtime
        .open_skills_dir()
        .await
        .map_err(|error| error.to_string())?
    else {
        return Ok(None);
    };
    let skills_dir = std::path::PathBuf::from(skills_dir);
    open_path_in_system_file_manager(&skills_dir)?;
    Ok(Some(skills_dir.display().to_string()))
}

pub(super) fn open_path_in_system_file_manager(path: &std::path::Path) -> Result<(), String> {
    let mut command = system_file_manager_command(path);
    let status = command
        .status()
        .map_err(|error| format!("failed to open {}: {error}", path.display()))?;

    if status.success() {
        return Ok(());
    }

    Err(format!(
        "failed to open {}: system opener exited with {status}",
        path.display()
    ))
}

#[cfg(target_os = "macos")]
fn system_file_manager_command(path: &std::path::Path) -> std::process::Command {
    let mut command = std::process::Command::new("open");
    command.arg(path);
    command
}

#[cfg(target_os = "windows")]
fn system_file_manager_command(path: &std::path::Path) -> std::process::Command {
    let mut command = std::process::Command::new("explorer");
    command.arg(path);
    command
}

#[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
fn system_file_manager_command(path: &std::path::Path) -> std::process::Command {
    let mut command = std::process::Command::new("xdg-open");
    command.arg(path);
    command
}

#[tauri::command]
#[specta::specta]
pub async fn refresh_config(state: State<'_, GuiState>) -> Result<(), String> {
    state.refresh_user_config()?;
    eprintln!(
        "User config refreshed: profiles={:?}",
        state.config.read().unwrap().profile_names()
    );
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn refresh_config_for_project(
    project_root: String,
    state: State<'_, GuiState>,
) -> Result<(), String> {
    let path = std::path::Path::new(&project_root);
    state.refresh_config_for_project(path)?;
    eprintln!(
        "Config refreshed for project: profiles={:?}",
        state.config.read().unwrap().profile_names()
    );
    Ok(())
}
