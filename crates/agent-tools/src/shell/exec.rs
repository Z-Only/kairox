//! [`ShellExecTool`] — the workspace-rooted shell command executor.

use crate::permission::ToolRisk;
use crate::registry::{Tool, ToolDefinition, ToolInvocation, ToolOutput};
use crate::ToolError::{ExecutionFailed, Timeout};
use async_trait::async_trait;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::Duration;

use super::parse::parse_command;
use super::risk::{classify_command, CommandRisk};
use super::sandbox::{
    apply_sandbox_env, default_max_output_bytes, default_timeout, truncate_bytes,
};
use super::SHELL_TOOL_ID;

pub struct ShellExecTool {
    workspace_root: PathBuf,
    pub(super) default_timeout: Duration,
    max_output_bytes: usize,
}

impl ShellExecTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            workspace_root,
            default_timeout: default_timeout(),
            max_output_bytes: default_max_output_bytes(),
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
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "The shell command to execute"
                    }
                },
                "required": ["command"]
            }),
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
        let mut risk = classify_command(&program, &arg_refs);
        if uses_shell_control_syntax(command) && !matches!(risk, CommandRisk::Destructive) {
            risk = CommandRisk::Write;
        }

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

        if command.trim().is_empty() {
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

        let mut cmd = shell_command(command);
        cmd.current_dir(&self.workspace_root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        apply_sandbox_env(&mut cmd);

        let child = cmd.spawn().map_err(|e| ExecutionFailed(e.to_string()))?;
        let mut cleanup = ProcessCleanup::new(&child);
        let mut output_fut = Box::pin(child.wait_with_output());

        let result = tokio::time::timeout(timeout_duration, &mut output_fut).await;

        match result {
            Ok(Ok(output)) => {
                cleanup.disarm();
                if output.status.success() {
                    let (text, truncated) = truncate_bytes(&output.stdout, output_limit);
                    Ok(ToolOutput {
                        text: String::from_utf8_lossy(&text).to_string(),
                        truncated,
                        exit_code: Some(0),
                        images: vec![],
                    })
                } else {
                    let exit_code = output.status.code().unwrap_or(-1);
                    let (stderr, stderr_truncated) = truncate_bytes(&output.stderr, output_limit);
                    let stderr_text = String::from_utf8_lossy(&stderr).to_string();
                    let (failure_text, truncated) = if stderr_text.trim().is_empty() {
                        let (stdout, stdout_truncated) =
                            truncate_bytes(&output.stdout, output_limit);
                        (
                            String::from_utf8_lossy(&stdout).to_string(),
                            stdout_truncated,
                        )
                    } else {
                        (stderr_text, stderr_truncated)
                    };
                    Ok(ToolOutput {
                        text: format!(
                            "exit code {}: {}{}",
                            exit_code,
                            failure_text,
                            if truncated { " [truncated]" } else { "" }
                        ),
                        truncated,
                        exit_code: Some(exit_code),
                        images: vec![],
                    })
                }
            }
            Ok(Err(e)) => {
                cleanup.disarm();
                Err(ExecutionFailed(e.to_string()))
            }
            Err(_) => {
                cleanup.kill();
                let _ = tokio::time::timeout(Duration::from_secs(1), &mut output_fut).await;
                cleanup.disarm();
                Err(Timeout(timeout_duration.as_millis() as u64))
            }
        }
    }
}

#[cfg(windows)]
fn shell_command(command: &str) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new("cmd");
    cmd.arg("/C").arg(command);
    cmd
}

#[cfg(not(windows))]
fn shell_command(command: &str) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new("sh");
    cmd.arg("-c").arg(command);
    cmd.process_group(0);
    cmd
}

struct ProcessCleanup {
    #[cfg(unix)]
    pgid: Option<i32>,
}

impl ProcessCleanup {
    fn new(child: &tokio::process::Child) -> Self {
        Self {
            #[cfg(unix)]
            pgid: child.id().map(|id| id as i32),
        }
    }

    fn disarm(&mut self) {
        #[cfg(unix)]
        {
            self.pgid = None;
        }
    }

    fn kill(&mut self) {
        #[cfg(unix)]
        if let Some(pgid) = self.pgid.take() {
            kill_process_group(pgid);
        }
    }
}

impl Drop for ProcessCleanup {
    fn drop(&mut self) {
        self.kill();
    }
}

#[cfg(unix)]
fn kill_process_group(pgid: i32) {
    // SAFETY: `pgid` is derived from the spawned shell child pid after placing
    // that child in its own process group. The group may have already exited.
    unsafe {
        libc::kill(-pgid, libc::SIGKILL);
    }
}

fn uses_shell_control_syntax(command: &str) -> bool {
    let mut in_single = false;
    let mut in_double = false;
    let mut escaped = false;

    for ch in command.chars() {
        if escaped {
            escaped = false;
            continue;
        }

        if ch == '\\' {
            escaped = true;
            continue;
        }

        if in_single {
            if ch == '\'' {
                in_single = false;
            }
            continue;
        }

        if in_double {
            if ch == '"' {
                in_double = false;
            }
            continue;
        }

        match ch {
            '\'' => in_single = true,
            '"' => in_double = true,
            '>' | '<' | '|' | ';' | '&' => return true,
            _ => {}
        }
    }

    false
}
