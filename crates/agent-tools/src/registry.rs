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

#[derive(Default)]
pub struct ToolRegistry {
    tools: std::collections::HashMap<String, Box<dyn Tool>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, tool: Box<dyn Tool>) {
        let id = tool.definition().tool_id.clone();
        self.tools.insert(id, tool);
    }

    pub fn get(&self, tool_id: &str) -> Option<&dyn Tool> {
        self.tools.get(tool_id).map(|t| t.as_ref())
    }

    pub fn list_definitions(&self) -> Vec<ToolDefinition> {
        let mut defs: Vec<_> = self.tools.values().map(|t| t.definition()).collect();
        defs.sort_by(|a, b| a.tool_id.cmp(&b.tool_id));
        defs
    }

    pub async fn invoke_with_permission(
        &self,
        engine: &PermissionEngine,
        invocation: ToolInvocation,
    ) -> crate::Result<ToolOutput> {
        let tool = self
            .tools
            .get(&invocation.tool_id)
            .ok_or_else(|| crate::ToolError::NotFound(invocation.tool_id.clone()))?;
        let risk = tool.risk(&invocation);
        match engine.decide(&risk) {
            PermissionOutcome::Allowed => tool.invoke(invocation).await,
            PermissionOutcome::RequiresApproval => Err(crate::ToolError::PermissionRequired(
                invocation.tool_id.clone(),
            )),
            PermissionOutcome::Denied(reason) => Err(crate::ToolError::PermissionDenied(reason)),
        }
    }
}

#[cfg(test)]
mod registry_tests {
    use super::*;
    use crate::permission::{PermissionEngine, PermissionMode};

    struct EchoTool;

    #[async_trait]
    impl Tool for EchoTool {
        fn definition(&self) -> ToolDefinition {
            ToolDefinition {
                tool_id: "echo".into(),
                description: "Echoes input".into(),
                required_capability: "echo".into(),
            }
        }

        fn risk(&self, _invocation: &ToolInvocation) -> ToolRisk {
            ToolRisk::read("echo")
        }

        async fn invoke(&self, invocation: ToolInvocation) -> crate::Result<ToolOutput> {
            Ok(ToolOutput {
                text: format!("echo: {}", invocation.arguments),
                truncated: false,
            })
        }
    }

    #[test]
    fn registers_and_lists_tools() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(EchoTool));
        let defs = registry.list_definitions();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].tool_id, "echo");
    }

    #[test]
    fn retrieves_tool_by_id() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(EchoTool));
        assert!(registry.get("echo").is_some());
        assert!(registry.get("nonexistent").is_none());
    }

    #[tokio::test]
    async fn invoke_with_permission_allows_reads_in_readonly_mode() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(EchoTool));
        let engine = PermissionEngine::new(PermissionMode::ReadOnly);
        let invocation = ToolInvocation {
            tool_id: "echo".into(),
            arguments: serde_json::json!({"text": "hello"}),
            workspace_id: "/tmp/test".into(),
            preview: "echo hello".into(),
            timeout_ms: 5000,
            output_limit_bytes: 10240,
        };
        let result = registry.invoke_with_permission(&engine, invocation).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn invoke_with_permission_denies_writes_in_readonly_mode() {
        use crate::ToolError;

        struct WriteTool;

        #[async_trait]
        impl Tool for WriteTool {
            fn definition(&self) -> ToolDefinition {
                ToolDefinition {
                    tool_id: "write".into(),
                    description: "Writes data".into(),
                    required_capability: "filesystem.write".into(),
                }
            }

            fn risk(&self, _invocation: &ToolInvocation) -> ToolRisk {
                ToolRisk::write("write")
            }

            async fn invoke(&self, _invocation: ToolInvocation) -> crate::Result<ToolOutput> {
                Ok(ToolOutput {
                    text: "wrote".into(),
                    truncated: false,
                })
            }
        }

        let mut registry = ToolRegistry::new();
        registry.register(Box::new(WriteTool));
        let engine = PermissionEngine::new(PermissionMode::ReadOnly);
        let invocation = ToolInvocation {
            tool_id: "write".into(),
            arguments: serde_json::json!({}),
            workspace_id: "/tmp/test".into(),
            preview: "write data".into(),
            timeout_ms: 5000,
            output_limit_bytes: 10240,
        };
        let result = registry.invoke_with_permission(&engine, invocation).await;
        assert!(matches!(result, Err(ToolError::PermissionDenied(_))));
    }
}
