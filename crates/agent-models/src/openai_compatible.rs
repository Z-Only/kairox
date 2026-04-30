use crate::{ModelError, ModelEvent, ModelRequest, Result};
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
        let url = format!(
            "{}/chat/completions",
            self.config.base_url.trim_end_matches('/')
        );

        let mut builder = self.http.post(&url).json(&body);
        if let Some(api_key) = self.config.api_key() {
            builder = builder.bearer_auth(&api_key);
        }
        for (key, value) in &self.config.headers {
            builder = builder.header(key.as_str(), value.as_str());
        }

        let response = builder
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
            .eventsource()
            .map_err(|e| ModelError::StreamParse(e.to_string()))
            .and_then(|event| async move {
                if event.data == "[DONE]" {
                    Ok(None)
                } else {
                    parse_openai_chunk(&event.data).map(Some)
                }
            })
            .filter_map(
                |result: std::result::Result<Option<Vec<ModelEvent>>, ModelError>| async move {
                    match result {
                        Ok(Some(events)) => {
                            let iter: Vec<Result<ModelEvent>> =
                                events.into_iter().map(Ok).collect();
                            Some(futures::stream::iter(iter).boxed())
                        }
                        Ok(None) => Some(futures::stream::empty::<Result<ModelEvent>>().boxed()),
                        Err(e) => Some(futures::stream::once(async { Err(e) }).boxed()),
                    }
                },
            )
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
                    let name = tc["function"]["name"].as_str().unwrap_or("").to_string();
                    let arguments_str = tc["function"]["arguments"].as_str().unwrap_or("{}");
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
    use crate::ModelClient;

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
            ModelEvent::ToolCallRequested {
                tool_call_id,
                tool_id,
                arguments,
            } => {
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
        assert!(
            matches!(&events[0], ModelEvent::Completed { usage: Some(u) } if u.input_tokens == 10 && u.output_tokens == 5)
        );
    }

    #[tokio::test]
    async fn streams_from_wiremock_server() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

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
        let stream: BoxStream<'static, Result<ModelEvent>> = client
            .stream(ModelRequest::user_text("fast", "hello"))
            .await
            .unwrap();

        let events: Vec<Result<ModelEvent>> = stream.collect().await;

        assert!(events
            .iter()
            .any(|e| matches!(e, Ok(ModelEvent::TokenDelta(t)) if t == "Hi")));
        assert!(events
            .iter()
            .any(|e| matches!(e, Ok(ModelEvent::TokenDelta(t)) if t == " there")));
        assert!(events
            .iter()
            .any(|e| matches!(e, Ok(ModelEvent::Completed { .. }))));
    }
}
