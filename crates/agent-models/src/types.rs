use async_trait::async_trait;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
/// A single message in a model conversation.
pub struct ModelMessage {
    pub role: String,
    pub content: String,
    /// For assistant messages: tool calls requested by the model in this turn.
    /// Used by model adapters to generate provider-specific tool_use blocks
    /// (e.g., Anthropic `tool_use` content blocks, OpenAI `tool_calls` array).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCall>,
    /// For tool messages: the ID of the tool call this result corresponds to.
    /// Used by model adapters to map results back to the correct tool call
    /// (e.g., Anthropic `tool_use_id`, OpenAI `tool_call_id`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
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
                tool_calls: Vec::new(),
                tool_call_id: None,
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
            tool_calls: Vec::new(),
            tool_call_id: None,
        });
        self
    }

    /// Add an assistant message with tool calls to the conversation.
    /// Used after the model requests tool calls, so that subsequent model
    /// requests include the assistant's tool_use blocks for API compatibility.
    pub fn add_assistant_with_tools(
        mut self,
        text: impl Into<String>,
        tool_calls: Vec<ToolCall>,
    ) -> Self {
        self.messages.push(ModelMessage {
            role: "assistant".into(),
            content: text.into(),
            tool_calls,
            tool_call_id: None,
        });
        self
    }

    /// Add a tool result message to the conversation.
    /// The `tool_call_id` maps the result back to the specific tool call
    /// it answers, which is required by Anthropic and OpenAI APIs.
    pub fn add_tool_result(
        mut self,
        tool_call_id: impl Into<String>,
        content: impl Into<String>,
    ) -> Self {
        self.messages.push(ModelMessage {
            role: "tool".into(),
            content: content.into(),
            tool_calls: Vec::new(),
            tool_call_id: Some(tool_call_id.into()),
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
