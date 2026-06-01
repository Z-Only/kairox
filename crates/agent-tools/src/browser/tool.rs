//! [`BrowserTool`] — Playwright-backed browser automation tool.

use super::playwright::PlaywrightManager;
use super::types::BrowserAction;
use crate::permission::{ToolEffect, ToolRisk};
use crate::registry::{Tool, ToolDefinition, ToolInvocation, ToolOutput};
use async_trait::async_trait;
use std::path::PathBuf;
use std::sync::Arc;

pub const BROWSER_TOOL_ID: &str = "browser.action";

pub struct BrowserTool {
    manager: Arc<PlaywrightManager>,
}

impl BrowserTool {
    pub fn new(workspace_root: PathBuf) -> Self {
        Self {
            manager: Arc::new(PlaywrightManager::new(workspace_root)),
        }
    }

    /// Returns the shared `PlaywrightManager` for use by related tools.
    pub fn manager(&self) -> Arc<PlaywrightManager> {
        self.manager.clone()
    }
}

#[async_trait]
impl Tool for BrowserTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_id: BROWSER_TOOL_ID.to_string(),
            description: "Interact with a web browser via Playwright. Supports navigation, clicking, typing, screenshots, and page inspection.".to_string(),
            required_capability: "browser.interact".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "description": "The action to perform",
                        "enum": ["navigate", "click", "type", "scroll", "hover", "screenshot", "get_text", "wait", "form_fill", "get_state", "close"]
                    },
                    "url": { "type": "string", "description": "URL for navigate action" },
                    "selector": { "type": "string", "description": "CSS selector or element ref" },
                    "text": { "type": "string", "description": "Text to type" },
                    "value": { "type": "string", "description": "Value for form_fill" },
                    "direction": { "type": "string", "description": "Scroll direction (up/down/left/right)" },
                    "amount": { "type": "integer", "description": "Scroll amount in pixels" },
                    "full_page": { "type": "boolean", "description": "Whether to capture full page screenshot" },
                    "timeout_ms": { "type": "integer", "description": "Timeout in milliseconds for wait" }
                },
                "required": ["action"]
            }),
        }
    }

    fn risk(&self, _invocation: &ToolInvocation) -> ToolRisk {
        ToolRisk {
            tool_id: BROWSER_TOOL_ID.to_string(),
            effect: ToolEffect::BrowserInteract,
        }
    }

    async fn invoke(&self, invocation: ToolInvocation) -> crate::Result<ToolOutput> {
        let action: BrowserAction = serde_json::from_value(invocation.arguments).map_err(|e| {
            crate::ToolError::ExecutionFailed(format!("Invalid browser action: {}", e))
        })?;

        let result = self
            .manager
            .execute(action)
            .await
            .map_err(crate::ToolError::ExecutionFailed)?;

        let text = serde_json::to_string_pretty(&result)
            .map_err(|e| crate::ToolError::ExecutionFailed(e.to_string()))?;

        Ok(ToolOutput {
            text,
            truncated: false,
        })
    }
}
