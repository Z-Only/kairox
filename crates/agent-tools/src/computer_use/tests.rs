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
        session_id: "ses_test".into(),
        preview: String::new(),
        timeout_ms: 30_000,
        output_limit_bytes: 1024 * 1024,
    }
}

/// Check whether a display is available for screenshot tests.
/// On CI (headless Linux) there is no monitor, so xcap will fail.
fn display_available() -> bool {
    xcap::Monitor::all()
        .map(|monitors| !monitors.is_empty())
        .unwrap_or(false)
}

// --- Unit tests (no display required) ---

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

#[test]
fn definition_schema_has_all_action_variants() {
    let tool = make_tool();
    let def = tool.definition();
    let actions = def.parameters["properties"]["action"]["enum"]
        .as_array()
        .expect("action enum should be an array");
    let action_strs: Vec<&str> = actions.iter().filter_map(|v| v.as_str()).collect();
    for expected in [
        "screenshot",
        "mouse_move",
        "mouse_click",
        "mouse_drag",
        "keyboard_type",
        "key_press",
        "scroll",
        "wait",
        "get_screen_size",
        "get_cursor_position",
    ] {
        assert!(
            action_strs.contains(&expected),
            "Missing action variant: {}",
            expected
        );
    }
}

#[tokio::test]
async fn invoke_mouse_move_simulated() {
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "action": "mouse_move",
        "x": 500,
        "y": 300
    }));
    let output = tool.invoke(invocation).await.unwrap();
    assert!(output.text.contains("Mouse moved to (500, 300)"));
    assert!(output.text.contains("simulated"));
    assert!(output.text.contains("\"success\": true"));
}

#[tokio::test]
async fn invoke_mouse_click_simulated() {
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
    assert!(output.text.contains("simulated"));
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
async fn invoke_mouse_drag_simulated() {
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
    assert!(output.text.contains("simulated"));
}

#[tokio::test]
async fn invoke_keyboard_type_simulated() {
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "action": "keyboard_type",
        "text": "hello world"
    }));
    let output = tool.invoke(invocation).await.unwrap();
    assert!(output.text.contains("Typed 11 characters"));
    assert!(output.text.contains("simulated"));
    assert!(output.text.contains("\"success\": true"));
}

#[tokio::test]
async fn invoke_key_press_simulated() {
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "action": "key_press",
        "keys": "cmd+c"
    }));
    let output = tool.invoke(invocation).await.unwrap();
    assert!(output.text.contains("Pressed keys: cmd+c"));
    assert!(output.text.contains("simulated"));
}

#[tokio::test]
async fn invoke_scroll_at_coordinates_simulated() {
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
    assert!(output.text.contains("simulated"));
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

// --- Integration tests (require a display / monitor) ---

#[tokio::test]
async fn invoke_screenshot_real() {
    if !display_available() {
        eprintln!("Skipping: no display available for screenshot");
        return;
    }
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "action": "screenshot"
    }));
    let output = tool.invoke(invocation).await.unwrap();
    assert!(!output.truncated);
    assert!(output.text.contains("Full screen screenshot captured"));
    assert!(output.text.contains("\"success\": true"));

    // Verify the screenshot is real base64 PNG data, not a placeholder
    let result: serde_json::Value = serde_json::from_str(&output.text).unwrap();
    let screenshot = result["screenshot"].as_str().unwrap_or("");
    assert!(
        !screenshot.contains("placeholder"),
        "Screenshot should be real data, not a placeholder"
    );
    assert!(
        screenshot.len() > 1000,
        "Real screenshot should be at least 1KB of base64 data, got {} bytes",
        screenshot.len()
    );
}

#[tokio::test]
async fn invoke_screenshot_with_region_real() {
    if !display_available() {
        eprintln!("Skipping: no display available for screenshot");
        return;
    }
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "action": "screenshot",
        "region": [100, 200, 300, 400]
    }));
    let output = tool.invoke(invocation).await.unwrap();
    assert!(output.text.contains("Screenshot of region"));
    assert!(output.text.contains("\"success\": true"));

    let result: serde_json::Value = serde_json::from_str(&output.text).unwrap();
    let screenshot = result["screenshot"].as_str().unwrap_or("");
    assert!(
        screenshot.len() > 100,
        "Region screenshot should produce real data"
    );
}

#[tokio::test]
async fn invoke_get_screen_size_real() {
    if !display_available() {
        eprintln!("Skipping: no display available for screen size");
        return;
    }
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "action": "get_screen_size"
    }));
    let output = tool.invoke(invocation).await.unwrap();
    assert!(output.text.contains("\"success\": true"));

    // Real screen size should be reasonable (not the old hardcoded 1920x1080)
    let result: serde_json::Value = serde_json::from_str(&output.text).unwrap();
    let width = result["screen_size"]["width"].as_u64().unwrap_or(0);
    let height = result["screen_size"]["height"].as_u64().unwrap_or(0);
    assert!(width > 0, "Screen width should be positive");
    assert!(height > 0, "Screen height should be positive");
}

#[tokio::test]
async fn invoke_get_cursor_position() {
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "action": "get_cursor_position"
    }));
    let output = tool.invoke(invocation).await.unwrap();
    assert!(output.text.contains("\"success\": true"));
    // Currently simulated, so just verify structure
    let result: serde_json::Value = serde_json::from_str(&output.text).unwrap();
    assert!(result["cursor_position"].is_object());
}

#[test]
fn backend_constructs_without_panic() {
    let _backend = crate::computer_use::platform::DesktopBackend::new();
}
