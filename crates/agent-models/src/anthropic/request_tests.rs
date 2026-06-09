use super::*;
use crate::{
    types::ServerTool, AnthropicConfig, ModelMessage, ModelRequest, ToolCall, ToolDefinition,
};

fn make_client() -> AnthropicClient {
    AnthropicClient::new(AnthropicConfig::default())
}

fn simple_request(content: &str) -> ModelRequest {
    ModelRequest::user_text("test", content)
}

#[test]
fn basic_user_message_produces_valid_body() {
    let client = make_client();
    let req = simple_request("Hello");
    let body = client.build_messages_request(&req);

    assert_eq!(body["model"], "claude-sonnet-4-20250514");
    assert_eq!(body["stream"], true);
    assert_eq!(body["max_tokens"], 16_384);

    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["role"], "user");
    assert_eq!(messages[0]["content"], "Hello");
}

#[test]
fn system_prompt_is_top_level_with_cache_control() {
    let client = make_client();
    let req = simple_request("Hi").with_system_prompt("You are helpful.");
    let body = client.build_messages_request(&req);

    let system = body["system"].as_array().unwrap();
    assert_eq!(system.len(), 1);
    assert_eq!(system[0]["type"], "text");
    assert_eq!(system[0]["text"], "You are helpful.");
    assert_eq!(system[0]["cache_control"]["type"], "ephemeral");
}

#[test]
fn empty_system_prompt_is_omitted() {
    let client = make_client();
    let req = simple_request("Hi").with_system_prompt("   ");
    let body = client.build_messages_request(&req);
    assert!(body.get("system").is_none());
}

#[test]
fn tool_definitions_use_safe_names() {
    let client = make_client();
    let req = simple_request("Hi").with_tools(vec![ToolDefinition {
        name: "fs.read".into(),
        description: "Read a file".into(),
        parameters: serde_json::json!({"type": "object"}),
    }]);
    let body = client.build_messages_request(&req);

    let tools = body["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["name"], "fs_read");
    assert_eq!(tools[0]["description"], "Read a file");
}

#[test]
fn tool_result_message_maps_to_user_tool_result() {
    let client = make_client();
    let req = simple_request("Hi")
        .add_assistant_with_tools(
            "Calling tool",
            vec![ToolCall {
                id: "tc_1".into(),
                name: "fs.read".into(),
                arguments: serde_json::json!({"path": "/tmp"}),
            }],
        )
        .add_tool_result("tc_1", "file contents here");

    let body = client.build_messages_request(&req);
    let messages = body["messages"].as_array().unwrap();

    // user + assistant + tool_result
    assert_eq!(messages.len(), 3);

    let tool_result_msg = &messages[2];
    assert_eq!(tool_result_msg["role"], "user");
    let content = tool_result_msg["content"].as_array().unwrap();
    assert_eq!(content[0]["type"], "tool_result");
    assert_eq!(content[0]["tool_use_id"], "tc_1");
}

#[test]
fn assistant_tool_calls_produce_tool_use_blocks() {
    let client = make_client();
    let req = simple_request("Hi").add_assistant_with_tools(
        "Let me check",
        vec![ToolCall {
            id: "tc_2".into(),
            name: "shell.exec".into(),
            arguments: serde_json::json!({"command": "ls"}),
        }],
    );

    let body = client.build_messages_request(&req);
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 2);

    let assistant = &messages[1];
    let blocks = assistant["content"].as_array().unwrap();
    assert!(blocks.iter().any(|b| b["type"] == "text"));
    let tool_use = blocks.iter().find(|b| b["type"] == "tool_use").unwrap();
    assert_eq!(tool_use["id"], "tc_2");
    assert_eq!(tool_use["name"], "shell_exec");
}

#[test]
fn empty_assistant_messages_are_skipped() {
    let client = make_client();
    let mut req = simple_request("Hi");
    req.messages.push(ModelMessage {
        role: "assistant".into(),
        content: "   ".into(),
        tool_calls: Vec::new(),
        tool_call_id: None,
    });
    req.messages.push(ModelMessage {
        role: "user".into(),
        content: "Next".into(),
        tool_calls: Vec::new(),
        tool_call_id: None,
    });

    let body = client.build_messages_request(&req);
    let messages = body["messages"].as_array().unwrap();
    // Empty assistant should be skipped
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0]["role"], "user");
    assert_eq!(messages[1]["role"], "user");
}

#[test]
fn temperature_and_sampling_params_applied() {
    let client = AnthropicClient::new(AnthropicConfig {
        temperature: Some(0.5),
        top_p: Some(0.9),
        top_k: Some(50),
        ..AnthropicConfig::default()
    });
    let body = client.build_messages_request(&simple_request("Hi"));

    assert!((body["temperature"].as_f64().unwrap() - 0.5).abs() < 0.01);
    assert!((body["top_p"].as_f64().unwrap() - 0.9).abs() < 0.01);
    assert_eq!(body["top_k"], 50);
}

#[test]
fn extra_params_merged_into_body() {
    let client = AnthropicClient::new(AnthropicConfig {
        extra_params: Some(serde_json::json!({"custom_field": 42})),
        ..AnthropicConfig::default()
    });
    let body = client.build_messages_request(&simple_request("Hi"));
    assert_eq!(body["custom_field"], 42);
}

#[test]
fn reasoning_effort_adds_thinking_block() {
    let client = make_client();
    let req = simple_request("Hi").with_reasoning_effort("high");
    let body = client.build_messages_request(&req);

    assert_eq!(body["thinking"]["type"], "enabled");
    assert_eq!(body["thinking"]["budget_tokens"], 8192);
}

#[test]
fn reasoning_effort_low_max_tokens_skips_thinking() {
    let client = AnthropicClient::new(AnthropicConfig {
        max_tokens: 512,
        ..AnthropicConfig::default()
    });
    let req = simple_request("Hi").with_reasoning_effort("high");
    let body = client.build_messages_request(&req);
    assert!(body.get("thinking").is_none());
}

#[test]
fn server_tool_code_execution_serializes() {
    let client = make_client();
    let req = simple_request("Hi").with_server_tools(vec![ServerTool::CodeExecution]);
    let body = client.build_messages_request(&req);

    let tools = body["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["type"], "code_execution_20250825");
    assert_eq!(tools[0]["name"], "code_execution");
}

#[test]
fn server_tool_web_search_serializes_with_domains() {
    let client = make_client();
    let req = simple_request("Hi").with_server_tools(vec![ServerTool::WebSearch {
        allowed_domains: vec!["example.com".into()],
        blocked_domains: vec!["blocked.com".into()],
        user_location: None,
    }]);
    let body = client.build_messages_request(&req);

    let tools = body["tools"].as_array().unwrap();
    assert_eq!(tools[0]["type"], "web_search_20250305");
    assert_eq!(tools[0]["allowed_domains"][0], "example.com");
    assert_eq!(tools[0]["blocked_domains"][0], "blocked.com");
}

#[test]
fn cache_breakpoints_added_to_last_tool_results() {
    let client = make_client();
    let mut req = simple_request("Hi");

    // Add 4 tool call/result pairs
    for i in 0..4 {
        req = req
            .add_assistant_with_tools(
                format!("Calling tool {i}"),
                vec![ToolCall {
                    id: format!("tc_{i}"),
                    name: "test".into(),
                    arguments: serde_json::json!({}),
                }],
            )
            .add_tool_result(format!("tc_{i}"), format!("result {i}"));
    }

    let body = client.build_messages_request(&req);
    let messages = body["messages"].as_array().unwrap();

    // Find tool_result messages with cache_control
    let cached_count = messages
        .iter()
        .filter(|m| {
            m["content"].as_array().is_some_and(|blocks| {
                blocks
                    .iter()
                    .any(|b| b["type"] == "tool_result" && !b["cache_control"].is_null())
            })
        })
        .count();

    // Should have at most 3 breakpoints
    assert!(cached_count <= 3);
    assert!(cached_count > 0);
}
