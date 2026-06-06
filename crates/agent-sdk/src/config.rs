//! SDK configuration types.
//!
//! These types mirror the runtime's policy types but provide a simpler,
//! SDK-consumer-friendly surface.

use std::path::PathBuf;

/// Top-level configuration for building a [`KairoxSdk`](crate::KairoxSdk).
#[derive(Debug, Clone)]
pub struct SdkConfig {
    /// Path to the workspace (project root). Required.
    pub workspace_path: PathBuf,

    /// Override the data directory (default: `~/.kairox`).
    pub data_dir: Option<PathBuf>,

    /// Override the home directory (default: `$HOME`).
    pub home_dir: Option<PathBuf>,

    /// SQLite database filename within `data_dir` (default: `kairox.db`).
    pub database_filename: String,

    /// Which model profile alias to use by default (uses config default if
    /// `None`).
    pub default_profile: Option<String>,

    /// Approval policy for tool execution.
    pub approval_policy: SdkApprovalPolicy,

    /// Sandbox policy for tool execution.
    pub sandbox_policy: SdkSandboxPolicy,

    /// Whether to wire up MCP servers from config.
    pub enable_mcp_servers: bool,

    /// Whether to wire up LSP servers from config.
    pub enable_lsp_servers: bool,

    /// Whether to enable the marketplace catalog.
    pub enable_marketplace: bool,
}

impl Default for SdkConfig {
    fn default() -> Self {
        Self {
            workspace_path: PathBuf::from("."),
            data_dir: None,
            home_dir: None,
            database_filename: "kairox.db".to_string(),
            default_profile: None,
            approval_policy: SdkApprovalPolicy::Never,
            sandbox_policy: SdkSandboxPolicy::WorkspaceWrite,
            enable_mcp_servers: true,
            enable_lsp_servers: false,
            enable_marketplace: false,
        }
    }
}

/// When to ask the user for approval before tool execution.
///
/// Maps to [`agent_tools::ApprovalPolicy`] internally.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SdkApprovalPolicy {
    /// Never ask — auto-approve everything.
    Never,
    /// Ask when a tool reports elevated risk.
    OnRequest,
    /// Always ask before any tool execution.
    Always,
}

impl From<SdkApprovalPolicy> for agent_tools::ApprovalPolicy {
    fn from(policy: SdkApprovalPolicy) -> Self {
        match policy {
            SdkApprovalPolicy::Never => agent_tools::ApprovalPolicy::Never,
            SdkApprovalPolicy::OnRequest => agent_tools::ApprovalPolicy::OnRequest,
            SdkApprovalPolicy::Always => agent_tools::ApprovalPolicy::Always,
        }
    }
}

/// What the sandbox structurally allows.
///
/// Maps to [`agent_tools::SandboxPolicy`] internally.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SdkSandboxPolicy {
    /// Read-only access — no writes, no network.
    ReadOnly,
    /// Workspace-scoped writes with optional network access.
    WorkspaceWrite,
    /// Full access — dangerous, use only for trusted agents.
    FullAccess,
}

impl SdkSandboxPolicy {
    pub(crate) fn into_runtime_policy(
        self,
        workspace_root: &std::path::Path,
    ) -> agent_tools::SandboxPolicy {
        match self {
            Self::ReadOnly => agent_tools::SandboxPolicy::ReadOnly,
            Self::WorkspaceWrite => agent_tools::SandboxPolicy::WorkspaceWrite {
                network_access: false,
                writable_roots: vec![workspace_root.to_path_buf()],
            },
            Self::FullAccess => agent_tools::SandboxPolicy::DangerFullAccess,
        }
    }
}
