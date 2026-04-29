pub mod fake;
pub mod ollama;
pub mod openai_compatible;
pub mod profile;
pub mod types;

pub use fake::FakeModelClient;
pub use profile::{ModelCapabilities, ModelProfile};
pub use types::{ModelClient, ModelEvent, ModelMessage, ModelRequest, ModelUsage};

#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("model request failed: {0}")]
    Request(String),
}

pub type Result<T> = std::result::Result<T, ModelError>;
