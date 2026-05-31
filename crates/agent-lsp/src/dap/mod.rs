use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use tokio::sync::Mutex;

use crate::error::{LspError, Result};
use crate::transport::Transport;
use crate::types::dap::*;
use crate::types::JsonRpcRequest;

/// DAP client wrapping a transport.
///
/// DAP uses the same Content-Length framing as LSP but a different message
/// structure (seq-based rather than JSON-RPC id-based). We bridge the gap
/// by serializing DAP requests as JSON and using the transport's
/// Content-Length framing, then interpreting responses ourselves.
pub struct DapClient {
    server_id: String,
    transport: Arc<Mutex<Box<dyn Transport>>>,
    next_seq: AtomicU64,
    initialized: std::sync::atomic::AtomicBool,
}

impl DapClient {
    pub fn new(server_id: String, transport: Box<dyn Transport>) -> Self {
        Self {
            server_id,
            transport: Arc::new(Mutex::new(transport)),
            next_seq: AtomicU64::new(1),
            initialized: std::sync::atomic::AtomicBool::new(false),
        }
    }

    pub fn server_id(&self) -> &str {
        &self.server_id
    }

    fn next_seq(&self) -> i64 {
        self.next_seq.fetch_add(1, Ordering::SeqCst) as i64
    }

    /// Send a DAP request and parse the response body.
    async fn send_dap_request(
        &self,
        command: &str,
        arguments: Option<serde_json::Value>,
    ) -> Result<Option<serde_json::Value>> {
        let seq = self.next_seq();
        let dap_request = DapRequest {
            seq,
            msg_type: "request".to_string(),
            command: command.to_string(),
            arguments,
        };
        // Serialize DAP request as JSON and send via Content-Length framing.
        // We reuse the JSON-RPC transport by wrapping DAP messages.
        let json_rpc = JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: serde_json::Value::Number(seq.into()),
            method: command.to_string(),
            params: Some(serde_json::to_value(&dap_request)?),
        };

        let mut transport = self.transport.lock().await;
        let response = transport.send_request(json_rpc).await?;

        // Try to parse the JSON-RPC result as a DAP response body.
        if let Some(result) = response.result {
            if let Ok(dap_resp) = serde_json::from_value::<DapResponse>(result.clone()) {
                if !dap_resp.success {
                    return Err(LspError::Protocol(format!(
                        "DAP command '{}' failed: {}",
                        command,
                        dap_resp.message.unwrap_or_default()
                    )));
                }
                return Ok(dap_resp.body);
            }
            return Ok(Some(result));
        }
        Ok(None)
    }

    pub async fn initialize(&self) -> Result<()> {
        let args = serde_json::json!({
            "clientID": "kairox",
            "clientName": "Kairox Agent",
            "adapterID": &self.server_id,
            "linesStartAt1": true,
            "columnsStartAt1": true,
            "pathFormat": "path",
            "supportsVariableType": true,
            "supportsVariablePaging": false,
            "supportsRunInTerminalRequest": false,
        });
        self.send_dap_request("initialize", Some(args)).await?;
        self.initialized
            .store(true, std::sync::atomic::Ordering::SeqCst);
        Ok(())
    }

    pub async fn launch(
        &self,
        program: &str,
        args: &[String],
        cwd: Option<&str>,
        env: Option<&HashMap<String, String>>,
    ) -> Result<()> {
        let mut launch_args = serde_json::json!({
            "program": program,
            "args": args,
            "noDebug": false,
        });
        if let Some(cwd) = cwd {
            launch_args["cwd"] = serde_json::Value::String(cwd.to_string());
        }
        if let Some(env) = env {
            launch_args["env"] = serde_json::to_value(env)?;
        }
        self.send_dap_request("launch", Some(launch_args)).await?;
        Ok(())
    }

    pub async fn set_breakpoints(
        &self,
        source_path: &str,
        lines: &[u32],
    ) -> Result<Vec<Breakpoint>> {
        let breakpoints: Vec<serde_json::Value> = lines
            .iter()
            .map(|&line| serde_json::json!({"line": line}))
            .collect();
        let args = serde_json::json!({
            "source": {"path": source_path},
            "breakpoints": breakpoints,
        });
        let body = self.send_dap_request("setBreakpoints", Some(args)).await?;
        match body {
            Some(body) => {
                let bps = body
                    .get("breakpoints")
                    .cloned()
                    .unwrap_or(serde_json::Value::Array(vec![]));
                serde_json::from_value(bps)
                    .map_err(|e| LspError::Protocol(format!("invalid breakpoints: {e}")))
            }
            None => Ok(vec![]),
        }
    }

    pub async fn continue_execution(&self, thread_id: i64) -> Result<()> {
        let args = serde_json::json!({"threadId": thread_id});
        self.send_dap_request("continue", Some(args)).await?;
        Ok(())
    }

    pub async fn step_over(&self, thread_id: i64) -> Result<()> {
        let args = serde_json::json!({"threadId": thread_id});
        self.send_dap_request("next", Some(args)).await?;
        Ok(())
    }

    pub async fn step_into(&self, thread_id: i64) -> Result<()> {
        let args = serde_json::json!({"threadId": thread_id});
        self.send_dap_request("stepIn", Some(args)).await?;
        Ok(())
    }

    pub async fn stack_trace(&self, thread_id: i64) -> Result<Vec<StackFrame>> {
        let args = serde_json::json!({"threadId": thread_id});
        let body = self.send_dap_request("stackTrace", Some(args)).await?;
        match body {
            Some(body) => {
                let frames = body
                    .get("stackFrames")
                    .cloned()
                    .unwrap_or(serde_json::Value::Array(vec![]));
                serde_json::from_value(frames)
                    .map_err(|e| LspError::Protocol(format!("invalid stackFrames: {e}")))
            }
            None => Ok(vec![]),
        }
    }

    pub async fn scopes(&self, frame_id: i64) -> Result<Vec<Scope>> {
        let args = serde_json::json!({"frameId": frame_id});
        let body = self.send_dap_request("scopes", Some(args)).await?;
        match body {
            Some(body) => {
                let scopes = body
                    .get("scopes")
                    .cloned()
                    .unwrap_or(serde_json::Value::Array(vec![]));
                serde_json::from_value(scopes)
                    .map_err(|e| LspError::Protocol(format!("invalid scopes: {e}")))
            }
            None => Ok(vec![]),
        }
    }

    pub async fn variables(&self, variables_reference: i64) -> Result<Vec<Variable>> {
        let args = serde_json::json!({"variablesReference": variables_reference});
        let body = self.send_dap_request("variables", Some(args)).await?;
        match body {
            Some(body) => {
                let vars = body
                    .get("variables")
                    .cloned()
                    .unwrap_or(serde_json::Value::Array(vec![]));
                serde_json::from_value(vars)
                    .map_err(|e| LspError::Protocol(format!("invalid variables: {e}")))
            }
            None => Ok(vec![]),
        }
    }

    pub async fn evaluate(&self, expression: &str, frame_id: Option<i64>) -> Result<String> {
        let mut args = serde_json::json!({
            "expression": expression,
            "context": "repl",
        });
        if let Some(fid) = frame_id {
            args["frameId"] = serde_json::Value::Number(fid.into());
        }
        let body = self.send_dap_request("evaluate", Some(args)).await?;
        match body {
            Some(body) => Ok(body
                .get("result")
                .and_then(|r| r.as_str())
                .unwrap_or("")
                .to_string()),
            None => Ok(String::new()),
        }
    }

    pub async fn disconnect(&self) -> Result<()> {
        let args = serde_json::json!({"terminateDebuggee": true});
        let _ = self.send_dap_request("disconnect", Some(args)).await;
        let mut transport = self.transport.lock().await;
        transport.close().await
    }
}

#[cfg(test)]
#[path = "dap_tests.rs"]
mod tests;
