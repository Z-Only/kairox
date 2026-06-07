use super::*;
use agent_core::facade::AgentsFacade;
use agent_models::FakeModelClient;
use agent_store::SqliteEventStore;

async fn build_runtime() -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec![]);
    LocalRuntime::new(store, model)
}

// --- normalize_project_root tests ---

#[test]
fn normalize_project_root_none_returns_none() {
    assert_eq!(normalize_project_root(None), None);
}

#[test]
fn normalize_project_root_empty_string_returns_none() {
    assert_eq!(normalize_project_root(Some("")), None);
}

#[test]
fn normalize_project_root_whitespace_only_returns_none() {
    assert_eq!(normalize_project_root(Some("   ")), None);
}

#[test]
fn normalize_project_root_valid_path_returns_pathbuf() {
    let result = normalize_project_root(Some("/path/to/project"));
    assert_eq!(result, Some(PathBuf::from("/path/to/project")));
}

#[test]
fn normalize_project_root_trims_whitespace() {
    let result = normalize_project_root(Some("  /path/to/project  "));
    assert_eq!(result, Some(PathBuf::from("/path/to/project")));
}

// --- AgentsFacade trait delegation tests ---

#[tokio::test]
async fn list_agent_settings_via_trait_does_not_panic() {
    let runtime = build_runtime().await;
    // With default (empty) roots, listing should return Ok (possibly empty vec)
    let result = AgentsFacade::list_agent_settings(&runtime).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn open_agents_dir_via_trait_returns_user_root_or_none() {
    let runtime = build_runtime().await;
    // Default runtime has no user_root configured, so should return Ok(None)
    let result = AgentsFacade::open_agents_dir(&runtime).await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), None);
}

// --- _for_project method tests ---

#[tokio::test]
async fn open_agents_dir_for_project_none_delegates_to_user_dir() {
    let runtime = build_runtime().await;
    let result = runtime.open_agents_dir_for_project(None).await;
    assert!(result.is_ok());
    // With default roots (no user_root), should return None
    assert_eq!(result.unwrap(), None);
}

#[tokio::test]
async fn open_agents_dir_for_project_with_path_returns_workspace_root() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec![]);
    let mut runtime = LocalRuntime::new(store, model);

    let tmp = tempfile::tempdir().unwrap();
    let workspace_path = tmp.path().join(".kairox").join("agents");
    std::fs::create_dir_all(&workspace_path).unwrap();

    runtime.agent_settings_roots = crate::agent_settings::AgentSettingsRoots {
        user_root: None,
        workspace_root: Some(workspace_path.clone()),
        builtin_root: None,
    };

    let result = runtime
        .open_agents_dir_for_project(Some(tmp.path().display().to_string()))
        .await;
    assert!(result.is_ok());
    // workspace_root is set, so it should return that path
    assert_eq!(result.unwrap(), Some(workspace_path.display().to_string()));
}

#[tokio::test]
async fn list_agent_settings_for_project_none_does_not_panic() {
    let runtime = build_runtime().await;
    let result = runtime.list_agent_settings_for_project(None).await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn open_agents_dir_for_project_empty_string_delegates_to_user_dir() {
    let runtime = build_runtime().await;
    // Empty string normalizes to None, so should delegate to user dir
    let result = runtime
        .open_agents_dir_for_project(Some(String::new()))
        .await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), None);
}

#[tokio::test]
async fn open_user_agents_dir_with_configured_root_returns_path() {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec![]);
    let mut runtime = LocalRuntime::new(store, model);

    let tmp = tempfile::tempdir().unwrap();
    let user_root = tmp.path().join("user-agents");
    std::fs::create_dir_all(&user_root).unwrap();

    runtime.agent_settings_roots = crate::agent_settings::AgentSettingsRoots {
        user_root: Some(user_root.clone()),
        workspace_root: None,
        builtin_root: None,
    };

    let result = runtime.open_user_agents_dir().await;
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), Some(user_root.display().to_string()));
}
