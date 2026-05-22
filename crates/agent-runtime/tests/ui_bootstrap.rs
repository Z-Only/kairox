use agent_core::{AppFacade, StartSessionRequest};
use agent_models::FakeModelClient;
use agent_runtime::ui_bootstrap::{ensure_workspace_session, load_config_with_profiles_overlay};
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;

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

#[tokio::test]
async fn ensure_workspace_session_reuses_most_recent_active_session() {
    let store = SqliteEventStore::in_memory()
        .await
        .expect("in-memory store");
    let runtime = LocalRuntime::new(store, FakeModelClient::new(vec!["ok".into()]));
    let workspace_path = "/tmp/kairox-ui-bootstrap".to_string();

    let first = ensure_workspace_session(&runtime, workspace_path.clone(), "fake".into(), None)
        .await
        .expect("initial workspace session should be created");
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    let second_session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: first.workspace.workspace_id.clone(),
            model_profile: "fake".into(),
            permission_mode: None,
        })
        .await
        .expect("second session should start");

    let restored = ensure_workspace_session(&runtime, workspace_path, "fake".into(), None)
        .await
        .expect("workspace session should restore");

    assert_eq!(restored.session_id, second_session_id);
    assert!(!restored.created_workspace);
    assert!(!restored.created_session);
}
