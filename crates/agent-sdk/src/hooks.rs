//! SDK hook system for pre/post tool execution interception.
//!
//! Hooks allow SDK consumers to observe and optionally reject tool calls
//! before they execute, or inspect results after execution completes.

use std::fmt;

/// Context provided to hooks before/after tool execution.
#[derive(Debug, Clone)]
pub struct ToolHookContext {
    /// The tool name (e.g. `shell.exec`, `fs.write`).
    pub tool_name: String,
    /// The tool input as a JSON value.
    pub tool_input: serde_json::Value,
    /// The session ID where the tool is being executed.
    pub session_id: String,
}

/// The action a hook returns to control tool execution flow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookAction {
    /// Allow the tool call to proceed.
    Continue,
    /// Reject the tool call with a reason.
    Reject(String),
}

/// Trait for SDK hooks that intercept tool execution.
///
/// Implement this trait to add custom behavior around tool calls.
///
/// # Example
///
/// ```rust,no_run
/// use agent_sdk::{SdkHook, ToolHookContext, HookAction};
///
/// struct AuditLogger;
///
/// #[async_trait::async_trait]
/// impl SdkHook for AuditLogger {
///     async fn before_tool(&self, context: &ToolHookContext) -> HookAction {
///         println!("Tool call: {} with {:?}", context.tool_name, context.tool_input);
///         HookAction::Continue
///     }
///
///     async fn after_tool(&self, context: &ToolHookContext, result: &str) {
///         println!("Tool result for {}: {}", context.tool_name, &result[..100.min(result.len())]);
///     }
/// }
/// ```
#[async_trait::async_trait]
pub trait SdkHook: Send + Sync {
    /// Called before a tool executes. Return [`HookAction::Reject`] to prevent
    /// execution.
    async fn before_tool(&self, context: &ToolHookContext) -> HookAction {
        let _ = context;
        HookAction::Continue
    }

    /// Called after a tool completes, with the tool's output.
    async fn after_tool(&self, context: &ToolHookContext, result: &str) {
        let _ = (context, result);
    }

    /// A human-readable name for this hook (used in diagnostics).
    fn name(&self) -> &str {
        "unnamed-hook"
    }
}

impl fmt::Debug for dyn SdkHook {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SdkHook({})", self.name())
    }
}
