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

/// Check whether an input controller (enigo) can be initialized.
/// On headless CI this will fail (no display server).
fn input_controller_available() -> bool {
    enigo::Enigo::new(&enigo::Settings::default()).is_ok()
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

// --- Key combination parser unit tests ---

#[test]
fn parse_key_combination_single_char() {
    use super::platform::parse_key_combination;
    let (mods, key) = parse_key_combination("a").unwrap();
    assert!(mods.is_empty());
    assert_eq!(key, enigo::Key::Unicode('a'));
}

#[test]
fn parse_key_combination_modifier_plus_char() {
    use super::platform::parse_key_combination;
    let (mods, key) = parse_key_combination("cmd+c").unwrap();
    assert_eq!(mods.len(), 1);
    assert_eq!(mods[0], enigo::Key::Meta);
    assert_eq!(key, enigo::Key::Unicode('c'));
}

#[test]
fn parse_key_combination_multi_modifier() {
    use super::platform::parse_key_combination;
    let (mods, key) = parse_key_combination("ctrl+shift+a").unwrap();
    assert_eq!(mods.len(), 2);
    assert_eq!(mods[0], enigo::Key::Control);
    assert_eq!(mods[1], enigo::Key::Shift);
    assert_eq!(key, enigo::Key::Unicode('a'));
}

#[test]
fn parse_key_combination_special_keys() {
    use super::platform::parse_key_combination;
    let (mods, key) = parse_key_combination("enter").unwrap();
    assert!(mods.is_empty());
    assert_eq!(key, enigo::Key::Return);

    let (mods2, key2) = parse_key_combination("ctrl+tab").unwrap();
    assert_eq!(mods2.len(), 1);
    assert_eq!(key2, enigo::Key::Tab);
}

#[test]
fn parse_key_combination_unknown_key_error() {
    use super::platform::parse_key_combination;
    let result = parse_key_combination("cmd+nonexistent");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Unknown key"));
}

#[test]
fn parse_key_combination_empty_error() {
    use super::platform::parse_key_combination;
    let result = parse_key_combination("");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("Empty key combination"));
}

// --- Mouse button parser tests ---

#[test]
fn parse_mouse_button_left() {
    use super::platform::parse_mouse_button;
    assert_eq!(parse_mouse_button(None), enigo::Button::Left);
    assert_eq!(parse_mouse_button(Some("left")), enigo::Button::Left);
}

#[test]
fn parse_mouse_button_right() {
    use super::platform::parse_mouse_button;
    assert_eq!(parse_mouse_button(Some("right")), enigo::Button::Right);
}

#[test]
fn parse_mouse_button_middle() {
    use super::platform::parse_mouse_button;
    assert_eq!(parse_mouse_button(Some("middle")), enigo::Button::Middle);
}

// --- Wait and invalid action tests ---

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
    assert!(output.text.contains("success: true"));
    // Screenshot should be embedded as a markdown data URI.
    assert!(
        output.text.contains("![screenshot](data:image/png;base64,"),
        "Screenshot should be embedded as a markdown data URI"
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
    assert!(output.text.contains("success: true"));
    assert!(
        output.text.contains("![screenshot](data:image/png;base64,"),
        "Region screenshot should be embedded as a markdown data URI"
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
    assert!(output.text.contains("success: true"));
    assert!(
        output.text.contains("screen_size:"),
        "Should contain screen_size line"
    );
}

// --- Input control integration tests (require input controller) ---

#[tokio::test]
async fn invoke_mouse_move_real() {
    if !input_controller_available() {
        eprintln!("Skipping: no input controller available");
        return;
    }
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "action": "mouse_move",
        "x": 500,
        "y": 300
    }));
    let output = tool.invoke(invocation).await.unwrap();
    assert!(output.text.contains("Mouse moved to (500, 300)"));
    assert!(!output.text.contains("simulated"));
}

#[tokio::test]
async fn invoke_mouse_click_real() {
    if !input_controller_available() {
        eprintln!("Skipping: no input controller available");
        return;
    }
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "action": "mouse_click",
        "x": 100,
        "y": 200,
        "button": "left",
        "click_count": 1
    }));
    let output = tool.invoke(invocation).await.unwrap();
    assert!(output.text.contains("left click (1) at (100, 200)"));
    assert!(!output.text.contains("simulated"));
}

#[tokio::test]
async fn invoke_get_cursor_position_real() {
    if !input_controller_available() {
        eprintln!("Skipping: no input controller available");
        return;
    }
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "action": "get_cursor_position"
    }));
    let output = tool.invoke(invocation).await.unwrap();
    assert!(output.text.contains("success: true"));
    assert!(
        output.text.contains("cursor_position:"),
        "Should contain cursor_position line"
    );
}

#[tokio::test]
async fn invoke_scroll_real() {
    if !input_controller_available() {
        eprintln!("Skipping: no input controller available");
        return;
    }
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "action": "scroll",
        "direction": "down",
        "amount": 3
    }));
    let output = tool.invoke(invocation).await.unwrap();
    assert!(output.text.contains("Scrolled down by 3"));
    assert!(!output.text.contains("simulated"));
}

#[tokio::test]
async fn invoke_keyboard_type_real() {
    if !input_controller_available() {
        eprintln!("Skipping: no input controller available");
        return;
    }
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "action": "keyboard_type",
        "text": ""
    }));
    let output = tool.invoke(invocation).await.unwrap();
    assert!(output.text.contains("Typed 0 characters"));
    assert!(!output.text.contains("simulated"));
}

#[tokio::test]
async fn invoke_scroll_invalid_direction_error() {
    if !input_controller_available() {
        eprintln!("Skipping: no input controller available");
        return;
    }
    let tool = make_tool();
    let invocation = make_invocation(serde_json::json!({
        "action": "scroll",
        "direction": "diagonal",
        "amount": 5
    }));
    let result = tool.invoke(invocation).await;
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("Invalid scroll direction"));
}

#[test]
fn backend_constructs_without_panic() {
    let _backend = crate::computer_use::platform::DesktopBackend::new();
}

// --- format_computer_result tests ---

#[test]
fn format_computer_result_without_screenshot() {
    use super::tool::format_computer_result;
    use super::types::ComputerResult;

    let result = ComputerResult {
        success: true,
        output: "Mouse moved to (100, 200)".into(),
        screenshot: None,
        screen_size: None,
        cursor_position: None,
    };
    let text = format_computer_result(&result);
    assert!(text.contains("success: true"));
    assert!(text.contains("output: Mouse moved to (100, 200)"));
    assert!(!text.contains("data:image/png;base64"));
}

#[test]
fn format_computer_result_with_screenshot_embeds_data_uri() {
    use super::tool::format_computer_result;
    use super::types::ComputerResult;

    let result = ComputerResult {
        success: true,
        output: "Full screen screenshot captured".into(),
        screenshot: Some("iVBORw0KGgo=".into()),
        screen_size: None,
        cursor_position: None,
    };
    let text = format_computer_result(&result);
    assert!(text.contains("success: true"));
    assert!(text.contains("![screenshot](data:image/png;base64,iVBORw0KGgo=)"));
}

#[test]
fn format_computer_result_with_screen_size_and_cursor() {
    use super::tool::format_computer_result;
    use super::types::{ComputerResult, CursorPosition, ScreenSize};

    let result = ComputerResult {
        success: true,
        output: String::new(),
        screenshot: None,
        screen_size: Some(ScreenSize {
            width: 1920,
            height: 1080,
        }),
        cursor_position: Some(CursorPosition { x: 42, y: 99 }),
    };
    let text = format_computer_result(&result);
    assert!(text.contains("screen_size: 1920x1080"));
    assert!(text.contains("cursor_position: (42, 99)"));
    // Empty output field should be omitted.
    assert!(!text.contains("output:"));
}
