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
        session_id: "ses_test".into(),
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
        session_id: "ses_test".into(),
        preview: command.to_string(),
        timeout_ms,
        output_limit_bytes: 102_400,
    }
}

fn make_invocation_with_output_limit(command: &str, output_limit_bytes: usize) -> ToolInvocation {
    ToolInvocation {
        tool_id: SHELL_TOOL_ID.to_string(),
        arguments: serde_json::json!({"command": command}),
        workspace_id: "test".to_string(),
        session_id: "ses_test".into(),
        preview: command.to_string(),
        timeout_ms: 5000,
        output_limit_bytes,
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
    assert_eq!(result.exit_code, Some(0));
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
async fn shell_exec_supports_workspace_redirection() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("redirect.txt");
    let tool = ShellExecTool::new(dir.path().to_path_buf());
    let invocation = make_invocation("echo SHELL_REDIRECT_OK > redirect.txt");

    let result = tool.invoke(invocation).await.unwrap();

    assert_eq!(result.text, "");
    assert_eq!(
        std::fs::read_to_string(marker).unwrap().trim(),
        "SHELL_REDIRECT_OK"
    );
}

#[tokio::test]
async fn shell_exec_captures_stderr_on_failure() {
    let dir = tempfile::tempdir().unwrap();
    let tool = ShellExecTool::new(dir.path().to_path_buf());
    let invocation = make_invocation("ls nonexistent_dir_xyz");
    let result = tool.invoke(invocation).await.unwrap();
    assert!(result.text.starts_with("exit code"));
    assert!(
        matches!(result.exit_code, Some(code) if code != 0),
        "failed shell command should record a non-zero exit code, got {:?}",
        result.exit_code
    );
    assert!(
        result.text.contains("nonexistent_dir_xyz"),
        "stderr should mention the missing path: got '{}'",
        result.text
    );
}

#[tokio::test]
async fn shell_exec_captures_stdout_when_failure_has_no_stderr() {
    let dir = tempfile::tempdir().unwrap();
    let tool = ShellExecTool::new(dir.path().to_path_buf());
    let invocation = make_invocation("echo stdout-detail && exit 7");
    let result = tool.invoke(invocation).await.unwrap();

    assert_eq!(result.exit_code, Some(7));
    assert!(
        result.text.contains("stdout-detail"),
        "stdout should remain visible when stderr is empty: got '{}'",
        result.text
    );
}

#[tokio::test]
async fn shell_exec_preserves_failure_stderr_tail_when_truncated() {
    let dir = tempfile::tempdir().unwrap();
    let tool = ShellExecTool::new(dir.path().to_path_buf());
    let command = "i=0; while [ $i -lt 20 ]; do printf 'noise-%03d\\n' \"$i\" >&2; i=$((i + 1)); done; printf 'FINAL_STDERR_MARKER\\n' >&2; exit 9";
    let invocation = make_invocation_with_output_limit(command, 80);

    let result = tool.invoke(invocation).await.unwrap();

    assert_eq!(result.exit_code, Some(9));
    assert!(result.truncated);
    assert!(
        result.text.contains("FINAL_STDERR_MARKER"),
        "truncated failure stderr should keep the tail with the actual error: got '{}'",
        result.text
    );
}

#[tokio::test]
async fn shell_exec_preserves_failure_stdout_tail_when_stderr_empty() {
    let dir = tempfile::tempdir().unwrap();
    let tool = ShellExecTool::new(dir.path().to_path_buf());
    let command = "i=0; while [ $i -lt 20 ]; do printf 'noise-%03d\\n' \"$i\"; i=$((i + 1)); done; printf 'FINAL_STDOUT_MARKER\\n'; exit 8";
    let invocation = make_invocation_with_output_limit(command, 80);

    let result = tool.invoke(invocation).await.unwrap();

    assert_eq!(result.exit_code, Some(8));
    assert!(result.truncated);
    assert!(
        result.text.contains("FINAL_STDOUT_MARKER"),
        "truncated failure stdout fallback should keep the tail with the actual error: got '{}'",
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
async fn dropping_shell_exec_future_kills_child_process() {
    let dir = tempfile::tempdir().unwrap();
    let marker = dir.path().join("should_not_exist.txt");
    let command = format!(
        "sh -lc 'sleep 1; printf SHOULD_NOT_EXIST > {}'",
        marker.display()
    );
    let tool = ShellExecTool::new(dir.path().to_path_buf());
    let invocation = make_invocation_with_timeout(&command, 5_000);

    let task = tokio::spawn(async move { tool.invoke(invocation).await });
    tokio::time::sleep(Duration::from_millis(150)).await;
    task.abort();
    let _ = task.await;
    tokio::time::sleep(Duration::from_millis(1_200)).await;

    assert!(
        !marker.exists(),
        "aborting shell.exec should kill the child before it writes the marker"
    );
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
fn risk_mapping_redirection_is_write() {
    let dir = tempfile::tempdir().unwrap();
    let tool = ShellExecTool::new(dir.path().to_path_buf());
    let inv = make_invocation("echo hello > output.txt");
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

// ── Edge-case: piped commands ────────────────────────────────────────

#[test]
fn parse_pipe_is_treated_as_single_token_stream() {
    // parse_command only splits program+args; pipe chars are just args
    let (program, args) = parse_command("ls | grep foo");
    assert_eq!(program, "ls");
    assert_eq!(args, vec!["|", "grep", "foo"]);
}

#[test]
fn parse_multi_pipe() {
    let (program, args) = parse_command("cat file.txt | sort | uniq -c");
    assert_eq!(program, "cat");
    assert_eq!(args, vec!["file.txt", "|", "sort", "|", "uniq", "-c"]);
}

// ── Edge-case: redirections ──────────────────────────────────────────

#[test]
fn parse_output_redirection() {
    let (program, args) = parse_command("echo foo > file.txt");
    assert_eq!(program, "echo");
    assert_eq!(args, vec!["foo", ">", "file.txt"]);
}

#[test]
fn parse_append_redirection() {
    let (program, args) = parse_command("echo bar >> log.txt");
    assert_eq!(program, "echo");
    assert_eq!(args, vec!["bar", ">>", "log.txt"]);
}

#[test]
fn parse_input_redirection() {
    let (program, args) = parse_command("wc -l < input.txt");
    assert_eq!(program, "wc");
    assert_eq!(args, vec!["-l", "<", "input.txt"]);
}

// ── Edge-case: quoted arguments with spaces ──────────────────────────

#[test]
fn parse_git_commit_multi_word_message() {
    let (program, args) = parse_command(r#"git commit -m "multi word message""#);
    assert_eq!(program, "git");
    assert_eq!(args, vec!["commit", "-m", "multi word message"]);
}

#[test]
fn parse_nested_quotes_single_in_double() {
    let (program, args) = parse_command(r#"echo "it's a test""#);
    assert_eq!(program, "echo");
    assert_eq!(args, vec!["it's a test"]);
}

#[test]
fn parse_nested_quotes_double_in_single() {
    let (program, args) = parse_command(r#"echo 'say "hello"'"#);
    assert_eq!(program, "echo");
    assert_eq!(args, vec![r#"say "hello""#]);
}

#[test]
fn parse_escaped_quote_inside_double_quotes() {
    let (program, args) = parse_command(r#"echo "she said \"hi\"""#);
    assert_eq!(program, "echo");
    assert_eq!(args, vec![r#"she said "hi""#]);
}

// ── Edge-case: subshell/compound commands ────────────────────────────

#[test]
fn parse_subshell_parens_are_tokens() {
    // Parentheses aren't special to our tokenizer; treated as part of tokens
    let (program, args) = parse_command("(cd dir && rm -rf .)");
    // The opening paren sticks to the first token
    assert_eq!(program, "(cd");
    assert_eq!(args, vec!["dir", "&&", "rm", "-rf", ".)"]);
}

#[test]
fn parse_and_operator() {
    let (program, args) = parse_command("mkdir foo && cd foo");
    assert_eq!(program, "mkdir");
    assert_eq!(args, vec!["foo", "&&", "cd", "foo"]);
}

#[test]
fn parse_semicolon_separator() {
    let (program, args) = parse_command("echo a; echo b");
    assert_eq!(program, "echo");
    assert_eq!(args, vec!["a;", "echo", "b"]);
}

// ── Edge-case: classify unknown/exotic commands ──────────────────────

#[test]
fn classify_custom_script_is_unknown() {
    assert_eq!(
        classify_command("my-custom-script", &[]),
        CommandRisk::Unknown
    );
}

#[test]
fn classify_path_based_binary_is_unknown() {
    assert_eq!(
        classify_command("/usr/local/bin/exotic", &["--flag"]),
        CommandRisk::Unknown
    );
}

#[test]
fn classify_dot_slash_script_is_unknown() {
    assert_eq!(
        classify_command("./build.sh", &["--release"]),
        CommandRisk::Unknown
    );
}

// ── Edge-case: empty/whitespace classify ─────────────────────────────

#[test]
fn classify_empty_program_is_unknown() {
    assert_eq!(classify_command("", &[]), CommandRisk::Unknown);
}

#[test]
fn classify_whitespace_program_is_unknown() {
    // After trim, whitespace-only becomes empty → Unknown
    assert_eq!(classify_command("   ", &[]), CommandRisk::Unknown);
}

// ── Edge-case: subcommand detection boundaries ───────────────────────

#[test]
fn git_subcommand_boundaries() {
    // Write subcommands
    assert_eq!(classify_command("git", &["push"]), CommandRisk::Write);
    assert_eq!(classify_command("git", &["commit"]), CommandRisk::Write);
    assert_eq!(classify_command("git", &["merge"]), CommandRisk::Write);
    assert_eq!(classify_command("git", &["rebase"]), CommandRisk::Write);
    assert_eq!(classify_command("git", &["reset"]), CommandRisk::Write);
    assert_eq!(
        classify_command("git", &["cherry-pick"]),
        CommandRisk::Write
    );
    assert_eq!(classify_command("git", &["tag"]), CommandRisk::Write);
    assert_eq!(classify_command("git", &["stash"]), CommandRisk::Write);

    // Destructive subcommands
    assert_eq!(
        classify_command("git", &["clean"]),
        CommandRisk::Destructive
    );

    // ReadOnly (base program, no subcommand match)
    assert_eq!(classify_command("git", &["log"]), CommandRisk::ReadOnly);
    assert_eq!(classify_command("git", &["diff"]), CommandRisk::ReadOnly);
    assert_eq!(classify_command("git", &["status"]), CommandRisk::ReadOnly);
    assert_eq!(classify_command("git", &["show"]), CommandRisk::ReadOnly);
    assert_eq!(classify_command("git", &["blame"]), CommandRisk::ReadOnly);
}

#[test]
fn docker_subcommand_boundaries() {
    // Write subcommands
    assert_eq!(classify_command("docker", &["build"]), CommandRisk::Write);
    assert_eq!(classify_command("docker", &["run"]), CommandRisk::Write);
    assert_eq!(classify_command("docker", &["push"]), CommandRisk::Write);
    assert_eq!(classify_command("docker", &["rm"]), CommandRisk::Write);
    assert_eq!(classify_command("docker", &["rmi"]), CommandRisk::Write);
    assert_eq!(classify_command("docker", &["stop"]), CommandRisk::Write);
    assert_eq!(classify_command("docker", &["kill"]), CommandRisk::Write);
    assert_eq!(classify_command("docker", &["compose"]), CommandRisk::Write);

    // Destructive subcommands
    assert_eq!(
        classify_command("docker", &["system"]),
        CommandRisk::Destructive
    );
    assert_eq!(
        classify_command("docker", &["volume"]),
        CommandRisk::Destructive
    );

    // Base Write (docker itself is a Write command, non-matching sub falls to base)
    assert_eq!(classify_command("docker", &["ps"]), CommandRisk::Write);
    assert_eq!(classify_command("docker", &["images"]), CommandRisk::Write);
}

#[test]
fn bun_subcommand_boundaries() {
    // Write subcommands
    assert_eq!(classify_command("bun", &["add"]), CommandRisk::Write);
    assert_eq!(classify_command("bun", &["install"]), CommandRisk::Write);
    assert_eq!(classify_command("bun", &["remove"]), CommandRisk::Write);
    assert_eq!(classify_command("bun", &["update"]), CommandRisk::Write);
    assert_eq!(classify_command("bun", &["publish"]), CommandRisk::Write);
    assert_eq!(classify_command("bun", &["pm"]), CommandRisk::Write);

    // ReadOnly (base program, no subcommand match)
    assert_eq!(
        classify_command("bun", &["run", "test"]),
        CommandRisk::ReadOnly
    );
    assert_eq!(classify_command("bun", &["test"]), CommandRisk::ReadOnly);
}

#[test]
fn cargo_subcommand_boundaries() {
    // Write subcommands
    assert_eq!(classify_command("cargo", &["publish"]), CommandRisk::Write);

    // ReadOnly (base program, no subcommand match)
    assert_eq!(classify_command("cargo", &["build"]), CommandRisk::ReadOnly);
    assert_eq!(classify_command("cargo", &["test"]), CommandRisk::ReadOnly);
    assert_eq!(classify_command("cargo", &["check"]), CommandRisk::ReadOnly);
    assert_eq!(
        classify_command("cargo", &["clippy"]),
        CommandRisk::ReadOnly
    );
}

#[test]
fn kubectl_subcommand_boundaries() {
    // Write subcommands
    assert_eq!(classify_command("kubectl", &["delete"]), CommandRisk::Write);
    assert_eq!(classify_command("kubectl", &["apply"]), CommandRisk::Write);
    assert_eq!(classify_command("kubectl", &["create"]), CommandRisk::Write);
    assert_eq!(classify_command("kubectl", &["edit"]), CommandRisk::Write);
    assert_eq!(classify_command("kubectl", &["patch"]), CommandRisk::Write);

    // Base Write (kubectl itself is a Write command)
    assert_eq!(classify_command("kubectl", &["get"]), CommandRisk::Write);
    assert_eq!(
        classify_command("kubectl", &["describe"]),
        CommandRisk::Write
    );
}

#[test]
fn helm_subcommand_boundaries() {
    // Write subcommands
    assert_eq!(classify_command("helm", &["install"]), CommandRisk::Write);
    assert_eq!(classify_command("helm", &["upgrade"]), CommandRisk::Write);
    assert_eq!(classify_command("helm", &["delete"]), CommandRisk::Write);
    assert_eq!(classify_command("helm", &["rollback"]), CommandRisk::Write);

    // Base Write (helm itself is a Write command)
    assert_eq!(classify_command("helm", &["list"]), CommandRisk::Write);
}

#[test]
fn pip_subcommand_boundaries() {
    assert_eq!(classify_command("pip", &["install"]), CommandRisk::Write);
    assert_eq!(classify_command("pip", &["uninstall"]), CommandRisk::Write);
    assert_eq!(classify_command("pip3", &["install"]), CommandRisk::Write);
    // pip with non-matching subcommand → ReadOnly (base)
    assert_eq!(classify_command("pip", &["list"]), CommandRisk::ReadOnly);
    assert_eq!(classify_command("pip", &["show"]), CommandRisk::ReadOnly);
}

#[test]
fn npm_subcommand_boundaries() {
    assert_eq!(classify_command("npm", &["install"]), CommandRisk::Write);
    assert_eq!(classify_command("npm", &["uninstall"]), CommandRisk::Write);
    assert_eq!(classify_command("npm", &["publish"]), CommandRisk::Write);
    assert_eq!(classify_command("npm", &["update"]), CommandRisk::Write);

    // Non-matching → ReadOnly (base)
    assert_eq!(classify_command("npm", &["list"]), CommandRisk::ReadOnly);
    assert_eq!(classify_command("npm", &["info"]), CommandRisk::ReadOnly);
}

// ── Edge-case: extra args after subcommand don't change risk ─────────

#[test]
fn subcommand_with_extra_args_still_classified_correctly() {
    assert_eq!(
        classify_command("git", &["push", "origin", "main", "--force"]),
        CommandRisk::Write
    );
    assert_eq!(
        classify_command("docker", &["system", "prune", "-a"]),
        CommandRisk::Destructive
    );
    assert_eq!(
        classify_command("kubectl", &["delete", "pod", "my-pod"]),
        CommandRisk::Write
    );
}

// ── Edge-case: parse_command with complex real-world commands ─────────

#[test]
fn parse_command_with_equals_in_arg() {
    let (program, args) = parse_command("cargo test --features=full");
    assert_eq!(program, "cargo");
    assert_eq!(args, vec!["test", "--features=full"]);
}

#[test]
fn parse_command_with_env_var_prefix() {
    // Env var assignment before command — tokenizer doesn't interpret = specially
    let (program, args) = parse_command("RUST_LOG=debug cargo test");
    assert_eq!(program, "RUST_LOG=debug");
    assert_eq!(args, vec!["cargo", "test"]);
}

#[test]
fn parse_command_with_dollar_in_double_quotes() {
    // $var inside double quotes — backslash-dollar is escaped
    let (program, args) = parse_command(r#"echo "price is \$5""#);
    assert_eq!(program, "echo");
    assert_eq!(args, vec!["price is $5"]);
}

#[test]
fn parse_command_with_backtick_in_double_quotes() {
    // Escaped backtick
    let (program, args) = parse_command(r#"echo "use \`cmd\`""#);
    assert_eq!(program, "echo");
    assert_eq!(args, vec!["use `cmd`"]);
}

#[test]
fn parse_command_trailing_backslash() {
    // Trailing backslash at end of input
    let (program, args) = parse_command(r"echo hello\");
    assert_eq!(program, "echo");
    assert_eq!(args, vec![r"hello\"]);
}

#[test]
fn parse_command_multiple_spaces_between_args() {
    let (program, args) = parse_command("ls    -la    /tmp");
    assert_eq!(program, "ls");
    assert_eq!(args, vec!["-la", "/tmp"]);
}

#[test]
fn parse_command_tab_separated() {
    let (program, args) = parse_command("ls\t-la\t/tmp");
    assert_eq!(program, "ls");
    assert_eq!(args, vec!["-la", "/tmp"]);
}
