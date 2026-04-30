use crate::registry::{Tool, ToolDefinition, ToolProvider};
use async_trait::async_trait;

/// Placeholder MCP provider. Full implementation deferred to a future task.
pub struct McpProvider {
    _config: (),
}

impl McpProvider {
    pub fn placeholder() -> Self {
        Self { _config: () }
    }
}

#[async_trait]
impl ToolProvider for McpProvider {
    async fn list_tools(&self) -> Vec<ToolDefinition> {
        Vec::new()
    }

    async fn get_tool(&self, _tool_id: &str) -> Option<Box<dyn Tool>> {
        None
    }

    fn name(&self) -> &str {
        "mcp"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mcp_provider_placeholder_returns_empty() {
        let provider = McpProvider::placeholder();
        assert!(provider.list_tools().await.is_empty());
        assert!(provider.get_tool("anything").await.is_none());
        assert_eq!(provider.name(), "mcp");
    }
}
