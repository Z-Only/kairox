//! Invocation requests — `tools/call`, `resources/read`, `prompts/get`.

use super::McpClient;
use crate::protocol::JsonRpcRequest;
use crate::types::*;
use crate::{McpError, Result};

impl McpClient {
    /// Call a tool on the server.
    pub async fn call_tool(
        &self,
        name: &str,
        arguments: serde_json::Value,
    ) -> Result<McpToolResult> {
        let id = self.next_id();
        let params = serde_json::json!({
            "name": name,
            "arguments": arguments,
        });
        let request = JsonRpcRequest::new(id, "tools/call", Some(params));
        let response = self.send_request(request).await?;
        let result: McpToolResult = serde_json::from_value(response.result)
            .map_err(|e| McpError::Protocol(format!("invalid tool result: {e}")))?;
        Ok(result)
    }

    /// Read a resource from the server.
    pub async fn read_resource(&self, uri: &str) -> Result<Vec<McpContentBlock>> {
        let id = self.next_id();
        let params = serde_json::json!({
            "uri": uri,
        });
        let request = JsonRpcRequest::new(id, "resources/read", Some(params));
        let response = self.send_request(request).await?;
        let contents: Vec<McpContentBlock> = response
            .result
            .get("contents")
            .ok_or_else(|| McpError::Protocol("resources/read response missing contents".into()))
            .and_then(|v| {
                serde_json::from_value(v.clone())
                    .map_err(|e| McpError::Protocol(format!("invalid contents: {e}")))
            })?;
        Ok(contents)
    }

    /// Get a prompt from the server.
    pub async fn get_prompt(
        &self,
        name: &str,
        arguments: std::collections::HashMap<String, String>,
    ) -> Result<Vec<McpContentBlock>> {
        let id = self.next_id();
        let params = serde_json::json!({
            "name": name,
            "arguments": arguments,
        });
        let request = JsonRpcRequest::new(id, "prompts/get", Some(params));
        let response = self.send_request(request).await?;
        let messages: Vec<McpContentBlock> = response
            .result
            .get("messages")
            .ok_or_else(|| McpError::Protocol("prompts/get response missing messages".into()))
            .and_then(|v| {
                serde_json::from_value(v.clone())
                    .map_err(|e| McpError::Protocol(format!("invalid messages: {e}")))
            })?;
        Ok(messages)
    }
}
