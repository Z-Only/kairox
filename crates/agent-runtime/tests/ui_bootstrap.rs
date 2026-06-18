use agent_core::{AppFacade, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_runtime::ui_bootstrap::{
    ensure_workspace_session, load_config_with_profiles_overlay, load_user_ui_config,
};
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use std::sync::Mutex;

static ENV_LOCK: Mutex<()> = Mutex::new(());

fn test_config() -> agent_config::Config {
    agent_config::load_from_str(
        r#"
[profiles.fast]
provider = "fake"
model_id = "base-fast"
"#,
        "test.toml",
    )
    .expect("config should parse")
}

#[test]
fn default_home_dir_prefers_kairox_home_over_home() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    let previous_home = std::env::var_os("HOME");
    let previous_kairox_home = std::env::var_os("KAIROX_HOME");
    let home = tempfile::tempdir().expect("home tempdir");
    let kairox_home = tempfile::tempdir().expect("kairox home tempdir");

    std::env::set_var("HOME", home.path());
    std::env::set_var("KAIROX_HOME", kairox_home.path());

    let resolved = agent_runtime::ui_bootstrap::default_home_dir();

    match previous_home {
        Some(value) => std::env::set_var("HOME", value),
        None => std::env::remove_var("HOME"),
    }
    match previous_kairox_home {
        Some(value) => std::env::set_var("KAIROX_HOME", value),
        None => std::env::remove_var("KAIROX_HOME"),
    }

    assert_eq!(resolved, kairox_home.path());
}

#[test]
fn profiles_overlay_adds_missing_profiles_without_replacing_base_profiles() {
    let tmp = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        tmp.path().join("profiles.toml"),
        r#"
[profiles.fast]
provider = "fake"
model_id = "overlay-fast"

[profiles.overlay]
provider = "fake"
model_id = "overlay-model"
"#,
    )
    .expect("profiles fixture should write");

    let loaded =
        load_config_with_profiles_overlay(test_config(), tmp.path()).expect("overlay should load");

    assert_eq!(
        loaded
            .config
            .get_profile("fast")
            .map(|p| p.model_id.as_str()),
        Some("base-fast")
    );
    assert_eq!(
        loaded
            .config
            .get_profile("overlay")
            .map(|p| p.model_id.as_str()),
        Some("overlay-model")
    );
    assert!(loaded.warnings.is_empty());
}

#[test]
fn user_ui_config_skips_project_config_discovered_from_cwd() {
    let _guard = ENV_LOCK.lock().expect("env lock");
    let previous_home = std::env::var_os("HOME");
    let previous_cwd = std::env::current_dir().expect("cwd should be readable");
    let home = tempfile::tempdir().expect("home tempdir");
    let project = tempfile::tempdir().expect("project tempdir");

    std::fs::create_dir_all(home.path().join(".kairox")).expect("create user config dir");
    std::fs::write(
        home.path().join(".kairox/config.toml"),
        r#"
[profiles.user]
provider = "fake"
model_id = "user-model"
"#,
    )
    .expect("write user config");
    std::fs::create_dir_all(project.path().join(".kairox")).expect("create project config dir");
    std::fs::write(
        project.path().join(".kairox/config.toml"),
        r#"
[profiles.project]
provider = "fake"
model_id = "project-model"
"#,
    )
    .expect("write project config");

    std::env::set_var("HOME", home.path());
    std::env::set_current_dir(project.path()).expect("set cwd");

    let loaded = load_user_ui_config(&home.path().join(".kairox"));

    std::env::set_current_dir(previous_cwd).expect("restore cwd");
    match previous_home {
        Some(value) => std::env::set_var("HOME", value),
        None => std::env::remove_var("HOME"),
    }

    assert_eq!(
        loaded
            .config
            .get_profile("user")
            .map(|profile| profile.model_id.as_str()),
        Some("user-model")
    );
    assert!(loaded.config.get_profile("project").is_none());
}

#[tokio::test]
async fn ensure_workspace_session_reuses_most_recent_active_session() {
    let store = SqliteEventStore::in_memory()
        .await
        .expect("in-memory store");
    let runtime = LocalRuntime::new(store, FakeModelClient::new(vec!["ok".into()]));
    let workspace_path = "/tmp/kairox-ui-bootstrap".to_string();

    let first = ensure_workspace_session(&runtime, workspace_path.clone(), "fake".into())
        .await
        .expect("initial workspace session should be created");
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    let second_session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: first.workspace.workspace_id.clone(),
            model_profile: "fake".into(),
            approval_policy: None,
            sandbox_policy: None,
        })
        .await
        .expect("second session should start");

    let restored = ensure_workspace_session(&runtime, workspace_path, "fake".into())
        .await
        .expect("workspace session should restore");

    assert_eq!(restored.session_id, second_session_id);
    assert!(!restored.created_workspace);
    assert!(!restored.created_session);
}
