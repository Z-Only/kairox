use crate::registry::ToolDefinition;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServerConfig {
    pub id: String,
    pub command: String,
    pub args: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpTool {
    pub server_id: String,
    pub definition: ToolDefinition,
}

pub fn map_mcp_tool(
    server_id: impl Into<String>,
    name: impl Into<String>,
    description: impl Into<String>,
) -> McpTool {
    let name = name.into();
    McpTool {
        server_id: server_id.into(),
        definition: ToolDefinition {
            tool_id: format!("mcp.{name}"),
            description: description.into(),
            required_capability: "mcp.invoke".into(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_tool_maps_to_shared_tool_definition() {
        let tool = map_mcp_tool("local", "read_doc", "Read a doc");
        assert_eq!(tool.server_id, "local");
        assert_eq!(tool.definition.tool_id, "mcp.read_doc");
        assert_eq!(tool.definition.required_capability, "mcp.invoke");
    }
}
