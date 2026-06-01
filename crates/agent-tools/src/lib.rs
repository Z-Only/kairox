pub mod browser;
pub mod filesystem;
pub mod fs_helpers;
pub mod fs_list;
pub mod fs_read;
pub mod fs_write;
pub mod monitor;
pub mod patch;
pub mod permission;
pub mod policy;
pub mod provider;
pub mod registry;
pub mod search;
pub mod shell;

pub use agent_mcp::McpServerDef;
pub use agent_mcp::McpTransportDef;
pub use browser::{BrowserAction, BrowserBatchTool, BrowserResult, BrowserState, BrowserTool};
pub use filesystem::{FsListEntry, FsListTool, FsReadTool, FsWriteTool};
pub use monitor::{
    MonitorInfo, MonitorListTool, MonitorRegistry, MonitorStartTool, MonitorStopTool,
};
pub use patch::{parse_unified_diff, FilePatch, Hunk, PatchApplyTool, PatchLine, PatchParseError};
pub use permission::{PermissionEngine, PermissionOutcome, ToolEffect, ToolRisk};
pub use policy::{
    ApprovalPolicy, ApprovalReason, PolicyDecision, PolicyEffect, PolicyEngine, PolicyRisk,
    SandboxPolicy,
};
pub use provider::{
    workspace_scoped_builtin_tool, BuiltinProvider, DapToolProvider, LspToolProvider,
    McpToolAdapter,
};
pub use registry::{
    require_permission, ArcTool, Tool, ToolDefinition, ToolInvocation, ToolOutput, ToolProvider,
    ToolRegistry,
};
pub use search::{glob_matches, RipgrepSearchTool, SearchEngine, SearchResult, SearchResults};
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
    #[error("patch parse error: {0}")]
    PatchParseFailed(String),
    #[error("patch context mismatch at line {line}: expected {expected:?}, got {actual:?}")]
    ContextMismatch {
        line: usize,
        expected: String,
        actual: String,
    },
    #[error("patch context matched multiple locations near line {line}: {candidates:?}")]
    AmbiguousPatchContext { line: usize, candidates: Vec<usize> },
}

pub type Result<T> = std::result::Result<T, ToolError>;
