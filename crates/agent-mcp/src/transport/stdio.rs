//! Stdio transport for MCP.
//!
//! Communicates with an MCP server by launching it as a child process and
//! exchanging line-delimited JSON-RPC messages over its stdin/stdout pipes.

use std::collections::HashMap;
use std::process::Stdio as StdioPipes;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::task::JoinHandle;

use crate::transport::Transport;
use crate::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse, McpError, Result};

/// Transport that communicates with an MCP server over stdin/stdout.
///
/// Spawns the server as a child process and exchanges line-delimited JSON-RPC
/// messages. Only one request may be in flight at a time (enforced by
/// `&mut self` on the [`Transport`] trait), so request/response correlation
/// is trivial.
pub struct StdioTransport {
    /// Buffered writer to the child's stdin.
    stdin: BufWriter<ChildStdin>,
    /// Buffered reader from the child's stdout.
    stdout: BufReader<ChildStdout>,
    /// The child process handle (used to kill / wait on close).
    child: Child,
    /// Handle for the background task that drains stderr.
    _stderr_handle: JoinHandle<()>,
}

impl StdioTransport {
    /// Spawn a child process and set up stdin/stdout pipes for JSON-RPC.
    ///
    /// The child's stderr is captured and logged via [`tracing::warn!`].
    pub async fn spawn(
        command: &str,
        args: &[&str],
        env: HashMap<String, String>,
        cwd: Option<&str>,
    ) -> Result<Self> {
        let mut cmd = Command::new(command);
        cmd.args(args)
            .envs(&env)
            .stdin(StdioPipes::piped())
            .stdout(StdioPipes::piped())
            .stderr(StdioPipes::piped())
            .kill_on_drop(true);

        if let Some(dir) = cwd {
            cmd.current_dir(dir);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| McpError::Transport(format!("failed to spawn '{command}': {e}")))?;

        let stdin = child.stdin.take().ok_or_else(|| {
            McpError::Transport("failed to capture stdin of child process".into())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            McpError::Transport("failed to capture stdout of child process".into())
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            McpError::Transport("failed to capture stderr of child process".into())
        })?;

        let stdin = BufWriter::new(stdin);
        let stdout = BufReader::new(stdout);

        // Spawn a background task to drain stderr and log each line.
        let stderr_handle = tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::warn!(target: "mcp::stderr", "{}", line);
            }
        });

        Ok(Self {
            stdin,
            stdout,
            child,
            _stderr_handle: stderr_handle,
        })
    }
}

#[async_trait::async_trait]
impl Transport for StdioTransport {
    async fn send_request(&mut self, request: JsonRpcRequest) -> Result<JsonRpcResponse> {
        // Serialize request and write to stdin.
        let mut line = serde_json::to_string(&request)?;
        line.push('\n');
        self.stdin.write_all(line.as_bytes()).await?;
        self.stdin.flush().await?;

        // Read one response line from stdout.
        let mut response_line = String::new();
        let bytes_read = self.stdout.read_line(&mut response_line).await?;
        if bytes_read == 0 {
            return Err(McpError::Transport(
                "child process stdout closed (EOF)".into(),
            ));
        }

        // Deserialize the response.
        let trimmed = response_line.trim_end();
        let response: JsonRpcResponse = serde_json::from_str(trimmed)?;
        Ok(response)
    }

    async fn send_notification(&mut self, notification: JsonRpcNotification) -> Result<()> {
        let mut line = serde_json::to_string(&notification)?;
        line.push('\n');
        self.stdin.write_all(line.as_bytes()).await?;
        self.stdin.flush().await?;
        Ok(())
    }

    async fn close(&mut self) -> Result<()> {
        // If the child has already exited, there is nothing to do.
        match self.child.try_wait() {
            Ok(Some(_status)) => return Ok(()),
            Ok(None) => {} // still running — proceed to kill
            Err(e) => {
                // Couldn't query the child; try killing anyway.
                tracing::debug!(target: "mcp::stdio", "try_wait failed: {e}");
            }
        }

        self.child
            .kill()
            .await
            .map_err(|e| McpError::Transport(format!("failed to kill child process: {e}")))?;

        // Reap the child to avoid zombies.
        let _ = self.child.wait().await;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
#[path = "stdio_tests.rs"]
mod tests;
