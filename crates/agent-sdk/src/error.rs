//! SDK error types.

/// Errors that can occur when using the Kairox SDK.
#[derive(Debug, thiserror::Error)]
pub enum SdkError {
    /// The workspace path does not exist or is not a directory.
    #[error("invalid workspace path: {0}")]
    InvalidWorkspacePath(String),

    /// Configuration loading failed.
    #[error("configuration error: {0}")]
    ConfigError(String),

    /// Runtime bootstrap failed.
    #[error("runtime initialization failed: {0}")]
    RuntimeInit(String),

    /// A domain/facade operation failed.
    #[error(transparent)]
    Core(#[from] agent_core::CoreError),

    /// A runtime-level operation failed.
    #[error(transparent)]
    Runtime(#[from] agent_runtime::RuntimeError),

    /// The session is no longer active.
    #[error("session not active: {0}")]
    SessionNotActive(String),

    /// A hook rejected the operation.
    #[error("hook rejected: {0}")]
    HookRejected(String),
}

/// Convenience alias used throughout the SDK.
pub type SdkResult<T> = std::result::Result<T, SdkError>;
