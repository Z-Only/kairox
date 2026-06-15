use super::config::AnthropicConfig;
use super::streaming::stream_anthropic_response;
use super::tool_accumulator::anthropic_tool_name_map;
use crate::retry::{with_retry, RetryConfig};
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
        let mut builder = reqwest::Client::builder()
            .connect_timeout(std::time::Duration::from_secs(config.connect_timeout_secs));

        if let Some(timeout_secs) = config.request_timeout_secs {
            builder = builder.timeout(std::time::Duration::from_secs(timeout_secs));
        }

        let http = builder.build().expect("failed to build reqwest client");
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

        let name_map = anthropic_tool_name_map(&request.tools);

        // Clone values needed inside the retry closure
        let http = self.http.clone();
        let headers = self.config.headers.clone();

        let config = RetryConfig::default();
        let response = with_retry(&config, || {
            let http = http.clone();
            let url = url.clone();
            let api_key = api_key.clone();
            let body = body.clone();
            let headers = headers.clone();
            async move {
                let mut builder = http
                    .post(&url)
                    .header("x-api-key", &api_key)
                    .header("anthropic-version", "2023-06-01")
                    .header("content-type", "application/json")
                    .json(&body);

                for (key, value) in &headers {
                    builder = builder.header(key.as_str(), value.as_str());
                }

                let response = builder.send().await.map_err(|e| {
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

                Ok(response)
            }
        })
        .await?;

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
