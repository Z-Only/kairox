//! Discovery requests — `tools/list`, `resources/list`, `prompts/list`.
//!
//! These methods always query the server; use
//! [`DiscoveryCache`][crate::DiscoveryCache] for a caching layer on top.

use super::McpClient;
use crate::protocol::JsonRpcRequest;
use crate::types::*;
use crate::{McpError, Result};

impl McpClient {
    /// List tools available on the server.
    ///
    /// Always queries the server; use [`DiscoveryCache`][crate::DiscoveryCache] for caching.
    pub async fn discover_tools(&self) -> Result<Vec<McpToolDef>> {
        let id = self.next_id();
        let request = JsonRpcRequest::new(id, "tools/list", Some(serde_json::json!({})));
        let response = self.send_request(request).await?;
        let tools: Vec<McpToolDef> = response
            .result
            .get("tools")
            .ok_or_else(|| McpError::Protocol("tools/list response missing tools".into()))
            .and_then(|v| {
                serde_json::from_value(v.clone())
                    .map_err(|e| McpError::Protocol(format!("invalid tools: {e}")))
            })?;
        Ok(tools)
    }

    /// List resources available on the server.
    ///
    /// Always queries the server; use [`DiscoveryCache`][crate::DiscoveryCache] for caching.
    pub async fn discover_resources(&self) -> Result<Vec<McpResourceDef>> {
        let id = self.next_id();
        let request = JsonRpcRequest::new(id, "resources/list", Some(serde_json::json!({})));
        let response = self.send_request(request).await?;
        let resources: Vec<McpResourceDef> = response
            .result
            .get("resources")
            .ok_or_else(|| McpError::Protocol("resources/list response missing resources".into()))
            .and_then(|v| {
                serde_json::from_value(v.clone())
                    .map_err(|e| McpError::Protocol(format!("invalid resources: {e}")))
            })?;
        Ok(resources)
    }

    /// List prompts available on the server.
    ///
    /// Always queries the server; use [`DiscoveryCache`][crate::DiscoveryCache] for caching.
    pub async fn discover_prompts(&self) -> Result<Vec<McpPromptDef>> {
        let id = self.next_id();
        let request = JsonRpcRequest::new(id, "prompts/list", Some(serde_json::json!({})));
        let response = self.send_request(request).await?;
        let prompts: Vec<McpPromptDef> = response
            .result
            .get("prompts")
            .ok_or_else(|| McpError::Protocol("prompts/list response missing prompts".into()))
            .and_then(|v| {
                serde_json::from_value(v.clone())
                    .map_err(|e| McpError::Protocol(format!("invalid prompts: {e}")))
            })?;
        Ok(prompts)
    }
}
