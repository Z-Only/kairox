use super::*;
use crate::permission::ToolRisk;
use crate::registry::{Tool, ToolInvocation};
use crate::ToolError::{ExecutionFailed, Timeout};
use std::time::Duration;

// ── Classification tests ──────────────────────────────────────────────

#[test]
fn classify_readonly_commands() {
    assert_eq!(classify_command("ls", &[]), CommandRisk::ReadOnly);
    assert_eq!(classify_command("cat", &[]), CommandRisk::ReadOnly);
    assert_eq!(classify_command("git", &["status"]), CommandRisk::ReadOnly);
    assert_eq!(classify_command("cargo", &["test"]), CommandRisk::ReadOnly);
    assert_eq!(
        classify_command("bun", &["run", "lint"]),
        CommandRisk::ReadOnly
    );
    assert_eq!(classify_command("echo", &[]), CommandRisk::ReadOnly);
    assert_eq!(classify_command("pwd", &[]), CommandRisk::ReadOnly);
}

#[test]
fn classify_write_commands() {
    assert_eq!(classify_command("cp", &[]), CommandRisk::Write);
    assert_eq!(classify_command("mkdir", &[]), CommandRisk::Write);
    assert_eq!(classify_command("git", &["commit"]), CommandRisk::Write);
    assert_eq!(classify_command("bun", &["install"]), CommandRisk::Write);
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
    let tool =
        ShellExecTool::new(dir.path().to_path_buf()).with_default_timeout(Duration::from_secs(60));
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

// ── Additional classification tests ──────────────────────────────────

#[test]
fn classify_read_only_command() {
    assert_eq!(classify_command("ls", &["-la"]), CommandRisk::ReadOnly);
}

#[test]
fn classify_write_command() {
    assert_eq!(classify_command("cp", &["a", "b"]), CommandRisk::Write);
}

#[test]
fn classify_destructive_command() {
    assert_eq!(
        classify_command("rm", &["-rf", "/"]),
        CommandRisk::Destructive
    );
}

#[test]
fn classify_unknown_command_returns_unknown() {
    assert_eq!(classify_command("foobarbaz", &[]), CommandRisk::Unknown);
}

// ── Additional parse_command tests ───────────────────────────────────

#[test]
fn parse_command_simple() {
    let (program, args) = parse_command("ls -la");
    assert_eq!(program, "ls");
    assert_eq!(args, vec!["-la"]);
}

#[test]
fn parse_command_double_quoted_arg() {
    let (program, args) = parse_command(r#"echo "hello world""#);
    assert_eq!(program, "echo");
    assert_eq!(args, vec!["hello world"]);
}

#[test]
fn parse_command_single_quoted_arg() {
    let (program, args) = parse_command("echo 'hello world'");
    assert_eq!(program, "echo");
    assert_eq!(args, vec!["hello world"]);
}

#[test]
fn parse_command_backslash_escape() {
    let (program, args) = parse_command(r"echo hello\ world");
    assert_eq!(program, "echo");
    assert_eq!(args, vec!["hello world"]);
}

#[test]
fn parse_command_mixed_quoted_and_unquoted() {
    let (program, args) = parse_command(r#"git commit -m "my message""#);
    assert_eq!(program, "git");
    assert_eq!(args, vec!["commit", "-m", "my message"]);
}

#[test]
fn parse_command_unclosed_double_quote_preserved() {
    let (program, args) = parse_command(r#"echo "hello world"#);
    assert_eq!(program, "echo");
    assert_eq!(args, vec!["\"hello world"]);
}

#[test]
fn parse_command_unclosed_single_quote_preserved() {
    let (program, args) = parse_command("echo 'hello world");
    assert_eq!(program, "echo");
    assert_eq!(args, vec!["'hello world"]);
}

#[test]
fn parse_command_empty_returns_empty() {
    let (program, args) = parse_command("");
    assert_eq!(program, "");
    assert_eq!(args, Vec::<String>::new());
}

#[test]
fn parse_command_whitespace_only() {
    let (program, args) = parse_command("   ");
    assert_eq!(program, "");
    assert!(args.is_empty());
}
