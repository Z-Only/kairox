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
mod tests {
    use super::*;
    use crate::types::JsonRpcRequest;
    use serde_json::json;

    /// Helper: a one-line Python script that echoes back JSON-RPC responses.
    ///
    /// Reads lines from stdin. For each line that is a JSON-RPC request (has an
    /// `id` field), it writes `{"jsonrpc":"2.0","id":<id>,"result":<params>}` to
    /// stdout so we can verify round-trip communication.
    const ECHO_SERVER_PY: &str = r#"
import sys, json
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    try:
        msg = json.loads(line)
    except json.JSONDecodeError:
        continue
    if "id" in msg:
        result = msg.get("params", {})
        resp = {"jsonrpc": "2.0", "id": msg["id"], "result": result}
        sys.stdout.write(json.dumps(resp) + "\n")
        sys.stdout.flush()
"#;

    /// Helper: spawn a Python-based echo MCP server.
    async fn spawn_echo_server() -> StdioTransport {
        StdioTransport::spawn("python3", &["-c", ECHO_SERVER_PY], HashMap::new(), None)
            .await
            .expect("failed to spawn echo server")
    }

    #[tokio::test]
    async fn spawn_and_communicate_with_echo_server() {
        let mut transport = spawn_echo_server().await;

        let request = JsonRpcRequest::new(42, "test/method", Some(json!({"key": "value"})));
        let response = transport
            .send_request(request)
            .await
            .expect("send_request failed");

        assert_eq!(response.id, json!(42));
        assert_eq!(response.result, json!({"key": "value"}));

        transport.close().await.expect("close failed");
    }

    #[tokio::test]
    async fn env_variables_passed_to_child() {
        // Use a tiny Python script that writes the value of KAIROX_TEST_VAR
        // as the JSON-RPC result.
        let script = r#"
import os, sys, json
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    msg = json.loads(line)
    if "id" in msg:
        val = os.environ.get("KAIROX_TEST_VAR", "MISSING")
        resp = {"jsonrpc": "2.0", "id": msg["id"], "result": val}
        sys.stdout.write(json.dumps(resp) + "\n")
        sys.stdout.flush()
"#;
        let mut env = HashMap::new();
        env.insert("KAIROX_TEST_VAR".to_string(), "hello-from-env".to_string());

        let mut transport = StdioTransport::spawn("python3", &["-c", script], env, None)
            .await
            .expect("failed to spawn env-test server");

        let request = JsonRpcRequest::new(1, "check_env", None);
        let response = transport
            .send_request(request)
            .await
            .expect("send_request failed");

        assert_eq!(response.result, json!("hello-from-env"));

        transport.close().await.expect("close failed");
    }

    #[tokio::test]
    async fn close_kills_process() {
        let mut transport = spawn_echo_server().await;

        // The child should still be running.
        let status = transport.child.try_wait().expect("try_wait failed");
        assert!(status.is_none(), "child should still be running");

        transport.close().await.expect("close failed");

        // After close, the child should have exited.
        let status = transport.child.try_wait().expect("try_wait failed");
        assert!(status.is_some(), "child should have exited after close");
    }

    #[tokio::test]
    async fn close_on_already_exited_process_is_ok() {
        let mut transport = spawn_echo_server().await;

        // Send a request and then let the child exit by closing its stdin.
        // Drop our stdin writer to close the pipe, then wait for the child.
        {
            // We can't easily drop stdin without going through close, so
            // let's just call close twice — the second call should be a no-op.
            transport.close().await.expect("first close failed");
        }

        // Second close should succeed without error.
        transport
            .close()
            .await
            .expect("second close should succeed");
    }

    #[tokio::test]
    async fn send_notification_does_not_await_response() {
        let mut transport = spawn_echo_server().await;

        let notification = JsonRpcNotification {
            jsonrpc: "2.0".to_string(),
            method: "notifications/cancelled".to_string(),
            params: Some(json!({"reason": "test"})),
        };

        // This should complete immediately without waiting for a response.
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            transport.send_notification(notification),
        )
        .await
        .expect("send_notification timed out")
        .expect("send_notification failed");

        transport.close().await.expect("close failed");
    }

    #[tokio::test]
    async fn send_request_on_closed_stdout_returns_error() {
        let mut transport = spawn_echo_server().await;

        // Close the transport, which kills the child.
        transport.close().await.expect("close failed");

        // Now trying to send a request should fail because stdout is closed.
        let request = JsonRpcRequest::new(99, "after_close", None);
        let result = transport.send_request(request).await;
        assert!(result.is_err(), "send_request after close should fail");
    }

    #[tokio::test]
    async fn stdio_command_construction() {
        // Verify that StdioTransport::spawn constructs the command correctly:
        // - command name is passed
        // - args are forwarded in order
        // - child process starts and can be communicated with
        let script = r#"
import sys, json
for line in sys.stdin:
    line = line.strip()
    if not line:
        continue
    msg = json.loads(line)
    if "id" in msg:
        # Echo the method name back as the result so we can verify round-trip.
        resp = {"jsonrpc": "2.0", "id": msg["id"], "result": msg.get("method", "UNKNOWN")}
        sys.stdout.write(json.dumps(resp) + "\n")
        sys.stdout.flush()
"#;
        let args = &["-c", script, "ignored-extra-arg"];
        let mut transport = StdioTransport::spawn("python3", args, HashMap::new(), None)
            .await
            .expect("failed to spawn echo server with extra arg");

        // Verify the child is alive.
        let status = transport.child.try_wait().expect("try_wait");
        assert!(status.is_none(), "child should be alive after spawn");

        // Communicate to confirm the command was constructed correctly.
        let request = JsonRpcRequest::new(7, "test/method_name", None);
        let response = transport
            .send_request(request)
            .await
            .expect("send_request after construction should work");
        assert_eq!(response.id, json!(7));
        assert_eq!(response.result, json!("test/method_name"));

        transport.close().await.expect("close failed");
    }
}
