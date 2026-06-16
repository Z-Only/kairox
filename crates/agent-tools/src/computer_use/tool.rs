//! [`ComputerUseTool`] — Desktop interaction tool for computer use automation.

use super::platform::DesktopBackend;
use super::types::ComputerAction;
use crate::permission::{ToolEffect, ToolRisk};
use crate::registry::{ImageAttachment, Tool, ToolDefinition, ToolInvocation, ToolOutput};
use async_trait::async_trait;
use std::sync::Arc;

pub const COMPUTER_USE_TOOL_ID: &str = "computer.use";

pub struct ComputerUseTool {
    backend: Arc<DesktopBackend>,
}

impl Default for ComputerUseTool {
    fn default() -> Self {
        Self::new()
    }
}

impl ComputerUseTool {
    pub fn new() -> Self {
        Self {
            backend: Arc::new(DesktopBackend::new()),
        }
    }
}

#[async_trait]
impl Tool for ComputerUseTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_id: COMPUTER_USE_TOOL_ID.to_string(),
            description: "Interact with the desktop: take screenshots, move/click mouse, type text, press keys, and scroll. For desktop application automation and testing.".to_string(),
            required_capability: "computer.interact".to_string(),
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "action": {
                        "type": "string",
                        "description": "The action to perform",
                        "enum": ["screenshot", "mouse_move", "mouse_click", "mouse_drag", "keyboard_type", "key_press", "scroll", "wait", "get_screen_size", "get_cursor_position"]
                    },
                    "x": { "type": "integer", "description": "X coordinate" },
                    "y": { "type": "integer", "description": "Y coordinate" },
                    "from_x": { "type": "integer" },
                    "from_y": { "type": "integer" },
                    "to_x": { "type": "integer" },
                    "to_y": { "type": "integer" },
                    "text": { "type": "string", "description": "Text to type" },
                    "keys": { "type": "string", "description": "Key combination (e.g., 'cmd+c')" },
                    "button": { "type": "string", "description": "Mouse button (left/right/middle)" },
                    "click_count": { "type": "integer", "description": "Number of clicks (1=single, 2=double)" },
                    "direction": { "type": "string", "description": "Scroll direction (up/down/left/right)" },
                    "amount": { "type": "integer", "description": "Scroll amount in pixels" },
                    "duration_ms": { "type": "integer", "description": "Wait duration in ms" },
                    "region": {
                        "type": "array",
                        "description": "Screenshot region [x, y, width, height]",
                        "items": { "type": "integer" }
                    }
                },
                "required": ["action"]
            }),
        }
    }

    fn risk(&self, _invocation: &ToolInvocation) -> ToolRisk {
        ToolRisk {
            tool_id: COMPUTER_USE_TOOL_ID.to_string(),
            effect: ToolEffect::Execute,
        }
    }

    async fn invoke(&self, invocation: ToolInvocation) -> crate::Result<ToolOutput> {
        let action: ComputerAction = serde_json::from_value(invocation.arguments).map_err(|e| {
            crate::ToolError::ExecutionFailed(format!("Invalid computer action: {}", e))
        })?;

        let result = self
            .backend
            .execute(action)
            .await
            .map_err(crate::ToolError::ExecutionFailed)?;

        let (text, images) = format_computer_result(&result);

        Ok(ToolOutput {
            text,
            truncated: false,
            exit_code: None,
            images,
        })
    }
}

/// Format [`ComputerResult`] for the model, returning text and any screenshot
/// as a separate [`ImageAttachment`] so the downstream multimodal pipeline can
/// pass it as a native image content block instead of counting it as text tokens.
pub(crate) fn format_computer_result(
    result: &super::types::ComputerResult,
) -> (String, Vec<ImageAttachment>) {
    use std::fmt::Write;
    let mut output = String::new();
    let mut images = Vec::new();
    let _ = writeln!(output, "success: {}", result.success);
    if !result.output.is_empty() {
        let _ = writeln!(output, "output: {}", result.output);
    }
    if let Some(ref size) = result.screen_size {
        let _ = writeln!(output, "screen_size: {}x{}", size.width, size.height);
    }
    if let Some(ref pos) = result.cursor_position {
        let _ = writeln!(output, "cursor_position: ({}, {})", pos.x, pos.y);
    }
    if let Some(ref screenshot) = result.screenshot {
        let _ = writeln!(output, "screenshot: [image attached]");
        images.push(ImageAttachment {
            media_type: "image/png".to_string(),
            data: screenshot.clone(),
            label: Some("screenshot".to_string()),
        });
    }
    (output, images)
}
