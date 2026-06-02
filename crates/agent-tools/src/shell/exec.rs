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
            .stderr(Stdio::piped())
            .kill_on_drop(true);

        apply_sandbox_env(&mut cmd);

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
