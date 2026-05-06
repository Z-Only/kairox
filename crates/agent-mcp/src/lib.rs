//! agent-mcp — MCP (Model Context Protocol) client crate.
//!
//! Provides types, transports, and lifecycle management for connecting to
//! MCP servers from within the Kairox agent workbench.

pub mod catalog;
pub mod client;
pub mod discovery;
pub mod installer;
pub mod lifecycle;
pub mod transport;
pub mod types;

pub use types::*;
// Re-export key top-level types
pub use client::McpClient;
pub use discovery::DiscoveryCache;
pub use lifecycle::ServerLifecycle;

/// Errors that can occur during MCP client operations.
#[derive(Debug, thiserror::Error)]
pub enum McpError {
    #[error("transport error: {0}")]
    Transport(String),
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("server not running: {0}")]
    NotRunning(String),
    #[error("handshake failed: {0}")]
    Handshake(String),
    #[error("tool not found: {0}")]
    ToolNotFound(String),
    #[error("resource not found: {0}")]
    ResourceNotFound(String),
    #[error("prompt not found: {0}")]
    PromptNotFound(String),
    #[error("invocation failed: {0}")]
    InvocationFailed(String),
    #[error("server crashed: {0}")]
    ServerCrash(String),
    #[error("max restart attempts exceeded for {0}")]
    MaxRestartsExceeded(String),
    #[error("catalog error: {0}")]
    Catalog(String),
    #[error("installer error: {0}")]
    Installer(String),
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
}

/// Result type for MCP operations.
pub type Result<T> = std::result::Result<T, McpError>;

impl From<crate::catalog::CatalogError> for McpError {
    fn from(e: crate::catalog::CatalogError) -> Self {
        McpError::Catalog(e.to_string())
    }
}
