mod app_state;
mod event_forwarder;

mod commands {
    #[tauri::command]
    pub fn list_model_profiles() -> Vec<String> {
        vec![
            "fake".into(),
            "fast".into(),
            "local-code".into(),
            "reviewer".into(),
        ]
    }
}

pub fn default_model_profiles() -> Vec<String> {
    commands::list_model_profiles()
}

#[cfg(not(test))]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::list_model_profiles])
        .run(tauri::generate_context!())
        .expect("failed to run tauri application");
}

#[cfg(test)]
pub fn run() {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exposes_default_profiles() {
        assert!(default_model_profiles().contains(&"fake".to_string()));
    }
}
