pub mod filesystem;
pub mod mcp;
pub mod patch;
pub mod permission;
pub mod registry;
pub mod search;
pub mod shell;

pub use filesystem::FsReadTool;
pub use mcp::{map_mcp_tool, McpServerConfig, McpTool};
pub use permission::{PermissionEngine, PermissionMode, PermissionOutcome, ToolEffect, ToolRisk};
pub use registry::{
    require_permission, ArcTool, Tool, ToolDefinition, ToolInvocation, ToolOutput, ToolProvider,
    ToolRegistry,
};
pub use shell::{classify_command, parse_command, CommandRisk, ShellExecTool};

#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("permission required for {0}")]
    PermissionRequired(String),
    #[error("permission denied: {0}")]
    PermissionDenied(String),
    #[error("path escapes workspace: {0}")]
    WorkspaceEscape(String),
    #[error("tool not found: {0}")]
    NotFound(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("execution failed: {0}")]
    ExecutionFailed(String),
    #[error("command timed out after {0}ms")]
    Timeout(u64),
}

pub type Result<T> = std::result::Result<T, ToolError>;
