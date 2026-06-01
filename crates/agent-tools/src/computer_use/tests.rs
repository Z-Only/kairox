use crate::computer_use::tool::COMPUTER_USE_TOOL_ID;
use crate::computer_use::ComputerUseTool;
use crate::permission::ToolEffect;
use crate::registry::{Tool, ToolInvocation};

fn make_tool() -> ComputerUseTool {
    ComputerUseTool::new()
}

fn make_invocation(args: serde_json::Value) -> ToolInvocation {
    ToolInvocation {
        tool_id: COMPUTER_USE_TOOL_ID.to_string(),
        arguments: args,
        workspace_id: "test".to_string(),
        preview: String::new(),
        timeout_ms: 30_000,
        output_limit_bytes: 1024 * 1024,
    }
}

#[test]
fn definition_has_correct_tool_id() {
    let tool = make_tool();
    let def = tool.definition();
    assert_eq!(def.tool_id, "computer.use");
    assert_eq!(def.required_capability, "computer.interact");
    assert!(!def.description.is_empty());
}

#[test]
fn risk_returns_execute_effect() {
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "action": "screenshot"
    }));
    let risk = tool.risk(&invocation);
    assert_eq!(risk.tool_id, "computer.use");
    assert_eq!(risk.effect, ToolEffect::Execute);
}

#[tokio::test]
async fn invoke_screenshot() {
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "action": "screenshot"
    }));
    let output = tool.invoke(invocation).await.unwrap();
    assert!(!output.truncated);
    assert!(output.text.contains("Full screen screenshot captured"));
    assert!(output.text.contains("base64-screenshot-placeholder"));
}

#[tokio::test]
async fn invoke_screenshot_with_region() {
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "action": "screenshot",
        "region": [100, 200, 300, 400]
    }));
    let output = tool.invoke(invocation).await.unwrap();
    assert!(output.text.contains("Screenshot of region"));
    assert!(output.text.contains("100"));
}

#[tokio::test]
async fn invoke_mouse_move() {
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "action": "mouse_move",
        "x": 500,
        "y": 300
    }));
    let output = tool.invoke(invocation).await.unwrap();
    assert!(output.text.contains("Mouse moved to (500, 300)"));
    assert!(output.text.contains("\"success\": true"));
}

#[tokio::test]
async fn invoke_mouse_click() {
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "action": "mouse_click",
        "x": 100,
        "y": 200,
        "button": "right",
        "click_count": 2
    }));
    let output = tool.invoke(invocation).await.unwrap();
    assert!(output.text.contains("right click (2) at (100, 200)"));
}

#[tokio::test]
async fn invoke_mouse_click_defaults_to_current_position() {
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "action": "mouse_click"
    }));
    let output = tool.invoke(invocation).await.unwrap();
    assert!(output.text.contains("left click (1) at current position"));
}

#[tokio::test]
async fn invoke_mouse_drag() {
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "action": "mouse_drag",
        "from_x": 10,
        "from_y": 20,
        "to_x": 300,
        "to_y": 400
    }));
    let output = tool.invoke(invocation).await.unwrap();
    assert!(output.text.contains("Dragged from (10, 20) to (300, 400)"));
}

#[tokio::test]
async fn invoke_keyboard_type() {
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "action": "keyboard_type",
        "text": "hello world"
    }));
    let output = tool.invoke(invocation).await.unwrap();
    assert!(output.text.contains("Typed 11 characters"));
    assert!(output.text.contains("\"success\": true"));
}

#[tokio::test]
async fn invoke_key_press() {
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "action": "key_press",
        "keys": "cmd+c"
    }));
    let output = tool.invoke(invocation).await.unwrap();
    assert!(output.text.contains("Pressed keys: cmd+c"));
}

#[tokio::test]
async fn invoke_scroll_at_coordinates() {
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "action": "scroll",
        "x": 50,
        "y": 75,
        "direction": "down",
        "amount": 480
    }));
    let output = tool.invoke(invocation).await.unwrap();
    assert!(output.text.contains("Scrolled down by 480 at (50, 75)"));
}

#[tokio::test]
async fn invoke_wait_caps_duration() {
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "action": "wait",
        "duration_ms": 1
    }));
    let output = tool.invoke(invocation).await.unwrap();
    assert!(output.text.contains("Waited 1ms"));
}

#[tokio::test]
async fn invoke_get_screen_size() {
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "action": "get_screen_size"
    }));
    let output = tool.invoke(invocation).await.unwrap();
    assert!(output.text.contains("1920"));
    assert!(output.text.contains("1080"));
    assert!(output.text.contains("\"success\": true"));
}

#[tokio::test]
async fn invoke_get_cursor_position() {
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "action": "get_cursor_position"
    }));
    let output = tool.invoke(invocation).await.unwrap();
    assert!(output.text.contains("960"));
    assert!(output.text.contains("540"));
}

#[tokio::test]
async fn invoke_invalid_action_returns_error() {
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "action": "nonexistent_action"
    }));
    let result = tool.invoke(invocation).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("Invalid computer action"));
}
