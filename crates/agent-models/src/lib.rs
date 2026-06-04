pub mod anthropic;
mod content_parts;
pub mod fake;
pub mod model_registry;
pub mod ollama;
pub mod openai_compatible;
pub mod profile;
pub mod retry;
pub mod router;
pub mod types;

pub use anthropic::AnthropicClient;
pub use anthropic::AnthropicConfig;
pub use content_parts::{
    estimate_data_uri_image_tokens, sanitize_markdown_data_uri_images, EmbeddedImageSummary,
    MultimodalContentPart, SanitizedMarkdownContent,
};
pub use fake::FakeModelClient;
pub use model_registry::{lookup as lookup_limits, LimitSource, ModelLimits};
pub use ollama::OllamaClient;
pub use ollama::OllamaConfig;
pub use openai_compatible::OpenAiCompatibleClient;
pub use openai_compatible::OpenAiCompatibleConfig;
pub use profile::{ModelCapabilities, ModelProfile};
pub use retry::{with_retry, RetryConfig};
pub use router::ModelRouter;
pub use types::{
    ModelClient, ModelEvent, ModelMessage, ModelRequest, ModelUsage, ToolCall, ToolDefinition,
};

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("model request failed: {0}")]
    Request(String),
    #[error("http error (status {status}): {message}")]
    Http { status: u16, message: String },
    #[error("connection error: {0}")]
    Connection(String),
    #[error("stream parse error: {0}")]
    StreamParse(String),
    #[error("api error (status {status}): {message}")]
    Api { status: u16, message: String },
}

impl ModelError {
    /// Classify whether this error is recoverable (worth retrying).
    ///
    /// Recoverable errors include rate-limit (429), server errors (5xx),
    /// connection/timeout errors, and responses containing "overloaded".
    pub fn is_recoverable(&self) -> bool {
        match self {
            ModelError::Connection(_) => true,
            ModelError::Http { status, message } | ModelError::Api { status, message } => {
                *status == 429 || *status >= 500 || message.to_lowercase().contains("overloaded")
            }
            ModelError::Request(_) | ModelError::StreamParse(_) => false,
        }
    }

    /// Extract HTTP status code if available.
    pub fn http_status(&self) -> Option<u16> {
        match self {
            ModelError::Http { status, .. } | ModelError::Api { status, .. } => Some(*status),
            _ => None,
        }
    }
}

pub type Result<T> = std::result::Result<T, ModelError>;

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
