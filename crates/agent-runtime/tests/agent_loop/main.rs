//! Integration tests for the agent loop, organised by theme.
//!
//! Each submodule covers one behavioural area:
//! - [`hooks`]: lifecycle hooks (session start, pre/post tool, stop).
//! - [`tool_calls`]: tool-call processing and tool-result feeding.
//! - [`text_turns`]: simple non-tool completions and loop-iteration guards.
//! - [`errors`]: error propagation from the model client.
//! - [`subscriptions`]: the live event stream produced by `subscribe_session`.

use std::sync::Arc;

use agent_tools::{Tool, ToolDefinition, ToolInvocation, ToolOutput};
use async_trait::async_trait;

mod errors;
mod hooks;
mod subscriptions;
mod text_turns;
mod tool_calls;
mod trajectory;

/// Build a minimal `Config` whose only opinion is the supplied hook list.
/// Tests that don't need hooks pass an empty `Vec`.
pub(crate) fn hook_test_config(hooks: Vec<agent_config::HookConfig>) -> Arc<agent_config::Config> {
    Arc::new(agent_config::Config {
        profiles: vec![],
        mcp_servers: vec![],
        source: agent_config::ConfigSource::Defaults,
        context: agent_config::ContextPolicy::default(),
        disabled_mcp_servers: vec![],
        instructions: None,
        features: agent_config::FeatureFlags { hooks: true },
        hooks,
        lsp_servers: vec![],
        dap_servers: vec![],
        advisor: agent_config::AdvisorConfig::default(),
    })
}

/// A simple echo tool used by the hook and tool-call tests.
pub(crate) struct EchoTool;

#[async_trait]
impl Tool for EchoTool {
    fn definition(&self) -> ToolDefinition {
        ToolDefinition {
            tool_id: "echo".into(),
            description: "Echoes input".into(),
            required_capability: "echo".into(),
            parameters: serde_json::json!({"type": "object"}),
        }
    }

    fn risk(&self, _invocation: &ToolInvocation) -> agent_tools::ToolRisk {
        agent_tools::ToolRisk::read("echo")
    }

    async fn invoke(&self, invocation: ToolInvocation) -> agent_tools::Result<ToolOutput> {
        Ok(ToolOutput {
            text: format!("echo: {}", invocation.arguments),
            truncated: false,
        })
    }
}
