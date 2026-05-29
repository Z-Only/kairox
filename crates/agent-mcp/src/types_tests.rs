use super::*;
use serde_json::json;

#[test]
fn test_mcp_server_def_stdio_roundtrip() {
    let def = McpServerDef {
        name: "my-server".to_string(),
        transport: McpTransportDef::Stdio {
            command: "npx".to_string(),
            cwd: None,
        },
        args: vec![
            "-y".to_string(),
            "@modelcontextprotocol/server-filesystem".to_string(),
        ],
        env: std::collections::HashMap::new(),
        keep_alive: false,
        idle_timeout_secs: 300,
        auto_restart: true,
        max_restart_attempts: 3,
    };

    // Serialize to JSON roundtrip
    let json_str = serde_json::to_string(&def).unwrap();
    let parsed: McpServerDef = serde_json::from_str(&json_str).unwrap();
    assert_eq!(parsed.name, "my-server");
    assert!(!parsed.keep_alive);
    assert_eq!(parsed.idle_timeout_secs, 300);
    assert!(parsed.auto_restart);
    assert_eq!(parsed.max_restart_attempts, 3);
    assert!(matches!(parsed.transport, McpTransportDef::Stdio { .. }));
    if let McpTransportDef::Stdio { command, cwd } = &parsed.transport {
        assert_eq!(command, "npx");
        assert!(cwd.is_none());
    }
    assert_eq!(
        parsed.args,
        vec!["-y", "@modelcontextprotocol/server-filesystem"]
    );

    // TOML roundtrip
    let toml_str = toml::to_string(&def).unwrap();
    let from_toml: McpServerDef = toml::from_str(&toml_str).unwrap();
    assert_eq!(from_toml.name, def.name);
    assert!(matches!(from_toml.transport, McpTransportDef::Stdio { .. }));
    if let McpTransportDef::Stdio { command, .. } = &from_toml.transport {
        assert_eq!(command, "npx");
    }
}

#[test]
fn test_mcp_server_def_sse_roundtrip() {
    let def = McpServerDef {
        name: "remote-server".to_string(),
        transport: McpTransportDef::Sse {
            url: "http://localhost:8080/sse".to_string(),
            api_key_env: None,
            headers: std::collections::HashMap::new(),
        },
        args: vec![],
        env: std::collections::HashMap::new(),
        keep_alive: true,
        idle_timeout_secs: 600,
        auto_restart: false,
        max_restart_attempts: 0,
    };

    let toml_str = toml::to_string(&def).unwrap();
    let from_toml: McpServerDef = toml::from_str(&toml_str).unwrap();
    assert_eq!(from_toml.name, "remote-server");
    assert!(matches!(from_toml.transport, McpTransportDef::Sse { .. }));
    if let McpTransportDef::Sse { url, .. } = &from_toml.transport {
        assert_eq!(url, "http://localhost:8080/sse");
    }
    assert!(from_toml.keep_alive);
    assert_eq!(from_toml.idle_timeout_secs, 600);
}

#[test]
fn test_mcp_tool_result_text_content() {
    let json = json!({
        "content": [
            { "type": "text", "text": "Hello, world!" }
        ]
    });

    let result: McpToolResult = serde_json::from_value(json).unwrap();
    assert_eq!(result.content.len(), 1);
    assert!(result.is_error.is_none());
    match &result.content[0] {
        McpContentBlock::Text { text } => assert_eq!(text, "Hello, world!"),
        other => panic!("Expected Text block, got {:?}", other),
    }
}

#[test]
fn test_mcp_tool_result_error() {
    let json = json!({
        "content": [
            { "type": "text", "text": "Tool execution failed" }
        ],
        "is_error": true
    });

    let result: McpToolResult = serde_json::from_value(json).unwrap();
    assert_eq!(result.is_error, Some(true));
}

#[test]
fn test_mcp_content_block_all_variants() {
    // Text variant
    let text_json = json!({ "type": "text", "text": "hello" });
    let block: McpContentBlock = serde_json::from_value(text_json).unwrap();
    assert!(matches!(block, McpContentBlock::Text { .. }));

    // Image variant
    let image_json = json!({
        "type": "image",
        "data": "iVBORw0KGgo=",
        "mime_type": "image/png"
    });
    let block: McpContentBlock = serde_json::from_value(image_json).unwrap();
    match &block {
        McpContentBlock::Image { data, mime_type } => {
            assert_eq!(data, "iVBORw0KGgo=");
            assert_eq!(mime_type, "image/png");
        }
        other => panic!("Expected Image block, got {:?}", other),
    }

    // Resource variant
    let resource_json = json!({
        "type": "resource",
        "resource": {
            "uri": "file:///tmp/readme.md",
            "mime_type": "text/markdown",
            "text": "# Hello"
        }
    });
    let block: McpContentBlock = serde_json::from_value(resource_json).unwrap();
    match &block {
        McpContentBlock::Resource { resource } => {
            assert_eq!(resource.uri, "file:///tmp/readme.md");
            assert_eq!(resource.mime_type, Some("text/markdown".to_string()));
            assert_eq!(resource.text, Some("# Hello".to_string()));
        }
        other => panic!("Expected Resource block, got {:?}", other),
    }
}

#[test]
fn test_mcp_server_status_serialization() {
    // snake_case serialization
    assert_eq!(
        serde_json::to_string(&McpServerStatus::Stopped).unwrap(),
        "\"stopped\""
    );
    assert_eq!(
        serde_json::to_string(&McpServerStatus::Starting).unwrap(),
        "\"starting\""
    );
    assert_eq!(
        serde_json::to_string(&McpServerStatus::Running).unwrap(),
        "\"running\""
    );
    assert_eq!(
        serde_json::to_string(&McpServerStatus::Failed).unwrap(),
        "\"failed\""
    );

    // Deserialization
    let status: McpServerStatus = serde_json::from_str("\"running\"").unwrap();
    assert_eq!(status, McpServerStatus::Running);

    // Display
    assert_eq!(McpServerStatus::Running.to_string(), "running");
    assert_eq!(McpServerStatus::Failed.to_string(), "failed");
}

#[test]
fn test_mcp_tool_def_with_input_schema() {
    let json = json!({
        "name": "read_file",
        "description": "Read a file from disk",
        "input_schema": {
            "type": "object",
            "properties": {
                "path": { "type": "string" }
            },
            "required": ["path"]
        }
    });

    let tool: McpToolDef = serde_json::from_value(json).unwrap();
    assert_eq!(tool.name, "read_file");
    assert!(tool.input_schema.is_some());
    // input_schema should be stored as a JSON string
    let schema_str = tool.input_schema.as_ref().unwrap().clone();
    let schema: serde_json::Value = serde_json::from_str(&schema_str).unwrap();
    assert_eq!(schema["type"], "object");

    // Roundtrip
    let roundtrip = serde_json::to_string(&tool).unwrap();
    let parsed: McpToolDef = serde_json::from_str(&roundtrip).unwrap();
    assert_eq!(parsed.name, "read_file");
}
