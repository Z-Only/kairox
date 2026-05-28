use std::io::{self, Write};
use std::path::PathBuf;

use agent_tools::{ApprovalPolicy, SandboxPolicy};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KnownWorkspace {
    pub workspace_id: String,
    pub path: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WorkspaceCliMode {
    CurrentDir,
    List,
    Select,
    Use(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CliAction {
    Run(WorkspaceCliMode),
    Help,
    Version,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceCli {
    pub action: CliAction,
    pub profile: Option<String>,
    pub approval_policy: Option<ApprovalPolicy>,
    pub sandbox_policy: Option<SandboxPolicy>,
}

pub fn parse_workspace_args<I, S>(args: I) -> Result<WorkspaceCli, String>
where
    I: IntoIterator<Item = S>,
    S: Into<String>,
{
    let mut args_iter = args.into_iter().map(Into::into).peekable();
    let mut workspace_mode = WorkspaceCliMode::CurrentDir;
    let mut profile = None;
    let mut approval_policy = None;
    let mut sandbox_policy = None;

    while let Some(arg) = args_iter.next() {
        match arg.as_str() {
            "--help" | "-h" => {
                return Ok(WorkspaceCli {
                    action: CliAction::Help,
                    profile: None,
                    approval_policy: None,
                    sandbox_policy: None,
                });
            }
            "--version" | "-V" => {
                return Ok(WorkspaceCli {
                    action: CliAction::Version,
                    profile: None,
                    approval_policy: None,
                    sandbox_policy: None,
                });
            }
            "--workspace-list" | "--workspaces" => {
                workspace_mode = WorkspaceCliMode::List;
            }
            "--workspace-select" => {
                workspace_mode = WorkspaceCliMode::Select;
            }
            "--workspace" | "-w" => {
                let Some(selector) = args_iter.next() else {
                    return Err(format!(
                        "--workspace requires an id, index, or path\n{}",
                        cli_usage()
                    ));
                };
                workspace_mode = WorkspaceCliMode::Use(selector);
            }
            "--profile" | "-p" => {
                let Some(value) = args_iter.next() else {
                    return Err(format!(
                        "--profile requires a profile name\n{}",
                        cli_usage()
                    ));
                };
                profile = Some(value);
            }
            "--approval-policy" => {
                let Some(value) = args_iter.next() else {
                    return Err(format!(
                        "--approval-policy requires a value (never|on_request|always)\n{}",
                        cli_usage()
                    ));
                };
                approval_policy = Some(
                    value
                        .parse::<ApprovalPolicy>()
                        .map_err(|e| format!("{e}\n{}", cli_usage()))?,
                );
            }
            "--sandbox-policy" => {
                let Some(value) = args_iter.next() else {
                    return Err(format!(
                        "--sandbox-policy requires a value (read_only|workspace_write|danger_full_access)\n{}",
                        cli_usage()
                    ));
                };
                sandbox_policy = Some(
                    value
                        .parse::<SandboxPolicy>()
                        .map_err(|e| format!("{e}\n{}", cli_usage()))?,
                );
            }
            other => {
                return Err(format!("unknown argument: {other}\n{}", cli_usage()));
            }
        }
    }

    Ok(WorkspaceCli {
        action: CliAction::Run(workspace_mode),
        profile,
        approval_policy,
        sandbox_policy,
    })
}

pub fn cli_usage() -> &'static str {
    "\
Usage: kairox [OPTIONS]

Options:
  -h, --help                    Print help and exit
  -V, --version                 Print version and exit
  -p, --profile <NAME>          Use a specific model profile
  --approval-policy <POLICY>    Set tool approval policy [never|on_request|always]
  --sandbox-policy <POLICY>     Set sandbox policy [read_only|workspace_write|danger_full_access]

Workspace options:
  --workspace-list              List known workspaces and exit
  --workspace-select            Prompt for a known workspace before launch
  -w, --workspace <ID|INDEX|PATH>  Launch in a known workspace or existing path"
}

pub fn format_known_workspaces(workspaces: &[KnownWorkspace]) -> String {
    if workspaces.is_empty() {
        return "No known workspaces.\n".to_string();
    }

    let mut output = String::from("Known workspaces:\n");
    for (index, workspace) in workspaces.iter().enumerate() {
        output.push_str(&format!(
            "{}. {}  {}\n",
            index + 1,
            workspace.workspace_id,
            workspace.path
        ));
    }
    output
}

pub fn prompt_workspace_selector(workspaces: &[KnownWorkspace]) -> Result<Option<String>, String> {
    print!("{}", format_known_workspaces(workspaces));
    print!("Select workspace number, id, or path (blank to cancel): ");
    io::stdout()
        .flush()
        .map_err(|error| format!("failed to flush prompt: {error}"))?;

    let mut input = String::new();
    io::stdin()
        .read_line(&mut input)
        .map_err(|error| format!("failed to read workspace selection: {error}"))?;
    let selector = input.trim();
    if selector.is_empty() {
        Ok(None)
    } else {
        Ok(Some(selector.to_string()))
    }
}

pub fn resolve_workspace_selector(
    workspaces: &[KnownWorkspace],
    selector: &str,
) -> Result<PathBuf, String> {
    let selector = selector.trim();
    if selector.is_empty() {
        return Err("workspace selector cannot be empty".to_string());
    }

    if let Ok(index) = selector.parse::<usize>() {
        if let Some(workspace) = index
            .checked_sub(1)
            .and_then(|zero_based| workspaces.get(zero_based))
        {
            return existing_known_workspace_path(workspace);
        }
    }

    if let Some(workspace) = workspaces
        .iter()
        .find(|workspace| workspace.workspace_id == selector || workspace.path == selector)
    {
        return existing_known_workspace_path(workspace);
    }

    let direct = PathBuf::from(selector);
    if direct.is_dir() {
        return Ok(direct);
    }

    Err(format!(
        "workspace selector not found: {selector}. Run --workspace-list to see known workspaces."
    ))
}

fn existing_known_workspace_path(workspace: &KnownWorkspace) -> Result<PathBuf, String> {
    let path = PathBuf::from(&workspace.path);
    if path.is_dir() {
        Ok(path)
    } else {
        Err(format!(
            "workspace path does not exist for {}: {}",
            workspace.workspace_id, workspace.path
        ))
    }
}

#[cfg(test)]
mod tests {
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
}
