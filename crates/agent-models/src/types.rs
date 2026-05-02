use async_trait::async_trait;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// A single message in a model conversation.
pub struct ModelMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// A tool call requested by the model during generation.
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Schema definition for a tool that the model can invoke.
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// A request to the model, including messages, system prompt, and available tools.
pub struct ModelRequest {
    pub model_profile: String,
    pub messages: Vec<ModelMessage>,
    pub system_prompt: Option<String>,
    pub tools: Vec<ToolDefinition>,
}

impl ModelRequest {
    pub fn user_text(model_profile: impl Into<String>, content: impl Into<String>) -> Self {
        Self {
            model_profile: model_profile.into(),
            messages: vec![ModelMessage {
                role: "user".into(),
                content: content.into(),
            }],
            system_prompt: None,
            tools: Vec::new(),
        }
    }

    pub fn with_system_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.system_prompt = Some(prompt.into());
        self
    }

    pub fn with_tools(mut self, tools: Vec<ToolDefinition>) -> Self {
        self.tools = tools;
        self
    }

    pub fn add_message(mut self, role: impl Into<String>, content: impl Into<String>) -> Self {
        self.messages.push(ModelMessage {
            role: role.into(),
            content: content.into(),
        });
        self
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Token usage statistics returned by the model.
pub struct ModelUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// Streaming events emitted by the model during generation.
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
/// Trait for model providers (OpenAI, Anthropic, Ollama, etc.).
///
/// Implementations stream [`ModelEvent`]s in response to a [`ModelRequest`].
/// Use [`FakeModelClient`](crate::FakeModelClient) for testing.
pub trait ModelClient: Send + Sync {
    /// Send a request to the model and receive a stream of events.
    async fn stream(
        &self,
        request: ModelRequest,
    ) -> crate::Result<BoxStream<'static, crate::Result<ModelEvent>>>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_call_serializes_with_id_name_and_arguments() {
        let tc = ToolCall {
            id: "call_abc".into(),
            name: "fs.read".into(),
            arguments: serde_json::json!({"path": "README.md"}),
        };
        let json = serde_json::to_value(&tc).unwrap();
        assert_eq!(json["id"], "call_abc");
        assert_eq!(json["name"], "fs.read");
        assert_eq!(json["arguments"]["path"], "README.md");
    }

    #[test]
    fn model_request_supports_system_prompt_and_tools() {
        let req = ModelRequest::user_text("fast", "hello")
            .with_system_prompt("You are helpful.")
            .with_tools(vec![ToolDefinition {
                name: "fs.read".into(),
                description: "Read a file".into(),
                parameters: serde_json::json!({"type": "object"}),
            }]);
        assert_eq!(req.system_prompt, Some("You are helpful.".into()));
        assert_eq!(req.tools.len(), 1);
    }
}
