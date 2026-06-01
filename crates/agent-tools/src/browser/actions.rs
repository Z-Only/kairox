//! Browser action helpers for converting between tool invocation args and typed actions.

use super::types::BrowserAction;

/// Parse a browser action from raw JSON arguments.
/// The `action` field determines which variant to construct.
pub fn parse_action(args: &serde_json::Value) -> Result<BrowserAction, String> {
    serde_json::from_value(args.clone())
        .map_err(|e| format!("Failed to parse browser action: {}", e))
}
