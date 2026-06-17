use agent_core::facade::{
    InstallRemoteSkillRequest, SkillInstallTarget, SkillUpdateState,
};

use super::DirectDownloadPackageManager;
use super::super::SkillPackageManager;

#[tokio::test]
async fn search_returns_empty_vec() {
    let manager = DirectDownloadPackageManager;
    let results = manager.search("anything").await.expect("search should succeed");
    assert!(results.is_empty());
}

#[tokio::test]
async fn check_updates_returns_unknown() {
    let manager = DirectDownloadPackageManager;
    let state = manager
        .check_updates("some-skill")
        .await
        .expect("check_updates should succeed");
    assert_eq!(state, SkillUpdateState::Unknown);
}

#[tokio::test]
async fn update_returns_not_supported_error() {
    let manager = DirectDownloadPackageManager;
    let error = manager
        .update("some-skill")
        .await
        .expect_err("update should fail");
    assert!(
        error.to_string().contains("not yet supported"),
        "error was: {error}"
    );
}

#[tokio::test]
async fn install_from_registry_rejects_plain_package_without_url() {
    let manager = DirectDownloadPackageManager;
    let install_root = tempfile::tempdir().expect("install root");

    let request = InstallRemoteSkillRequest {
        package: "plain-package-name".to_string(),
        source: "direct".to_string(),
        target: SkillInstallTarget::Project,
        package_url: None,
    };

    let error = manager
        .install_from_registry(install_root.path(), &request)
        .await
        .expect_err("should reject missing URL");
    let message = error.to_string();
    assert!(
        message.contains("no package_url"),
        "message was: {message}"
    );
    assert!(
        message.contains("plain-package-name"),
        "message was: {message}"
    );
}

#[tokio::test]
async fn install_from_registry_attempts_download_when_package_url_present() {
    let manager = DirectDownloadPackageManager;
    let install_root = tempfile::tempdir().expect("install root");

    let request = InstallRemoteSkillRequest {
        package: "my-skill".to_string(),
        source: "direct".to_string(),
        target: SkillInstallTarget::Project,
        // package_url points to a non-routable address so the download will fail,
        // but the URL resolution step should succeed.
        package_url: Some("http://127.0.0.1:1/nonexistent.zip".to_string()),
    };

    let error = manager
        .install_from_registry(install_root.path(), &request)
        .await
        .expect_err("download from bogus URL should fail");
    let message = error.to_string();
    assert!(
        !message.contains("no package_url"),
        "should have attempted download, but got: {message}"
    );
    assert!(
        message.contains("download"),
        "expected download error, got: {message}"
    );
}

#[tokio::test]
async fn install_from_registry_uses_https_package_field_as_download_url() {
    let manager = DirectDownloadPackageManager;
    let install_root = tempfile::tempdir().expect("install root");

    let request = InstallRemoteSkillRequest {
        package: "https://example.com/skill.zip".to_string(),
        source: "direct".to_string(),
        target: SkillInstallTarget::Project,
        package_url: None,
    };

    let error = manager
        .install_from_registry(install_root.path(), &request)
        .await
        .expect_err("download should fail (no server)");
    let message = error.to_string();
    assert!(
        !message.contains("no package_url"),
        "should use package field as URL, got: {message}"
    );
}

#[tokio::test]
async fn install_from_registry_uses_http_package_field_as_download_url() {
    let manager = DirectDownloadPackageManager;
    let install_root = tempfile::tempdir().expect("install root");

    let request = InstallRemoteSkillRequest {
        package: "http://example.com/skill.zip".to_string(),
        source: "direct".to_string(),
        target: SkillInstallTarget::Project,
        package_url: None,
    };

    let error = manager
        .install_from_registry(install_root.path(), &request)
        .await
        .expect_err("download should fail (no server)");
    let message = error.to_string();
    assert!(
        !message.contains("no package_url"),
        "should use http:// package field as URL, got: {message}"
    );
}

#[tokio::test]
async fn install_from_registry_prefers_package_url_over_http_package_field() {
    let manager = DirectDownloadPackageManager;
    let install_root = tempfile::tempdir().expect("install root");

    let request = InstallRemoteSkillRequest {
        package: "https://example.com/wrong.zip".to_string(),
        source: "direct".to_string(),
        target: SkillInstallTarget::Project,
        package_url: Some("http://127.0.0.1:1/correct.zip".to_string()),
    };

    let error = manager
        .install_from_registry(install_root.path(), &request)
        .await
        .expect_err("download should fail");
    let message = error.to_string();
    assert!(
        !message.contains("no package_url"),
        "should have used package_url, got: {message}"
    );
}
