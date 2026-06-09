use super::*;

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
pub struct ConnectivityTestResult {
    pub ok: bool,
    pub error: Option<String>,
    pub status: String,
    pub message: String,
    pub response_preview: Option<String>,
}

impl ConnectivityTestResult {
    pub(super) fn chat_ready(subject: &str, response_preview: Option<String>) -> Self {
        Self {
            ok: true,
            error: None,
            status: "chat_ready".into(),
            message: format!("{subject} is ready to chat."),
            response_preview,
        }
    }

    pub(super) fn endpoint_reachable(subject: &str) -> Self {
        Self {
            ok: true,
            error: None,
            status: "endpoint_reachable".into(),
            message: format!("{subject} endpoint is reachable."),
            response_preview: None,
        }
    }

    pub(super) fn failed(status: &str, subject: &str, detail: impl Into<String>) -> Self {
        let detail = detail.into();
        Self {
            ok: false,
            error: Some(detail.clone()),
            status: status.into(),
            message: failure_message(status, subject, &detail),
            response_preview: None,
        }
    }
}

#[tauri::command]
#[specta::specta]
pub async fn test_url_connectivity(url: String) -> Result<ConnectivityTestResult, String> {
    let trimmed = url.trim().to_string();
    if trimmed.is_empty() {
        return Ok(ConnectivityTestResult::failed(
            "invalid_config",
            "Endpoint",
            "no URL provided",
        ));
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
                let status = response.status();
                if status.is_success() {
                    return Ok(ConnectivityTestResult::endpoint_reachable(endpoint));
                }
                if status.is_client_error() {
                    let body = response.text().await.unwrap_or_default();
                    if status.as_u16() == 404 && endpoint == &trimmed {
                        last_error = Some(format!("unexpected status: {status}: {body}"));
                        continue;
                    }
                    if status.as_u16() == 400 || status.as_u16() == 405 {
                        return Ok(ConnectivityTestResult::endpoint_reachable(endpoint));
                    }
                    return Ok(classify_http_failure(status.as_u16(), &body, endpoint));
                }
                let body = response.text().await.unwrap_or_default();
                return Ok(classify_http_failure(status.as_u16(), &body, endpoint));
            }
            Err(e) => {
                last_error = Some(format!("connection failed: {e}"));
            }
        }
    }

    Ok(ConnectivityTestResult::failed(
        "network_error",
        &trimmed,
        last_error.unwrap_or_else(|| "connection failed".into()),
    ))
}

pub(super) fn classify_http_failure(
    status: u16,
    body: &str,
    subject: &str,
) -> ConnectivityTestResult {
    let normalized = body.to_ascii_lowercase();
    let code_status = if status == 401 {
        "auth_failed"
    } else if status == 402 || body_indicates_quota_or_plan_limit(body) {
        "quota_or_plan_blocked"
    } else if status == 403 || normalized.contains("permission") || normalized.contains("forbidden")
    {
        "permission_denied"
    } else if status == 404 || normalized.contains("model_not_found") {
        "model_unavailable"
    } else if status == 429 {
        "rate_limited"
    } else if status >= 500 {
        "server_error"
    } else {
        "request_failed"
    };
    let detail = if body.trim().is_empty() {
        format!("api error (status {status})")
    } else {
        format!("api error (status {status}): {}", body.trim())
    };
    ConnectivityTestResult::failed(code_status, subject, detail)
}

fn body_indicates_quota_or_plan_limit(body: &str) -> bool {
    let normalized = body.to_ascii_lowercase();
    let english_terms = [
        "insufficient_quota",
        "quota",
        "billing",
        "plan",
        "balance",
        "credit",
        "subscription",
        "limit exceeded",
    ];
    let chinese_terms = [
        "套餐",
        "计划",
        "超限",
        "限额",
        "额度",
        "余额",
        "账单",
        "计费",
        "订阅",
        "仅限",
        "已达上限",
    ];

    english_terms.iter().any(|term| normalized.contains(term))
        || chinese_terms.iter().any(|term| body.contains(term))
}

pub(super) fn classify_model_error(
    error: &agent_models::ModelError,
    subject: &str,
) -> ConnectivityTestResult {
    match error {
        agent_models::ModelError::Connection(message) => {
            ConnectivityTestResult::failed("network_error", subject, message)
        }
        agent_models::ModelError::Request(message) => {
            ConnectivityTestResult::failed("invalid_config", subject, message)
        }
        agent_models::ModelError::StreamParse(message) => {
            ConnectivityTestResult::failed("request_failed", subject, message)
        }
        agent_models::ModelError::Http { status, message }
        | agent_models::ModelError::Api { status, message } => {
            classify_http_failure(*status, message, subject)
        }
    }
}

pub(super) fn classify_provider_failure_message(
    message: impl Into<String>,
    subject: &str,
) -> ConnectivityTestResult {
    let message = message.into();
    if body_indicates_quota_or_plan_limit(&message) {
        ConnectivityTestResult::failed("quota_or_plan_blocked", subject, message)
    } else {
        ConnectivityTestResult::failed("request_failed", subject, message)
    }
}

fn failure_message(status: &str, subject: &str, detail: &str) -> String {
    match status {
        "auth_failed" => format!("{subject} authentication failed. Check the API key."),
        "quota_or_plan_blocked" => {
            format!("{subject} endpoint is reachable, but chat is blocked by quota or plan limits.")
        }
        "rate_limited" => format!("{subject} is rate limited. Try again later."),
        "permission_denied" => {
            format!("{subject} endpoint is reachable, but this key does not have permission.")
        }
        "model_unavailable" => format!("{subject} model is unavailable or not found."),
        "server_error" => format!("{subject} server returned an error. Try again later."),
        "empty_response" => {
            format!("{subject} returned no chat output. Check model availability, quota, or plan.")
        }
        "network_error" => format!("{subject} connection failed: {detail}"),
        "invalid_config" => format!("{subject} configuration is invalid: {detail}"),
        _ => format!("{subject} connectivity test failed: {detail}"),
    }
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

/// Open the config file for the given scope ("user" or "project").
///
/// - `"user"` → `~/.kairox/config.toml`
/// - `"project"` → `<project_root>/.kairox/config.toml`
///
/// Creates the file (with an empty `[profiles]` section) if it doesn't exist.
#[tauri::command]
#[specta::specta]
pub async fn open_config_file_for_scope(
    scope: String,
    project_root: Option<String>,
) -> Result<Option<String>, String> {
    let config_path = match scope.as_str() {
        "project" => {
            let root = project_root
                .ok_or_else(|| "project_root is required when scope is \"project\"".to_string())?;
            std::path::PathBuf::from(root)
                .join(".kairox")
                .join("config.toml")
        }
        _ => {
            // user scope
            let home = std::env::var("HOME")
                .map_err(|_| "HOME environment variable not set".to_string())?;
            std::path::PathBuf::from(home)
                .join(".kairox")
                .join("config.toml")
        }
    };

    // Ensure the parent directory and file exist so the system editor can open it.
    if let Some(parent) = config_path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("failed to create config directory: {e}"))?;
        }
    }
    if !config_path.exists() {
        std::fs::write(&config_path, "# Kairox configuration\n# See kairox.toml.example for available options.\n\n[profiles]\n")
            .map_err(|e| format!("failed to create config file: {e}"))?;
    }

    open_path_in_system_file_manager(&config_path)?;
    Ok(Some(config_path.display().to_string()))
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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};

    async fn serve_once(status_line: &'static str, body: &'static str) -> String {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            let (mut socket, _) = listener.accept().await.unwrap();
            let mut request = [0; 1024];
            let _ = socket.read(&mut request).await.unwrap();
            let response = format!(
                "HTTP/1.1 {status_line}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        });
        format!("http://{addr}")
    }

    #[tokio::test]
    async fn url_connectivity_treats_quota_errors_as_failed() {
        let url = serve_once(
            "429 Too Many Requests",
            r#"{"error":{"code":"insufficient_quota","message":"quota exceeded"}}"#,
        )
        .await;

        let result = test_url_connectivity(url).await.unwrap();

        assert!(!result.ok);
        assert_eq!(result.status, "quota_or_plan_blocked");
        assert!(result.error.unwrap().contains("quota"));
    }

    #[test]
    fn http_failure_classifies_chinese_plan_limit_errors() {
        let result = classify_http_failure(403, "opus计划仅限在claude code中使用", "Model ali-mo");

        assert!(!result.ok);
        assert_eq!(result.status, "quota_or_plan_blocked");
        assert!(result.message.contains("blocked by quota or plan"));
    }

    #[test]
    fn provider_failure_message_classifies_plan_limit_errors() {
        let result =
            classify_provider_failure_message("opus计划仅限在claude code中使用", "Model ali-mo");

        assert!(!result.ok);
        assert_eq!(result.status, "quota_or_plan_blocked");
        assert_eq!(
            result.message,
            "Model ali-mo endpoint is reachable, but chat is blocked by quota or plan limits."
        );
    }
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
