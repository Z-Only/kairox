//! SDK builder — fluent API for constructing a [`KairoxSdk`] instance.

use std::path::PathBuf;
use std::sync::Arc;

use agent_runtime::ui_bootstrap::{
    build_ui_runtime_from_store, connect_ui_event_store, default_data_dir, default_home_dir,
    load_catalog_sources, load_ui_config, UiRuntimeOptions,
};

use crate::config::{SdkApprovalPolicy, SdkConfig, SdkSandboxPolicy};
use crate::error::{SdkError, SdkResult};
use crate::hooks::SdkHook;
use crate::KairoxSdk;

/// Fluent builder for [`KairoxSdk`].
///
/// # Example
///
/// ```rust,no_run
/// # use agent_sdk::KairoxSdk;
/// # async fn example() -> Result<(), agent_sdk::SdkError> {
/// let sdk = KairoxSdk::builder()
///     .workspace("/path/to/project")
///     .approval_policy(agent_sdk::SdkApprovalPolicy::Never)
///     .sandbox_policy(agent_sdk::SdkSandboxPolicy::WorkspaceWrite)
///     .build()
///     .await?;
/// # Ok(())
/// # }
/// ```
pub struct SdkBuilder {
    config: SdkConfig,
    hooks: Vec<Arc<dyn SdkHook>>,
}

impl SdkBuilder {
    pub(crate) fn new() -> Self {
        Self {
            config: SdkConfig::default(),
            hooks: Vec::new(),
        }
    }

    /// Set the workspace (project) root path. **Required**.
    pub fn workspace(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.workspace_path = path.into();
        self
    }

    /// Override the data directory (default: `~/.kairox`).
    pub fn data_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.data_dir = Some(path.into());
        self
    }

    /// Override the home directory used for config discovery.
    pub fn home_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.config.home_dir = Some(path.into());
        self
    }

    /// Set the SQLite database filename (default: `kairox.db`).
    pub fn database_filename(mut self, filename: impl Into<String>) -> Self {
        self.config.database_filename = filename.into();
        self
    }

    /// Set the default model profile alias.
    pub fn default_profile(mut self, alias: impl Into<String>) -> Self {
        self.config.default_profile = Some(alias.into());
        self
    }

    /// Set the approval policy for tool execution.
    pub fn approval_policy(mut self, policy: SdkApprovalPolicy) -> Self {
        self.config.approval_policy = policy;
        self
    }

    /// Set the sandbox policy for tool execution.
    pub fn sandbox_policy(mut self, policy: SdkSandboxPolicy) -> Self {
        self.config.sandbox_policy = policy;
        self
    }

    /// Enable or disable MCP server wiring (default: `true`).
    pub fn enable_mcp_servers(mut self, enabled: bool) -> Self {
        self.config.enable_mcp_servers = enabled;
        self
    }

    /// Enable or disable LSP server wiring (default: `false`).
    pub fn enable_lsp_servers(mut self, enabled: bool) -> Self {
        self.config.enable_lsp_servers = enabled;
        self
    }

    /// Enable or disable the marketplace catalog (default: `false`).
    pub fn enable_marketplace(mut self, enabled: bool) -> Self {
        self.config.enable_marketplace = enabled;
        self
    }

    /// Register a hook for tool execution interception.
    pub fn hook(mut self, hook: impl SdkHook + 'static) -> Self {
        self.hooks.push(Arc::new(hook));
        self
    }

    /// Register a pre-built hook behind an `Arc`.
    pub fn hook_arc(mut self, hook: Arc<dyn SdkHook>) -> Self {
        self.hooks.push(hook);
        self
    }

    /// Build the SDK instance, bootstrapping the runtime.
    pub async fn build(self) -> SdkResult<KairoxSdk> {
        let workspace_path = std::fs::canonicalize(&self.config.workspace_path).map_err(|err| {
            SdkError::InvalidWorkspacePath(format!(
                "{}: {}",
                self.config.workspace_path.display(),
                err
            ))
        })?;

        if !workspace_path.is_dir() {
            return Err(SdkError::InvalidWorkspacePath(format!(
                "{} is not a directory",
                workspace_path.display()
            )));
        }

        let home_dir = self.config.home_dir.unwrap_or_else(default_home_dir);
        let data_dir = self
            .config
            .data_dir
            .unwrap_or_else(|| default_data_dir(&home_dir));

        let ui_config_load = load_ui_config(&data_dir);
        for warning in &ui_config_load.warnings {
            tracing::warn!("{warning}");
        }

        let catalog_load = load_catalog_sources(&data_dir);
        for warning in &catalog_load.warnings {
            tracing::warn!("{warning}");
        }

        let approval_policy: agent_tools::ApprovalPolicy = self.config.approval_policy.into();
        let sandbox_policy = self
            .config
            .sandbox_policy
            .into_runtime_policy(&workspace_path);

        let store = connect_ui_event_store(&data_dir, &self.config.database_filename)
            .await
            .map_err(|err| SdkError::RuntimeInit(err.to_string()))?;

        let options = UiRuntimeOptions {
            home_dir,
            data_dir,
            database_filename: self.config.database_filename,
            workspace_root: workspace_path.clone(),
            approval_policy,
            sandbox_policy,
            config: ui_config_load.config,
            catalog_sources: catalog_load.sources,
            enable_marketplace: self.config.enable_marketplace,
            enable_mcp_servers: self.config.enable_mcp_servers,
            enable_lsp_servers: self.config.enable_lsp_servers,
            enable_plugin_skill_roots: true,
        };

        let bootstrap = build_ui_runtime_from_store(store, options)
            .await
            .map_err(|err| SdkError::RuntimeInit(err.to_string()))?;

        Ok(KairoxSdk {
            runtime: Arc::new(bootstrap.runtime),
            workspace_path,
            hooks: self.hooks,
        })
    }
}

#[cfg(test)]
#[path = "builder_tests.rs"]
mod tests;
