use crate::permission::{PermissionEngine, PermissionOutcome, ToolRisk};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Schema definition exposed by a tool to the model and runtime.
pub struct ToolDefinition {
    pub tool_id: String,
    pub description: String,
    pub required_capability: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// A request to invoke a tool with specific arguments.
pub struct ToolInvocation {
    pub tool_id: String,
    pub arguments: serde_json::Value,
    pub workspace_id: String,
    pub session_id: String,
    pub preview: String,
    pub timeout_ms: u64,
    pub output_limit_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// An image attachment produced by a tool invocation (e.g. screenshot).
pub struct ImageAttachment {
    /// MIME type, e.g. `"image/png"`.
    pub media_type: String,
    /// Base64-encoded image data (no `data:` prefix).
    pub data: String,
    /// Optional human-readable label, e.g. `"screenshot"`.
    pub label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// The result of a tool invocation.
pub struct ToolOutput {
    pub text: String,
    pub truncated: bool,
    /// Image attachments produced by the tool (e.g. screenshots).
    /// Tools that don't produce images leave this empty.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub images: Vec<ImageAttachment>,
}

#[async_trait]
/// Trait for a single tool that the agent can invoke.
///
/// Each tool provides a definition, a risk assessment, and an async invoke method.
pub trait Tool: Send + Sync {
    /// Return the tool's schema definition.
    fn definition(&self) -> ToolDefinition;
    /// Assess the risk level of a prospective invocation.
    fn risk(&self, invocation: &ToolInvocation) -> ToolRisk;
    /// Execute the tool invocation and return the output.
    async fn invoke(&self, invocation: ToolInvocation) -> crate::Result<ToolOutput>;
}

#[async_trait]
/// Trait for a provider that supplies a collection of tools.
///
/// Implementations include [`BuiltinProvider`](crate::BuiltinProvider) for built-in tools
/// and [`McpToolAdapter`](crate::McpToolAdapter) for MCP server tools.
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
        PermissionOutcome::Pending => Err(crate::ToolError::PermissionRequired(
            "awaiting user confirmation".into(),
        )),
        PermissionOutcome::PromptWithTrust => {
            Err(crate::ToolError::PermissionRequired(risk.tool_id.clone()))
        }
    }
}

#[derive(Default)]
pub struct ToolRegistry {
    /// Tools registered via register() — stored as Arc for cheap cloning.
    internal: HashMap<String, Arc<dyn Tool>>,
    /// Provider list (1-based index in self.index).
    providers: Vec<Box<dyn ToolProvider>>,
    /// tool_id → 0 means internal, 1..=N means providers[N-1].
    index: HashMap<String, usize>,
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

    /// Unregister a tool by id. Removes from both `internal` and `index`.
    pub fn unregister(&mut self, tool_id: &str) {
        self.internal.remove(tool_id);
        self.index.remove(tool_id);
    }

    /// Add a provider. Calls `provider.list_tools().await`, populates `index`, stores provider.
    pub async fn add_provider(&mut self, provider: Box<dyn ToolProvider>) {
        let provider_name = provider.name().to_string();
        if self
            .providers
            .iter()
            .any(|existing| existing.name() == provider_name)
        {
            self.providers
                .retain(|existing| existing.name() != provider_name);
            self.rebuild_provider_index().await;
        }
        let provider_idx = self.providers.len() + 1; // 1-based
        let defs = provider.list_tools().await;
        for def in defs {
            self.index.entry(def.tool_id).or_insert(provider_idx);
        }
        self.providers.push(provider);
    }

    async fn rebuild_provider_index(&mut self) {
        self.index.retain(|_, idx| *idx == 0);
        for (pos, provider) in self.providers.iter().enumerate() {
            let provider_idx = pos + 1;
            for def in provider.list_tools().await {
                self.index.entry(def.tool_id).or_insert(provider_idx);
            }
        }
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
            PermissionOutcome::Pending => Err(crate::ToolError::PermissionRequired(
                "awaiting user confirmation".into(),
            )),
            PermissionOutcome::PromptWithTrust => Err(crate::ToolError::PermissionRequired(
                "MCP tool requires trust approval".into(),
            )),
        }
    }
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
