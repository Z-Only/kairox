use super::*;

#[test]
fn no_args_defaults_to_current_dir() {
    let cli = parse_workspace_args(Vec::<String>::new()).unwrap();
    assert_eq!(cli.action, CliAction::Run(WorkspaceCliMode::CurrentDir));
    assert_eq!(cli.profile, None);
    assert_eq!(cli.approval_policy, None);
    assert_eq!(cli.sandbox_policy, None);
}

#[test]
fn help_flag() {
    let cli = parse_workspace_args(["--help"]).unwrap();
    assert_eq!(cli.action, CliAction::Help);

    let cli = parse_workspace_args(["-h"]).unwrap();
    assert_eq!(cli.action, CliAction::Help);
}

#[test]
fn version_flag() {
    let cli = parse_workspace_args(["--version"]).unwrap();
    assert_eq!(cli.action, CliAction::Version);

    let cli = parse_workspace_args(["-V"]).unwrap();
    assert_eq!(cli.action, CliAction::Version);
}

#[test]
fn profile_flag() {
    let cli = parse_workspace_args(["--profile", "fast"]).unwrap();
    assert_eq!(cli.action, CliAction::Run(WorkspaceCliMode::CurrentDir));
    assert_eq!(cli.profile.as_deref(), Some("fast"));

    let cli = parse_workspace_args(["-p", "local-code"]).unwrap();
    assert_eq!(cli.profile.as_deref(), Some("local-code"));
}

#[test]
fn approval_policy_flag() {
    let cli = parse_workspace_args(["--approval-policy", "never"]).unwrap();
    assert_eq!(cli.approval_policy, Some(ApprovalPolicy::Never));

    let cli = parse_workspace_args(["--approval-policy", "always"]).unwrap();
    assert_eq!(cli.approval_policy, Some(ApprovalPolicy::Always));
}

#[test]
fn sandbox_policy_flag() {
    let cli = parse_workspace_args(["--sandbox-policy", "read_only"]).unwrap();
    assert_eq!(cli.sandbox_policy, Some(SandboxPolicy::ReadOnly));

    let cli = parse_workspace_args(["--sandbox-policy", "danger_full_access"]).unwrap();
    assert_eq!(cli.sandbox_policy, Some(SandboxPolicy::DangerFullAccess));
}

#[test]
fn combined_flags() {
    let cli = parse_workspace_args([
        "--profile",
        "fast",
        "--approval-policy",
        "always",
        "--sandbox-policy",
        "workspace_write",
        "--workspace",
        "/tmp/proj",
    ])
    .unwrap();
    assert_eq!(
        cli.action,
        CliAction::Run(WorkspaceCliMode::Use("/tmp/proj".into()))
    );
    assert_eq!(cli.profile.as_deref(), Some("fast"));
    assert_eq!(cli.approval_policy, Some(ApprovalPolicy::Always));
    assert_eq!(
        cli.sandbox_policy.as_ref().map(|p| p.kind_str()),
        Some("workspace_write")
    );
}

#[test]
fn unknown_arg_errors() {
    assert!(parse_workspace_args(["--bogus"]).is_err());
}

#[test]
fn missing_value_errors() {
    assert!(parse_workspace_args(["--profile"]).is_err());
    assert!(parse_workspace_args(["--approval-policy"]).is_err());
    assert!(parse_workspace_args(["--sandbox-policy"]).is_err());
    assert!(parse_workspace_args(["--workspace"]).is_err());
}

#[test]
fn invalid_policy_errors() {
    assert!(parse_workspace_args(["--approval-policy", "bogus"]).is_err());
    assert!(parse_workspace_args(["--sandbox-policy", "bogus"]).is_err());
}
