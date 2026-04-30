use crate::permission::{PermissionEngine, PermissionOutcome, ToolRisk};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

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

#[async_trait]
pub trait ToolProvider: Send + Sync {
    async fn list_tools(&self) -> Vec<ToolDefinition>;
    async fn get_tool(&self, tool_id: &str) -> Option<Box<dyn Tool>>;
    fn name(&self) -> &str;
}

/// Wrapper to return `Box<dyn Tool>` from `Arc<dyn Tool>`.
pub struct ArcTool {
    pub inner: Arc<dyn Tool>,
}

#[async_trait]
impl Tool for ArcTool {
    fn definition(&self) -> ToolDefinition {
        self.inner.definition()
    }
    fn risk(&self, invocation: &ToolInvocation) -> ToolRisk {
        self.inner.risk(invocation)
    }
    async fn invoke(&self, invocation: ToolInvocation) -> crate::Result<ToolOutput> {
        self.inner.invoke(invocation).await
    }
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

pub struct ToolRegistry {
    /// Tools registered via register() — stored as Arc for cheap cloning.
    internal: HashMap<String, Arc<dyn Tool>>,
    /// Provider list (1-based index in self.index).
    providers: Vec<Box<dyn ToolProvider>>,
    /// tool_id → 0 means internal, 1..=N means providers[N-1].
    index: HashMap<String, usize>,
}

impl Default for ToolRegistry {
    fn default() -> Self {
        Self {
            internal: HashMap::new(),
            providers: Vec::new(),
            index: HashMap::new(),
        }
    }
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a tool directly. Backward-compatible — stores in `internal`, index=0.
    pub fn register(&mut self, tool: Box<dyn Tool>) {
        let id = tool.definition().tool_id.clone();
        self.internal.insert(id.clone(), Arc::from(tool));
        self.index.insert(id, 0);
    }

    /// Add a provider. Calls `provider.list_tools().await`, populates `index`, stores provider.
    pub async fn add_provider(&mut self, provider: Box<dyn ToolProvider>) {
        let provider_idx = self.providers.len() + 1; // 1-based
        let defs = provider.list_tools().await;
        for def in defs {
            self.index.entry(def.tool_id).or_insert(provider_idx);
        }
        self.providers.push(provider);
    }

    /// List definitions from internal tools only. Backward-compatible.
    pub fn list_definitions(&self) -> Vec<ToolDefinition> {
        let mut defs: Vec<_> = self.internal.values().map(|t| t.definition()).collect();
        defs.sort_by(|a, b| a.tool_id.cmp(&b.tool_id));
        defs
    }

    /// List all tool definitions — internal + all providers. Async.
    pub async fn list_all(&self) -> Vec<ToolDefinition> {
        let mut defs = self.list_definitions();
        for provider in &self.providers {
            let mut provider_defs = provider.list_tools().await;
            defs.append(&mut provider_defs);
        }
        defs.sort_by(|a, b| a.tool_id.cmp(&b.tool_id));
        defs
    }

    /// Get a tool by ID. Async — checks internal first (via Arc), then providers in order.
    pub async fn get(&self, tool_id: &str) -> Option<Box<dyn Tool>> {
        match self.index.get(tool_id) {
            Some(&0) => {
                // Internal tool — wrap Arc in ArcTool
                self.internal
                    .get(tool_id)
                    .map(|arc| Box::new(ArcTool { inner: arc.clone() }) as Box<dyn Tool>)
            }
            Some(&idx) if idx >= 1 => {
                let provider = self.providers.get(idx - 1)?;
                provider.get_tool(tool_id).await
            }
            _ => None,
        }
    }

    /// Invoke a tool with permission checking. Now async since it calls `get()`.
    pub async fn invoke_with_permission(
        &self,
        engine: &PermissionEngine,
        invocation: ToolInvocation,
    ) -> crate::Result<ToolOutput> {
        let tool = self
            .get(&invocation.tool_id)
            .await
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

    #[tokio::test]
    async fn retrieves_tool_by_id() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(EchoTool));
        assert!(registry.get("echo").await.is_some());
        assert!(registry.get("nonexistent").await.is_none());
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

#[cfg(test)]
mod provider_tests {
    use super::*;
    use crate::permission::{PermissionEngine, PermissionMode};

    struct SingleToolProvider {
        tool_id: String,
    }

    #[async_trait]
    impl ToolProvider for SingleToolProvider {
        async fn list_tools(&self) -> Vec<ToolDefinition> {
            vec![ToolDefinition {
                tool_id: self.tool_id.clone(),
                description: format!("{} tool", self.tool_id),
                required_capability: self.tool_id.clone(),
            }]
        }

        async fn get_tool(&self, tool_id: &str) -> Option<Box<dyn Tool>> {
            if tool_id == self.tool_id {
                Some(Box::new(crate::filesystem::FsReadTool::new(
                    std::path::PathBuf::from("/tmp"),
                )))
            } else {
                None
            }
        }

        fn name(&self) -> &str {
            "single"
        }
    }

    struct MultiToolProvider {
        tool_ids: Vec<String>,
        provider_name: String,
    }

    #[async_trait]
    impl ToolProvider for MultiToolProvider {
        async fn list_tools(&self) -> Vec<ToolDefinition> {
            self.tool_ids
                .iter()
                .map(|id| ToolDefinition {
                    tool_id: id.clone(),
                    description: format!("{} tool", id),
                    required_capability: id.clone(),
                })
                .collect()
        }

        async fn get_tool(&self, tool_id: &str) -> Option<Box<dyn Tool>> {
            if self.tool_ids.iter().any(|id| id == tool_id) {
                Some(Box::new(crate::filesystem::FsReadTool::new(
                    std::path::PathBuf::from("/tmp"),
                )))
            } else {
                None
            }
        }

        fn name(&self) -> &str {
            &self.provider_name
        }
    }

    #[tokio::test]
    async fn registry_discovers_tools_from_provider() {
        let mut registry = ToolRegistry::new();
        registry
            .add_provider(Box::new(SingleToolProvider {
                tool_id: "fs.read".into(),
            }))
            .await;

        // list_all should include the provider's tool
        let all = registry.list_all().await;
        assert!(all.iter().any(|d| d.tool_id == "fs.read"));

        // get should find it
        let tool = registry.get("fs.read").await;
        assert!(tool.is_some());
    }

    #[tokio::test]
    async fn provider_priority_first_wins() {
        let mut registry = ToolRegistry::new();
        // Register an internal "fs.read" tool first
        registry.register(Box::new(crate::filesystem::FsReadTool::new(
            std::path::PathBuf::from("/tmp"),
        )));
        // Add provider with same tool_id — internal (index 0) takes priority
        registry
            .add_provider(Box::new(SingleToolProvider {
                tool_id: "fs.read".into(),
            }))
            .await;

        // get() should return the internal tool (index=0), not the provider's
        let tool = registry.get("fs.read").await;
        assert!(tool.is_some());
        // verify it's the internal one by checking that it comes from internal
        assert_eq!(registry.index.get("fs.read"), Some(&0));
    }

    #[tokio::test]
    async fn register_backward_compatible_wraps_anonymous_provider() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(crate::filesystem::FsReadTool::new(
            std::path::PathBuf::from("/tmp"),
        )));

        // It's in internal (index=0)
        assert_eq!(registry.index.get("fs.read"), Some(&0));

        // list_definitions returns internal tools
        let defs = registry.list_definitions();
        assert_eq!(defs.len(), 1);
        assert_eq!(defs[0].tool_id, "fs.read");

        // get() works
        let tool = registry.get("fs.read").await;
        assert!(tool.is_some());
    }

    #[tokio::test]
    async fn list_all_aggregates_across_providers() {
        let mut registry = ToolRegistry::new();
        registry.register(Box::new(crate::filesystem::FsReadTool::new(
            std::path::PathBuf::from("/tmp"),
        )));
        registry
            .add_provider(Box::new(SingleToolProvider {
                tool_id: "custom.search".into(),
            }))
            .await;
        registry
            .add_provider(Box::new(MultiToolProvider {
                tool_ids: vec!["custom.build".into(), "custom.test".into()],
                provider_name: "build_test".into(),
            }))
            .await;

        let all = registry.list_all().await;
        assert_eq!(all.len(), 4);
        let ids: Vec<&str> = all.iter().map(|d| d.tool_id.as_str()).collect();
        assert!(ids.contains(&"fs.read"));
        assert!(ids.contains(&"custom.search"));
        assert!(ids.contains(&"custom.build"));
        assert!(ids.contains(&"custom.test"));

        // list_definitions should only return internal
        let internal = registry.list_definitions();
        assert_eq!(internal.len(), 1);
        assert_eq!(internal[0].tool_id, "fs.read");
    }

    #[tokio::test]
    async fn invoke_with_permission_works_with_provider_tools() {
        let mut registry = ToolRegistry::new();
        registry
            .add_provider(Box::new(SingleToolProvider {
                tool_id: "fs.read".into(),
            }))
            .await;

        let engine = PermissionEngine::new(PermissionMode::ReadOnly);
        let invocation = ToolInvocation {
            tool_id: "fs.read".into(),
            arguments: serde_json::json!({"path": "test.txt"}),
            workspace_id: "/tmp/test".into(),
            preview: "fs.read test.txt".into(),
            timeout_ms: 5000,
            output_limit_bytes: 10240,
        };

        // fs.read is a read tool, so it should be allowed in ReadOnly mode
        let result = registry.invoke_with_permission(&engine, invocation).await;
        // The result might fail due to file not existing, but it should not be a permission error
        match result {
            Ok(_) => {}
            Err(crate::ToolError::PermissionDenied(_)) => {
                panic!("fs.read should be allowed in ReadOnly mode")
            }
            Err(crate::ToolError::PermissionRequired(_)) => {
                panic!("fs.read should not require approval in ReadOnly mode")
            }
            Err(_) => {} // Other errors (e.g. file not found) are fine
        }
    }
}
