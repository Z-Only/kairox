use super::*;
use agent_core::AppFacade;
use agent_models::FakeModelClient;
use agent_store::SqliteEventStore;
use std::sync::Arc;

fn three_profiles_toml() -> &'static str {
    r#"
[profiles.alpha]
provider = "fake"
model_id = "alpha"

[profiles.bravo]
provider = "fake"
model_id = "bravo"

[profiles.charlie]
provider = "fake"
model_id = "charlie"
"#
}

#[tokio::test]
async fn move_profile_in_order_uses_current_display_order_for_unordered_profiles() {
    let config_dir = tempfile::tempdir().expect("config dir");
    std::fs::write(
        config_dir.path().join("profiles.toml"),
        three_profiles_toml(),
    )
    .expect("profiles.toml should be written");
    let store = SqliteEventStore::in_memory()
        .await
        .expect("in-memory store");
    let runtime = LocalRuntime::new(store, FakeModelClient::new(vec!["ok".into()]))
        .with_config(Arc::new(agent_config::Config::defaults()))
        .with_marketplace(config_dir.path().to_path_buf())
        .expect("marketplace wiring");

    runtime
        .move_profile_in_order("charlie".into(), -1)
        .await
        .expect("profile should move up");

    let aliases = AppFacade::list_profile_settings(&runtime, None)
        .await
        .expect("profiles should list")
        .into_iter()
        .map(|profile| profile.alias)
        .filter(|alias| ["alpha", "bravo", "charlie"].contains(&alias.as_str()))
        .collect::<Vec<_>>();

    assert_eq!(aliases, vec!["alpha", "charlie", "bravo"]);
}
