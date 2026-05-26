use crate::dag_executor::{DagConfig, DagExecutor};
#[cfg(test)]
use crate::facade_runtime::ExecutionMode;
use crate::facade_runtime::LocalRuntime;
use crate::skill_package::SkillPackageManager;
use crate::McpServerManager;
use agent_core::PermissionDecision;
#[cfg(test)]
use agent_core::SendMessageRequest;
use agent_mcp::types::McpServerDef;
use agent_memory::{ContextAssembler, MemoryStore};
use agent_store::{EventStore, ProjectMetaRepository};
use agent_tools::{
    ApprovalPolicy, BuiltinProvider, PermissionEngine, SandboxPolicy, ToolProvider, ToolRegistry,
};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

impl<S, M> LocalRuntime<S, M>
where
    S: EventStore + 'static,
    M: agent_models::ModelClient + 'static,
{
    /// Builder: set the permission engine from an explicit
    /// `(ApprovalPolicy, SandboxPolicy)` pair.
    pub fn with_approval_and_sandbox(
        mut self,
        approval: ApprovalPolicy,
        sandbox: SandboxPolicy,
    ) -> Self {
        self.permission_engine = Arc::new(Mutex::new(PermissionEngine::new(approval, sandbox)));
        self
    }

    pub fn with_skill_registry(mut self, registry: Arc<dyn agent_skills::SkillRegistry>) -> Self {
        self.skill_registry = Some(registry);
        self
    }

    pub fn with_skill_package_manager(mut self, manager: Arc<dyn SkillPackageManager>) -> Self {
        self.skill_package_manager = manager;
        self
    }

    pub fn with_skill_settings_roots(
        mut self,
        roots: crate::skill_settings::SkillSettingsRoots,
    ) -> Self {
        self.skill_settings_roots = roots;
        self
    }

    pub(crate) fn skill_settings_roots(&self) -> crate::skill_settings::SkillSettingsRoots {
        self.skill_settings_roots.clone()
    }

    pub fn with_agent_settings_roots(
        mut self,
        roots: crate::agent_settings::AgentSettingsRoots,
    ) -> Self {
        self.agent_settings_roots = roots;
        self
    }

    pub(crate) fn agent_settings_roots(&self) -> crate::agent_settings::AgentSettingsRoots {
        self.agent_settings_roots.clone()
    }

    pub fn with_plugin_settings_roots(
        mut self,
        roots: crate::plugin_settings::PluginSettingsRoots,
    ) -> Self {
        self.plugin_settings_roots = roots;
        self
    }

    pub(crate) fn plugin_settings_roots(&self) -> crate::plugin_settings::PluginSettingsRoots {
        self.plugin_settings_roots.clone()
    }

    /// Legacy builder kept for compatibility. The `max_tokens` argument is
    /// ignored — Task 8 will replace this with per-session `ContextBudget`
    /// configuration. Until then call sites can keep passing their old value.
    pub fn with_context_limit(mut self, _max_tokens: usize) -> Self {
        self.context_assembler = ContextAssembler::new_standalone();
        self
    }

    pub fn tool_registry(&self) -> Arc<Mutex<ToolRegistry>> {
        self.tool_registry.clone()
    }

    pub(crate) fn project_repository(&self) -> agent_core::Result<ProjectMetaRepository> {
        self.store
            .sqlite_pool()
            .map(ProjectMetaRepository::new)
            .ok_or_else(crate::project::invalid_project_store_error)
    }

    /// Get the current approval policy.
    pub async fn approval_policy(&self) -> ApprovalPolicy {
        self.permission_engine.lock().await.approval_policy()
    }

    /// Get the current sandbox policy.
    pub async fn sandbox_policy(&self) -> SandboxPolicy {
        self.permission_engine.lock().await.sandbox_policy().clone()
    }

    /// Set the current approval policy (session-scoped, in-memory).
    pub async fn set_approval_policy(&self, approval: ApprovalPolicy) {
        self.permission_engine
            .lock()
            .await
            .set_approval_policy(approval);
    }

    /// Set the current sandbox policy (session-scoped, in-memory).
    pub async fn set_sandbox_policy(&self, sandbox: SandboxPolicy) {
        self.permission_engine
            .lock()
            .await
            .set_sandbox_policy(sandbox);
    }

    /// Persist and activate approval policy for a specific session.
    pub async fn set_session_approval_policy(
        &self,
        session_id: &agent_core::SessionId,
        approval: ApprovalPolicy,
    ) -> agent_core::Result<()> {
        let approval_str = approval.to_string();
        self.store
            .update_approval_policy(session_id.as_str(), &approval_str)
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;
        self.set_approval_policy(approval).await;
        Ok(())
    }

    /// Persist and activate sandbox policy for a specific session.
    pub async fn set_session_sandbox_policy(
        &self,
        session_id: &agent_core::SessionId,
        sandbox: &SandboxPolicy,
    ) -> agent_core::Result<()> {
        let json = serde_json::to_string(sandbox)
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;
        self.store
            .update_sandbox_policy(session_id.as_str(), &json)
            .await
            .map_err(|e| agent_core::CoreError::InvalidState(e.to_string()))?;
        self.set_sandbox_policy(sandbox.clone()).await;
        Ok(())
    }

    /// Set the memory store for persistent memory.
    pub fn with_memory_store(mut self, store: Arc<dyn MemoryStore>) -> Self {
        self.memory_store = Some(store.clone());
        self.context_assembler = ContextAssembler::new(store);
        self
    }

    /// Get a reference to the memory store (if configured).
    pub fn memory_store(&self) -> Option<Arc<dyn MemoryStore>> {
        self.memory_store.clone()
    }

    /// Register builtin tools (shell.exec, search.ripgrep, patch.apply, fs.read)
    pub async fn with_builtin_tools(mut self, workspace_root: PathBuf) -> Self {
        if self.skill_settings_roots.workspace_root.is_none()
            && self.skill_settings_roots.user_root.is_none()
        {
            let home_dir = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            self.skill_settings_roots =
                crate::skills::build_default_skill_settings_roots(&home_dir, &workspace_root);
        }
        if self.agent_settings_roots.workspace_root.is_none()
            && self.agent_settings_roots.user_root.is_none()
        {
            let home_dir = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            self.agent_settings_roots = crate::agent_settings::build_default_agent_settings_roots(
                &home_dir,
                &workspace_root,
            );
        }
        if self.plugin_settings_roots.workspace_root.is_none()
            && self.plugin_settings_roots.user_root.is_none()
        {
            let home_dir = std::env::var_os("HOME")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."));
            self.plugin_settings_roots =
                crate::plugin_settings::build_default_plugin_settings_roots(
                    &home_dir,
                    &workspace_root,
                );
        }
        let provider = BuiltinProvider::with_defaults(workspace_root);
        self.tool_registry
            .lock()
            .await
            .add_provider(Box::new(provider))
            .await;
        self
    }

    /// Register a custom tool provider
    pub async fn with_provider(self, provider: Box<dyn ToolProvider>) -> Self {
        self.tool_registry.lock().await.add_provider(provider).await;
        self
    }

    /// Configure MCP servers from parsed config definitions.
    pub async fn with_mcp_servers(mut self, configs: Vec<McpServerDef>) -> Self {
        if configs.is_empty() {
            return self;
        }
        let mut manager = McpServerManager::from_config(
            configs,
            self.tool_registry.clone(),
            self.permission_engine.clone(),
            Some(self.event_tx.clone()),
        );
        let results = manager.start_persistent_servers().await;
        for result in &results {
            if let Err(e) = result {
                tracing::warn!("MCP server startup warning: {}", e);
            }
        }
        self.mcp_manager = Some(Arc::new(Mutex::new(manager)));
        self
    }

    /// Get a reference to the MCP server manager (if configured).
    pub fn mcp_manager(&self) -> Option<Arc<Mutex<McpServerManager>>> {
        self.mcp_manager.clone()
    }

    /// Check health of an MCP server: start + discover tools.
    /// Returns tools + healthy flag. Healthy = tools fetched successfully.
    /// Also syncs disabled tools from config into the manager.
    pub async fn check_mcp_health(
        &self,
        server_id: &str,
    ) -> agent_core::Result<agent_mcp::types::CheckHealthResult> {
        // Sync disabled tools from config into manager
        if let Some(config_path) =
            crate::mcp_settings::writable_mcp_config_path(self.marketplace_dir.as_deref())?
        {
            if let Some(manager) = self.mcp_manager() {
                let disabled =
                    crate::mcp_settings::get_mcp_disabled_tools(&config_path, server_id).await?;
                let mut manager = manager.lock().await;
                manager.load_disabled_tools(server_id, disabled);
            }
        }

        match self.mcp_manager() {
            Some(manager) => {
                let mut manager = manager.lock().await;
                Ok(manager
                    .check_health(server_id, Some(std::time::Duration::from_secs(15)))
                    .await)
            }
            None => Ok(agent_mcp::types::CheckHealthResult {
                tools: Vec::new(),
                healthy: false,
                error: Some("No MCP servers configured".into()),
            }),
        }
    }

    /// Enable or disable a specific tool on an MCP server.
    /// Updates both the runtime state (tool registry) and the config file.
    pub async fn set_mcp_tool_disabled(
        &self,
        server_id: &str,
        tool_name: &str,
        disabled: bool,
    ) -> agent_core::Result<()> {
        // Persist to config file
        if let Some(config_path) =
            crate::mcp_settings::writable_mcp_config_path(self.marketplace_dir.as_deref())?
        {
            crate::mcp_settings::set_mcp_tool_disabled_in_file(
                &config_path,
                server_id,
                tool_name,
                disabled,
            )
            .await?;
        }

        // Update runtime state
        if let Some(manager) = self.mcp_manager() {
            let mut manager = manager.lock().await;
            manager
                .set_tool_disabled(server_id, tool_name, disabled)
                .await
                .map_err(|e| {
                    agent_core::CoreError::InvalidState(format!("failed to update tool state: {e}"))
                })?;
        }

        Ok(())
    }

    /// Get disabled tool names for a server from the config file.
    pub async fn get_mcp_disabled_tools(
        &self,
        server_id: &str,
    ) -> agent_core::Result<std::collections::HashSet<String>> {
        match crate::mcp_settings::writable_mcp_config_path(self.marketplace_dir.as_deref())? {
            Some(config_path) => {
                crate::mcp_settings::get_mcp_disabled_tools(&config_path, server_id).await
            }
            None => Ok(std::collections::HashSet::new()),
        }
    }

    /// Enable DAG execution mode with the default configuration.
    pub async fn with_dag_execution(mut self) -> Self {
        self.dag_config = DagConfig::default();
        self.dag_executor = Some(Arc::new(
            DagExecutor::new(
                self.store.clone(),
                self.model.clone(),
                self.event_tx.clone(),
                self.tool_registry.clone(),
                self.permission_engine.clone(),
                self.pending_permissions.clone(),
                self.memory_store.clone(),
                self.dag_config.clone(),
                self.agent_settings_roots.clone(),
            )
            .await,
        ));
        self
    }

    /// Enable DAG execution mode with a custom configuration.
    pub async fn with_dag_config(mut self, config: DagConfig) -> Self {
        self.dag_config = config.clone();
        self.dag_executor = Some(Arc::new(
            DagExecutor::new(
                self.store.clone(),
                self.model.clone(),
                self.event_tx.clone(),
                self.tool_registry.clone(),
                self.permission_engine.clone(),
                self.pending_permissions.clone(),
                self.memory_store.clone(),
                config,
                self.agent_settings_roots.clone(),
            )
            .await,
        ));
        self
    }

    /// Determine the execution mode for a given request.
    #[cfg(test)]
    pub(crate) fn execution_mode(&self, request: &SendMessageRequest) -> ExecutionMode {
        if request.content.starts_with("/plan ") && self.dag_executor.is_some() {
            ExecutionMode::DagExecution
        } else {
            ExecutionMode::SingleStep
        }
    }

    pub async fn resolve_permission(
        &self,
        request_id: &str,
        decision: PermissionDecision,
    ) -> agent_core::Result<()> {
        crate::permission::resolve_permission(&self.pending_permissions, request_id, decision).await
    }
}
