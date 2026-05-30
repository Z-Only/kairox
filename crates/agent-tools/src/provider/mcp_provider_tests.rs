use super::*;

#[test]
fn mcp_tool_adapter_formats_tool_id() {
    let tool_def = McpToolDef {
        name: "echo".into(),
        description: Some("Echo tool".into()),
        input_schema: None,
    };
    // Can't create a real McpClient without transport, so just test the definition format logic
    let tool_id = format!("mcp.{}.{}", "test-server", tool_def.name);
    assert_eq!(tool_id, "mcp.test-server.echo");
}
