mod support;

use std::sync::Arc;

use agent_core::facade::{
    InstallGithubSkillRequest, InstallRemoteSkillRequest, SkillInstallTarget, SkillUpdateState,
};
use agent_core::AppFacade;
use agent_runtime::skill_package::{FakeSkillPackageManager, SkillPackageManager};

use support::skills_helpers::{build_runtime_with_package_manager, remote_result};

#[tokio::test]
async fn search_remote_skills_delegates_to_package_manager() {
    let manager = Arc::new(FakeSkillPackageManager::default());
    let expected = remote_result(
        "code-review",
        "Review code changes",
        "obra/superpowers",
        1200,
    );
    manager.search_results.lock().await.push(expected.clone());

    let runtime = build_runtime_with_package_manager(manager.clone()).await;
    let results = runtime
        .search_remote_skills("review".into())
        .await
        .expect("search should succeed");

    assert_eq!(results, vec![expected]);
    assert_eq!(manager.search_queries.lock().await.as_slice(), ["review"]);
}

#[tokio::test]
async fn search_remote_skills_propagates_package_manager_error() {
    let manager = Arc::new(FakeSkillPackageManager::default());
    *manager.search_error.lock().await = Some("registry unavailable".to_string());

    let runtime = build_runtime_with_package_manager(manager).await;
    let error = runtime
        .search_remote_skills("review".into())
        .await
        .expect_err("search should fail");

    assert!(error.to_string().contains("registry unavailable"));
}

#[tokio::test]
async fn fake_package_manager_records_install_requests() {
    let manager = FakeSkillPackageManager::default();

    let registry_request = InstallRemoteSkillRequest {
        package: "@skills/code-review".into(),
        source: "registry".into(),
        target: SkillInstallTarget::Project,
        package_url: None,
    };
    let github_request = InstallGithubSkillRequest {
        source: "obra/superpowers".into(),
        target: SkillInstallTarget::User,
    };

    let project_root = tempfile::tempdir().expect("project root");
    let user_root = tempfile::tempdir().expect("user root");

    manager
        .install_from_registry(project_root.path(), &registry_request)
        .await
        .expect("registry install should succeed");
    manager
        .install_from_github(user_root.path(), &github_request)
        .await
        .expect("github install should succeed");

    assert_eq!(manager.registry_install_requests.lock().await.len(), 1);
    assert_eq!(
        manager.registry_install_requests.lock().await[0].package,
        "@skills/code-review"
    );
    assert_eq!(manager.github_install_requests.lock().await.len(), 1);
    assert_eq!(
        manager.github_install_requests.lock().await[0].source,
        "obra/superpowers"
    );
    // Verify install roots recorded
    assert_eq!(
        manager.registry_install_roots.lock().await.as_slice(),
        [project_root.path().to_path_buf()]
    );
    assert_eq!(
        manager.github_install_roots.lock().await.as_slice(),
        [user_root.path().to_path_buf()]
    );
}

#[tokio::test]
async fn fake_package_manager_check_updates_states() {
    let manager = FakeSkillPackageManager::default();

    // Default: Unknown
    assert_eq!(
        manager.check_updates("code-review").await.unwrap(),
        SkillUpdateState::Unknown
    );
    assert_eq!(
        manager.check_update_skill_ids.lock().await.as_slice(),
        ["code-review"]
    );

    // UpToDate
    *manager.check_updates_result.lock().await = SkillUpdateState::UpToDate;
    assert_eq!(
        manager.check_updates("code-review").await.unwrap(),
        SkillUpdateState::UpToDate
    );

    // UpdateAvailable
    *manager.check_updates_result.lock().await = SkillUpdateState::UpdateAvailable;
    assert_eq!(
        manager.check_updates("code-review").await.unwrap(),
        SkillUpdateState::UpdateAvailable
    );

    // Verify all calls recorded
    assert_eq!(manager.check_update_skill_ids.lock().await.len(), 3);
}

#[tokio::test]
async fn fake_package_manager_update_records_and_propagates_error() {
    let manager = FakeSkillPackageManager::default();

    // Successful update
    manager
        .update("code-review")
        .await
        .expect("update should succeed");
    assert_eq!(
        manager.update_skill_ids.lock().await.as_slice(),
        ["code-review"]
    );

    // Failed update
    *manager.update_error.lock().await = Some("network timeout".to_string());
    let error = manager
        .update("code-review")
        .await
        .expect_err("update should fail");
    assert!(error.to_string().contains("network timeout"));
    assert_eq!(manager.update_skill_ids.lock().await.len(), 2);
}

#[tokio::test]
async fn fake_package_manager_install_errors_propagate() {
    let manager = FakeSkillPackageManager::default();

    *manager.registry_install_error.lock().await = Some("registry refused install".to_string());
    *manager.github_install_error.lock().await = Some("repository not found".to_string());

    let registry_err = manager
        .install_from_registry(
            tempfile::tempdir().unwrap().path(),
            &InstallRemoteSkillRequest {
                package: "bad-pkg".into(),
                source: "registry".into(),
                target: SkillInstallTarget::User,
                package_url: None,
            },
        )
        .await
        .expect_err("registry install should fail");
    assert!(registry_err
        .to_string()
        .contains("registry refused install"));

    let github_err = manager
        .install_from_github(
            tempfile::tempdir().unwrap().path(),
            &InstallGithubSkillRequest {
                source: "bad/repo".into(),
                target: SkillInstallTarget::Project,
            },
        )
        .await
        .expect_err("github install should fail");
    assert!(github_err.to_string().contains("repository not found"));

    // Requests still recorded despite errors
    assert_eq!(manager.registry_install_requests.lock().await.len(), 1);
    assert_eq!(manager.github_install_requests.lock().await.len(), 1);
}

#[tokio::test]
async fn fake_package_manager_empty_search_returns_empty_vec() {
    let manager = FakeSkillPackageManager::default();
    // No search_results configured — defaults to empty

    let results = manager
        .search("nonexistent")
        .await
        .expect("search should not error for empty results");
    assert!(results.is_empty());
    assert_eq!(
        manager.search_queries.lock().await.as_slice(),
        ["nonexistent"]
    );
}

#[tokio::test]
async fn fake_package_manager_multiple_search_results() {
    let manager = FakeSkillPackageManager::default();
    let results = vec![
        remote_result("code-review", "Review code", "obra/cr", 1200),
        remote_result("brainstorming", "Brainstorm ideas", "obra/bs", 800),
        remote_result("debugging", "Debug issues", "obra/dbg", 500),
    ];
    *manager.search_results.lock().await = results.clone();

    let search_results = manager.search("obra").await.expect("search should succeed");
    assert_eq!(search_results, results);
    assert_eq!(search_results.len(), 3);
}

#[tokio::test]
async fn fake_package_manager_check_updates_error_propagates() {
    let manager = FakeSkillPackageManager::default();
    *manager.check_updates_error.lock().await = Some("unable to reach registry".to_string());

    let error = manager
        .check_updates("code-review")
        .await
        .expect_err("check updates should fail");
    assert!(error.to_string().contains("unable to reach registry"));
}
