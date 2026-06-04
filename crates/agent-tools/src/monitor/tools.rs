use crate::permission::ToolRisk;
use crate::registry::{Tool, ToolDefinition, ToolInvocation, ToolOutput};
use agent_core::{SessionId, WorkspaceId};
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;

use super::registry::MonitorRegistry;

pub const MONITOR_START_TOOL_ID: &str = "monitor.start";
pub const MONITOR_STOP_TOOL_ID: &str = "monitor.stop";
pub const MONITOR_LIST_TOOL_ID: &str = "monitor.list";

pub struct MonitorStartTool {
    registry: Arc<MonitorRegistry>,
    workspace_root: Option<PathBuf>,
}

impl MonitorStartTool {
    pub fn new(registry: Arc<MonitorRegistry>) -> Self {
        Self {
            registry,
            workspace_root: None,
        }
    }

    pub fn for_workspace(registry: Arc<MonitorRegistry>, workspace_root: PathBuf) -> Self {
        Self {
            registry,
            workspace_root: Some(workspace_root),
        }
    }
}

#[async_trait]
impl Tool for MonitorStartTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_id: MONITOR_START_TOOL_ID.to_string(),
            description: "Start a background monitor that streams stdout lines as events"
                .to_string(),
            required_capability: "monitor.start".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "command": {
                        "type": "string",
                        "description": "Shell command to run. Each stdout line becomes a monitor event."
                    },
                    "description": {
                        "type": "string",
                        "description": "Short description of what this monitor watches."
                    },
                    "persistent": {
                        "type": "boolean",
                        "description": "If true, run for the session lifetime (no timeout).",
                        "default": false
                    },
                    "timeout_ms": {
                        "type": "integer",
                        "description": "Kill after this many milliseconds. Default 300000 (5 min). Ignored when persistent.",
                        "default": 300000
                    }
                },
                "required": ["command", "description"]
            }),
        }
    }

    fn risk(&self, _invocation: &ToolInvocation) -> ToolRisk {
        ToolRisk::read(MONITOR_START_TOOL_ID)
    }

    async fn invoke(&self, invocation: ToolInvocation) -> crate::Result<ToolOutput> {
        let command = invocation
            .arguments
            .get("command")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::ToolError::ExecutionFailed("missing 'command'".into()))?
            .to_string();

        let description = invocation
            .arguments
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("monitor")
            .to_string();

        let persistent = invocation
            .arguments
            .get("persistent")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        let timeout_ms = invocation
            .arguments
            .get("timeout_ms")
            .and_then(|v| v.as_u64());

        let workspace_id: WorkspaceId = WorkspaceId::from(invocation.workspace_id.clone());
        let session_id = SessionId::from(invocation.session_id.clone());

        let monitor_id = if let Some(workspace_root) = &self.workspace_root {
            self.registry
                .start_in_workspace(
                    workspace_root.clone(),
                    description,
                    command,
                    persistent,
                    timeout_ms,
                    workspace_id,
                    session_id,
                )
                .await?
        } else {
            self.registry
                .start(
                    description,
                    command,
                    persistent,
                    timeout_ms,
                    workspace_id,
                    session_id,
                )
                .await?
        };

        Ok(ToolOutput {
            text: format!("Monitor started: {monitor_id}"),
            truncated: false,
        })
    }
}

pub struct MonitorStopTool {
    registry: Arc<MonitorRegistry>,
}

impl MonitorStopTool {
    pub fn new(registry: Arc<MonitorRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl Tool for MonitorStopTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_id: MONITOR_STOP_TOOL_ID.to_string(),
            description: "Stop a running background monitor".to_string(),
            required_capability: "monitor.stop".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "monitor_id": {
                        "type": "string",
                        "description": "The monitor ID to stop (e.g. mon_1)."
                    }
                },
                "required": ["monitor_id"]
            }),
        }
    }

    fn risk(&self, _invocation: &ToolInvocation) -> ToolRisk {
        ToolRisk::read(MONITOR_STOP_TOOL_ID)
    }

    async fn invoke(&self, invocation: ToolInvocation) -> crate::Result<ToolOutput> {
        let monitor_id = invocation
            .arguments
            .get("monitor_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| crate::ToolError::ExecutionFailed("missing 'monitor_id'".into()))?;

        self.registry.stop(monitor_id).await?;

        Ok(ToolOutput {
            text: format!("Monitor stopped: {monitor_id}"),
            truncated: false,
        })
    }
}

pub struct MonitorListTool {
    registry: Arc<MonitorRegistry>,
}

impl MonitorListTool {
    pub fn new(registry: Arc<MonitorRegistry>) -> Self {
        Self { registry }
    }
}

#[async_trait]
impl Tool for MonitorListTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_id: MONITOR_LIST_TOOL_ID.to_string(),
            description: "List all active background monitors".to_string(),
            required_capability: "monitor.list".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
        }
    }

    fn risk(&self, _invocation: &ToolInvocation) -> ToolRisk {
        ToolRisk::read(MONITOR_LIST_TOOL_ID)
    }

    async fn invoke(&self, _invocation: ToolInvocation) -> crate::Result<ToolOutput> {
        let monitors = self.registry.list().await;

        if monitors.is_empty() {
            return Ok(ToolOutput {
                text: "No active monitors.".into(),
                truncated: false,
            });
        }

        let mut output = String::new();
        for m in &monitors {
            output.push_str(&format!(
                "- {} ({}): persistent={}, timeout={}ms\n",
                m.monitor_id, m.description, m.persistent, m.timeout_ms
            ));
        }

        Ok(ToolOutput {
            text: output,
            truncated: false,
        })
    }
}

#[cfg(test)]
#[path = "tools_tests.rs"]
mod tests;
