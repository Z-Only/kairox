mod app_state;
mod commands;
mod event_forwarder;

#[cfg(not(test))]
pub fn run() {
    tauri::Builder::default()
        .invoke_handler(tauri::generate_handler![commands::list_profiles])
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
