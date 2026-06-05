use super::*;
use crate::facade::{SkillFieldMappingView, SkillInstallTarget};

/// A bare struct that implements `SkillsFacade` with no overrides,
/// exercising every default method.
struct BareSkillsFacade;

#[async_trait::async_trait]
impl SkillsFacade for BareSkillsFacade {}

#[tokio::test]
async fn list_skills_returns_empty() {
    let facade = BareSkillsFacade;
    let result = facade.list_skills().await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn get_skill_returns_none() {
    let facade = BareSkillsFacade;
    let result = facade.get_skill("review".into()).await.unwrap();
    assert_eq!(result, None);
}

#[tokio::test]
async fn activate_skill_returns_error() {
    let facade = BareSkillsFacade;
    let req = ActivateSkillRequest {
        workspace_id: crate::WorkspaceId::new(),
        session_id: crate::SessionId::new(),
        skill_id: "review".into(),
    };
    let err = facade.activate_skill(req).await.unwrap_err();
    assert!(err.to_string().contains("not supported"));
}

#[tokio::test]
async fn deactivate_skill_succeeds() {
    let facade = BareSkillsFacade;
    let req = DeactivateSkillRequest {
        workspace_id: crate::WorkspaceId::new(),
        session_id: crate::SessionId::new(),
        skill_id: "review".into(),
    };
    facade.deactivate_skill(req).await.unwrap();
}

#[tokio::test]
async fn list_active_skills_returns_empty() {
    let facade = BareSkillsFacade;
    let result = facade
        .list_active_skills(crate::SessionId::new())
        .await
        .unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn list_skill_settings_returns_empty() {
    let facade = BareSkillsFacade;
    let result = facade.list_skill_settings().await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn get_skill_settings_detail_returns_none() {
    let facade = BareSkillsFacade;
    let result = facade
        .get_skill_settings_detail("review".into())
        .await
        .unwrap();
    assert_eq!(result, None);
}

#[tokio::test]
async fn set_skill_enabled_returns_error() {
    let facade = BareSkillsFacade;
    let err = facade
        .set_skill_enabled("review".into(), true)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("not supported"));
}

#[tokio::test]
async fn delete_skill_settings_returns_error() {
    let facade = BareSkillsFacade;
    let err = facade
        .delete_skill_settings("review".into())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("not supported"));
}

#[tokio::test]
async fn search_remote_skills_returns_empty() {
    let facade = BareSkillsFacade;
    let result = facade.search_remote_skills("lint".into()).await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn install_remote_skill_returns_error() {
    let facade = BareSkillsFacade;
    let req = InstallRemoteSkillRequest {
        package: "my-skill".into(),
        source: "registry".into(),
        target: SkillInstallTarget::User,
        package_url: None,
    };
    let err = facade.install_remote_skill(req).await.unwrap_err();
    assert!(err.to_string().contains("not supported"));
}

#[tokio::test]
async fn install_github_skill_returns_error() {
    let facade = BareSkillsFacade;
    let req = InstallGithubSkillRequest {
        source: "https://github.com/user/repo".into(),
        target: SkillInstallTarget::Project,
    };
    let err = facade.install_github_skill(req).await.unwrap_err();
    assert!(err.to_string().contains("not supported"));
}

#[tokio::test]
async fn update_skill_returns_error() {
    let facade = BareSkillsFacade;
    let err = facade.update_skill("review".into()).await.unwrap_err();
    assert!(err.to_string().contains("not supported"));
}

#[tokio::test]
async fn list_skill_catalog_returns_empty() {
    let facade = BareSkillsFacade;
    let result = facade
        .list_skill_catalog(SkillCatalogQuery::default())
        .await
        .unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn list_skill_sources_returns_empty() {
    let facade = BareSkillsFacade;
    let result = facade.list_skill_sources().await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn add_skill_source_returns_error() {
    let facade = BareSkillsFacade;
    let config = SkillSourceView {
        id: "custom".into(),
        display_name: "Custom".into(),
        kind: "registry".into(),
        url: "https://example.com".into(),
        search_template: "/search?q={query}".into(),
        download_template: "/download/{id}".into(),
        list_template: None,
        detail_template: None,
        field_mapping: SkillFieldMappingView::default(),
        enabled: true,
        priority: 0,
        cache_ttl_seconds: 3600,
        last_error: None,
    };
    let err = facade.add_skill_source(config).await.unwrap_err();
    assert!(err.to_string().contains("not configured"));
}

#[tokio::test]
async fn remove_skill_source_returns_error() {
    let facade = BareSkillsFacade;
    let err = facade
        .remove_skill_source("custom".into())
        .await
        .unwrap_err();
    assert!(err.to_string().contains("not configured"));
}

#[tokio::test]
async fn set_skill_source_enabled_returns_error() {
    let facade = BareSkillsFacade;
    let err = facade
        .set_skill_source_enabled("custom".into(), false)
        .await
        .unwrap_err();
    assert!(err.to_string().contains("not configured"));
}

#[tokio::test]
async fn refresh_skill_catalog_succeeds() {
    let facade = BareSkillsFacade;
    facade.refresh_skill_catalog().await.unwrap();
}

#[tokio::test]
async fn open_skills_dir_returns_none() {
    let facade = BareSkillsFacade;
    assert_eq!(facade.open_skills_dir().await.unwrap(), None);
}
