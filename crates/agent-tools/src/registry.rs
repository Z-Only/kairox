use crate::permission::{PermissionEngine, PermissionOutcome, ToolRisk};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub tool_id: String,
    pub description: String,
    pub required_capability: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolInvocation {
    pub tool_id: String,
    pub arguments: serde_json::Value,
    pub workspace_id: String,
    pub preview: String,
    pub timeout_ms: u64,
    pub output_limit_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolOutput {
    pub text: String,
    pub truncated: bool,
}

#[async_trait]
pub trait Tool: Send + Sync {
    fn definition(&self) -> ToolDefinition;
    fn risk(&self, invocation: &ToolInvocation) -> ToolRisk;
    async fn invoke(&self, invocation: ToolInvocation) -> crate::Result<ToolOutput>;
}

pub fn require_permission(engine: &PermissionEngine, risk: &ToolRisk) -> crate::Result<()> {
    match engine.decide(risk) {
        PermissionOutcome::Allowed => Ok(()),
        PermissionOutcome::RequiresApproval => {
            Err(crate::ToolError::PermissionRequired(risk.tool_id.clone()))
        }
        PermissionOutcome::Denied(reason) => Err(crate::ToolError::PermissionDenied(reason)),
    }
}
