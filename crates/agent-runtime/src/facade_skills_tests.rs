use crate::facade_runtime::LocalRuntime;
use agent_core::facade::SkillsFacade;
use agent_core::{ActivateSkillRequest, DeactivateSkillRequest, SessionId, WorkspaceId};
use agent_models::FakeModelClient;
use agent_store::SqliteEventStore;

async fn build_runtime() -> LocalRuntime<SqliteEventStore, FakeModelClient> {
    let store = SqliteEventStore::in_memory().await.unwrap();
    let model = FakeModelClient::new(vec![]);
    LocalRuntime::new(store, model)
}

// ---------------------------------------------------------------------------
// list_skills / list_skills_with_roots
// ---------------------------------------------------------------------------

#[tokio::test]
async fn list_skills_no_registry_returns_empty() {
    let runtime = build_runtime().await;
    let skills = runtime.list_skills().await.unwrap();
    assert!(skills.is_empty());
}

#[tokio::test]
async fn list_skills_with_roots_no_registry_returns_empty() {
    let runtime = build_runtime().await;
    let roots = crate::skill_settings::SkillSettingsRoots::default();
    let skills = runtime.list_skills_with_roots(roots).await.unwrap();
    assert!(skills.is_empty());
}

// ---------------------------------------------------------------------------
// get_skill / get_skill_with_roots
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_skill_no_registry_returns_none() {
    let runtime = build_runtime().await;
    let result = runtime.get_skill("nonexistent".into()).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn get_skill_with_roots_no_registry_returns_none() {
    let runtime = build_runtime().await;
    let roots = crate::skill_settings::SkillSettingsRoots::default();
    let result = runtime
        .get_skill_with_roots(roots, "nonexistent".into())
        .await
        .unwrap();
    assert!(result.is_none());
}

// ---------------------------------------------------------------------------
// activate_skill / activate_skill_with_roots
// ---------------------------------------------------------------------------

#[tokio::test]
async fn activate_skill_no_registry_returns_invalid_state() {
    let runtime = build_runtime().await;
    let request = ActivateSkillRequest {
        workspace_id: WorkspaceId::new(),
        session_id: SessionId::new(),
        skill_id: "some-skill".into(),
    };
    let result = runtime.activate_skill(request).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("skill registry not configured"), "got: {err}");
}

#[tokio::test]
async fn activate_skill_with_roots_no_registry_returns_invalid_state() {
    let runtime = build_runtime().await;
    let roots = crate::skill_settings::SkillSettingsRoots::default();
    let request = ActivateSkillRequest {
        workspace_id: WorkspaceId::new(),
        session_id: SessionId::new(),
        skill_id: "some-skill".into(),
    };
    let result = runtime.activate_skill_with_roots(roots, request).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("skill registry not configured"), "got: {err}");
}

// ---------------------------------------------------------------------------
// deactivate_skill / deactivate_skill_with_roots
// ---------------------------------------------------------------------------

#[tokio::test]
async fn deactivate_skill_no_registry_returns_invalid_state() {
    let runtime = build_runtime().await;
    let request = DeactivateSkillRequest {
        workspace_id: WorkspaceId::new(),
        session_id: SessionId::new(),
        skill_id: "some-skill".into(),
    };
    let result = runtime.deactivate_skill(request).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("skill registry not configured"), "got: {err}");
}

#[tokio::test]
async fn deactivate_skill_with_roots_no_registry_returns_invalid_state() {
    let runtime = build_runtime().await;
    let roots = crate::skill_settings::SkillSettingsRoots::default();
    let request = DeactivateSkillRequest {
        workspace_id: WorkspaceId::new(),
        session_id: SessionId::new(),
        skill_id: "some-skill".into(),
    };
    let result = runtime.deactivate_skill_with_roots(roots, request).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("skill registry not configured"), "got: {err}");
}

// ---------------------------------------------------------------------------
// list_active_skills / list_active_skills_with_roots
// ---------------------------------------------------------------------------

#[tokio::test]
async fn list_active_skills_no_registry_returns_empty() {
    let runtime = build_runtime().await;
    let session_id = SessionId::new();
    let skills = runtime.list_active_skills(session_id).await.unwrap();
    assert!(skills.is_empty());
}

#[tokio::test]
async fn list_active_skills_with_roots_no_registry_returns_empty() {
    let runtime = build_runtime().await;
    let roots = crate::skill_settings::SkillSettingsRoots::default();
    let session_id = SessionId::new();
    let skills = runtime
        .list_active_skills_with_roots(roots, session_id)
        .await
        .unwrap();
    assert!(skills.is_empty());
}

// ---------------------------------------------------------------------------
// list_skill_settings
// ---------------------------------------------------------------------------

#[tokio::test]
async fn list_skill_settings_default_roots_returns_empty() {
    let runtime = build_runtime().await;
    let settings = runtime.list_skill_settings().await.unwrap();
    assert!(settings.is_empty());
}

// ---------------------------------------------------------------------------
// get_skill_settings_detail
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_skill_settings_detail_returns_none() {
    let runtime = build_runtime().await;
    let detail = runtime
        .get_skill_settings_detail("nonexistent".into())
        .await
        .unwrap();
    assert!(detail.is_none());
}

// ---------------------------------------------------------------------------
// set_skill_enabled
// ---------------------------------------------------------------------------

#[tokio::test]
async fn set_skill_enabled_no_config_returns_error() {
    let runtime = build_runtime().await;
    let result = runtime.set_skill_enabled("nonexistent".into(), true).await;
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// delete_skill_settings
// ---------------------------------------------------------------------------

#[tokio::test]
async fn delete_skill_settings_no_config_returns_error() {
    let runtime = build_runtime().await;
    let result = runtime.delete_skill_settings("nonexistent".into()).await;
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// search_remote_skills
// ---------------------------------------------------------------------------

#[tokio::test]
async fn search_remote_skills_default_package_manager() {
    let runtime = build_runtime().await;
    let results = runtime
        .search_remote_skills("anything".into())
        .await
        .unwrap();
    assert!(results.is_empty());
}

// ---------------------------------------------------------------------------
// list_skill_catalog
// ---------------------------------------------------------------------------

#[tokio::test]
async fn list_skill_catalog_no_catalog_returns_invalid_state() {
    let runtime = build_runtime().await;
    let query = agent_core::facade::SkillCatalogQuery {
        keyword: None,
        sources: None,
        limit: None,
    };
    let result = runtime.list_skill_catalog(query).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("skill catalog not configured"), "got: {err}");
}

// ---------------------------------------------------------------------------
// list_skill_sources
// ---------------------------------------------------------------------------

#[tokio::test]
async fn list_skill_sources_no_toml_returns_defaults() {
    let runtime = build_runtime().await;
    let sources = runtime.list_skill_sources().await.unwrap();
    assert!(!sources.is_empty(), "default sources should be non-empty");
}

// ---------------------------------------------------------------------------
// open_skills_dir
// ---------------------------------------------------------------------------

#[tokio::test]
async fn open_skills_dir_returns_path_string() {
    let runtime = build_runtime().await;
    let path = runtime.open_skills_dir().await.unwrap();
    assert!(path.is_some());
    let path_str = path.unwrap();
    assert!(path_str.contains("skills"), "got: {path_str}");
}

// ---------------------------------------------------------------------------
// refresh_skill_catalog
// ---------------------------------------------------------------------------

#[tokio::test]
async fn refresh_skill_catalog_no_catalog_returns_invalid_state() {
    let runtime = build_runtime().await;
    let result = runtime.refresh_skill_catalog().await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("skill catalog not configured"), "got: {err}");
}

// ---------------------------------------------------------------------------
// skill_settings_roots_for_session
// ---------------------------------------------------------------------------

#[tokio::test]
async fn skill_settings_roots_for_session_returns_default() {
    let runtime = build_runtime().await;
    let session_id = SessionId::new();
    let roots = runtime.skill_settings_roots_for_session(&session_id).await;
    assert_eq!(roots, crate::skill_settings::SkillSettingsRoots::default());
}

// ---------------------------------------------------------------------------
// skill_registry_for_roots
// ---------------------------------------------------------------------------

#[tokio::test]
async fn skill_registry_for_roots_no_registry_returns_none() {
    let runtime = build_runtime().await;
    let roots = crate::skill_settings::SkillSettingsRoots::default();
    let registry = runtime.skill_registry_for_roots(roots).await.unwrap();
    assert!(registry.is_none());
}

// ---------------------------------------------------------------------------
// SkillsFacade trait delegation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn trait_list_skills_delegates_to_inherent() {
    let runtime = build_runtime().await;
    let facade: &dyn SkillsFacade = &runtime;
    let skills = facade.list_skills().await.unwrap();
    assert!(skills.is_empty());
}

#[tokio::test]
async fn trait_get_skill_delegates_to_inherent() {
    let runtime = build_runtime().await;
    let facade: &dyn SkillsFacade = &runtime;
    let result = facade.get_skill("nonexistent".into()).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn trait_activate_skill_delegates_to_inherent() {
    let runtime = build_runtime().await;
    let facade: &dyn SkillsFacade = &runtime;
    let request = ActivateSkillRequest {
        workspace_id: WorkspaceId::new(),
        session_id: SessionId::new(),
        skill_id: "some-skill".into(),
    };
    let result = facade.activate_skill(request).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn trait_list_active_skills_delegates_to_inherent() {
    let runtime = build_runtime().await;
    let facade: &dyn SkillsFacade = &runtime;
    let skills = facade.list_active_skills(SessionId::new()).await.unwrap();
    assert!(skills.is_empty());
}
