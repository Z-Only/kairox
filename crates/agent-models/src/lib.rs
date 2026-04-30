pub mod fake;
pub mod ollama;
pub mod openai_compatible;
pub mod profile;
pub mod types;

pub use fake::FakeModelClient;
pub use profile::{ModelCapabilities, ModelProfile};
pub use types::{
    ModelClient, ModelEvent, ModelMessage, ModelRequest, ModelUsage, ToolCall, ToolDefinition,
};

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("model request failed: {0}")]
    Request(String),
    #[error("http error: {0}")]
    Http(String),
    #[error("stream parse error: {0}")]
    StreamParse(String),
    #[error("api error: {0}")]
    Api(String),
}

pub type Result<T> = std::result::Result<T, ModelError>;
