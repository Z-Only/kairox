use super::*;

#[test]
fn tool_call_serializes_with_id_name_and_arguments() {
    let tc = ToolCall {
        id: "call_abc".into(),
        name: "fs.read".into(),
        arguments: serde_json::json!({"path": "README.md"}),
    };
    let json = serde_json::to_value(&tc).unwrap();
    assert_eq!(json["id"], "call_abc");
    assert_eq!(json["name"], "fs.read");
    assert_eq!(json["arguments"]["path"], "README.md");
}

#[test]
fn model_request_supports_system_prompt_and_tools() {
    let req = ModelRequest::user_text("fast", "hello")
        .with_system_prompt("You are helpful.")
        .with_tools(vec![ToolDefinition {
            name: "fs.read".into(),
            description: "Read a file".into(),
            parameters: serde_json::json!({"type": "object"}),
        }]);
    assert_eq!(req.system_prompt, Some("You are helpful.".into()));
    assert_eq!(req.tools.len(), 1);
}
