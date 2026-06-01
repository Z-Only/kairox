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

#[test]
fn server_tools_from_profile_builds_correct_vec() {
    let both = server_tools_from_profile(true, true);
    assert_eq!(both.len(), 2);
    assert!(matches!(both[0], ServerTool::CodeExecution));
    assert!(matches!(both[1], ServerTool::WebSearch { .. }));

    let code_only = server_tools_from_profile(true, false);
    assert_eq!(code_only.len(), 1);
    assert!(matches!(code_only[0], ServerTool::CodeExecution));

    let web_only = server_tools_from_profile(false, true);
    assert_eq!(web_only.len(), 1);
    assert!(matches!(web_only[0], ServerTool::WebSearch { .. }));

    let none = server_tools_from_profile(false, false);
    assert!(none.is_empty());
}

#[test]
fn model_request_with_server_tools_builder() {
    let req = ModelRequest::user_text("fast", "hello").with_server_tools(vec![
        ServerTool::CodeExecution,
        ServerTool::WebSearch {
            allowed_domains: vec!["example.com".into()],
            blocked_domains: Vec::new(),
            user_location: None,
        },
    ]);
    assert_eq!(req.server_tools.len(), 2);
}
