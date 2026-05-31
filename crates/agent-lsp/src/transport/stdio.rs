use std::collections::HashMap;
use std::process::Stdio as StdioPipes;

use async_trait::async_trait;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};
use tokio::task::JoinHandle;

use crate::error::{LspError, Result};
use crate::types::{JsonRpcNotification, JsonRpcRequest, JsonRpcResponse};

use super::Transport;

/// Stdio transport using Content-Length framing (LSP/DAP base protocol).
///
/// Messages are framed as:
/// ```text
/// Content-Length: <byte-count>\r\n
/// \r\n
/// <json-payload>
/// ```
pub struct LspStdioTransport {
    stdin: BufWriter<ChildStdin>,
    stdout: BufReader<ChildStdout>,
    child: Child,
    _stderr_handle: JoinHandle<()>,
}

impl LspStdioTransport {
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
            .map_err(|e| LspError::Transport(format!("failed to spawn '{command}': {e}")))?;

        let stdin = child.stdin.take().ok_or_else(|| {
            LspError::Transport("failed to capture stdin of child process".into())
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            LspError::Transport("failed to capture stdout of child process".into())
        })?;
        let stderr = child.stderr.take().ok_or_else(|| {
            LspError::Transport("failed to capture stderr of child process".into())
        })?;

        let stdin = BufWriter::new(stdin);
        let stdout = BufReader::new(stdout);

        let stderr_handle = tokio::spawn(async move {
            let reader = BufReader::new(stderr);
            let mut lines = reader.lines();
            while let Ok(Some(line)) = lines.next_line().await {
                tracing::warn!(target: "lsp::stderr", "{}", line);
            }
        });

        Ok(Self {
            stdin,
            stdout,
            child,
            _stderr_handle: stderr_handle,
        })
    }

    /// Write a Content-Length framed message to stdin.
    async fn write_message(&mut self, payload: &[u8]) -> Result<()> {
        let header = format!("Content-Length: {}\r\n\r\n", payload.len());
        self.stdin
            .write_all(header.as_bytes())
            .await
            .map_err(|e| LspError::Transport(format!("failed to write header: {e}")))?;
        self.stdin
            .write_all(payload)
            .await
            .map_err(|e| LspError::Transport(format!("failed to write payload: {e}")))?;
        self.stdin
            .flush()
            .await
            .map_err(|e| LspError::Transport(format!("failed to flush: {e}")))?;
        Ok(())
    }

    /// Read a Content-Length framed message from stdout.
    async fn read_message(&mut self) -> Result<serde_json::Value> {
        let content_length = self.read_headers().await?;
        let mut buf = vec![0u8; content_length];
        self.stdout
            .read_exact(&mut buf)
            .await
            .map_err(|e| LspError::Transport(format!("failed to read payload: {e}")))?;
        serde_json::from_slice(&buf).map_err(|e| {
            LspError::Protocol(format!(
                "invalid JSON in payload: {e}: {}",
                String::from_utf8_lossy(&buf)
            ))
        })
    }

    /// Parse headers and return Content-Length value.
    async fn read_headers(&mut self) -> Result<usize> {
        let mut content_length: Option<usize> = None;
        loop {
            let mut line = String::new();
            let bytes_read = self
                .stdout
                .read_line(&mut line)
                .await
                .map_err(|e| LspError::Transport(format!("failed to read header line: {e}")))?;
            if bytes_read == 0 {
                return Err(LspError::Transport("unexpected EOF reading headers".into()));
            }
            let trimmed = line.trim();
            if trimmed.is_empty() {
                break;
            }
            if let Some(value) = trimmed.strip_prefix("Content-Length:") {
                content_length = Some(
                    value
                        .trim()
                        .parse::<usize>()
                        .map_err(|e| LspError::Protocol(format!("invalid Content-Length: {e}")))?,
                );
            }
        }
        content_length.ok_or_else(|| LspError::Protocol("missing Content-Length header".into()))
    }
}

#[async_trait]
impl Transport for LspStdioTransport {
    async fn send_request(&mut self, request: JsonRpcRequest) -> Result<JsonRpcResponse> {
        let payload = serde_json::to_vec(&request)?;
        self.write_message(&payload).await?;

        loop {
            let value = self.read_message().await?;
            // Skip notifications (no "id" field) — wait for matching response.
            if value.get("id").is_some() {
                if let Some(error) = value.get("error").and_then(|e| e.as_object()) {
                    let code = error.get("code").and_then(|c| c.as_i64()).unwrap_or(-1);
                    let message = error
                        .get("message")
                        .and_then(|m| m.as_str())
                        .unwrap_or("unknown error");
                    return Err(LspError::JsonRpc {
                        code,
                        message: message.to_string(),
                    });
                }
                return serde_json::from_value(value)
                    .map_err(|e| LspError::Protocol(format!("invalid JSON-RPC response: {e}")));
            }
            // Server-initiated notification/request — log and continue.
            if let Some(method) = value.get("method").and_then(|m| m.as_str()) {
                tracing::debug!(target: "lsp::notification", method = method, "server notification");
            }
        }
    }

    async fn send_notification(&mut self, notification: JsonRpcNotification) -> Result<()> {
        let payload = serde_json::to_vec(&notification)?;
        self.write_message(&payload).await
    }

    async fn close(&mut self) -> Result<()> {
        let _ = self.child.kill().await;
        Ok(())
    }
}

#[cfg(test)]
#[path = "stdio_tests.rs"]
mod tests;
