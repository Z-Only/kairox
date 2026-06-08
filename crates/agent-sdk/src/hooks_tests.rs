use super::*;

// ── ToolHookContext ─────────────────────────────────────────────────

fn sample_context() -> ToolHookContext {
    ToolHookContext {
        tool_name: "shell.exec".to_string(),
        tool_input: serde_json::json!({"command": "ls"}),
        session_id: "sess-42".to_string(),
    }
}

#[test]
fn tool_hook_context_debug() {
    let ctx = sample_context();
    let debug = format!("{ctx:?}");
    assert!(debug.contains("shell.exec"), "missing tool_name: {debug}");
    assert!(debug.contains("sess-42"), "missing session_id: {debug}");
}

#[test]
fn tool_hook_context_clone() {
    let original = sample_context();
    let cloned = original.clone();
    assert_eq!(cloned.tool_name, "shell.exec");
    assert_eq!(cloned.session_id, "sess-42");
    assert_eq!(cloned.tool_input, serde_json::json!({"command": "ls"}));
}

// ── HookAction ──────────────────────────────────────────────────────

#[test]
fn hook_action_continue_eq() {
    assert_eq!(HookAction::Continue, HookAction::Continue);
}

#[test]
fn hook_action_reject_eq_same_reason() {
    let action_a = HookAction::Reject("no".to_string());
    let action_b = HookAction::Reject("no".to_string());
    assert_eq!(action_a, action_b);
}

#[test]
fn hook_action_reject_ne_different_reason() {
    let action_a = HookAction::Reject("reason A".to_string());
    let action_b = HookAction::Reject("reason B".to_string());
    assert_ne!(action_a, action_b);
}

#[test]
fn hook_action_continue_ne_reject() {
    assert_ne!(HookAction::Continue, HookAction::Reject("x".to_string()));
}

#[test]
fn hook_action_clone() {
    let original = HookAction::Reject("cloned".to_string());
    let cloned = original.clone();
    assert_eq!(cloned, HookAction::Reject("cloned".to_string()));
}

#[test]
fn hook_action_debug_continue() {
    assert_eq!(format!("{:?}", HookAction::Continue), "Continue");
}

#[test]
fn hook_action_debug_reject() {
    let debug = format!("{:?}", HookAction::Reject("bad".to_string()));
    assert!(debug.contains("Reject"), "missing Reject: {debug}");
    assert!(debug.contains("bad"), "missing reason: {debug}");
}

// ── SdkHook default implementations ────────────────────────────────

/// A minimal struct that relies entirely on default trait methods.
struct DefaultHook;

#[async_trait::async_trait]
impl SdkHook for DefaultHook {}

#[tokio::test]
async fn default_before_tool_returns_continue() {
    let hook = DefaultHook;
    let ctx = sample_context();
    let action = hook.before_tool(&ctx).await;
    assert_eq!(action, HookAction::Continue);
}

#[tokio::test]
async fn default_after_tool_does_not_panic() {
    let hook = DefaultHook;
    let ctx = sample_context();
    // Should complete without panicking.
    hook.after_tool(&ctx, "some result output").await;
}

#[test]
fn default_name_returns_unnamed_hook() {
    let hook = DefaultHook;
    assert_eq!(hook.name(), "unnamed-hook");
}

// ── dyn SdkHook Debug ───────────────────────────────────────────────

struct NamedHook;

#[async_trait::async_trait]
impl SdkHook for NamedHook {
    fn name(&self) -> &str {
        "my-audit-hook"
    }
}

#[test]
fn dyn_sdk_hook_debug_uses_name() {
    let hook: &dyn SdkHook = &NamedHook;
    let debug = format!("{hook:?}");
    assert_eq!(debug, "SdkHook(my-audit-hook)");
}

#[test]
fn dyn_sdk_hook_debug_default_name() {
    let hook: &dyn SdkHook = &DefaultHook;
    let debug = format!("{hook:?}");
    assert_eq!(debug, "SdkHook(unnamed-hook)");
}
