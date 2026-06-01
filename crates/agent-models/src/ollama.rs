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

    /// Best-effort discovery of a model's native context window.
    ///
    /// POSTs to `/api/show` and reads `model_info.<arch>.context_length`.
    /// Returns `None` on any transport, parse, or "missing field" error
    /// so callers can fall back to the built-in registry / static default.
    ///
    /// 3-second hard timeout — never blocks a session for long.
    pub async fn probe_context_window(&self, model_id: &str) -> Option<u64> {
        let url = format!("{}/api/show", self.config.base_url.trim_end_matches('/'));
        let body = serde_json::json!({ "name": model_id });

        let resp = tokio::time::timeout(
            std::time::Duration::from_secs(3),
            self.http.post(&url).json(&body).send(),
        )
        .await
        .ok()?
        .ok()?;

        if !resp.status().is_success() {
            return None;
        }

        let value: serde_json::Value = resp.json().await.ok()?;
        let model_info = value.get("model_info")?.as_object()?;

        for (key, val) in model_info {
            if key.ends_with(".context_length") {
                if let Some(n) = val.as_u64() {
                    return Some(n);
                }
            }
        }
        None
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

        let response = self.http.post(&url).json(&body).send().await.map_err(|e| {
            if e.is_connect() || e.is_timeout() {
                ModelError::Connection(e.to_string())
            } else {
                ModelError::Http {
                    status: e.status().map_or(0, |s| s.as_u16()),
                    message: e.to_string(),
                }
            }
        })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(ModelError::Api {
                status: status.as_u16(),
                message: body,
            });
        }

        let stream = response
            .bytes_stream()
            .map_err(|e| ModelError::Http {
                status: 0,
                message: e.to_string(),
            })
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
#[path = "ollama_tests.rs"]
mod tests;
