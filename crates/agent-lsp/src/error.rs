use thiserror::Error;

#[derive(Debug, Error)]
pub enum LspError {
    #[error("transport error: {0}")]
    Transport(String),
    #[error("protocol error: {0}")]
    Protocol(String),
    #[error("initialization failed: {0}")]
    Init(String),
    #[error("server not initialized")]
    NotInitialized,
    #[error("request timed out")]
    Timeout,
    #[error("JSON-RPC error {code}: {message}")]
    JsonRpc { code: i64, message: String },
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, LspError>;
