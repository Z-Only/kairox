use super::*;
use crate::config::{SdkApprovalPolicy, SdkSandboxPolicy};

#[test]
fn builder_default_config_matches_sdk_config_default() {
    let builder = SdkBuilder::new();
    let config = &builder.config;

    assert_eq!(config.workspace_path, std::path::PathBuf::from("."));
    assert!(config.data_dir.is_none());
    assert!(config.home_dir.is_none());
    assert_eq!(config.database_filename, "kairox.db");
    assert!(config.default_profile.is_none());
    assert_eq!(config.approval_policy, SdkApprovalPolicy::Never);
    assert_eq!(config.sandbox_policy, SdkSandboxPolicy::WorkspaceWrite);
    assert!(config.enable_mcp_servers);
    assert!(!config.enable_lsp_servers);
    assert!(!config.enable_marketplace);
}

#[test]
fn builder_has_no_hooks_by_default() {
    let builder = SdkBuilder::new();
    assert!(builder.hooks.is_empty());
}

#[test]
fn workspace_sets_path() {
    let builder = SdkBuilder::new().workspace("/tmp/project");
    assert_eq!(
        builder.config.workspace_path,
        std::path::PathBuf::from("/tmp/project")
    );
}

#[test]
fn data_dir_sets_override() {
    let builder = SdkBuilder::new().data_dir("/custom/data");
    assert_eq!(
        builder.config.data_dir,
        Some(std::path::PathBuf::from("/custom/data"))
    );
}

#[test]
fn home_dir_sets_override() {
    let builder = SdkBuilder::new().home_dir("/custom/home");
    assert_eq!(
        builder.config.home_dir,
        Some(std::path::PathBuf::from("/custom/home"))
    );
}

#[test]
fn database_filename_sets_custom_name() {
    let builder = SdkBuilder::new().database_filename("custom.db");
    assert_eq!(builder.config.database_filename, "custom.db");
}

#[test]
fn default_profile_sets_alias() {
    let builder = SdkBuilder::new().default_profile("gpt-4o");
    assert_eq!(builder.config.default_profile.as_deref(), Some("gpt-4o"));
}

#[test]
fn approval_policy_sets_value() {
    let builder = SdkBuilder::new().approval_policy(SdkApprovalPolicy::Always);
    assert_eq!(builder.config.approval_policy, SdkApprovalPolicy::Always);
}

#[test]
fn sandbox_policy_sets_value() {
    let builder = SdkBuilder::new().sandbox_policy(SdkSandboxPolicy::ReadOnly);
    assert_eq!(builder.config.sandbox_policy, SdkSandboxPolicy::ReadOnly);
}

#[test]
fn enable_mcp_servers_toggles() {
    let builder = SdkBuilder::new().enable_mcp_servers(false);
    assert!(!builder.config.enable_mcp_servers);
}

#[test]
fn enable_lsp_servers_toggles() {
    let builder = SdkBuilder::new().enable_lsp_servers(true);
    assert!(builder.config.enable_lsp_servers);
}

#[test]
fn enable_marketplace_toggles() {
    let builder = SdkBuilder::new().enable_marketplace(true);
    assert!(builder.config.enable_marketplace);
}

#[test]
fn chained_setters_compose() {
    let builder = SdkBuilder::new()
        .workspace("/my/project")
        .data_dir("/data")
        .home_dir("/home")
        .database_filename("test.db")
        .default_profile("claude")
        .approval_policy(SdkApprovalPolicy::OnRequest)
        .sandbox_policy(SdkSandboxPolicy::FullAccess)
        .enable_mcp_servers(false)
        .enable_lsp_servers(true)
        .enable_marketplace(true);

    assert_eq!(
        builder.config.workspace_path,
        std::path::PathBuf::from("/my/project")
    );
    assert_eq!(
        builder.config.data_dir,
        Some(std::path::PathBuf::from("/data"))
    );
    assert_eq!(
        builder.config.home_dir,
        Some(std::path::PathBuf::from("/home"))
    );
    assert_eq!(builder.config.database_filename, "test.db");
    assert_eq!(builder.config.default_profile.as_deref(), Some("claude"));
    assert_eq!(builder.config.approval_policy, SdkApprovalPolicy::OnRequest);
    assert_eq!(builder.config.sandbox_policy, SdkSandboxPolicy::FullAccess);
    assert!(!builder.config.enable_mcp_servers);
    assert!(builder.config.enable_lsp_servers);
    assert!(builder.config.enable_marketplace);
}
