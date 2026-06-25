use super::*;
use crate::{ModelMessage, ModelRequest, OpenAiCompatibleConfig, ToolCall, ToolDefinition};

fn make_client() -> OpenAiCompatibleClient {
    OpenAiCompatibleClient::new(OpenAiCompatibleConfig {
        base_url: "https://api.openai.com".into(),
        api_key_env: "OPENAI_API_KEY".into(),
        default_model: "gpt-4o".into(),
        headers: Vec::new(),
        capability_overrides: None,
        temperature: None,
        top_p: None,
        extra_params: None,
    })
}

fn simple_request(content: &str) -> ModelRequest {
    ModelRequest::user_text("test", content)
}

#[test]
fn basic_user_message_produces_valid_body() {
    let client = make_client();
    let req = simple_request("Hello");
    let body = client.build_chat_request(&req).unwrap();

    assert_eq!(body["model"], "gpt-4o");
    assert_eq!(body["stream"], true);

    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["role"], "user");
    assert_eq!(messages[0]["content"], "Hello");
}

#[test]
fn system_prompt_becomes_system_message() {
    let client = make_client();
    let req = simple_request("Hi").with_system_prompt("Be helpful.");
    let body = client.build_chat_request(&req).unwrap();

    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0]["role"], "system");
    assert_eq!(messages[0]["content"], "Be helpful.");
    assert_eq!(messages[1]["role"], "user");
}

#[test]
fn empty_system_prompt_is_omitted() {
    let client = make_client();
    let req = simple_request("Hi").with_system_prompt("   ");
    let body = client.build_chat_request(&req).unwrap();

    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0]["role"], "user");
}

#[test]
fn tool_definitions_use_function_format() {
    let client = make_client();
    let req = simple_request("Hi").with_tools(vec![ToolDefinition {
        name: "fs.read".into(),
        description: "Read a file".into(),
        parameters: serde_json::json!({"type": "object"}),
    }]);
    let body = client.build_chat_request(&req).unwrap();

    let tools = body["tools"].as_array().unwrap();
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0]["type"], "function");
    assert_eq!(tools[0]["function"]["name"], "fs_read");
    assert_eq!(tools[0]["function"]["description"], "Read a file");
}

#[test]
fn assistant_with_tool_calls_includes_tool_calls_array() {
    let client = make_client();
    let req = simple_request("Hi").add_assistant_with_tools(
        "Let me check",
        vec![ToolCall {
            id: "call_1".into(),
            name: "shell.exec".into(),
            arguments: serde_json::json!({"command": "ls"}),
        }],
    );

    let body = client.build_chat_request(&req).unwrap();
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 2);

    let assistant = &messages[1];
    assert_eq!(assistant["role"], "assistant");
    let tool_calls = assistant["tool_calls"].as_array().unwrap();
    assert_eq!(tool_calls.len(), 1);
    assert_eq!(tool_calls[0]["id"], "call_1");
    assert_eq!(tool_calls[0]["type"], "function");
    assert_eq!(tool_calls[0]["function"]["name"], "shell_exec");
}

#[test]
fn tool_result_message_includes_tool_call_id() {
    let client = make_client();
    let req = simple_request("Hi")
        .add_assistant_with_tools(
            "Checking",
            vec![ToolCall {
                id: "call_2".into(),
                name: "test".into(),
                arguments: serde_json::json!({}),
            }],
        )
        .add_tool_result("call_2", "result data");

    let body = client.build_chat_request(&req).unwrap();
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 3);

    let tool_msg = &messages[2];
    assert_eq!(tool_msg["role"], "tool");
    assert_eq!(tool_msg["tool_call_id"], "call_2");
    assert_eq!(tool_msg["content"], "result data");
}

#[test]
fn empty_user_messages_are_skipped() {
    let client = make_client();
    let mut req = simple_request("Hello");
    req.messages.push(ModelMessage {
        role: "user".into(),
        content: "   ".into(),
        tool_calls: Vec::new(),
        tool_call_id: None,
    });
    req.messages.push(ModelMessage {
        role: "user".into(),
        content: "World".into(),
        tool_calls: Vec::new(),
        tool_call_id: None,
    });

    let body = client.build_chat_request(&req).unwrap();
    let messages = body["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0]["content"], "Hello");
    assert_eq!(messages[1]["content"], "World");
}

#[test]
fn temperature_and_top_p_applied() {
    let client = OpenAiCompatibleClient::new(OpenAiCompatibleConfig {
        base_url: "https://api.openai.com".into(),
        api_key_env: "KEY".into(),
        default_model: "gpt-4o".into(),
        headers: Vec::new(),
        capability_overrides: None,
        temperature: Some(0.3),
        top_p: Some(0.85),
        extra_params: None,
    });
    let body = client.build_chat_request(&simple_request("Hi")).unwrap();
    assert!((body["temperature"].as_f64().unwrap() - 0.3).abs() < 0.01);
    assert!((body["top_p"].as_f64().unwrap() - 0.85).abs() < 0.01);
}

#[test]
fn extra_params_merged_into_body() {
    let client = OpenAiCompatibleClient::new(OpenAiCompatibleConfig {
        base_url: "https://api.openai.com".into(),
        api_key_env: "KEY".into(),
        default_model: "gpt-4o".into(),
        headers: Vec::new(),
        capability_overrides: None,
        temperature: None,
        top_p: None,
        extra_params: Some(serde_json::json!({"frequency_penalty": 0.5})),
    });
    let body = client.build_chat_request(&simple_request("Hi")).unwrap();
    assert_eq!(body["frequency_penalty"], 0.5);
}

#[test]
fn reasoning_effort_passed_through() {
    let client = make_client();
    let req = simple_request("Hi").with_reasoning_effort("high");
    let body = client.build_chat_request(&req).unwrap();
    assert_eq!(body["reasoning_effort"], "high");
}
