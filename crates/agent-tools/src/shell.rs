use crate::permission::ToolRisk;
use crate::registry::{Tool, ToolDefinition, ToolInvocation, ToolOutput};
use crate::ToolError::{ExecutionFailed, Timeout};
use async_trait::async_trait;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

// ── Constants (keep — other modules reference these) ──────────────────────

pub const SHELL_TOOL_ID: &str = "shell.exec";
pub const PATCH_TOOL_ID: &str = "patch.apply";
pub const SEARCH_TOOL_ID: &str = "search.ripgrep";

// ── CommandRisk ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommandRisk {
    ReadOnly,
    Write,
    Destructive,
    Unknown,
}

// ── Classification helpers ────────────────────────────────────────────────

const READONLY_COMMANDS: &[&str] = &[
    "ls", "cat", "head", "tail", "grep", "egrep", "rg", "find", "wc", "sort", "uniq", "diff",
    "echo", "pwd", "which", "whoami", "env", "printenv", "stat", "file", "du", "df", "free",
    "uptime", "ps", "curl", "wget", "git", "gh", "cargo", "rustc", "node", "python3", "python",
    "java", "go", "make", "cmake", "npm", "npx", "pnpm", "yarn", "pip", "pip3", "test", "true",
    "false", "date", "uname", "hostname", "id", "arch",
];

const WRITE_COMMANDS: &[&str] = &[
    "cp", "mv", "mkdir", "touch", "chmod", "chown", "ln", "tee", "docker", "kubectl", "helm",
];

const DESTRUCTIVE_COMMANDS: &[&str] = &["rm", "sudo", "su", "mkfs", "dd", "format"];

fn is_write_subcommand(program: &str, sub: &str) -> bool {
    match program {
        "git" => matches!(
            sub,
            "push"
                | "commit"
                | "merge"
                | "rebase"
                | "reset"
                | "checkout"
                | "branch"
                | "tag"
                | "stash"
                | "cherry-pick"
        ),
        "npm" => matches!(sub, "install" | "uninstall" | "publish" | "update"),
        "pip" | "pip3" => matches!(sub, "install" | "uninstall"),
        "cargo" => matches!(sub, "publish"),
        "docker" => matches!(
            sub,
            "rm" | "rmi" | "stop" | "kill" | "build" | "run" | "push" | "compose"
        ),
        "kubectl" => matches!(sub, "delete" | "apply" | "create" | "edit" | "patch"),
        "helm" => matches!(sub, "install" | "upgrade" | "delete" | "rollback"),
        _ => false,
    }
}

fn is_destructive_subcommand(program: &str, sub: &str, _args: &[&str]) -> bool {
    match program {
        "git" => matches!(sub, "clean"),
        "docker" => matches!(sub, "system" | "volume"),
        _ => false,
    }
}

pub fn classify_command(program: &str, args: &[&str]) -> CommandRisk {
    let prog = program.trim();

    // Check subcommand upgrades first (most specific)
    if let Some(sub) = args.first().map(|s| s as &str) {
        if is_destructive_subcommand(prog, sub, &args[1..]) {
            return CommandRisk::Destructive;
        }
        if is_write_subcommand(prog, sub) {
            return CommandRisk::Write;
        }
    }

    // Then check base program classification
    if READONLY_COMMANDS.contains(&prog) {
        return CommandRisk::ReadOnly;
    }
    if WRITE_COMMANDS.contains(&prog) {
        return CommandRisk::Write;
    }
    if DESTRUCTIVE_COMMANDS.contains(&prog) {
        return CommandRisk::Destructive;
    }

    CommandRisk::Unknown
}

// ── Command parsing ──────────────────────────────────────────────────────

pub fn parse_command(command: &str) -> (String, Vec<String>) {
    let tokens: Vec<String> = command.split_whitespace().map(|s| s.to_string()).collect();
    match tokens.split_first() {
        Some((program, args)) => (program.clone(), args.to_vec()),
        None => (String::new(), Vec::new()),
    }
}

// ── ShellExecTool ─────────────────────────────────────────────────────────

const DEFAULT_TIMEOUT_SECS: u64 = 30;
const DEFAULT_MAX_OUTPUT_BYTES: usize = 102_400; // 100 KB
const ALLOWED_ENV_VARS: &[&str] = &["PATH", "HOME", "LANG", "TERM", "USER", "TMPDIR", "SHELL"];

pub struct ShellExecTool {
    workspace_root: PathBuf,
    default_timeout: Duration,
    max_output_bytes: usize,
}

impl ShellExecTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            workspace_root,
            default_timeout: Duration::from_secs(DEFAULT_TIMEOUT_SECS),
            max_output_bytes: DEFAULT_MAX_OUTPUT_BYTES,
        }
    }

    pub fn with_default_timeout(mut self, timeout: Duration) -> Self {
        self.default_timeout = timeout;
        self
    }
}

#[async_trait]
impl Tool for ShellExecTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_id: SHELL_TOOL_ID.to_string(),
            description: "Execute shell commands within the workspace sandbox".to_string(),
            required_capability: "shell.exec".to_string(),
        }
    }

    fn risk(&self, invocation: &ToolInvocation) -> ToolRisk {
        let command = invocation
            .arguments
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let (program, args) = parse_command(command);
        let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let risk = classify_command(&program, &arg_refs);

        match risk {
            CommandRisk::ReadOnly => ToolRisk::read(SHELL_TOOL_ID),
            CommandRisk::Write | CommandRisk::Unknown => ToolRisk::write(SHELL_TOOL_ID),
            CommandRisk::Destructive => ToolRisk::destructive(SHELL_TOOL_ID),
        }
    }

    async fn invoke(&self, invocation: ToolInvocation) -> crate::Result<ToolOutput> {
        let command = invocation
            .arguments
            .get("command")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let (program, args) = parse_command(command);
        if program.is_empty() {
            return Err(ExecutionFailed("empty command".to_string()));
        }

        let timeout_duration = if invocation.timeout_ms > 0 {
            Duration::from_millis(invocation.timeout_ms)
        } else {
            self.default_timeout
        };
        let output_limit = if invocation.output_limit_bytes > 0 {
            invocation.output_limit_bytes
        } else {
            self.max_output_bytes
        };

        let mut cmd = tokio::process::Command::new(&program);
        cmd.args(&args)
            .current_dir(&self.workspace_root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());

        // Clear environment and restore only allowed vars
        cmd.env_clear();
        for var in ALLOWED_ENV_VARS {
            if let Ok(val) = std::env::var(var) {
                cmd.env(var, val);
            }
        }

        let result = tokio::time::timeout(timeout_duration, cmd.output()).await;

        match result {
            Ok(Ok(output)) => {
                if output.status.success() {
                    let (text, truncated) = truncate_bytes(&output.stdout, output_limit);
                    Ok(ToolOutput {
                        text: String::from_utf8_lossy(&text).to_string(),
                        truncated,
                    })
                } else {
                    let exit_code = output.status.code().unwrap_or(-1);
                    let (text, truncated) = truncate_bytes(&output.stderr, output_limit);
                    let stderr_text = String::from_utf8_lossy(&text);
                    Ok(ToolOutput {
                        text: format!(
                            "exit code {}: {}{}",
                            exit_code,
                            stderr_text,
                            if truncated { " [truncated]" } else { "" }
                        ),
                        truncated,
                    })
                }
            }
            Ok(Err(e)) => Err(ExecutionFailed(e.to_string())),
            Err(_) => Err(Timeout(timeout_duration.as_millis() as u64)),
        }
    }
}

/// Truncate bytes to `limit`, returning (truncated_bytes, was_truncated).
fn truncate_bytes(data: &[u8], limit: usize) -> (Vec<u8>, bool) {
    if data.len() <= limit {
        (data.to_vec(), false)
    } else {
        (data[..limit].to_vec(), true)
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    // ── Classification tests ──────────────────────────────────────────────

    #[test]
    fn classify_readonly_commands() {
        assert_eq!(classify_command("ls", &[]), CommandRisk::ReadOnly);
        assert_eq!(classify_command("cat", &[]), CommandRisk::ReadOnly);
        assert_eq!(classify_command("git", &["status"]), CommandRisk::ReadOnly);
        assert_eq!(classify_command("cargo", &["test"]), CommandRisk::ReadOnly);
        assert_eq!(classify_command("echo", &[]), CommandRisk::ReadOnly);
        assert_eq!(classify_command("pwd", &[]), CommandRisk::ReadOnly);
    }

    #[test]
    fn classify_write_commands() {
        assert_eq!(classify_command("cp", &[]), CommandRisk::Write);
        assert_eq!(classify_command("mkdir", &[]), CommandRisk::Write);
        assert_eq!(classify_command("git", &["commit"]), CommandRisk::Write);
        assert_eq!(classify_command("npm", &["install"]), CommandRisk::Write);
        assert_eq!(classify_command("docker", &["build"]), CommandRisk::Write);
    }

    #[test]
    fn classify_destructive_commands() {
        assert_eq!(classify_command("rm", &[]), CommandRisk::Destructive);
        assert_eq!(classify_command("sudo", &[]), CommandRisk::Destructive);
        assert_eq!(classify_command("mkfs", &[]), CommandRisk::Destructive);
        assert_eq!(
            classify_command("git", &["clean"]),
            CommandRisk::Destructive
        );
        assert_eq!(
            classify_command("docker", &["system"]),
            CommandRisk::Destructive
        );
        assert_eq!(
            classify_command("docker", &["volume"]),
            CommandRisk::Destructive
        );
    }

    #[test]
    fn classify_unknown_defaults_conservative() {
        assert_eq!(classify_command("unknown_bin", &[]), CommandRisk::Unknown);
        assert_eq!(classify_command("foobar", &["baz"]), CommandRisk::Unknown);
    }

    #[test]
    fn subcommand_upgrades_risk() {
        // git checkout is Write (subcommand upgrade), not ReadOnly (base)
        assert_eq!(classify_command("git", &["checkout"]), CommandRisk::Write);
        // git push → Write
        assert_eq!(classify_command("git", &["push"]), CommandRisk::Write);
        // kubectl apply → Write
        assert_eq!(classify_command("kubectl", &["apply"]), CommandRisk::Write);
        // helm upgrade → Write
        assert_eq!(classify_command("helm", &["upgrade"]), CommandRisk::Write);
    }

    // ── parse_command tests ───────────────────────────────────────────────

    #[test]
    fn parse_command_splits_program_and_args() {
        let (program, args) = parse_command("echo hello world");
        assert_eq!(program, "echo");
        assert_eq!(args, vec!["hello", "world"]);
    }

    #[test]
    fn parse_command_empty_input() {
        let (program, args) = parse_command("");
        assert_eq!(program, "");
        assert!(args.is_empty());
    }

    #[test]
    fn parse_command_program_only() {
        let (program, args) = parse_command("ls");
        assert_eq!(program, "ls");
        assert!(args.is_empty());
    }

    // ── ShellExecTool integration tests ───────────────────────────────────

    fn make_invocation(command: &str) -> ToolInvocation {
        ToolInvocation {
            tool_id: SHELL_TOOL_ID.to_string(),
            arguments: serde_json::json!({"command": command}),
            workspace_id: "test".to_string(),
            preview: command.to_string(),
            timeout_ms: 5000,
            output_limit_bytes: 102_400,
        }
    }

    fn make_invocation_with_timeout(command: &str, timeout_ms: u64) -> ToolInvocation {
        ToolInvocation {
            tool_id: SHELL_TOOL_ID.to_string(),
            arguments: serde_json::json!({"command": command}),
            workspace_id: "test".to_string(),
            preview: command.to_string(),
            timeout_ms,
            output_limit_bytes: 102_400,
        }
    }

    #[tokio::test]
    async fn shell_exec_readonly_command_succeeds() {
        let dir = tempfile::tempdir().unwrap();
        let tool = ShellExecTool::new(dir.path().to_path_buf());
        let invocation = make_invocation("echo hello");
        let result = tool.invoke(invocation).await.unwrap();
        assert!(result.text.contains("hello"));
        assert!(!result.truncated);
    }

    #[tokio::test]
    async fn shell_exec_pwd_is_workspace_root() {
        let dir = tempfile::tempdir().unwrap();
        let workspace = dir.path().canonicalize().unwrap();
        let tool = ShellExecTool::new(dir.path().to_path_buf());
        let invocation = make_invocation("pwd");
        let result = tool.invoke(invocation).await.unwrap();
        // On macOS /var is a symlink to /private/var, so canonicalize both
        let result_path = std::path::Path::new(result.text.trim())
            .canonicalize()
            .unwrap_or_else(|_| std::path::PathBuf::from(result.text.trim()));
        assert_eq!(result_path, workspace);
    }

    #[tokio::test]
    async fn shell_exec_captures_stderr_on_failure() {
        let dir = tempfile::tempdir().unwrap();
        let tool = ShellExecTool::new(dir.path().to_path_buf());
        let invocation = make_invocation("ls nonexistent_dir_xyz");
        let result = tool.invoke(invocation).await.unwrap();
        assert!(result.text.starts_with("exit code"));
        assert!(
            result.text.contains("nonexistent_dir_xyz"),
            "stderr should mention the missing path: got '{}'",
            result.text
        );
    }

    #[tokio::test]
    async fn shell_exec_timeout_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let tool = ShellExecTool::new(dir.path().to_path_buf());
        let invocation = make_invocation_with_timeout("sleep 10", 200);
        let result = tool.invoke(invocation).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            Timeout(ms) => assert_eq!(ms, 200),
            other => panic!("expected Timeout, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn shell_exec_empty_command_returns_error() {
        let dir = tempfile::tempdir().unwrap();
        let tool = ShellExecTool::new(dir.path().to_path_buf());
        let invocation = make_invocation("");
        let result = tool.invoke(invocation).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            ExecutionFailed(msg) => assert_eq!(msg, "empty command"),
            other => panic!("expected ExecutionFailed, got {:?}", other),
        }
    }

    // ── Tool trait risk tests ─────────────────────────────────────────────

    #[test]
    fn risk_mapping_readonly() {
        let dir = tempfile::tempdir().unwrap();
        let tool = ShellExecTool::new(dir.path().to_path_buf());
        let inv = make_invocation("ls");
        let risk = tool.risk(&inv);
        assert_eq!(risk, ToolRisk::read(SHELL_TOOL_ID));
    }

    #[test]
    fn risk_mapping_write() {
        let dir = tempfile::tempdir().unwrap();
        let tool = ShellExecTool::new(dir.path().to_path_buf());
        let inv = make_invocation("cp file1 file2");
        let risk = tool.risk(&inv);
        assert_eq!(risk, ToolRisk::write(SHELL_TOOL_ID));
    }

    #[test]
    fn risk_mapping_destructive() {
        let dir = tempfile::tempdir().unwrap();
        let tool = ShellExecTool::new(dir.path().to_path_buf());
        let inv = make_invocation("rm file");
        let risk = tool.risk(&inv);
        assert_eq!(risk, ToolRisk::destructive(SHELL_TOOL_ID));
    }

    #[test]
    fn risk_mapping_unknown_is_write() {
        let dir = tempfile::tempdir().unwrap();
        let tool = ShellExecTool::new(dir.path().to_path_buf());
        let inv = make_invocation("unknown_cmd arg1");
        let risk = tool.risk(&inv);
        assert_eq!(risk, ToolRisk::write(SHELL_TOOL_ID));
    }

    // ── Builder pattern test ──────────────────────────────────────────────

    #[test]
    fn builder_with_custom_timeout() {
        let dir = tempfile::tempdir().unwrap();
        let tool = ShellExecTool::new(dir.path().to_path_buf())
            .with_default_timeout(Duration::from_secs(60));
        assert_eq!(tool.default_timeout, Duration::from_secs(60));
    }

    // ── Definition test ───────────────────────────────────────────────────

    #[test]
    fn definition_returns_correct_id() {
        let dir = tempfile::tempdir().unwrap();
        let tool = ShellExecTool::new(dir.path().to_path_buf());
        let def = tool.definition();
        assert_eq!(def.tool_id, SHELL_TOOL_ID);
        assert_eq!(def.required_capability, "shell.exec");
    }
}
