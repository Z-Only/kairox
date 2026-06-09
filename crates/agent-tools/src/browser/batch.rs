//! [`BrowserBatchTool`] — execute multiple browser actions in a single invocation.

use super::playwright::PlaywrightManager;
use super::types::BrowserAction;
use crate::permission::{ToolEffect, ToolRisk};
use crate::registry::{Tool, ToolDefinition, ToolInvocation, ToolOutput};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

pub const BROWSER_BATCH_TOOL_ID: &str = "browser.batch";

/// Executes a batch of browser actions in sequence, returning results for each.
/// Reduces API round-trips for deterministic multi-step operations.
pub struct BrowserBatchTool {
    manager: Arc<PlaywrightManager>,
}

impl BrowserBatchTool {
    pub fn new(manager: Arc<PlaywrightManager>) -> Self {
        Self { manager }
    }
}

#[async_trait]
impl Tool for BrowserBatchTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_id: BROWSER_BATCH_TOOL_ID.to_string(),
            description: "Execute multiple browser actions in sequence without intermediate screenshots. Use for deterministic multi-step operations to reduce round-trips.".to_string(),
            required_capability: "browser.interact".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "actions": {
                        "type": "array",
                        "description": "Ordered list of browser actions to execute",
                        "items": {
                            "type": "object",
                            "properties": {
                                "action": { "type": "string" },
                                "url": { "type": "string" },
                                "selector": { "type": "string" },
                                "text": { "type": "string" },
                                "value": { "type": "string" },
                                "direction": { "type": "string" },
                                "amount": { "type": "integer" },
                                "full_page": { "type": "boolean" },
                                "timeout_ms": { "type": "integer" }
                            },
                            "required": ["action"]
                        }
                    },
                    "stop_on_error": {
                        "type": "boolean",
                        "description": "Whether to stop executing on first error (default: true)"
                    }
                },
                "required": ["actions"]
            }),
        }
    }

    fn risk(&self, _invocation: &ToolInvocation) -> ToolRisk {
        ToolRisk {
            tool_id: BROWSER_BATCH_TOOL_ID.to_string(),
            effect: ToolEffect::BrowserInteract,
        }
    }

    async fn invoke(&self, invocation: ToolInvocation) -> crate::Result<ToolOutput> {
        let actions: Vec<BrowserAction> = invocation
            .arguments
            .get("actions")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .ok_or_else(|| {
                crate::ToolError::ExecutionFailed("Missing or invalid 'actions' array".into())
            })?;

        let stop_on_error = invocation
            .arguments
            .get("stop_on_error")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        let mut results: Vec<BatchStepResult> = Vec::with_capacity(actions.len());

        for (index, action) in actions.into_iter().enumerate() {
            let action_desc = format!("{:?}", action);
            match self.manager.execute(action).await {
                Ok(result) => {
                    let success = result.success;
                    results.push(BatchStepResult {
                        index,
                        action: action_desc,
                        success,
                        output: result.output,
                        error: None,
                    });
                    if !success && stop_on_error {
                        break;
                    }
                }
                Err(e) => {
                    results.push(BatchStepResult {
                        index,
                        action: action_desc,
                        success: false,
                        output: String::new(),
                        error: Some(e),
                    });
                    if stop_on_error {
                        break;
                    }
                }
            }
        }

        let summary = BatchResult {
            total: results.len(),
            succeeded: results.iter().filter(|r| r.success).count(),
            failed: results.iter().filter(|r| !r.success).count(),
            steps: results,
        };

        let text = serde_json::to_string_pretty(&summary)
            .map_err(|e| crate::ToolError::ExecutionFailed(e.to_string()))?;

        Ok(ToolOutput {
            text,
            truncated: false,
            images: vec![],
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BatchStepResult {
    index: usize,
    action: String,
    success: bool,
    output: String,
    error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct BatchResult {
    total: usize,
    succeeded: usize,
    failed: usize,
    steps: Vec<BatchStepResult>,
}

#[cfg(test)]
#[path = "batch_tests.rs"]
mod tests;
