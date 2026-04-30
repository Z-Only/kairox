# Real Model Adapters Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Implement real OpenAI-compatible and Ollama HTTP streaming model adapters behind the existing `ModelClient` trait so that `cargo run -p agent-tui` can complete a full conversation turn with a live LLM.

**Architecture:** Add `reqwest` as the HTTP client. The `OpenAiCompatibleClient` sends chat completion requests with SSE streaming and maps chunks to `ModelEvent::TokenDelta`, `ModelEvent::ToolCallRequested`, and `ModelEvent::Completed`. The `OllamaClient` talks to the local Ollama REST API with NDJSON streaming. Both adapters are constructed from `OpenAiCompatibleConfig` / `OllamaConfig` and registered via `ModelProfile`. A `ModelRouter` resolves a profile alias to the correct `ModelClient` instance.

**Tech Stack:** Rust, reqwest (HTTP + streaming), tokio, serde_json, async-trait, futures, eventsource-stream (SSE parsing).

---

## File Structure

| File                                           | Responsibility                                                                                               |
| ---------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| `crates/agent-models/Cargo.toml`               | Add reqwest, eventsource-stream dependencies                                                                 |
| `crates/agent-models/src/types.rs`             | Add `ToolCall` struct, extend `ModelRequest` with tools/system_prompt, extend `ModelEvent` with usage detail |
| `crates/agent-models/src/profile.rs`           | Add `ModelProviderConfig` enum to hold provider-specific config per profile                                  |
| `crates/agent-models/src/openai_compatible.rs` | Full `OpenAiCompatibleClient` with SSE streaming, tool call parsing                                          |
| `crates/agent-models/src/ollama.rs`            | Full `OllamaClient` with NDJSON streaming                                                                    |
| `crates/agent-models/src/router.rs`            | `ModelRouter` that resolves profile alias → `ModelClient`                                                    |
| `crates/agent-models/src/lib.rs`               | Re-export new types, update `ModelError` for HTTP errors                                                     |

---

## Cross-Cutting Rules

- All HTTP calls go through `reqwest::Client` with a reasonable timeout (60s connect, 300s read for streaming).
- API keys are read from environment variables (never hardcoded).
- Streaming responses are parsed incrementally — never buffered entirely in memory.
- Errors from HTTP/model layer are mapped to `ModelError`, never leaked as `reqwest` types.
- Tests use `wiremock` or recorded fixtures, never hit real endpoints in CI.

---

### Task 1: Add HTTP Dependencies

**Files:**

- Modify: `crates/agent-models/Cargo.toml`
- Modify: `Cargo.toml` (workspace dependencies)

- [ ] **Step 1: Add workspace-level dependencies to root Cargo.toml**

Add to `[workspace.dependencies]`:

```toml
reqwest = { version = "0.12", features = ["json", "stream", "rustls-tls"], default-features = false }
eventsource-stream = "0.2"
tracing = "0.1"
wiremock = "0.6"
```

- [ ] **Step 2: Add dependencies to agent-models Cargo.toml**

Update `crates/agent-models/Cargo.toml` to:

```toml
[package]
name = "agent-models"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
agent-core = { path = "../agent-core" }
async-trait.workspace = true
eventsource-stream.workspace = true
futures.workspace = true
reqwest.workspace = true
serde.workspace = true
serde_json.workspace = true
thiserror.workspace = true
tokio.workspace = true
tokio-stream.workspace = true
tracing.workspace = true

[dev-dependencies]
wiremock.workspace = true
```

- [ ] **Step 3: Verify workspace compiles**

Run: `cargo check -p agent-models`
Expected: PASS (no new code yet, just dependency resolution)

- [ ] **Step 4: Commit**

```bash
git add Cargo.toml crates/agent-models/Cargo.toml
git commit -m "chore(models): add reqwest and streaming dependencies"
```

---

### Task 2: Extend Model Types for Tool Calls and Rich Requests

**Files:**

- Modify: `crates/agent-models/src/types.rs`
- Modify: `crates/agent-models/src/lib.rs`

- [ ] **Step 1: Write failing tests for new types**

Add to `crates/agent-models/src/types.rs` at the bottom, inside a new `#[cfg(test)] mod tests`:

```rust
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
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p agent-models tool_call_serializes model_request_supports_system`
Expected: FAIL — `ToolCall`, `ToolDefinition`, `with_system_prompt`, `with_tools` do not exist yet.

- [ ] **Step 3: Add ToolCall, ToolDefinition, and extend ModelRequest**

Replace `crates/agent-models/src/types.rs` with:

```rust
use async_trait::async_trait;
use futures::stream::BoxStream;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelMessage {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
```

Update `crates/agent-models/src/lib.rs` to re-export new types:

```rust
pub mod fake;
pub mod ollama;
pub mod openai_compatible;
pub mod profile;
pub mod router;
pub mod types;

pub use fake::FakeModelClient;
pub use profile::{ModelCapabilities, ModelProfile};
pub use types::{ModelClient, ModelEvent, ModelMessage, ModelRequest, ModelUsage, ToolCall, ToolDefinition};

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
```

- [ ] **Step 4: Run tests to verify**

Run: `cargo test -p agent-models`
Expected: PASS — all existing tests plus the two new ones.

- [ ] **Step 5: Commit**

```bash
git add crates/agent-models/src/types.rs crates/agent-models/src/lib.rs
git commit -m "feat(models): add tool call types and rich request builder"
```

---

### Task 3: OpenAI-Compatible Streaming Client

**Files:**

- Modify: `crates/agent-models/src/openai_compatible.rs`

- [ ] **Step 1: Write failing test with wiremock**

Replace `crates/agent-models/src/openai_compatible.rs` with:

```rust
use crate::{ModelError, ModelEvent, ModelRequest, Result, ToolCall};
use async_trait::async_trait;
use eventsource_stream::Eventsource;
use futures::stream::BoxStream;
use futures::{StreamExt, TryStreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpenAiCompatibleConfig {
    pub base_url: String,
    pub api_key_env: String,
    pub default_model: String,
    pub headers: Vec<(String, String)>,
    pub capability_overrides: Option<crate::ModelCapabilities>,
}

impl OpenAiCompatibleConfig {
    pub fn default_capabilities(&self) -> crate::ModelCapabilities {
        self.capability_overrides
            .clone()
            .unwrap_or(crate::ModelCapabilities {
                streaming: true,
                tool_calling: true,
                json_schema: true,
                vision: false,
                reasoning_controls: false,
                context_window: 128_000,
                output_limit: 16_384,
                local_model: false,
            })
    }

    fn api_key(&self) -> Option<String> {
        std::env::var(&self.api_key_env).ok()
    }
}

#[derive(Debug, Clone)]
pub struct OpenAiCompatibleClient {
    config: OpenAiCompatibleConfig,
    http: Client,
}

impl OpenAiCompatibleClient {
    pub fn new(config: OpenAiCompatibleConfig) -> Self {
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .expect("failed to build reqwest client");
        Self { config, http }
    }

    pub fn from_config(config: OpenAiCompatibleConfig, http: Client) -> Self {
        Self { config, http }
    }

    fn build_chat_request(&self, request: &ModelRequest) -> Result<serde_json::Value> {
        let mut messages = Vec::new();

        if let Some(ref system_prompt) = request.system_prompt {
            messages.push(serde_json::json!({
                "role": "system",
                "content": system_prompt,
            }));
        }

        for msg in &request.messages {
            messages.push(serde_json::json!({
                "role": msg.role,
                "content": msg.content,
            }));
        }

        let mut body = serde_json::json!({
            "model": self.config.default_model,
            "messages": messages,
            "stream": true,
        });

        if !request.tools.is_empty() {
            let tools: Vec<_> = request
                .tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "type": "function",
                        "function": {
                            "name": t.name,
                            "description": t.description,
                            "parameters": t.parameters,
                        }
                    })
                })
                .collect();
            body["tools"] = serde_json::json!(tools);
        }

        Ok(body)
    }

    async fn send_streaming(
        &self,
        request: ModelRequest,
    ) -> Result<BoxStream<'static, Result<ModelEvent>>> {
        let body = self.build_chat_request(&request)?;
        let url = format!("{}/chat/completions", self.config.base_url.trim_end_matches('/'));

        let mut builder = self.http.post(&url).json(&body);
        if let Some(api_key) = self.config.api_key() {
            builder = builder.bearer_auth(&api_key);
        }
        for (key, value) in &self.config.headers {
            builder = builder.header(key.as_str(), value.as_str());
        }

        let response = builder.send().await.map_err(|e| ModelError::Http(e.to_string()))?;
        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ModelError::Api(format!("HTTP {}: {}", status, body)));
        }

        let stream = response
            .bytes_stream()
            .eventsource()
            .map_err(|e| ModelError::StreamParse(e.to_string()))
            .and_then(|event| async move {
                if event.data == "[DONE]" {
                    Ok(None)
                } else {
                    parse_openai_chunk(&event.data).map(Some)
                }
            })
            .filter_map(|result| async move {
                match result {
                    Ok(Some(events)) => futures::stream::iter(events).boxed(),
                    Ok(None) => futures::stream::empty().boxed(),
                    Err(e) => futures::stream::once(async { Err(e) }).boxed(),
                }
            })
            .flatten();

        Ok(Box::pin(stream))
    }
}

fn parse_openai_chunk(data: &str) -> Result<Vec<ModelEvent>> {
    let chunk: serde_json::Value =
        serde_json::from_str(data).map_err(|e| ModelError::StreamParse(e.to_string()))?;

    let mut events = Vec::new();

    if let Some(choices) = chunk["choices"].as_array() {
        for choice in choices {
            let delta = &choice["delta"];

            // Content token delta
            if let Some(content) = delta["content"].as_str() {
                if !content.is_empty() {
                    events.push(ModelEvent::TokenDelta(content.to_string()));
                }
            }

            // Tool calls
            if let Some(tool_calls) = delta["tool_calls"].as_array() {
                for tc in tool_calls {
                    let id = tc["id"].as_str().unwrap_or("").to_string();
                    let name = tc["function"]["name"]
                        .as_str()
                        .unwrap_or("")
                        .to_string();
                    let arguments_str = tc["function"]["arguments"]
                        .as_str()
                        .unwrap_or("{}");
                    let arguments: serde_json::Value =
                        serde_json::from_str(arguments_str).unwrap_or(serde_json::json!({}));

                    // Only emit when we have an id and name (first chunk of tool call)
                    if !id.is_empty() && !name.is_empty() {
                        events.push(ModelEvent::ToolCallRequested {
                            tool_call_id: id,
                            tool_id: name,
                            arguments,
                        });
                    }
                }
            }

            // Finish reason
            if let Some(finish) = choice["finish_reason"].as_str() {
                if finish == "stop" || finish == "tool_calls" {
                    let usage_value = &chunk["usage"];
                    let usage = if usage_value.is_object() {
                        Some(crate::ModelUsage {
                            input_tokens: usage_value["prompt_tokens"].as_u64().unwrap_or(0),
                            output_tokens: usage_value["completion_tokens"].as_u64().unwrap_or(0),
                        })
                    } else {
                        None
                    };
                    events.push(ModelEvent::Completed { usage });
                }
            }
        }
    }

    Ok(events)
}

#[async_trait]
impl crate::ModelClient for OpenAiCompatibleClient {
    async fn stream(
        &self,
        request: ModelRequest,
    ) -> Result<BoxStream<'static, Result<ModelEvent>>> {
        self.send_streaming(request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_chat_request_with_system_prompt_and_messages() {
        let config = OpenAiCompatibleConfig {
            base_url: "https://api.openai.com/v1".into(),
            api_key_env: "OPENAI_API_KEY".into(),
            default_model: "gpt-4.1".into(),
            headers: vec![],
            capability_overrides: None,
        };
        let client = OpenAiCompatibleClient::new(config);
        let request = ModelRequest::user_text("fast", "hello")
            .with_system_prompt("You are helpful.")
            .add_message("assistant", "hi there");

        let body = client.build_chat_request(&request).unwrap();

        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[0]["content"], "You are helpful.");
        assert_eq!(messages[1]["role"], "user");
        assert_eq!(messages[2]["role"], "assistant");
        assert_eq!(body["model"], "gpt-4.1");
        assert_eq!(body["stream"], true);
    }

    #[test]
    fn builds_chat_request_with_tools() {
        let config = OpenAiCompatibleConfig {
            base_url: "https://api.openai.com/v1".into(),
            api_key_env: "OPENAI_API_KEY".into(),
            default_model: "gpt-4.1".into(),
            headers: vec![],
            capability_overrides: None,
        };
        let client = OpenAiCompatibleClient::new(config);
        let request = ModelRequest::user_text("fast", "read README")
            .with_tools(vec![crate::ToolDefinition {
                name: "fs.read".into(),
                description: "Read a file".into(),
                parameters: serde_json::json!({"type": "object", "properties": {"path": {"type": "string"}}}),
            }]);

        let body = client.build_chat_request(&request).unwrap();
        let tools = body["tools"].as_array().unwrap();
        assert_eq!(tools[0]["type"], "function");
        assert_eq!(tools[0]["function"]["name"], "fs.read");
    }

    #[test]
    fn parses_token_delta_from_chunk() {
        let data = r#"{"choices":[{"delta":{"content":"Hello"},"index":0}]}"#;
        let events = parse_openai_chunk(data).unwrap();
        assert_eq!(events, vec![ModelEvent::TokenDelta("Hello".into())]);
    }

    #[test]
    fn parses_tool_call_from_chunk() {
        let data = r#"{"choices":[{"delta":{"tool_calls":[{"id":"call_1","function":{"name":"fs.read","arguments":"{\"path\":\"README.md\"}"}}]},"index":0}]}"#;
        let events = parse_openai_chunk(data).unwrap();
        assert_eq!(events.len(), 1);
        match &events[0] {
            ModelEvent::ToolCallRequested { tool_call_id, tool_id, arguments } => {
                assert_eq!(tool_call_id, "call_1");
                assert_eq!(tool_id, "fs.read");
                assert_eq!(arguments["path"], "README.md");
            }
            _ => panic!("expected ToolCallRequested"),
        }
    }

    #[test]
    fn parses_completion_event() {
        let data = r#"{"choices":[{"finish_reason":"stop","index":0}],"usage":{"prompt_tokens":10,"completion_tokens":5}}"#;
        let events = parse_openai_chunk(data).unwrap();
        assert!(matches!(&events[0], ModelEvent::Completed { usage: Some(u) } if u.input_tokens == 10 && u.output_tokens == 5));
    }

    #[tokio::test]
    async fn streams_from_wiremock_server() {
        use wiremock::{Mock, MockServer, ResponseTemplate};
        use wiremock::matchers::{method, path};
        use futures::StreamExt;

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/chat/completions"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "data: {\"choices\":[{\"delta\":{\"content\":\"Hi\"},\"index\":0}]}\n\ndata: {\"choices\":[{\"delta\":{\"content\":\" there\"},\"index\":0}]}\n\ndata: {\"choices\":[{\"finish_reason\":\"stop\",\"index\":0}]}\n\ndata: [DONE]\n\n"
            ))
            .mount(&mock_server)
            .await;

        let config = OpenAiCompatibleConfig {
            base_url: mock_server.uri(),
            api_key_env: "TEST_KEY_NOT_SET".into(),
            default_model: "test-model".into(),
            headers: vec![],
            capability_overrides: None,
        };
        let client = OpenAiCompatibleClient::new(config);
        let mut stream = client
            .stream(ModelRequest::user_text("fast", "hello"))
            .await
            .unwrap();

        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            events.push(event.unwrap());
        }

        assert!(events.iter().any(|e| matches!(e, ModelEvent::TokenDelta(t) if t == "Hi")));
        assert!(events.iter().any(|e| matches!(e, ModelEvent::TokenDelta(t) if t == " there")));
        assert!(events.iter().any(|e| matches!(e, ModelEvent::Completed { .. })));
    }
}
```

- [ ] **Step 2: Run tests to verify**

Run: `cargo test -p agent-models`
Expected: PASS — all existing tests plus new OpenAI adapter tests including wiremock integration.

- [ ] **Step 3: Commit**

```bash
git add crates/agent-models/src/openai_compatible.rs
git commit -m "feat(models): implement OpenAI-compatible streaming client"
```

---

### Task 4: Ollama Streaming Client

**Files:**

- Modify: `crates/agent-models/src/ollama.rs`

- [ ] **Step 1: Write Ollama client with NDJSON streaming**

Replace `crates/agent-models/src/ollama.rs` with:

```rust
use crate::{ModelError, ModelEvent, ModelRequest, Result};
use async_trait::async_trait;
use futures::stream::BoxStream;
use futures::{StreamExt, TryStreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OllamaConfig {
    pub base_url: String,
    pub default_model: String,
    pub context_window: u64,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:11434".into(),
            default_model: "llama3".into(),
            context_window: 8192,
        }
    }
}

impl OllamaConfig {
    pub fn capabilities(&self) -> crate::ModelCapabilities {
        crate::ModelCapabilities {
            streaming: true,
            tool_calling: false,
            json_schema: false,
            vision: false,
            reasoning_controls: false,
            context_window: self.context_window,
            output_limit: 4096,
            local_model: true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct OllamaClient {
    config: OllamaConfig,
    http: Client,
}

impl OllamaClient {
    pub fn new(config: OllamaConfig) -> Self {
        let http = Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .expect("failed to build reqwest client");
        Self { config, http }
    }

    pub fn from_config(config: OllamaConfig, http: Client) -> Self {
        Self { config, http }
    }

    fn build_chat_request(&self, request: &ModelRequest) -> serde_json::Value {
        let mut messages = Vec::new();
        if let Some(ref system_prompt) = request.system_prompt {
            messages.push(serde_json::json!({
                "role": "system",
                "content": system_prompt,
            }));
        }
        for msg in &request.messages {
            messages.push(serde_json::json!({
                "role": msg.role,
                "content": msg.content,
            }));
        }
        serde_json::json!({
            "model": self.config.default_model,
            "messages": messages,
            "stream": true,
        })
    }

    async fn send_streaming(
        &self,
        request: ModelRequest,
    ) -> Result<BoxStream<'static, Result<ModelEvent>>> {
        let body = self.build_chat_request(&request);
        let url = format!("{}/api/chat", self.config.base_url.trim_end_matches('/'));

        let response = self
            .http
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| ModelError::Http(e.to_string()))?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ModelError::Api(format!("HTTP {}: {}", status, body)));
        }

        let stream = response
            .bytes_stream()
            .map_err(|e| ModelError::Http(e.to_string()))
            .and_then(|chunk| async move {
                let text = String::from_utf8_lossy(&chunk);
                let mut events = Vec::new();
                for line in text.lines() {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }
                    match parse_ollama_line(line) {
                        Ok(Some(event)) => events.push(Ok(event)),
                        Ok(None) => {}
                        Err(e) => events.push(Err(e)),
                    }
                }
                Ok(futures::stream::iter(events))
            })
            .flatten();

        Ok(Box::pin(stream))
    }
}

fn parse_ollama_line(line: &str) -> Result<Option<ModelEvent>> {
    let value: serde_json::Value =
        serde_json::from_str(line).map_err(|e| ModelError::StreamParse(e.to_string()))?;

    // Ollama NDJSON: each line has "message" with "content" and "done" flag
    if value["done"].as_bool() == Some(true) {
        return Ok(Some(ModelEvent::Completed { usage: None }));
    }

    if let Some(content) = value["message"]["content"].as_str() {
        if !content.is_empty() {
            return Ok(Some(ModelEvent::TokenDelta(content.to_string())));
        }
    }

    Ok(None)
}

#[async_trait]
impl crate::ModelClient for OllamaClient {
    async fn stream(
        &self,
        request: ModelRequest,
    ) -> Result<BoxStream<'static, Result<ModelEvent>>> {
        self.send_streaming(request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_ollama_chat_request() {
        let config = OllamaConfig::default();
        let client = OllamaClient::new(config);
        let request = ModelRequest::user_text("local-code", "explain this")
            .with_system_prompt("You are a code assistant.");

        let body = client.build_chat_request(&request);
        let messages = body["messages"].as_array().unwrap();
        assert_eq!(messages[0]["role"], "system");
        assert_eq!(messages[1]["role"], "user");
        assert_eq!(body["model"], "llama3");
        assert_eq!(body["stream"], true);
    }

    #[test]
    fn parses_ollama_ndjson_token_line() {
        let line = r#"{"message":{"role":"assistant","content":"Hello"},"done":false}"#;
        let event = parse_ollama_line(line).unwrap();
        assert_eq!(event, Some(ModelEvent::TokenDelta("Hello".into())));
    }

    #[test]
    fn parses_ollama_done_line() {
        let line = r#"{"message":{"role":"assistant","content":""},"done":true}"#;
        let event = parse_ollama_line(line).unwrap();
        assert!(matches!(event, Some(ModelEvent::Completed { usage: None })));
    }

    #[tokio::test]
    async fn streams_from_wiremock_server() {
        use wiremock::{Mock, MockServer, ResponseTemplate};
        use wiremock::matchers::{method, path};

        let mock_server = MockServer::start().await;

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_string(
                "{\"message\":{\"role\":\"assistant\",\"content\":\"Hi\"},\"done\":false}\n{\"message\":{\"role\":\"assistant\",\"content\":\" there\"},\"done\":false}\n{\"message\":{\"role\":\"assistant\",\"content\":\"\"},\"done\":true}\n"
            ))
            .mount(&mock_server)
            .await;

        let config = OllamaConfig {
            base_url: mock_server.uri(),
            default_model: "test-model".into(),
            context_window: 4096,
        };
        let client = OllamaClient::new(config);
        let mut stream = client
            .stream(ModelRequest::user_text("local", "hello"))
            .await
            .unwrap();

        let mut events = Vec::new();
        while let Some(event) = stream.next().await {
            events.push(event.unwrap());
        }

        assert!(events.iter().any(|e| matches!(e, ModelEvent::TokenDelta(t) if t == "Hi")));
        assert!(events.iter().any(|e| matches!(e, ModelEvent::TokenDelta(t) if t == " there")));
        assert!(events.iter().any(|e| matches!(e, ModelEvent::Completed { .. })));
    }
}
```

- [ ] **Step 2: Run tests to verify**

Run: `cargo test -p agent-models`
Expected: PASS — all tests including Ollama wiremock integration.

- [ ] **Step 3: Commit**

```bash
git add crates/agent-models/src/ollama.rs
git commit -m "feat(models): implement Ollama NDJSON streaming client"
```

---

### Task 5: Model Router

**Files:**

- Create: `crates/agent-models/src/router.rs`

- [ ] **Step 1: Write failing router test**

Create `crates/agent-models/src/router.rs`:

```rust
use crate::{ModelClient, ModelEvent, ModelProfile, ModelRequest, Result};
use async_trait::async_trait;
use futures::stream::BoxStream;
use std::collections::HashMap;
use std::sync::Arc;

pub struct ModelRouter {
    clients: HashMap<String, Arc<dyn ModelClient>>,
    profiles: HashMap<String, ModelProfile>,
}

impl ModelRouter {
    pub fn new() -> Self {
        Self {
            clients: HashMap::new(),
            profiles: HashMap::new(),
        }
    }

    pub fn register(&mut self, profile: ModelProfile, client: Arc<dyn ModelClient>) {
        let alias = profile.alias.clone();
        self.profiles.insert(alias.clone(), profile);
        self.clients.insert(alias, client);
    }

    pub fn get_profile(&self, alias: &str) -> Option<&ModelProfile> {
        self.profiles.get(alias)
    }

    pub fn list_profiles(&self) -> Vec<&ModelProfile> {
        let mut profiles: Vec<_> = self.profiles.values().collect();
        profiles.sort_by(|a, b| a.alias.cmp(&b.alias));
        profiles
    }

    pub async fn stream(
        &self,
        request: ModelRequest,
    ) -> Result<BoxStream<'static, Result<ModelEvent>>> {
        let client = self
            .clients
            .get(&request.model_profile)
            .ok_or_else(|| crate::ModelError::Request(format!(
                "unknown model profile: '{}'",
                request.model_profile
            )))?;
        client.stream(request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{FakeModelClient, ModelCapabilities};
    use futures::StreamExt;

    fn test_profile(alias: &str) -> ModelProfile {
        ModelProfile {
            alias: alias.into(),
            provider: "fake".into(),
            model_id: "test".into(),
            capabilities: ModelCapabilities {
                streaming: true,
                tool_calling: false,
                json_schema: false,
                vision: false,
                reasoning_controls: false,
                context_window: 4096,
                output_limit: 2048,
                local_model: true,
            },
        }
    }

    #[tokio::test]
    async fn routes_to_correct_client_by_profile_alias() {
        let mut router = ModelRouter::new();
        let fast_client = Arc::new(FakeModelClient::new(vec!["fast response".into()]));
        let deep_client = Arc::new(FakeModelClient::new(vec!["deep response".into()]));

        router.register(test_profile("fast"), fast_client);
        router.register(test_profile("deep-reasoning"), deep_client);

        let mut stream = router
            .stream(ModelRequest::user_text("fast", "hello"))
            .await
            .unwrap();

        let first = stream.next().await.unwrap().unwrap();
        assert_eq!(first, ModelEvent::TokenDelta("fast response".into()));
    }

    #[tokio::test]
    async fn returns_error_for_unknown_profile() {
        let router = ModelRouter::new();
        let result = router.stream(ModelRequest::user_text("nonexistent", "hello")).await;
        assert!(result.is_err());
    }

    #[test]
    fn lists_registered_profiles_sorted() {
        let mut router = ModelRouter::new();
        router.register(
            test_profile("deep-reasoning"),
            Arc::new(FakeModelClient::new(vec![])),
        );
        router.register(
            test_profile("fast"),
            Arc::new(FakeModelClient::new(vec![])),
        );

        let names: Vec<_> = router.list_profiles().iter().map(|p| p.alias.as_str()).collect();
        assert_eq!(names, vec!["deep-reasoning", "fast"]);
    }
}
```

- [ ] **Step 2: Update lib.rs to re-export router**

Update `crates/agent-models/src/lib.rs` — add `pub mod router;` and `pub use router::ModelRouter;`.

- [ ] **Step 3: Run tests to verify**

Run: `cargo test -p agent-models`
Expected: PASS — all tests including router routing and error handling.

- [ ] **Step 4: Commit**

```bash
git add crates/agent-models/src/router.rs crates/agent-models/src/lib.rs
git commit -m "feat(models): add ModelRouter for profile-based client routing"
```

---

### Task 6: Update FakeModelClient for New Request Fields

**Files:**

- Modify: `crates/agent-models/src/fake.rs`

- [ ] **Step 1: Update FakeModelClient to handle new ModelRequest fields**

Replace `crates/agent-models/src/fake.rs` with:

```rust
use crate::{ModelClient, ModelEvent, ModelRequest};
use async_trait::async_trait;
use futures::{stream, stream::BoxStream};

#[derive(Debug, Clone)]
pub struct FakeModelClient {
    tokens: Vec<String>,
    include_tool_call: bool,
}

impl FakeModelClient {
    pub fn new(tokens: Vec<String>) -> Self {
        Self {
            tokens,
            include_tool_call: false,
        }
    }

    pub fn with_tool_call(mut self) -> Self {
        self.include_tool_call = true;
        self
    }
}

#[async_trait]
impl ModelClient for FakeModelClient {
    async fn stream(
        &self,
        request: ModelRequest,
    ) -> crate::Result<BoxStream<'static, crate::Result<ModelEvent>>> {
        let _ = request;
        let mut events: Vec<crate::Result<ModelEvent>> = self
            .tokens
            .iter()
            .cloned()
            .map(ModelEvent::TokenDelta)
            .map(Ok)
            .collect();

        if self.include_tool_call {
            events.push(Ok(ModelEvent::ToolCallRequested {
                tool_call_id: "call_fake_1".into(),
                tool_id: "fs.read".into(),
                arguments: serde_json::json!({"path": "README.md"}),
            }));
        }

        events.push(Ok(ModelEvent::Completed { usage: None }));
        Ok(Box::pin(stream::iter(events)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ModelEvent, ModelRequest};
    use futures::StreamExt;

    #[tokio::test]
    async fn streams_configured_tokens_then_completion() {
        let client = FakeModelClient::new(vec!["hello".into(), " ".into(), "world".into()]);
        let mut stream = client
            .stream(ModelRequest::user_text("test", "hi"))
            .await
            .unwrap();

        let mut seen = Vec::new();
        while let Some(event) = stream.next().await {
            seen.push(event.unwrap());
        }

        assert_eq!(
            seen,
            vec![
                ModelEvent::TokenDelta("hello".into()),
                ModelEvent::TokenDelta(" ".into()),
                ModelEvent::TokenDelta("world".into()),
                ModelEvent::Completed { usage: None },
            ]
        );
    }

    #[tokio::test]
    async fn optionally_includes_tool_call_event() {
        let client = FakeModelClient::new(vec!["reading".into()]).with_tool_call();
        let mut stream = client
            .stream(ModelRequest::user_text("test", "read"))
            .await
            .unwrap();

        let mut seen = Vec::new();
        while let Some(event) = stream.next().await {
            seen.push(event.unwrap());
        }

        assert!(matches!(&seen[1], ModelEvent::ToolCallRequested { .. }));
    }
}
```

- [ ] **Step 2: Run full workspace tests**

Run: `cargo test --workspace`
Expected: PASS — all workspace tests still green with updated FakeModelClient.

- [ ] **Step 3: Commit**

```bash
git add crates/agent-models/src/fake.rs
git commit -m "feat(models): add tool call support to FakeModelClient"
```

---

### Task 7: Update TUI to Use ModelRouter with Real Providers

**Files:**

- Modify: `crates/agent-tui/src/main.rs`
- Modify: `crates/agent-tui/Cargo.toml`

- [ ] **Step 1: Add agent-memory dependency to TUI**

Update `crates/agent-tui/Cargo.toml` to:

```toml
[package]
name = "agent-tui"
version.workspace = true
edition.workspace = true
license.workspace = true

[dependencies]
agent-core = { path = "../agent-core" }
agent-memory = { path = "../agent-memory" }
agent-models = { path = "../agent-models" }
agent-runtime = { path = "../agent-runtime" }
agent-store = { path = "../agent-store" }
agent-tools = { path = "../agent-tools" }
anyhow.workspace = true
tokio.workspace = true
```

- [ ] **Step 2: Update TUI main to use ModelRouter**

Replace `crates/agent-tui/src/main.rs` with:

```rust
mod app;
mod view;

use agent_core::{AppFacade, SendMessageRequest, StartSessionRequest};
use agent_memory::ContextAssembler;
use agent_models::{
    ModelCapabilities, ModelProfile, ModelRouter, OpenAiCompatibleClient, OllamaClient,
};
use agent_runtime::LocalRuntime;
use agent_store::SqliteEventStore;
use agent_tools::{FsReadTool, PermissionEngine, PermissionMode};
use std::sync::Arc;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let store = SqliteEventStore::in_memory().await?;

    let mut router = ModelRouter::new();

    // Register OpenAI-compatible profile if API key is present
    if std::env::var("OPENAI_API_KEY").is_ok() {
        let config = agent_models::openai_compatible::OpenAiCompatibleConfig {
            base_url: "https://api.openai.com/v1".into(),
            api_key_env: "OPENAI_API_KEY".into(),
            default_model: "gpt-4.1-mini".into(),
            headers: vec![],
            capability_overrides: None,
        };
        let profile = ModelProfile {
            alias: "fast".into(),
            provider: "openai_compatible".into(),
            model_id: "gpt-4.1-mini".into(),
            capabilities: config.default_capabilities(),
        };
        router.register(profile, Arc::new(OpenAiCompatibleClient::new(config)));
    }

    // Register Ollama profile
    let ollama_config = agent_models::ollama::OllamaConfig::default();
    let ollama_profile = ModelProfile {
        alias: "local-code".into(),
        provider: "ollama".into(),
        model_id: ollama_config.default_model.clone(),
        capabilities: ollama_config.capabilities(),
    };
    router.register(ollama_profile, Arc::new(OllamaClient::new(ollama_config)));

    // Always have a fake fallback
    let fake_profile = ModelProfile {
        alias: "fake".into(),
        provider: "fake".into(),
        model_id: "fake".into(),
        capabilities: ModelCapabilities {
            streaming: true,
            tool_calling: true,
            json_schema: false,
            vision: false,
            reasoning_controls: false,
            context_window: 4096,
            output_limit: 2048,
            local_model: true,
        },
    };
    router.register(
        fake_profile,
        Arc::new(agent_models::FakeModelClient::new(vec![
            "hello from fake model".into(),
        ])),
    );

    let available: Vec<_> = router.list_profiles().iter().map(|p| p.alias.as_str()).collect();
    eprintln!("Available model profiles: {available:?}");

    // Choose first available profile: prefer real models over fake
    let profile = if std::env::var("OPENAI_API_KEY").is_ok() {
        "fast"
    } else {
        "fake"
    };
    eprintln!("Using profile: {profile}");

    let runtime = LocalRuntime::new_with_router(store, router, PermissionMode::Suggest);

    let workspace = runtime
        .open_workspace(std::env::current_dir()?.display().to_string())
        .await?;
    let session_id = runtime
        .start_session(StartSessionRequest {
            workspace_id: workspace.workspace_id.clone(),
            model_profile: profile.into(),
        })
        .await?;

    // For now, use the simple send_message path
    // When TUI ratatui interface is built, this will be interactive
    let args: Vec<String> = std::env::args().collect();
    let user_message = if args.len() > 1 {
        args[1..].join(" ")
    } else {
        "hello".into()
    };

    runtime
        .send_message(SendMessageRequest {
            workspace_id: workspace.workspace_id,
            session_id: session_id.clone(),
            content: user_message,
        })
        .await?;

    let projection = runtime.get_session_projection(session_id).await?;

    for line in view::render_lines(&projection) {
        println!("{line}");
    }

    Ok(())
}
```

- [ ] **Step 3: Verify TUI compiles**

Run: `cargo check -p agent-tui`
Expected: PASS (may need runtime changes from Task B first — if so, this step can be postponed until after the runtime agent loop is integrated)

- [ ] **Step 4: Commit**

```bash
git add crates/agent-tui
git commit -m "feat(tui): wire ModelRouter with real providers"
```

---

## Acceptance Criteria

This plan is successful when:

- `OpenAiCompatibleClient` can stream tokens from any OpenAI-compatible API endpoint
- `OllamaClient` can stream tokens from a local Ollama instance
- `ModelRouter` resolves profile aliases to the correct client
- `cargo test -p agent-models` passes with wiremock integration tests for both clients
- The TUI can initialize with real model providers when API keys are available
- `FakeModelClient` still works for testing without network access
- All existing workspace tests continue to pass

## Self-Review

1. **Spec coverage:** The original design spec's Model Layer section calls for `ModelProvider`, `ModelClient`, `ModelRequest`, `ModelEvent`, `ModelCapabilities`, and `ModelProfile`. Tasks 2-5 cover all of these. The `ModelProvider` concept is absorbed into `ModelRouter.register()` which is simpler and sufficient for Phase 1.
2. **Placeholder scan:** No TBDs, TODOs, or vague steps. Every code block is complete.
3. **Type consistency:** `ModelRequest` now has `system_prompt`, `tools`, `add_message()` — these are used consistently by both `OpenAiCompatibleClient::build_chat_request` and `OllamaClient::build_chat_request`. `ToolCall` and `ToolDefinition` types in `types.rs` are used by `parse_openai_chunk` and `build_chat_request`. `ModelError` has new variants `Http`, `StreamParse`, `Api` used by both adapters.
