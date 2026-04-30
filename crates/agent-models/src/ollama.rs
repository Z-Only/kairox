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
            .map(|chunk_result| match chunk_result {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    let events: Vec<Result<ModelEvent>> = text
                        .lines()
                        .map(|line| line.trim())
                        .filter(|line| !line.is_empty())
                        .filter_map(|line| match parse_ollama_line(line) {
                            Ok(Some(event)) => Some(Ok(event)),
                            Ok(None) => None,
                            Err(e) => Some(Err(e)),
                        })
                        .collect();
                    futures::stream::iter(events).boxed()
                }
                Err(e) => futures::stream::once(async { Err(e) }).boxed(),
            })
            .flatten();

        Ok(Box::pin(stream))
    }
}

fn parse_ollama_line(line: &str) -> Result<Option<ModelEvent>> {
    let value: serde_json::Value =
        serde_json::from_str(line).map_err(|e| ModelError::StreamParse(e.to_string()))?;

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
    use crate::ModelClient;

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
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        let ndjson_body = format!(
            "{}\n{}\n{}\n",
            r#"{"message":{"role":"assistant","content":"Hi"},"done":false}"#,
            r#"{"message":{"role":"assistant","content":" there"},"done":false}"#,
            r#"{"message":{"role":"assistant","content":""},"done":true}"#
        );

        Mock::given(method("POST"))
            .and(path("/api/chat"))
            .respond_with(ResponseTemplate::new(200).set_body_string(ndjson_body))
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

        assert!(events
            .iter()
            .any(|e| matches!(e, ModelEvent::TokenDelta(t) if t == "Hi")));
        assert!(events
            .iter()
            .any(|e| matches!(e, ModelEvent::TokenDelta(t) if t == " there")));
        assert!(events
            .iter()
            .any(|e| matches!(e, ModelEvent::Completed { .. })));
    }
}
