use super::config::AnthropicConfig;
use super::streaming::stream_anthropic_response;
use super::tool_accumulator::anthropic_tool_name_map;
use crate::{ModelError, ModelEvent, ModelRequest, Result};
use async_trait::async_trait;
use futures::stream::BoxStream;

#[derive(Debug, Clone)]
pub struct AnthropicClient {
    pub(super) config: AnthropicConfig,
    http: reqwest::Client,
}

impl AnthropicClient {
    pub fn new(config: AnthropicConfig) -> Self {
        let http = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(300))
            .build()
            .expect("failed to build reqwest client");
        Self { config, http }
    }

    async fn send_streaming(
        &self,
        request: ModelRequest,
    ) -> Result<BoxStream<'static, Result<ModelEvent>>> {
        let body = self.build_messages_request(&request);
        let url = format!("{}/v1/messages", self.config.base_url.trim_end_matches('/'));

        let api_key = self
            .config
            .api_key()
            .ok_or_else(|| ModelError::Request("Anthropic API key not set".into()))?;

        let mut builder = self
            .http
            .post(&url)
            .header("x-api-key", &api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&body);

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

        let name_map = anthropic_tool_name_map(&request.tools);
        Ok(stream_anthropic_response(response, name_map))
    }
}

#[async_trait]
impl crate::ModelClient for AnthropicClient {
    async fn stream(
        &self,
        request: ModelRequest,
    ) -> Result<BoxStream<'static, Result<ModelEvent>>> {
        self.send_streaming(request).await
    }
}
