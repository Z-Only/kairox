use super::*;

// ── SdkConfig defaults ──────────────────────────────────────────────

#[test]
fn default_workspace_path_is_dot() {
    let config = SdkConfig::default();
    assert_eq!(config.workspace_path, std::path::PathBuf::from("."));
}

#[test]
fn default_data_dir_is_none() {
    assert!(SdkConfig::default().data_dir.is_none());
}

#[test]
fn default_home_dir_is_none() {
    assert!(SdkConfig::default().home_dir.is_none());
}

#[test]
fn default_database_filename() {
    assert_eq!(SdkConfig::default().database_filename, "kairox.db");
}

#[test]
fn default_profile_is_none() {
    assert!(SdkConfig::default().default_profile.is_none());
}

#[test]
fn default_approval_policy_is_never() {
    assert_eq!(
        SdkConfig::default().approval_policy,
        SdkApprovalPolicy::Never
    );
}

#[test]
fn default_sandbox_policy_is_workspace_write() {
    assert_eq!(
        SdkConfig::default().sandbox_policy,
        SdkSandboxPolicy::WorkspaceWrite
    );
}

#[test]
fn default_enable_mcp_servers_is_true() {
    assert!(SdkConfig::default().enable_mcp_servers);
}

#[test]
fn default_enable_lsp_servers_is_false() {
    assert!(!SdkConfig::default().enable_lsp_servers);
}

#[test]
fn default_enable_marketplace_is_false() {
    assert!(!SdkConfig::default().enable_marketplace);
}

// ── SdkConfig Clone ─────────────────────────────────────────────────

#[test]
fn sdk_config_clone_preserves_all_fields() {
    let original = SdkConfig {
        workspace_path: "/some/path".into(),
        data_dir: Some("/data".into()),
        home_dir: Some("/home".into()),
        database_filename: "custom.db".into(),
        default_profile: Some("fast".into()),
        approval_policy: SdkApprovalPolicy::Always,
        sandbox_policy: SdkSandboxPolicy::FullAccess,
        enable_mcp_servers: false,
        enable_lsp_servers: true,
        enable_marketplace: true,
    };
    let cloned = original.clone();

    assert_eq!(
        cloned.workspace_path,
        std::path::PathBuf::from("/some/path")
    );
    assert_eq!(cloned.data_dir, Some(std::path::PathBuf::from("/data")));
    assert_eq!(cloned.home_dir, Some(std::path::PathBuf::from("/home")));
    assert_eq!(cloned.database_filename, "custom.db");
    assert_eq!(cloned.default_profile.as_deref(), Some("fast"));
    assert_eq!(cloned.approval_policy, SdkApprovalPolicy::Always);
    assert_eq!(cloned.sandbox_policy, SdkSandboxPolicy::FullAccess);
    assert!(!cloned.enable_mcp_servers);
    assert!(cloned.enable_lsp_servers);
    assert!(cloned.enable_marketplace);
}

// ── SdkApprovalPolicy traits ────────────────────────────────────────

#[test]
fn approval_policy_clone_and_copy() {
    let policy = SdkApprovalPolicy::OnRequest;
    let copied = policy; // Copy
    let also_copied = policy; // Copy again — proves Copy works
    assert_eq!(copied, also_copied);
    assert_eq!(policy, SdkApprovalPolicy::OnRequest);
}

#[test]
fn approval_policy_partial_eq() {
    assert_eq!(SdkApprovalPolicy::Never, SdkApprovalPolicy::Never);
    assert_eq!(SdkApprovalPolicy::OnRequest, SdkApprovalPolicy::OnRequest);
    assert_eq!(SdkApprovalPolicy::Always, SdkApprovalPolicy::Always);
    assert_ne!(SdkApprovalPolicy::Never, SdkApprovalPolicy::Always);
    assert_ne!(SdkApprovalPolicy::OnRequest, SdkApprovalPolicy::Never);
}

#[test]
fn approval_policy_debug() {
    assert_eq!(format!("{:?}", SdkApprovalPolicy::Never), "Never");
    assert_eq!(format!("{:?}", SdkApprovalPolicy::OnRequest), "OnRequest");
    assert_eq!(format!("{:?}", SdkApprovalPolicy::Always), "Always");
}

// ── SdkSandboxPolicy traits ─────────────────────────────────────────

#[test]
fn sandbox_policy_clone_and_eq() {
    let policy = SdkSandboxPolicy::WorkspaceWrite;
    let cloned = policy.clone();
    assert_eq!(cloned, SdkSandboxPolicy::WorkspaceWrite);
}

#[test]
fn sandbox_policy_debug() {
    assert_eq!(format!("{:?}", SdkSandboxPolicy::ReadOnly), "ReadOnly");
    assert_eq!(
        format!("{:?}", SdkSandboxPolicy::WorkspaceWrite),
        "WorkspaceWrite"
    );
    assert_eq!(format!("{:?}", SdkSandboxPolicy::FullAccess), "FullAccess");
}

#[test]
fn sandbox_policy_partial_eq_across_variants() {
    assert_ne!(SdkSandboxPolicy::ReadOnly, SdkSandboxPolicy::FullAccess);
    assert_ne!(SdkSandboxPolicy::ReadOnly, SdkSandboxPolicy::WorkspaceWrite);
    assert_ne!(
        SdkSandboxPolicy::WorkspaceWrite,
        SdkSandboxPolicy::FullAccess
    );
}

// ── SdkSandboxPolicy::into_runtime_policy ───────────────────────────

#[test]
fn into_runtime_policy_read_only() {
    let workspace = std::path::Path::new("/workspace");
    let policy = SdkSandboxPolicy::ReadOnly.into_runtime_policy(workspace);
    assert!(matches!(policy, agent_tools::SandboxPolicy::ReadOnly));
}

#[test]
fn into_runtime_policy_full_access() {
    let workspace = std::path::Path::new("/workspace");
    let policy = SdkSandboxPolicy::FullAccess.into_runtime_policy(workspace);
    assert!(matches!(
        policy,
        agent_tools::SandboxPolicy::DangerFullAccess
    ));
}

#[test]
fn into_runtime_policy_workspace_write_contains_workspace_root() {
    let workspace = std::path::Path::new("/my/project");
    let policy = SdkSandboxPolicy::WorkspaceWrite.into_runtime_policy(workspace);
    match policy {
        agent_tools::SandboxPolicy::WorkspaceWrite {
            network_access,
            writable_roots,
        } => {
            assert!(!network_access, "network_access should be false");
            assert_eq!(writable_roots.len(), 1);
            assert_eq!(writable_roots[0], std::path::PathBuf::from("/my/project"));
        }
        other => panic!("expected WorkspaceWrite, got: {other:?}"),
    }
}

#[test]
fn into_runtime_policy_workspace_write_with_different_paths() {
    for path_str in ["/tmp/test", "/home/user/code", "/a/b/c/d"] {
        let workspace = std::path::Path::new(path_str);
        let policy = SdkSandboxPolicy::WorkspaceWrite.into_runtime_policy(workspace);
        match policy {
            agent_tools::SandboxPolicy::WorkspaceWrite { writable_roots, .. } => {
                assert_eq!(
                    writable_roots,
                    vec![workspace.to_path_buf()],
                    "writable_roots mismatch for {path_str}"
                );
            }
            other => panic!("expected WorkspaceWrite for {path_str}, got: {other:?}"),
        }
    }
}
