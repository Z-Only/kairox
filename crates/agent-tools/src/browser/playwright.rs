//! Playwright process management.
//!
//! Manages a Playwright browser instance via a Node.js helper script that
//! communicates over stdin/stdout using a JSON-line protocol. Each request
//! is a single JSON line containing `{ "id": <u64>, "action": <BrowserAction> }`.
//! The bridge responds with `{ "id": <u64>, "result": <BrowserResult> }` or
//! `{ "id": <u64>, "error": "<message>" }`.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::Mutex;

use super::types::{BrowserAction, BrowserResult, BrowserState};

/// The Node.js bridge script, embedded at compile time.
const BRIDGE_SCRIPT: &str = include_str!("playwright_bridge.js");

/// Manages a Playwright browser instance via a Node.js helper script.
pub struct PlaywrightManager {
    state: Mutex<BrowserState>,
    process: Mutex<Option<BridgeProcess>>,
    workspace_root: PathBuf,
    next_id: AtomicU64,
}

/// Holds the child process and its I/O handles.
struct BridgeProcess {
    child: Child,
    stdin: tokio::process::ChildStdin,
    reader: BufReader<tokio::process::ChildStdout>,
}

impl PlaywrightManager {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            state: Mutex::new(BrowserState::NotStarted),
            process: Mutex::new(None),
            workspace_root,
            next_id: AtomicU64::new(1),
        }
    }

    /// Ensure the Node.js bridge process is running.
    pub async fn ensure_running(&self) -> Result<(), String> {
        let mut state = self.state.lock().await;
        match &*state {
            BrowserState::Running => return Ok(()),
            BrowserState::NotStarted | BrowserState::Closed | BrowserState::Error(_) => {}
        }

        // Find Node.js binary
        let node_path = find_node().map_err(|e| {
            let msg = format!(
                "Node.js not found: {}. Install Node.js >= 18 to use browser tools.",
                e
            );
            *state = BrowserState::Error(msg.clone());
            msg
        })?;

        // Write the bridge script to a temp file
        let script_dir = self.workspace_root.join(".kairox").join("tmp");
        tokio::fs::create_dir_all(&script_dir)
            .await
            .map_err(|e| format!("Failed to create temp dir: {}", e))?;
        let script_path = script_dir.join("playwright_bridge.js");
        tokio::fs::write(&script_path, BRIDGE_SCRIPT)
            .await
            .map_err(|e| format!("Failed to write bridge script: {}", e))?;

        // Spawn the Node.js process
        let mut child = Command::new(&node_path)
            .arg(&script_path)
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .current_dir(&self.workspace_root)
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| {
                let msg = format!("Failed to spawn Node.js bridge: {}", e);
                *state = BrowserState::Error(msg.clone());
                msg
            })?;

        let stdin = child
            .stdin
            .take()
            .ok_or_else(|| "Failed to capture bridge stdin".to_string())?;
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| "Failed to capture bridge stdout".to_string())?;
        let stderr = child.stderr.take();

        // Wait for the "ready" signal on stderr (with timeout)
        if let Some(stderr_stream) = stderr {
            let mut stderr_reader = BufReader::new(stderr_stream);
            let mut ready_line = String::new();
            let ready_result = tokio::time::timeout(
                std::time::Duration::from_secs(15),
                stderr_reader.read_line(&mut ready_line),
            )
            .await;

            match ready_result {
                Ok(Ok(_)) if ready_line.contains("ready") => {
                    // Bridge is ready
                }
                Ok(Ok(_)) => {
                    // Got a line but not "ready" — might be an error
                    let trimmed = ready_line.trim();
                    if !trimmed.is_empty() {
                        let msg = format!(
                            "Playwright bridge startup error: {}. \
                             Ensure Playwright is installed: npx playwright install chromium",
                            trimmed
                        );
                        *state = BrowserState::Error(msg.clone());
                        let _ = child.kill().await;
                        return Err(msg);
                    }
                }
                Ok(Err(e)) => {
                    let msg = format!("Failed to read bridge stderr: {}", e);
                    *state = BrowserState::Error(msg.clone());
                    let _ = child.kill().await;
                    return Err(msg);
                }
                Err(_) => {
                    let msg = "Playwright bridge startup timed out (15s). \
                               Ensure Node.js and Playwright are installed correctly."
                        .to_string();
                    *state = BrowserState::Error(msg.clone());
                    let _ = child.kill().await;
                    return Err(msg);
                }
            }
        }

        let reader = BufReader::new(stdout);
        let mut proc = self.process.lock().await;
        *proc = Some(BridgeProcess {
            child,
            stdin,
            reader,
        });
        *state = BrowserState::Running;
        Ok(())
    }

    /// Execute a browser action by sending it to the Node.js bridge.
    pub async fn execute(&self, action: BrowserAction) -> Result<BrowserResult, String> {
        self.ensure_running().await?;

        let request_id = self.next_id.fetch_add(1, Ordering::Relaxed);

        let request = serde_json::json!({
            "id": request_id,
            "action": action,
        });

        let mut request_line = serde_json::to_string(&request)
            .map_err(|e| format!("Failed to serialize request: {}", e))?;
        request_line.push('\n');

        let mut proc_guard = self.process.lock().await;
        let bridge = proc_guard
            .as_mut()
            .ok_or_else(|| "Bridge process not running".to_string())?;

        // Send request
        bridge
            .stdin
            .write_all(request_line.as_bytes())
            .await
            .map_err(|e| format!("Failed to write to bridge: {}", e))?;
        bridge
            .stdin
            .flush()
            .await
            .map_err(|e| format!("Failed to flush bridge stdin: {}", e))?;

        // Read response (with timeout)
        let mut response_line = String::new();
        let read_result = tokio::time::timeout(
            std::time::Duration::from_secs(60),
            bridge.reader.read_line(&mut response_line),
        )
        .await;

        match read_result {
            Ok(Ok(0)) => {
                // EOF — process died
                let mut state = self.state.lock().await;
                *state = BrowserState::Error("Bridge process terminated unexpectedly".into());
                drop(proc_guard);
                Err("Playwright bridge process terminated unexpectedly. \
                     Check that Playwright is installed: npx playwright install chromium"
                    .to_string())
            }
            Ok(Ok(_)) => {
                let response: serde_json::Value = serde_json::from_str(response_line.trim())
                    .map_err(|e| format!("Invalid bridge response: {}", e))?;

                if let Some(error) = response.get("error").and_then(|v| v.as_str()) {
                    // If the action was "close", update state even on error
                    if matches!(action, BrowserAction::Close {}) {
                        let mut state = self.state.lock().await;
                        *state = BrowserState::Closed;
                    }
                    return Err(error.to_string());
                }

                let result: BrowserResult = serde_json::from_value(
                    response
                        .get("result")
                        .cloned()
                        .unwrap_or(serde_json::Value::Null),
                )
                .map_err(|e| format!("Failed to parse bridge result: {}", e))?;

                // Update state on close
                if matches!(action, BrowserAction::Close {}) {
                    let mut state = self.state.lock().await;
                    *state = BrowserState::Closed;
                }

                Ok(result)
            }
            Ok(Err(e)) => Err(format!("Failed to read bridge response: {}", e)),
            Err(_) => Err("Browser action timed out after 60s".to_string()),
        }
    }

    /// Shut down the browser process.
    pub async fn shutdown(&self) {
        let mut proc = self.process.lock().await;
        if let Some(ref mut bridge) = *proc {
            // Try graceful close first
            let close_req = serde_json::json!({
                "id": 0,
                "action": { "action": "close" },
            });
            if let Ok(mut line) = serde_json::to_string(&close_req) {
                line.push('\n');
                let _ = bridge.stdin.write_all(line.as_bytes()).await;
                let _ = bridge.stdin.flush().await;
                // Give it a moment to close gracefully
                let _ = tokio::time::timeout(std::time::Duration::from_secs(3), async {
                    let mut buf = String::new();
                    let _ = bridge.reader.read_line(&mut buf).await;
                })
                .await;
            }
            let _ = bridge.child.kill().await;
        }
        *proc = None;
        let mut state = self.state.lock().await;
        *state = BrowserState::Closed;
    }
}

/// Locate the `node` binary on the system PATH.
fn find_node() -> Result<PathBuf, String> {
    which::which("node").map_err(|_| {
        "Could not find 'node' on PATH. \
         Install Node.js >= 18: https://nodejs.org/"
            .to_string()
    })
}
