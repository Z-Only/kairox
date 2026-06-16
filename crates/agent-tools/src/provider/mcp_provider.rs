use crate::permission::{ToolEffect, ToolRisk};
use crate::registry::{Tool, ToolDefinition, ToolInvocation, ToolOutput};
use agent_mcp::McpClient;
use agent_mcp::McpToolDef;
use async_trait::async_trait;
use std::sync::Arc;

pub struct McpToolAdapter {
    server_id: String,
    tool_def: McpToolDef,
    client: Arc<McpClient>,
}

impl McpToolAdapter {
    pub fn new(server_id: String, tool_def: McpToolDef, client: Arc<McpClient>) -> Self {
        Self {
            server_id,
            tool_def,
            client,
        }
    }
}

#[async_trait]
impl Tool for McpToolAdapter {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_id: format!("mcp.{}.{}", self.server_id, self.tool_def.name),
            description: self.tool_def.description.clone().unwrap_or_default(),
            required_capability: "mcp.invoke".into(),
            parameters: self
                .tool_def
                .input_schema
                .as_ref()
                .and_then(|s| serde_json::from_str(s).ok())
                .unwrap_or_else(|| serde_json::json!({"type": "object"})),
        }
    }

    fn risk(&self, _invocation: &ToolInvocation) -> ToolRisk {
        ToolRisk {
            tool_id: format!("mcp.{}.{}", self.server_id, self.tool_def.name),
            effect: ToolEffect::McpInvoke,
        }
    }

    async fn invoke(&self, invocation: ToolInvocation) -> crate::Result<ToolOutput> {
        let result = self
            .client
            .call_tool(&self.tool_def.name, invocation.arguments)
            .await
            .map_err(|e| crate::ToolError::ExecutionFailed(e.to_string()))?;

        let text: String = result
            .content
            .iter()
            .map(|block| match block {
                agent_mcp::McpContentBlock::Text { text } => text.clone(),
                agent_mcp::McpContentBlock::Image { data, .. } => {
                    format!("[image: {} bytes]", data.len())
                }
                agent_mcp::McpContentBlock::Resource { resource } => {
                    format!("[resource: {}]", resource.uri)
                }
            })
            .collect::<Vec<_>>()
            .join("\n");

        Ok(ToolOutput {
            text,
            truncated: result.is_error.unwrap_or(false),
            exit_code: None,
            images: vec![],
        })
    }
}

#[cfg(test)]
#[path = "mcp_provider_tests.rs"]
mod tests;
