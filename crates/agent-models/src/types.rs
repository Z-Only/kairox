use async_trait::async_trait;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelRequest {
    pub model_profile: String,
    pub messages: Vec<ModelMessage>,
}

impl ModelRequest {
    pub fn user_text(model_profile: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            model_profile: model_profile.into(),
            messages: vec![ModelMessage {
                role: "user".into(),
                content: content.into(),
            }],
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ModelEvent {
    TokenDelta(String),
    ToolCallRequested {
        tool_call_id: String,
        tool_id: String,
        arguments: serde_json::Value,
    },
    Completed {
        usage: Option<ModelUsage>,
    },
    Failed {
        message: String,
    },
}

#[async_trait]
pub trait ModelClient: Send + Sync {
    async fn stream(
        &self,
        request: ModelRequest,
    ) -> crate::Result<BoxStream<'static, crate::Result<ModelEvent>>>;
}
