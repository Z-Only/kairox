mod app_state;
mod commands;
mod event_forwarder;

#[cfg(not(test))]
use app_state::GuiState;

use agent_models::FakeModelClient;
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use agent_tools::PermissionMode;

pub fn build_runtime() -> Result<LocalRuntime<SqliteEventStore, FakeModelClient>, String> {
    let tokio_rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("Failed to create tokio runtime: {e}"))?;

    let runtime: Result<LocalRuntime<SqliteEventStore, FakeModelClient>, String> = tokio_rt
        .block_on(async {
            let store = SqliteEventStore::in_memory()
                .await
                .map_err(|e| format!("Failed to create in-memory store: {e}"))?;
            let model = FakeModelClient::new(vec!["hello from Kairox".into()]);
            let cwd =
                std::env::current_dir().map_err(|e| format!("Cannot get current dir: {e}"))?;

            let runtime = LocalRuntime::new(store, model)
                .with_permission_mode(PermissionMode::Suggest)
                .with_context_limit(100_000)
                .with_builtin_tools(cwd)
                .await;

            Ok(runtime)
        });

    runtime
}

#[cfg(not(test))]
pub fn run() {
    let runtime = build_runtime().expect("failed to build runtime");

    tauri::Builder::default()
        .manage(GuiState::new(runtime))
        .invoke_handler(tauri::generate_handler![
            commands::list_profiles,
            commands::initialize_workspace,
            commands::start_session,
            commands::send_message,
            commands::switch_session,
            commands::list_sessions,
        ])
        .run(tauri::generate_context!())
        .expect("failed to run tauri application");
}

#[cfg(test)]
pub fn run() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detect_profiles_always_includes_fake() {
        assert!(commands::detect_profiles().contains(&"fake".to_string()));
    }

    #[test]
    fn choose_default_profile_prefers_fast() {
        let profiles = vec![
            "fast".to_string(),
            "local-code".to_string(),
            "fake".to_string(),
        ];
        assert_eq!(commands::choose_default_profile(&profiles), "fast");
    }

    #[test]
    fn choose_default_profile_falls_back_to_local_code() {
        let profiles = vec!["local-code".to_string(), "fake".to_string()];
        assert_eq!(commands::choose_default_profile(&profiles), "local-code");
    }

    #[test]
    fn choose_default_profile_falls_back_to_fake() {
        let profiles = vec!["fake".to_string()];
        assert_eq!(commands::choose_default_profile(&profiles), "fake");
    }
}
